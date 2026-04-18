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

    // Check for --test-mode flag (enables file-based command injection for e2e tests)
    let test_mode = std::env::args().any(|a| a == "--test-mode");

    // Build the Tauri app step-by-step so that debug-only plugins can be
    // inserted via conditional compilation without losing the chain.
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(sessions)
        .setup(move |app| {
            // In test mode, spawn a watcher thread that polls for commands
            if test_mode {
                use tauri::Manager;
                let window = app.webview_windows().into_values().next()
                    .expect("No window found");
                std::thread::spawn(move || {
                    let cmd_path = std::env::temp_dir().join("hp_test_cmd");
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        if let Ok(cmd) = std::fs::read_to_string(&cmd_path) {
                            let cmd = cmd.trim().to_string();
                            if cmd.is_empty() { continue; }
                            eprintln!("[test-mode] command: {cmd}");
                            if let Some(name) = cmd.strip_prefix("click:") {
                                // Execute JS to find and click the button
                                let js = format!(
                                    r#"(function(){{
                                        var buttons = document.querySelectorAll('button');
                                        for (var b of buttons) {{
                                            if (b.textContent.includes('{name}')) {{
                                                b.click();
                                                return 'clicked';
                                            }}
                                        }}
                                        return 'not-found';
                                    }})()"#,
                                    name = name.replace('\'', "\\'")
                                );
                                let _ = window.eval(&js);
                            }
                            // Delete the file to signal completion
                            std::fs::remove_file(&cmd_path).ok();
                        }
                    }
                });
                eprintln!("[test-mode] command watcher started");
            }
            Ok(())
        })
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
