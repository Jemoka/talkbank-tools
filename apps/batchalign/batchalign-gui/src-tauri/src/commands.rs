//! `#[tauri::command]` handlers — all the frontend invokes hit here.
//!
//! Pattern: every command returns `Result<T, String>` so the frontend
//! sees plain-text reasons instead of an opaque IPC error. Heavy work
//! (HTTP, FS scans) happens in tokio tasks; commands themselves return
//! as fast as possible.

use std::path::{Path, PathBuf};

use serde::Serialize;
use tauri::{AppHandle, State};
use walkdir::WalkDir;

use crate::daemon;
use crate::events as event_pump;
use crate::state::AppState;

const AUDIO_EXTS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "opus", "webm",
];
const CHAT_EXTS: &[&str] = &["cha", "chat"];

#[tauri::command]
pub async fn ensure_daemon(app: AppHandle) -> Result<u16, String> {
    daemon::ensure(app).await
}

#[tauri::command]
pub async fn daemon_port(state: State<'_, AppState>) -> Result<Option<u16>, String> {
    Ok(state.daemon_port())
}

#[derive(Debug, Serialize)]
pub struct FolderFile {
    pub source_id: String,
    pub stem: String,
    pub filename: String,
    pub size_bytes: u64,
    pub duration_ms: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct FolderSummary {
    pub files: Vec<FolderFile>,
}

#[tauri::command]
pub async fn list_folder_files(path: String) -> Result<FolderSummary, String> {
    let root = PathBuf::from(&path);
    if !root.is_dir() {
        return Err(format!("not a directory: {path}"));
    }
    let mut files: Vec<FolderFile> = Vec::new();
    // Surface only audio/cha files; the daemon will decide what to do
    // with each in the pipeline. We don't probe audio duration here
    // (avoids pulling a heavy decoder); the table simply shows "—" for
    // duration until the daemon emits it.
    for entry in WalkDir::new(&root).follow_links(false).into_iter().flatten()
    {
        if !entry.file_type().is_file() {
            continue;
        }
        let path = entry.path();
        let ext = path
            .extension()
            .and_then(|s| s.to_str())
            .map(|s| s.to_ascii_lowercase())
            .unwrap_or_default();
        if !(AUDIO_EXTS.contains(&ext.as_str()) || CHAT_EXTS.contains(&ext.as_str())) {
            continue;
        }
        let meta = entry.metadata().map_err(|e| e.to_string())?;
        let stem = path
            .file_stem()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let filename = path
            .file_name()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_string();
        let rel = path
            .strip_prefix(&root)
            .unwrap_or(path)
            .to_string_lossy()
            .to_string();
        files.push(FolderFile {
            source_id: rel,
            stem,
            filename,
            size_bytes: meta.len(),
            duration_ms: None,
        });
    }
    files.sort_by(|a, b| a.source_id.cmp(&b.source_id));
    Ok(FolderSummary { files })
}

#[tauri::command]
pub async fn reveal_in_file_manager(path: String) -> Result<(), String> {
    // Use the opener plugin's reveal-in-dir from the frontend if available;
    // here we fall back to a platform-specific open. The opener plugin's
    // `reveal_item_in_dir` covers macOS Finder / Linux file managers /
    // Windows Explorer; for the MVP we just open the parent dir.
    let p = Path::new(&path);
    let target = if p.is_dir() {
        p.to_path_buf()
    } else {
        p.parent().unwrap_or(p).to_path_buf()
    };
    open_path(&target).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
fn open_path(p: &Path) -> std::io::Result<()> {
    std::process::Command::new("open").arg(p).status()?;
    Ok(())
}

#[cfg(target_os = "linux")]
fn open_path(p: &Path) -> std::io::Result<()> {
    std::process::Command::new("xdg-open").arg(p).status()?;
    Ok(())
}

#[cfg(target_os = "windows")]
fn open_path(p: &Path) -> std::io::Result<()> {
    std::process::Command::new("explorer").arg(p).status()?;
    Ok(())
}

#[tauri::command]
pub async fn start_batch_pump(
    app: AppHandle,
    batch_id: String,
    job_id: String,
) -> Result<(), String> {
    tauri::async_runtime::spawn(async move {
        event_pump::pump(app, batch_id, job_id).await;
    });
    Ok(())
}
