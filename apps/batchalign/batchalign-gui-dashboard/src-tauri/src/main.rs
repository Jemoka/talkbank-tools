//! Tauri desktop shell for Batchalign3.
//!
//! Provides native file dialog integration, a `discover_files` command for
//! enumerating input files, server lifecycle management (auto-start on launch,
//! auto-stop on exit), user config management for the setup wizard, and one
//! explicit protocol module that mirrors the frontend's desktop boundary.

#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod config;
mod protocol;
mod server;

use std::path::Path;

use tauri::Manager;

use server::ServerProcess;

/// Enumerate files under one root directory for the desktop shell.
///
/// The helper stays separate from the Tauri command so the shell can unit-test
/// the directory contract without standing up a full desktop app instance.
fn discover_files_in_dir(root: &Path, extensions: &[String]) -> Result<Vec<String>, String> {
    if !root.is_dir() {
        return Err(format!("Not a directory: {}", root.display()));
    }

    let exts: Vec<String> = extensions.iter().map(|e| e.to_lowercase()).collect();
    let mut files = Vec::new();

    fn walk(dir: &Path, exts: &[String], out: &mut Vec<String>) -> Result<(), String> {
        let entries =
            std::fs::read_dir(dir).map_err(|e| format!("Cannot read {}: {e}", dir.display()))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Read error in {}: {e}", dir.display()))?;
            let path = entry.path();

            if path.is_dir() {
                walk(&path, exts, out)?;
            } else if let Some(ext) = path.extension() {
                let ext_lower = ext.to_string_lossy().to_lowercase();
                if exts.is_empty() || exts.contains(&ext_lower) {
                    out.push(path.to_string_lossy().into_owned());
                }
            }
        }
        Ok(())
    }

    walk(root, &exts, &mut files)?;
    files.sort();
    Ok(files)
}

/// Walk a directory and return paths matching the given extensions.
///
/// The batchalign server's `POST /jobs` endpoint in paths_mode requires individual
/// file paths in `source_paths`. This command bridges the gap between the user
/// picking a folder and the API needing file paths.
#[tauri::command]
fn discover_files(dir: String, extensions: Vec<String>) -> Result<Vec<String>, String> {
    discover_files_in_dir(Path::new(&dir), &extensions)
}

fn main() {
    let server_state = ServerProcess::new();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_shell::init())
        .manage(server_state)
        .invoke_handler(tauri::generate_handler![
            discover_files,
            server::start_server,
            server::stop_server,
            server::server_status,
            server::get_batchalign_path,
            config::is_first_launch,
            config::read_config,
            config::write_config,
        ])
        .setup(|app| {
            // Auto-start the server on app launch.
            let state = app.state::<ServerProcess>();
            match server::start_managed_server(app.handle(), state.inner()) {
                Ok(status) => eprintln!(
                    "[tauri] Server running={}, pid={:?}, port={}",
                    status.running, status.pid, status.port
                ),
                Err(e) => eprintln!("[tauri] Server auto-start failed: {e}"),
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            // Auto-stop server when the main window is destroyed (app closing).
            if let tauri::WindowEvent::Destroyed = event {
                if window.label() == "main" {
                    let state = window.state::<ServerProcess>();
                    server::shutdown_server(state.inner());
                }
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running batchalign desktop");
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    struct TempDirGuard {
        path: PathBuf,
    }

    impl TempDirGuard {
        fn new() -> Self {
            let unique = format!(
                "batchalign-dashboard-desktop-main-{}-{}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .expect("system time before unix epoch")
                    .as_nanos()
            );
            let path = std::env::temp_dir().join(unique);
            fs::create_dir_all(&path).expect("create temp dir");
            Self { path }
        }

        fn path(&self) -> &Path {
            &self.path
        }
    }

    impl Drop for TempDirGuard {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.path);
        }
    }

    #[test]
    fn discover_files_recurses_filters_and_sorts() {
        let temp = TempDirGuard::new();
        let nested = temp.path().join("nested");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(temp.path().join("keep.CHA"), "").expect("write first file");
        fs::write(temp.path().join("skip.txt"), "").expect("write skipped file");
        fs::write(nested.join("also-keep.wav"), "").expect("write nested file");

        let files = discover_files_in_dir(temp.path(), &["cha".into(), "WAV".into()])
            .expect("discover matching files");

        assert_eq!(
            files,
            vec![
                temp.path().join("keep.CHA").to_string_lossy().into_owned(),
                nested.join("also-keep.wav").to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn discover_files_with_empty_extensions_returns_all_files() {
        let temp = TempDirGuard::new();
        let nested = temp.path().join("nested");
        fs::create_dir_all(&nested).expect("create nested dir");
        fs::write(temp.path().join("alpha.cha"), "").expect("write first file");
        fs::write(temp.path().join("beta.txt"), "").expect("write second file");
        fs::write(nested.join("gamma.wav"), "").expect("write nested file");

        let files = discover_files_in_dir(temp.path(), &[]).expect("discover all files");

        assert_eq!(
            files,
            vec![
                temp.path().join("alpha.cha").to_string_lossy().into_owned(),
                temp.path().join("beta.txt").to_string_lossy().into_owned(),
                nested.join("gamma.wav").to_string_lossy().into_owned(),
            ]
        );
    }

    #[test]
    fn discover_files_rejects_non_directories() {
        let temp = TempDirGuard::new();
        let file_path = temp.path().join("file.cha");
        fs::write(&file_path, "").expect("write temp file");

        let error = discover_files_in_dir(&file_path, &[]).expect_err("reject plain file path");

        assert!(error.contains("Not a directory"));
    }
}
