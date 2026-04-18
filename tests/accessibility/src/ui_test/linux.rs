//! Linux backend: xdotool for window management + coordinate-based clicks
//!
//! Note: Linux xdotool can find windows but can't query button names
//! from the accessibility tree without python3-atspi. We use known
//! UI layout positions as a practical workaround.
//! Button positions are based on the 80px left sidebar layout.

use super::App;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn wait_for_window(app: &App, timeout: Duration) -> bool {
    let pid = app.pid().to_string();
    let start = Instant::now();
    while start.elapsed() < timeout {
        // Try by PID
        if let Ok(o) = Command::new("xdotool")
            .args(["search", "--pid", &pid, "--onlyvisible", "--name", ""])
            .output()
        {
            if !String::from_utf8_lossy(&o.stdout).trim().is_empty() {
                return true;
            }
        }
        // Try by window title
        if let Ok(o) = Command::new("xdotool")
            .args(["search", "--onlyvisible", "--name", "House Puzzle"])
            .output()
        {
            if !String::from_utf8_lossy(&o.stdout).trim().is_empty() {
                return true;
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}

/// Get the window ID and geometry.
fn get_window_info() -> Option<(String, i32, i32, i32, i32)> {
    let wid_out = Command::new("xdotool")
        .args(["search", "--onlyvisible", "--name", "House Puzzle"])
        .output().ok()?;
    let wid = String::from_utf8_lossy(&wid_out.stdout)
        .trim().lines().next()?.to_string();
    if wid.is_empty() { return None; }

    let geom_out = Command::new("xdotool")
        .args(["getwindowgeometry", "--shell", &wid])
        .output().ok()?;
    let geom = String::from_utf8_lossy(&geom_out.stdout).to_string();

    let mut x = 0i32;
    let mut y = 0i32;
    let mut w = 1280i32;
    let mut h = 800i32;
    for line in geom.lines() {
        if let Some(v) = line.strip_prefix("X=") { x = v.parse().unwrap_or(0); }
        if let Some(v) = line.strip_prefix("Y=") { y = v.parse().unwrap_or(0); }
        if let Some(v) = line.strip_prefix("WIDTH=") { w = v.parse().unwrap_or(1280); }
        if let Some(v) = line.strip_prefix("HEIGHT=") { h = v.parse().unwrap_or(800); }
    }
    Some((wid, x, y, w, h))
}

pub fn click_button(_app: &App, name: &str) {
    let (wid, wx, wy, ww, wh) = match get_window_info() {
        Some(info) => info,
        None => {
            println!("[linux] click_button('{name}'): window not found");
            return;
        }
    };

    // Activate window
    Command::new("xdotool").args(["windowactivate", "--sync", &wid]).status().ok();
    std::thread::sleep(Duration::from_millis(300));

    // Map button name to position in the known UI layout:
    // Left sidebar (80px wide): nav buttons stacked ~50px apart starting at ~70px
    // Main content area: center of remaining space
    // Right tools pane: ~260px wide on the right
    let (cx, cy) = match name {
        n if n.contains("_NY") => {
            // File entry: center of the main content area
            (wx + ww / 2, wy + wh / 3)
        }
        "Start" | "Reset" | "Loading…" => (wx + 40, wy + 70),
        "Import" | "Importing…" => (wx + 40, wy + 120),
        "Pieces" => (wx + 40, wy + 170),
        "Blueprint" => (wx + 40, wy + 220),
        "Groups" => (wx + 40, wy + 270),
        "Waves" => (wx + 40, wy + 320),
        "Export" => (wx + 40, wy + 370),
        "Generate Puzzle" | "Generating…" => {
            // Primary button in the right tools pane
            (wx + ww - 130, wy + wh / 2)
        }
        _ => {
            println!("[linux] click_button('{name}'): unknown button");
            return;
        }
    };

    println!("[linux] click_button('{name}') at ({cx}, {cy})");
    Command::new("xdotool")
        .args(["mousemove", "--sync", &cx.to_string(), &cy.to_string()])
        .status().ok();
    std::thread::sleep(Duration::from_millis(200));
    Command::new("xdotool").args(["click", "1"]).status().ok();
}

pub fn screenshot(path: &str) {
    // Try scrot (focused window), then full screen
    let ok = Command::new("scrot").args(["-u", path]).status()
        .map(|s| s.success()).unwrap_or(false);
    if !ok {
        Command::new("scrot").args([path]).status().ok();
    }
}
