// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
                                        var names = [];
                                        for (var b of buttons) {{
                                            var t = b.textContent.trim();
                                            names.push(t);
                                            if (t.includes('{name}')) {{
                                                b.click();
                                                console.log('[test-click] clicked: ' + t);
                                                return;
                                            }}
                                        }}
                                        console.log('[test-click] not found: {name}, buttons: ' + names.join(', '));
                                    }})()"#,
                                    name = name.replace('\'', "\\'")
                                );
                                let _ = window.eval(&js);
                            } else if let Some(path) = cmd.strip_prefix("screenshot:") {
                                // Call save_screenshot via JS invoke (triggers native capture)
                                let save_path = path.replace('\\', "\\\\").replace('\'', "\\'");
                                let js = format!(
                                    "window.__TAURI__.core.invoke('save_screenshot', {{ path: '{save_path}' }}).catch(function(e) {{ console.error('screenshot:', e); }});"
                                );
                                let _ = window.eval(&js);
                                std::thread::sleep(std::time::Duration::from_secs(3));
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
            commands::save_screenshot,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
