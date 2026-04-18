// Prevents additional console window on Windows in release, DO NOT REMOVE!!
// Exception: when the "webdriver" feature is enabled (e.g. Windows CI), we
// need a subsystem that doesn't block stdin/stdout so the WebDriver server can
// start – but actually "windows" subsystem is fine for network servers too.
#![cfg_attr(
    all(not(debug_assertions), not(feature = "webdriver")),
    windows_subsystem = "windows"
)]

mod commands;
mod session;

fn main() {
    let sessions = session::new_session_store();


    // Build the Tauri app step-by-step so that debug-only plugins can be
    // inserted via conditional compilation without losing the chain.
    let builder = tauri::Builder::default()
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
        ]);

    // When the "webdriver" feature is enabled, embed a W3C WebDriver server
    // (port 4445) so that WebDriverIO E2E tests can automate the app on every
    // platform, including macOS where the standalone `tauri-driver` binary is
    // not supported. Build with `--features webdriver` for E2E CI builds.
    #[cfg(feature = "webdriver")]
    let builder = builder.plugin(tauri_plugin_webdriver::init());

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
