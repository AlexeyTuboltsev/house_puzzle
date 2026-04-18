//! macOS backend: file-based command injection
//!
//! macOS System Events can't see into WKWebView. Instead, we write
//! commands to a temp file that the app watches and executes via JS eval.
//! This is the same mechanism used on Windows (see common module).

use super::App;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn wait_for_window(app: &App, timeout: Duration) -> bool {
    let start = Instant::now();
    while start.elapsed() < timeout {
        let out = Command::new("osascript")
            .args(["-e", &format!(
                "tell application \"System Events\" to return count of windows of process \"{}\"",
                app.name
            )])
            .output();
        if let Ok(o) = out {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.parse::<i32>().unwrap_or(0) > 0 {
                return true;
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}

pub fn click_button(app: &App, name: &str) {
    super::file_cmd::send_click(name);
}

pub fn screenshot(path: &str) {
    // screencapture works on CI but shows a permission dialog overlay.
    // The app content is still visible behind the dialog.
    // This is a known macOS CI limitation — the screenshots are usable
    // for verifying the app state even with the dialog present.
    Command::new("screencapture").args(["-x", path]).status().ok();
}
