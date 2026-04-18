//! File-based command injection for platforms where accessibility APIs
//! can't reach into the webview (macOS, Windows).
//!
//! Protocol:
//!   1. Test runner writes a command to HP_TEST_CMD_FILE (temp path)
//!   2. App polls this file every 500ms (when --test-mode is enabled)
//!   3. App reads the command, executes JS in the webview, deletes the file
//!   4. Test runner waits for the file to be deleted (= command executed)
//!
//! Command format: "click:<button_text>"

use std::path::PathBuf;
use std::time::Duration;

/// The path to the command file, shared between test runner and app.
pub fn cmd_file_path() -> PathBuf {
    std::env::temp_dir().join("hp_test_cmd")
}

/// Write a click command and wait for the app to process it.
pub fn send_click(name: &str) {
    let path = cmd_file_path();
    let cmd = format!("click:{name}");
    std::fs::write(&path, &cmd).ok();
    println!("[file_cmd] wrote: {cmd}");

    // Wait for the file to be deleted (app processed it)
    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(10) {
        if !path.exists() {
            println!("[file_cmd] command processed");
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("[file_cmd] WARNING: command not processed within 10s");
}
