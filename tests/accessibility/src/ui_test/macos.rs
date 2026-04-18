//! macOS backend: AppleScript + System Events

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
    let script = format!(r#"
tell application "System Events"
    tell process "{process}"
        set frontmost to true
        delay 0.5
        try
            repeat with w in windows
                -- Direct buttons
                repeat with b in buttons of w
                    try
                        if name of b contains "{name}" then
                            click b
                            return "clicked"
                        end if
                    end try
                end repeat
                -- Buttons inside groups (webview)
                repeat with g in groups of w
                    repeat with b in buttons of g
                        try
                            if name of b contains "{name}" then
                                click b
                                return "clicked"
                            end if
                        end try
                    end repeat
                    -- Nested groups
                    repeat with g2 in groups of g
                        repeat with b in buttons of g2
                            try
                                if name of b contains "{name}" then
                                    click b
                                    return "clicked"
                                end if
                            end try
                        end repeat
                    end repeat
                end repeat
            end repeat
        end try
        return "not found"
    end tell
end tell
"#, process = app.name, name = name);

    match Command::new("osascript").args(["-e", &script]).output() {
        Ok(o) => {
            let r = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let e = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if !e.is_empty() {
                println!("[macos] click_button('{name}'): stderr={e}");
            }
            println!("[macos] click_button('{name}'): {r}");
        }
        Err(e) => println!("[macos] click_button('{name}') failed: {e}"),
    }
}

pub fn screenshot(path: &str) {
    Command::new("screencapture").args(["-x", path]).status().ok();
}
