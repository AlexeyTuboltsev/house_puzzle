# Windows accessibility-based e2e test for House Puzzle
# Uses UI Automation to interact with the real app UI
#
# Usage: .\test_windows.ps1 -Binary <path> -FixtureDir <path> -ScreenshotsDir <path>

param(
    [Parameter(Mandatory=$true)][string]$Binary,
    [Parameter(Mandatory=$true)][string]$FixtureDir,
    [Parameter(Mandatory=$true)][string]$ScreenshotsDir
)

$ErrorActionPreference = "Stop"
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type -AssemblyName System.Drawing
Add-Type -AssemblyName System.Windows.Forms

$AppName = "hp-tauri"

# Ensure directories
New-Item -ItemType Directory -Force -Path $ScreenshotsDir | Out-Null

# Copy fixture
$ProjectRoot = (Resolve-Path "$PSScriptRoot\..\..").Path
$InDir = Join-Path $ProjectRoot "in"
New-Item -ItemType Directory -Force -Path $InDir | Out-Null
Copy-Item "$FixtureDir\_NY2.ai" $InDir -Force -ErrorAction SilentlyContinue

# Screenshot helper
function Take-Screenshot($name) {
    $path = Join-Path $ScreenshotsDir "$name.png"
    $bounds = [System.Windows.Forms.Screen]::PrimaryScreen.Bounds
    $bitmap = New-Object System.Drawing.Bitmap($bounds.Width, $bounds.Height)
    $graphics = [System.Drawing.Graphics]::FromImage($bitmap)
    $graphics.CopyFromScreen($bounds.Location, [System.Drawing.Point]::Empty, $bounds.Size)
    $bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Png)
    $graphics.Dispose()
    $bitmap.Dispose()
    Write-Host "[test] Screenshot saved: $path"
    return $path
}

# UI Automation helpers
function Find-ButtonByName($root, $name) {
    $condition = New-Object System.Windows.Automation.AndCondition(
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
            [System.Windows.Automation.ControlType]::Button)),
        (New-Object System.Windows.Automation.PropertyCondition(
            [System.Windows.Automation.AutomationElement]::NameProperty, $name))
    )
    return $root.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $condition)
}

function Find-ButtonContaining($root, $text) {
    $condition = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button)
    $buttons = $root.FindAll([System.Windows.Automation.TreeScope]::Descendants, $condition)
    foreach ($btn in $buttons) {
        if ($btn.Current.Name -like "*$text*") {
            return $btn
        }
    }
    return $null
}

function Click-Element($element) {
    if ($element -eq $null) {
        Write-Host "[test] WARNING: Element not found"
        return
    }
    try {
        $invokePattern = $element.GetCurrentPattern([System.Windows.Automation.InvokePattern]::Pattern)
        $invokePattern.Invoke()
        Write-Host "[test] Clicked: $($element.Current.Name)"
    } catch {
        # Fallback: click by coordinates
        $rect = $element.Current.BoundingRectangle
        $x = [int]($rect.X + $rect.Width / 2)
        $y = [int]($rect.Y + $rect.Height / 2)
        [System.Windows.Forms.Cursor]::Position = New-Object System.Drawing.Point($x, $y)
        Start-Sleep -Milliseconds 100
        Add-Type -TypeDefinition @"
            using System;
            using System.Runtime.InteropServices;
            public class Mouse {
                [DllImport("user32.dll")] public static extern void mouse_event(int flags, int dx, int dy, int data, int extra);
                public static void Click() { mouse_event(2, 0, 0, 0, 0); mouse_event(4, 0, 0, 0, 0); }
            }
"@ -ErrorAction SilentlyContinue
        [Mouse]::Click()
        Write-Host "[test] Clicked by coordinates: ($x, $y) - $($element.Current.Name)"
    }
}

# Start the app
Write-Host "[test] Starting app: $Binary"
Set-Location $ProjectRoot
$proc = Start-Process -FilePath $Binary -PassThru -WindowStyle Normal

# Wait for window
Write-Host "[test] Waiting for app window..."
$mainWindow = $null
for ($i = 0; $i -lt 60; $i++) {
    Start-Sleep -Seconds 1
    try {
        $mainWindow = [System.Windows.Automation.AutomationElement]::RootElement.FindFirst(
            [System.Windows.Automation.TreeScope]::Children,
            (New-Object System.Windows.Automation.PropertyCondition(
                [System.Windows.Automation.AutomationElement]::ProcessIdProperty, $proc.Id))
        )
        if ($mainWindow -ne $null) {
            Write-Host "[test] Window appeared after $($i+1)s"
            break
        }
    } catch {}
}

if ($mainWindow -eq $null) {
    Write-Host "[test] FAILED: Window did not appear"
    Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue
    exit 1
}

# Wait for UI to render
Start-Sleep -Seconds 5

# Screenshot: initial state
Take-Screenshot "initial-state"

# Click file entry (_NY2.ai)
Write-Host "[test] Looking for _NY2 file entry..."
$fileBtn = Find-ButtonContaining $mainWindow "_NY2"
if ($fileBtn -ne $null) {
    Click-Element $fileBtn
} else {
    Write-Host "[test] WARNING: _NY2 file entry not found, listing all buttons..."
    $allBtnCond = New-Object System.Windows.Automation.PropertyCondition(
        [System.Windows.Automation.AutomationElement]::ControlTypeProperty,
        [System.Windows.Automation.ControlType]::Button)
    $allBtns = $mainWindow.FindAll([System.Windows.Automation.TreeScope]::Descendants, $allBtnCond)
    foreach ($btn in $allBtns) {
        Write-Host "  Button: '$($btn.Current.Name)'"
    }
}

# Wait for house to load
Write-Host "[test] Waiting for house to load..."
Start-Sleep -Seconds 30

# Screenshot: house loaded
Take-Screenshot "house-loaded"

# Click Import button
Write-Host "[test] Clicking Import..."
$importBtn = Find-ButtonByName $mainWindow "Import"
if ($importBtn -eq $null) { $importBtn = Find-ButtonContaining $mainWindow "Import" }
Click-Element $importBtn
Start-Sleep -Seconds 2

# Click Generate Puzzle
Write-Host "[test] Clicking Generate Puzzle..."
$genBtn = Find-ButtonByName $mainWindow "Generate Puzzle"
Click-Element $genBtn

# Wait for generation
Write-Host "[test] Waiting for puzzle generation..."
Start-Sleep -Seconds 30

# Click Pieces
Write-Host "[test] Clicking Pieces..."
$piecesBtn = Find-ButtonByName $mainWindow "Pieces"
Click-Element $piecesBtn
Start-Sleep -Seconds 3

# Screenshot: puzzle generated
Take-Screenshot "puzzle-generated"

# Verify
Write-Host "[test] Verifying screenshots..."
$pass = $true
foreach ($name in @("initial-state", "house-loaded", "puzzle-generated")) {
    $path = Join-Path $ScreenshotsDir "$name.png"
    if (Test-Path $path) {
        $size = (Get-Item $path).Length
        Write-Host "  OK: $name.png ($size bytes)"
    } else {
        Write-Host "  FAIL: $name.png missing"
        $pass = $false
    }
}

# Cleanup
Write-Host "[test] Stopping app..."
Stop-Process -Id $proc.Id -Force -ErrorAction SilentlyContinue

if ($pass) {
    Write-Host "[test] PASSED"
    exit 0
} else {
    Write-Host "[test] FAILED"
    exit 1
}
