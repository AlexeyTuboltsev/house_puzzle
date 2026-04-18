//! Cross-platform UI test utility library.
//!
//! Provides a uniform API for interacting with native app UIs:
//! - `App::launch()` — start an app binary
//! - `app.wait_for_window()` — wait for the main window to appear
//! - `app.find_element(by)` — find a UI element by name or test ID
//! - `app.click_element(element)` — click an element
//! - `app.click_button(name)` — find and click a button by name (substring match)
//! - `app.screenshot(dir, name)` — capture a screenshot
//! - `app.sleep(secs)` — wait
//! - `app.close()` — kill the app
//!
//! Platform backends:
//! - macOS: AppleScript + System Events
//! - Windows: PowerShell + UIAutomation
//! - Linux: xdotool (coordinate-based, layout-dependent)

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;
#[cfg(target_os = "linux")]
mod linux;

/// File-based command injection (macOS/Windows fallback)
#[cfg(any(target_os = "macos", target_os = "windows"))]
pub mod file_cmd;

use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::time::Duration;

/// A running application under test.
pub struct App {
    process: Child,
    /// Process name (binary filename without path/extension)
    name: String,
    /// Working directory
    #[allow(dead_code)]
    cwd: String,
}

impl App {
    /// Launch the app binary from the given working directory.
    /// Sets HP_IN_DIR to `cwd/in` so the app can find fixture files.
    /// On macOS/Windows, passes --test-mode for file-based command injection.
    pub fn launch(binary: &str, cwd: &Path) -> Self {
        let in_dir = cwd.join("in");

        // On macOS/Windows, use file-based command injection (--test-mode)
        // On Linux, AT-SPI works directly
        let mut args: Vec<&str> = Vec::new();
        #[cfg(not(target_os = "linux"))]
        args.push("--test-mode");

        println!("[ui_test] Launching: {binary} {:?} (cwd={}, HP_IN_DIR={})", args, cwd.display(), in_dir.display());
        let process = Command::new(binary)
            .args(&args)
            .current_dir(cwd)
            .env("HP_IN_DIR", &in_dir)
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap_or_else(|e| panic!("Failed to launch {binary}: {e}"));

        let name = Path::new(binary)
            .file_stem()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();

        App {
            process,
            name,
            cwd: cwd.to_string_lossy().to_string(),
        }
    }

    /// Process ID of the running app.
    pub fn pid(&self) -> u32 {
        self.process.id()
    }

    /// Wait for the main window to appear, up to `timeout_secs`.
    /// Panics if the window doesn't appear.
    pub fn wait_for_window(&self, timeout_secs: u64) {
        println!("[ui_test] Waiting for window (up to {timeout_secs}s)...");
        let found = platform_wait_for_window(self, Duration::from_secs(timeout_secs));
        if !found {
            panic!("[ui_test] Window did not appear within {timeout_secs}s");
        }
        println!("[ui_test] Window appeared");
    }

    /// Find and click a button by name (substring match).
    /// The name is matched against the button's accessibility label / text content.
    pub fn click_button(&self, name: &str) {
        println!("[ui_test] click_button('{name}')");
        platform_click_button(self, name);
    }

    /// Take a screenshot and save it as `{dir}/{name}.png`.
    pub fn screenshot(&self, dir: &str, name: &str) {
        let path = format!("{dir}/{name}.png");
        println!("[ui_test] screenshot → {path}");
        platform_screenshot(self, &path);
    }

    /// Sleep for the given number of seconds.
    pub fn sleep(&self, secs: u64) {
        std::thread::sleep(Duration::from_secs(secs));
    }

    /// Kill the app process.
    pub fn close(mut self) {
        println!("[ui_test] Closing app");
        self.process.kill().ok();
        self.process.wait().ok();
    }
}

// =============================================================================
// Platform dispatch — each platform module implements these functions
// =============================================================================

#[cfg(target_os = "macos")]
fn platform_wait_for_window(app: &App, timeout: Duration) -> bool {
    macos::wait_for_window(app, timeout)
}
#[cfg(target_os = "macos")]
fn platform_click_button(app: &App, name: &str) {
    macos::click_button(app, name);
}
#[cfg(target_os = "macos")]
fn platform_screenshot(_app: &App, path: &str) {
    macos::screenshot(path);
}

#[cfg(target_os = "windows")]
fn platform_wait_for_window(app: &App, timeout: Duration) -> bool {
    windows::wait_for_window(app, timeout)
}
#[cfg(target_os = "windows")]
fn platform_click_button(app: &App, name: &str) {
    windows::click_button(app, name);
}
#[cfg(target_os = "windows")]
fn platform_screenshot(_app: &App, path: &str) {
    windows::screenshot(path);
}

#[cfg(target_os = "linux")]
fn platform_wait_for_window(app: &App, timeout: Duration) -> bool {
    linux::wait_for_window(app, timeout)
}
#[cfg(target_os = "linux")]
fn platform_click_button(app: &App, name: &str) {
    linux::click_button(app, name);
}
#[cfg(target_os = "linux")]
fn platform_screenshot(_app: &App, path: &str) {
    linux::screenshot(path);
}
