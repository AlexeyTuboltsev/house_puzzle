//! Windows backend: PowerShell + UIAutomation

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

pub fn click_button(_app: &App, name: &str) {
    let script = format!(r#"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
$root = [System.Windows.Automation.AutomationElement]::RootElement
$cond = New-Object System.Windows.Automation.PropertyCondition(
    [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
    [System.Windows.Automation.ControlType]::Button)
$buttons = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $cond)
foreach ($btn in $buttons) {{
    if ($btn.Current.Name -like "*{name}*") {{
        try {{
            $inv = $btn.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
            $inv.Invoke()
            Write-Output "clicked"
            exit
        }} catch {{
            $r = $btn.Current.BoundingRectangle
            Add-Type -TypeDefinition 'using System; using System.Runtime.InteropServices; public class MouseClick {{ [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y); [DllImport("user32.dll")] public static extern void mouse_event(int f,int x,int y,int d,int e); }}' -ErrorAction SilentlyContinue
            [MouseClick]::SetCursorPos([int]($r.X+$r.Width/2), [int]($r.Y+$r.Height/2))
            Start-Sleep -Milliseconds 100
            [MouseClick]::mouse_event(2,0,0,0,0); [MouseClick]::mouse_event(4,0,0,0,0)
            Write-Output "clicked-coords"
            exit
        }}
    }}
}}
Write-Output "not-found"
"#);
    match Command::new("powershell").args(["-Command", &script]).output() {
        Ok(o) => {
            let r = String::from_utf8_lossy(&o.stdout).trim().to_string();
            println!("[windows] click_button('{name}'): {r}");
        }
        Err(e) => println!("[windows] click_button('{name}') failed: {e}"),
    }
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
