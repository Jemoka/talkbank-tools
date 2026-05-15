//! Tauri command handlers.

use std::sync::Arc;

use arc_swap::ArcSwapOption;
use crossbeam_channel::Sender;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::protocol::commands::{
    ExportFormat, ExportResultsRequest, OpenInClanRequest, ValidateRequest,
};
use crate::validation::validate_target_streaming;

/// Shared state: cancel sender for the current validation run.
///
/// Uses `ArcSwapOption` for lock-free atomic swap — no mutex needed.
pub struct ValidationState {
    cancel_tx: ArcSwapOption<Sender<()>>,
}

impl ValidationState {
    pub fn new() -> Self {
        Self {
            cancel_tx: ArcSwapOption::empty(),
        }
    }
}

/// Start validation on a single file or folder target.
#[tauri::command]
pub async fn validate(
    app: AppHandle,
    state: State<'_, ValidationState>,
    path: String,
) -> Result<(), String> {
    let request = ValidateRequest { path };
    if request.path.is_empty() {
        return Err("No path provided".into());
    }

    let (rx, cancel_tx) = validate_target_streaming(request.path.into())?;

    // Atomically store the cancel sender (lock-free)
    state.cancel_tx.store(Some(Arc::new(cancel_tx)));

    // Spawn a thread to forward events to the frontend
    let app_clone = app.clone();
    std::thread::spawn(move || {
        while let Ok(event) = rx.recv() {
            let _ = app_clone.emit(crate::protocol::events::VALIDATION, &event);
        }
    });

    Ok(())
}

/// Cancel the current validation run.
#[tauri::command]
pub async fn cancel_validation(state: State<'_, ValidationState>) -> Result<(), String> {
    // Atomically take the cancel sender (lock-free)
    if let Some(tx) = state.cancel_tx.swap(None) {
        let _ = tx.send(());
    }
    Ok(())
}

/// Check if CLAN app is available on this platform.
#[tauri::command]
pub async fn check_clan_available() -> bool {
    send2clan::is_clan_available()
}

/// Open a file location in the CLAN app.
///
/// Uses `resolve_clan_location` from `talkbank-model` — the same function the
/// TUI uses. Resolves line/column from byte offset when not provided, adjusts
/// for CLAN hidden headers.
#[tauri::command]
pub async fn open_in_clan(
    file: String,
    line: i32,
    col: i32,
    byte_offset: u32,
    msg: String,
) -> Result<(), String> {
    open_in_clan_request(OpenInClanRequest {
        file,
        line,
        col,
        byte_offset,
        msg,
    })
}

pub fn open_in_clan_request(request: OpenInClanRequest) -> Result<(), String> {
    let source = std::fs::read_to_string(&request.file).map_err(|e| e.to_string())?;

    let location = talkbank_model::SourceLocation {
        span: talkbank_model::Span::new(request.byte_offset, request.byte_offset),
        line: if request.line >= 1 {
            Some(request.line as usize)
        } else {
            None
        },
        column: if request.col >= 1 {
            Some(request.col as usize)
        } else {
            None
        },
    };

    let clan_loc =
        talkbank_model::resolve_clan_location(&location, &source).map_err(|e| e.to_string())?;

    send2clan::send_to_clan(
        0,
        &request.file,
        clan_loc.line as i32,
        clan_loc.column as i32,
        Some(&request.msg),
    )
    .map_err(|e| e.to_string())
}

/// Install the bundled CLI binary to a system path (VS Code-style).
///
/// On macOS/Linux: symlinks to `/usr/local/bin/chatter`.
/// On Windows: copies to a user-writable PATH location.
#[tauri::command]
pub async fn install_cli(app: AppHandle) -> Result<String, String> {
    let resource_path = app
        .path()
        .resource_dir()
        .map_err(|e| e.to_string())?
        .join("resources")
        .join("chatter");

    if !resource_path.exists() {
        return Err(format!(
            "Bundled CLI not found at {}. Build with `cargo build --release -p chatter-cli` first.",
            resource_path.display()
        ));
    }

    #[cfg(unix)]
    {
        let target = std::path::PathBuf::from("/usr/local/bin/chatter");
        // Remove existing symlink or file
        if target.exists() || target.is_symlink() {
            std::fs::remove_file(&target).map_err(|e| {
                format!(
                    "Cannot remove existing {}: {}. Try running with sudo.",
                    target.display(),
                    e
                )
            })?;
        }
        std::os::unix::fs::symlink(&resource_path, &target).map_err(|e| {
            format!(
                "Cannot create symlink at {}: {}. Try running with sudo.",
                target.display(),
                e
            )
        })?;
        Ok(format!(
            "CLI installed: {} -> {}",
            target.display(),
            resource_path.display()
        ))
    }

    #[cfg(windows)]
    {
        let target = dirs::data_local_dir()
            .ok_or("Cannot determine local app data directory")?
            .join("Chatter")
            .join("chatter.exe");
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::copy(&resource_path, &target).map_err(|e| e.to_string())?;
        Ok(format!(
            "CLI installed to {}. Add this directory to your PATH.",
            target.display()
        ))
    }
}

/// Reveal a file in the platform file manager (Finder, Explorer, etc.).
#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), String> {
    let path = std::path::Path::new(&path);
    if !path.exists() {
        return Err(format!("Path does not exist: {}", path.display()));
    }

    #[cfg(target_os = "macos")]
    {
        std::process::Command::new("open")
            .arg("-R")
            .arg(path)
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("explorer")
            .arg(format!("/select,{}", path.display()))
            .spawn()
            .map_err(|e| e.to_string())?;
    }

    #[cfg(target_os = "linux")]
    {
        if let Some(parent) = path.parent() {
            std::process::Command::new("xdg-open")
                .arg(parent)
                .spawn()
                .map_err(|e| e.to_string())?;
        }
    }

    Ok(())
}

/// Export validation results to a file.
#[tauri::command]
pub async fn export_results(
    results: String,
    format: ExportFormat,
    path: String,
) -> Result<(), String> {
    export_results_request(ExportResultsRequest {
        results,
        format,
        path,
    })
}

pub fn export_results_request(request: ExportResultsRequest) -> Result<(), String> {
    let output = match request.format {
        ExportFormat::Json => {
            let parsed: serde_json::Value =
                serde_json::from_str(&request.results).map_err(|e| e.to_string())?;
            serde_json::to_string_pretty(&parsed).map_err(|e| e.to_string())?
        }
        ExportFormat::Text => {
            let parsed: Vec<serde_json::Value> =
                serde_json::from_str(&request.results).map_err(|e| e.to_string())?;
            let mut lines = Vec::new();
            for file_entry in &parsed {
                let path = file_entry["path"].as_str().unwrap_or("?");
                if let Some(errors) = file_entry["errors"].as_array() {
                    for error in errors {
                        let code = error["code"].as_str().unwrap_or("?");
                        let msg = error["message"].as_str().unwrap_or("?");
                        let line = error["location"]["line"]
                            .as_u64()
                            .map(|n| n.to_string())
                            .unwrap_or_default();
                        lines.push(format!("{path}:{line}: {code} {msg}"));
                    }
                }
            }
            lines.join("\n")
        }
    };

    std::fs::write(&request.path, output).map_err(|e| e.to_string())
}
