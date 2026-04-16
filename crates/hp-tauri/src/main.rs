// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;
mod session;

fn main() {
    let sessions = session::new_session_store();

    tauri::Builder::default()
        .manage(sessions)
        .invoke_handler(tauri::generate_handler![
            commands::get_version,
            commands::list_pdfs,
            commands::load_pdf,
            commands::merge_pieces,
            commands::get_image,
            commands::get_brick_image,
            commands::get_piece_image,
            commands::get_piece_outline_image,
            commands::export_data,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
