//! Batchalign desktop GUI — Tauri v2 backend.
//!
//! Responsibilities (and *only* these — domain logic lives in the
//! daemon):
//!   1. Spawn the bundled sidecar daemon and announce its port to the
//!      frontend via `daemon-ready`.
//!   2. Expose filesystem helpers (folder scan, reveal-in-finder).
//!   3. Relay daemon SSE progress events into one Tauri channel
//!      (`progress-v2`) tagged with the originating `batchId`.

mod commands;
mod daemon;
mod events;
mod protocol;
mod state;

use crate::state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .manage(AppState::new())
        .setup(|app| {
            // Spawn the daemon as early as we can; the frontend will
            // start listening for `daemon-ready` on mount.
            daemon::spawn(app.handle().clone());
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::ensure_daemon,
            commands::daemon_port,
            commands::list_folder_files,
            commands::reveal_in_file_manager,
            commands::start_batch_pump,
        ])
        .run(tauri::generate_context!())
        .expect("error while running batchalign desktop");
}
