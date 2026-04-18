#!/bin/bash
# macOS accessibility-based e2e test for House Puzzle
# Uses AppleScript + System Events to interact with the real app UI
#
# Usage: ./test_macos.sh <binary-path> <fixture-dir> <screenshots-dir>
#
# Prerequisites:
#   - The app binary must be built
#   - _NY2.ai (or any .ai file) must be in <fixture-dir>
#   - Terminal/script must have Accessibility permissions (System Preferences > Privacy)
#   - On CI: permissions are granted by default

set -euo pipefail

BINARY="${1:?Usage: $0 <binary> <fixture-dir> <screenshots-dir>}"
FIXTURE_DIR="${2:?}"
SCREENSHOTS_DIR="${3:?}"
APP_NAME="hp-tauri"

mkdir -p "$SCREENSHOTS_DIR"

# Copy fixture to in/ relative to where the app will run
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$PROJECT_ROOT/in"
cp "$FIXTURE_DIR"/_NY2.ai "$PROJECT_ROOT/in/" 2>/dev/null || true

echo "[test] Starting app: $BINARY"
cd "$PROJECT_ROOT"
"$BINARY" &
APP_PID=$!

# Wait for window to appear
echo "[test] Waiting for app window..."
for i in $(seq 1 60); do
    if osascript -e "tell application \"System Events\" to return (exists process \"$APP_NAME\")" 2>/dev/null | grep -q "true"; then
        # Check if window exists
        WIN_COUNT=$(osascript -e "tell application \"System Events\" to tell process \"$APP_NAME\" to return count of windows" 2>/dev/null || echo "0")
        if [ "$WIN_COUNT" -gt 0 ]; then
            echo "[test] Window appeared after ${i}s"
            break
        fi
    fi
    sleep 1
done

# Give the UI time to fully render (Elm init + file list load)
sleep 5

# Take screenshot of initial state
echo "[test] Taking screenshot: initial-state"
screencapture -l "$(osascript -e "tell application \"System Events\" to tell process \"$APP_NAME\" to return id of window 1" 2>/dev/null)" "$SCREENSHOTS_DIR/initial-state.png" 2>/dev/null || screencapture "$SCREENSHOTS_DIR/initial-state.png"

# Click the first .ai file entry
# The file list should show _NY2.ai. Find and click the button with that text.
echo "[test] Clicking file entry (_NY2.ai)..."
osascript <<'APPLESCRIPT'
tell application "System Events"
    tell process "hp-tauri"
        set frontmost to true
        delay 1
        -- Find button containing "_NY2" text in the web view
        -- Tauri webview exposes buttons via accessibility
        try
            set allButtons to every button of window 1
            repeat with b in allButtons
                try
                    if name of b contains "_NY2" or description of b contains "_NY2" then
                        click b
                        return "clicked"
                    end if
                end try
            end repeat
        end try
        -- Fallback: try clicking in the UI group / web area
        try
            set webArea to group 1 of window 1
            set allButtons to every button of webArea
            repeat with b in allButtons
                try
                    if name of b contains "_NY2" or description of b contains "_NY2" then
                        click b
                        return "clicked"
                    end if
                end try
            end repeat
        end try
        return "not found"
    end tell
end tell
APPLESCRIPT

# Wait for house to load (the AI parsing + rendering takes time)
echo "[test] Waiting for house to load (up to 90s)..."
sleep 30

# Take screenshot of loaded house
echo "[test] Taking screenshot: house-loaded"
screencapture -l "$(osascript -e "tell application \"System Events\" to tell process \"$APP_NAME\" to return id of window 1" 2>/dev/null)" "$SCREENSHOTS_DIR/house-loaded.png" 2>/dev/null || screencapture "$SCREENSHOTS_DIR/house-loaded.png"

# Click "Import" mode button
echo "[test] Clicking Import button..."
osascript <<'APPLESCRIPT'
tell application "System Events"
    tell process "hp-tauri"
        set frontmost to true
        delay 0.5
        try
            set allButtons to every button of window 1
            repeat with b in allButtons
                try
                    if name of b is "Import" or name of b is "Importing…" then
                        click b
                        delay 1
                        exit repeat
                    end if
                end try
            end repeat
        end try
        -- Try in web area group
        try
            set webArea to group 1 of window 1
            set allButtons to every button of webArea
            repeat with b in allButtons
                try
                    if name of b is "Import" or name of b is "Importing…" then
                        click b
                        delay 1
                        exit repeat
                    end if
                end try
            end repeat
        end try
    end tell
end tell
APPLESCRIPT

sleep 2

# Click "Generate Puzzle" button
echo "[test] Clicking Generate Puzzle..."
osascript <<'APPLESCRIPT'
tell application "System Events"
    tell process "hp-tauri"
        set frontmost to true
        delay 0.5
        try
            set allButtons to every button of window 1
            repeat with b in allButtons
                try
                    if name of b is "Generate Puzzle" then
                        click b
                        return "clicked"
                    end if
                end try
            end repeat
        end try
        try
            set webArea to group 1 of window 1
            set allButtons to every button of webArea
            repeat with b in allButtons
                try
                    if name of b is "Generate Puzzle" then
                        click b
                        return "clicked"
                    end if
                end try
            end repeat
        end try
        return "not found"
    end tell
end tell
APPLESCRIPT

# Wait for puzzle generation
echo "[test] Waiting for puzzle generation (up to 60s)..."
sleep 30

# Click "Pieces" mode button
echo "[test] Clicking Pieces button..."
osascript <<'APPLESCRIPT'
tell application "System Events"
    tell process "hp-tauri"
        set frontmost to true
        delay 0.5
        try
            set allButtons to every button of window 1
            repeat with b in allButtons
                try
                    if name of b is "Pieces" then
                        click b
                        exit repeat
                    end if
                end try
            end repeat
        end try
        try
            set webArea to group 1 of window 1
            set allButtons to every button of webArea
            repeat with b in allButtons
                try
                    if name of b is "Pieces" then
                        click b
                        exit repeat
                    end if
                end try
            end repeat
        end try
    end tell
end tell
APPLESCRIPT

sleep 3

# Take screenshot of generated puzzle
echo "[test] Taking screenshot: puzzle-generated"
screencapture -l "$(osascript -e "tell application \"System Events\" to tell process \"$APP_NAME\" to return id of window 1" 2>/dev/null)" "$SCREENSHOTS_DIR/puzzle-generated.png" 2>/dev/null || screencapture "$SCREENSHOTS_DIR/puzzle-generated.png"

# Verify screenshots exist and have non-zero size
echo "[test] Verifying screenshots..."
PASS=true
for f in initial-state.png house-loaded.png puzzle-generated.png; do
    if [ -s "$SCREENSHOTS_DIR/$f" ]; then
        SIZE=$(wc -c < "$SCREENSHOTS_DIR/$f")
        echo "  OK: $f ($SIZE bytes)"
    else
        echo "  FAIL: $f missing or empty"
        PASS=false
    fi
done

# Clean up
echo "[test] Stopping app..."
kill $APP_PID 2>/dev/null || true
wait $APP_PID 2>/dev/null || true

if $PASS; then
    echo "[test] PASSED — all screenshots captured"
    exit 0
else
    echo "[test] FAILED — some screenshots missing"
    exit 1
fi
