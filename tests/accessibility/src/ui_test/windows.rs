//! Windows backend: file-based command injection
//!
//! Windows UIAutomation can't see into WebView2. Instead, we write
//! commands to a temp file that the app watches and executes via JS eval.

use super::App;
use std::process::Command;
use std::time::{Duration, Instant};

pub fn wait_for_window(app: &App, timeout: Duration) -> bool {
    let pid = app.pid();
    let start = Instant::now();
    while start.elapsed() < timeout {
        let script = format!(
            "Add-Type -AssemblyName UIAutomationClient; \
             [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(\
             [System.Windows.Automation.TreeScope]::Children, \
             (New-Object System.Windows.Automation.PropertyCondition(\
             [System.Windows.Automation.AutomationElement]::ProcessIdProperty, {pid}))) -ne $null"
        );
        let out = Command::new("powershell").args(["-Command", &script]).output();
        if let Ok(o) = out {
            if String::from_utf8_lossy(&o.stdout).trim() == "True" {
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
    let path_escaped = path.replace('/', "\\");
    let script = format!(r#"
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms
$b = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
$bmp = New-Object System.Drawing.Bitmap($b.Width,$b.Height)
$g = [System.Drawing.Graphics]::FromImage($bmp)
$g.CopyFromScreen($b.Location,[System.Drawing.Point]::Empty,$b.Size)
$bmp.Save('{path_escaped}',[System.Drawing.Imaging.ImageFormat]::Png)
$g.Dispose(); $bmp.Dispose()
"#);
    Command::new("powershell").args(["-Command", &script]).status().ok();
}
