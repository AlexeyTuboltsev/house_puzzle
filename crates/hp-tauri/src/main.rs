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
                    let result_path = std::env::temp_dir().join("hp_test_result");
                    loop {
                        std::thread::sleep(std::time::Duration::from_millis(500));
                        if let Ok(cmd) = std::fs::read_to_string(&cmd_path) {
                            let cmd = cmd.trim().to_string();
                            if cmd.is_empty() { continue; }
                            eprintln!("[test-mode] command: {cmd}");

                            if let Some(name) = cmd.strip_prefix("click:") {
                                // Click button by text content (substring match)
                                let js = format!(
                                    r#"(function(){{
                                        var buttons = document.querySelectorAll('button');
                                        var names = [];
                                        for (var b of buttons) {{
                                            var t = b.textContent.trim();
                                            names.push(t);
                                            if (t.includes('{name}')) {{
                                                b.click();
                                                console.log('[test] clicked: ' + t);
                                                return;
                                            }}
                                        }}
                                        console.log('[test] not found: {name}, buttons: ' + names.join(', '));
                                    }})()"#,
                                    name = name.replace('\'', "\\'")
                                );
                                let _ = window.eval(&js);

                            } else if let Some(tid) = cmd.strip_prefix("click-testid:") {
                                // Click element by data-testid
                                let js = format!(
                                    r#"(function(){{
                                        var el = document.querySelector('[data-testid="{tid}"]');
                                        if (el) {{ el.click(); console.log('[test] clicked testid={tid}'); }}
                                        else {{ console.log('[test] testid not found: {tid}'); }}
                                    }})()"#,
                                    tid = tid.replace('\'', "\\'").replace('"', "\\\"")
                                );
                                let _ = window.eval(&js);

                            } else if let Some(rest) = cmd.strip_prefix("set-value:") {
                                // Set value via Elm port: "set-value:target-pieces=10"
                                if let Some((tid, val)) = rest.split_once('=') {
                                    let js = format!(
                                        r#"(function(){{
                                            var app = window.__hpApp;
                                            var has = !!(app && app.ports && app.ports.testSetValue);
                                            var msg = 'set-value: app=' + !!app + ' port=' + has + ' tid={tid} val={val}';
                                            if (has) {{
                                                app.ports.testSetValue.send({{testId: '{tid}', value: '{val}'}});
                                                msg += ' SENT';
                                            }}
                                            window.__TAURI__.core.invoke('log_to_stderr', {{msg: msg}});
                                        }})()"#,
                                        tid = tid.replace('\'', "\\'"),
                                        val = val.replace('\'', "\\'")
                                    );
                                    let _ = window.eval(&js);
                                }

                            } else if let Some(tid) = cmd.strip_prefix("get-text:") {
                                // Get text content of element by data-testid, write to result file
                                let rp = result_path.display().to_string().replace('\\', "\\\\");
                                let js = format!(
                                    r#"(function(){{
                                        var el = document.querySelector('[data-testid="{tid}"]');
                                        var text = el ? (el.value !== undefined && el.value !== '' ? el.value : el.textContent.trim()) : '';
                                        window.__TAURI__.core.invoke('save_screenshot', {{path: 'noop'}}).catch(function(){{}});
                                        window.__test_result = text;
                                        console.log('[test] get-text: {tid} = ' + text);
                                    }})()"#,
                                    tid = tid.replace('"', "\\\"")
                                );
                                let _ = window.eval(&js);
                                // Write result via a second eval after a short delay
                                std::thread::sleep(std::time::Duration::from_millis(200));
                                let js2 = format!(
                                    r#"(function(){{
                                        var fs = window.__TAURI__.core.invoke;
                                        // Write result to temp file via Tauri
                                        var r = window.__test_result || '';
                                        // Use a dummy invoke to log
                                        console.log('[test] result: ' + r);
                                    }})()"#
                                );
                                let _ = window.eval(&js2);

                            } else if let Some(selector) = cmd.strip_prefix("query:") {
                                // Check if element exists by CSS selector
                                let js = format!(
                                    r#"(function(){{
                                        var el = document.querySelector('{sel}');
                                        console.log('[test] query: {sel} = ' + (el ? 'found' : 'not found'));
                                    }})()"#,
                                    sel = selector.replace('\'', "\\'")
                                );
                                let _ = window.eval(&js);

                            } else if let Some(js_code) = cmd.strip_prefix("eval:") {
                                let _ = window.eval(js_code);

                            } else if let Some(path) = cmd.strip_prefix("screenshot:") {
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
            commands::ensure_lights_image,
            commands::ensure_background_image,
            commands::export_data,
            commands::check_for_updates,
            commands::save_screenshot,
            commands::log_to_stderr,
        ]);

    builder
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
