mod commands;
pub mod events;
pub mod protocol;
pub mod validation;

use commands::ValidationState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .manage(ValidationState::new())
        .invoke_handler(tauri::generate_handler![
            commands::validate,
            commands::cancel_validation,
            commands::check_clan_available,
            commands::open_in_clan,
            commands::export_results,
            commands::reveal_in_file_manager,
            commands::install_cli,
        ])
        .run(tauri::generate_context!())
        .expect("error while running chatter desktop");
}
