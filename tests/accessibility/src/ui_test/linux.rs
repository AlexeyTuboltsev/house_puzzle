//! Linux backend: AT-SPI accessibility via python3 subprocess
//!
//! WebKitGTK exposes the full web accessibility tree through AT-SPI.
//! We use python3 with gi.repository.Atspi to find and click buttons
//! by their accessible name — this is the only reliable approach on Linux
//! as xdotool coordinate clicks don't register in the webview.

use super::App;
use std::process::Command;
use std::time::{Duration, Instant};

/// Python script that finds and clicks a button via AT-SPI.
/// Takes app process name and button name as arguments.
const ATSPI_CLICK_SCRIPT: &str = r#"
import sys, gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

Atspi.init()
app_name = sys.argv[1]
button_name = sys.argv[2]

desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app_name in (app.get_name() or ''):
        def find_button(node, name):
            try:
                if node.get_role_name() == 'push button' and name in (node.get_name() or ''):
                    return node
                for j in range(node.get_child_count()):
                    r = find_button(node.get_child_at_index(j), name)
                    if r:
                        return r
            except:
                pass
            return None
        btn = find_button(app, button_name)
        if btn:
            btn.get_action_iface().do_action(0)
            print("clicked")
            sys.exit(0)
        else:
            print("not-found")
            sys.exit(1)

print("app-not-found")
sys.exit(1)
"#;

/// Python script that lists all buttons (for debugging).
const ATSPI_LIST_SCRIPT: &str = r#"
import sys, gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi

Atspi.init()
app_name = sys.argv[1]

desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if app_name in (app.get_name() or ''):
        def list_buttons(node):
            try:
                if node.get_role_name() == 'push button':
                    print(node.get_name() or "(unnamed)")
                for j in range(node.get_child_count()):
                    list_buttons(node.get_child_at_index(j))
            except:
                pass
        list_buttons(app)
        sys.exit(0)

print("app-not-found")
sys.exit(1)
"#;

pub fn wait_for_window(app: &App, timeout: Duration) -> bool {
    let start = Instant::now();
    let name = &app.name;
    while start.elapsed() < timeout {
        // Check if AT-SPI can find the app with at least one child
        let out = Command::new("python3")
            .args(["-c", &format!(r#"
import gi
gi.require_version('Atspi', '2.0')
from gi.repository import Atspi
Atspi.init()
desktop = Atspi.get_desktop(0)
for i in range(desktop.get_child_count()):
    app = desktop.get_child_at_index(i)
    if '{name}' in (app.get_name() or ''):
        if app.get_child_count() > 0:
            print('found')
            exit(0)
print('not-found')
"#)])
            .output();
        if let Ok(o) = out {
            if String::from_utf8_lossy(&o.stdout).trim() == "found" {
                return true;
            }
        }
        std::thread::sleep(Duration::from_secs(1));
    }
    false
}

pub fn click_button(app: &App, name: &str) {
    let out = Command::new("python3")
        .args(["-c", ATSPI_CLICK_SCRIPT, &app.name, name])
        .output();
    match out {
        Ok(o) => {
            let result = String::from_utf8_lossy(&o.stdout).trim().to_string();
            let stderr = String::from_utf8_lossy(&o.stderr).trim().to_string();
            if !stderr.is_empty() && !stderr.contains("DeprecationWarning") {
                println!("[linux] click_button('{name}'): stderr={stderr}");
            }
            println!("[linux] click_button('{name}'): {result}");
            if result == "not-found" {
                // Debug: list all available buttons
                println!("[linux] Available buttons:");
                if let Ok(o2) = Command::new("python3")
                    .args(["-c", ATSPI_LIST_SCRIPT, &app.name])
                    .output()
                {
                    for line in String::from_utf8_lossy(&o2.stdout).lines() {
                        println!("[linux]   - \"{line}\"");
                    }
                }
            }
        }
        Err(e) => println!("[linux] click_button('{name}') failed: {e}"),
    }
}

pub fn screenshot(path: &str) {
    // Try gnome-screenshot (window), then scrot, then import
    let ok = Command::new("gnome-screenshot")
        .args(["--window", "--file", path])
        .status()
        .map(|s| s.success())
        .unwrap_or(false);
    if !ok {
        let ok2 = Command::new("scrot")
            .args([path])  // full screen fallback
            .status()
            .map(|s| s.success())
            .unwrap_or(false);
        if !ok2 {
            Command::new("import")
                .args(["-window", "root", path])
                .status()
                .ok();
        }
    }
}
