// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod session;

fn main() {
    let sessions = session::new_session_store();

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(sessions)
        .invoke_handler(tauri::generate_handler![
            commands::get_version,
            commands::list_pdfs,
            commands::load_pdf,
            commands::pick_file,
            commands::merge_pieces,
            commands::get_image,
            commands::get_brick_image,
            commands::get_piece_image,
            commands::get_piece_outline_image,
            commands::export_data,
            commands::check_for_updates,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
