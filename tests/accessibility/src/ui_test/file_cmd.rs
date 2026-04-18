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

/// Write a command and wait for the app to process it (delete the file).
fn send_cmd(cmd: &str, timeout_secs: u64) {
    let path = cmd_file_path();
    std::fs::write(&path, cmd).ok();
    println!("[file_cmd] wrote: {cmd}");

    let start = std::time::Instant::now();
    while start.elapsed() < Duration::from_secs(timeout_secs) {
        if !path.exists() {
            println!("[file_cmd] command processed");
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    println!("[file_cmd] WARNING: command not processed within {timeout_secs}s");
}

/// Click a button by text content.
pub fn send_click(name: &str) {
    send_cmd(&format!("click:{name}"), 10);
}

/// Take a screenshot from inside the webview (no OS permissions needed).
/// The app captures the DOM via canvas and saves to disk via Tauri IPC.
pub fn send_screenshot(path: &str) {
    send_cmd(&format!("screenshot:{path}"), 15);
    // Extra wait for canvas render + file write
    std::thread::sleep(Duration::from_secs(3));
}
