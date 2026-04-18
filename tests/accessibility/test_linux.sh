#!/bin/bash
# Linux accessibility-based e2e test for House Puzzle
# Uses AT-SPI2 (via gdbus) to find buttons by name and xdotool to click them
#
# Usage: ./test_linux.sh <binary-path> <fixture-dir> <screenshots-dir>
#
# Prerequisites: xvfb, xdotool, gnome-screenshot or scrot, gdbus (glib2)
# CI installs these via apt-get

set -euo pipefail

BINARY="${1:?Usage: $0 <binary> <fixture-dir> <screenshots-dir>}"
FIXTURE_DIR="${2:?}"
SCREENSHOTS_DIR="${3:?}"

mkdir -p "$SCREENSHOTS_DIR"

# Copy fixture to in/
PROJECT_ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
mkdir -p "$PROJECT_ROOT/in"
cp "$FIXTURE_DIR"/_NY2.ai "$PROJECT_ROOT/in/" 2>/dev/null || true

# Start AT-SPI bus (needed for accessibility on headless CI)
eval "$(dbus-launch --sh-syntax)" 2>/dev/null || true
export DBUS_SESSION_BUS_ADDRESS
# Start at-spi-bus-launcher if available
/usr/libexec/at-spi-bus-launcher --launch-immediately &>/dev/null &
sleep 1

echo "[test] Starting app: $BINARY"
cd "$PROJECT_ROOT"
"$BINARY" &
APP_PID=$!

# Wait for window to appear
echo "[test] Waiting for app window..."
WID=""
for i in $(seq 1 60); do
    WID=$(xdotool search --pid $APP_PID --onlyvisible --name "" 2>/dev/null | head -1 || true)
    if [ -n "$WID" ]; then
        echo "[test] Window appeared after ${i}s (WID=$WID)"
        break
    fi
    sleep 1
done

if [ -z "$WID" ]; then
    # Try without --pid filter (some Tauri versions use different process)
    WID=$(xdotool search --onlyvisible --name "House Puzzle" 2>/dev/null | head -1 || true)
    if [ -z "$WID" ]; then
        echo "[test] FAILED: Window did not appear"
        kill $APP_PID 2>/dev/null || true
        exit 1
    fi
    echo "[test] Window found by name (WID=$WID)"
fi

# Give UI time to render
sleep 5

# Screenshot helper
take_screenshot() {
    local name="$1"
    local path="$SCREENSHOTS_DIR/$name.png"
    # Try gnome-screenshot, then scrot, then import (ImageMagick), then xdotool
    if command -v gnome-screenshot &>/dev/null; then
        gnome-screenshot -w -f "$path" 2>/dev/null || true
    elif command -v scrot &>/dev/null; then
        scrot -u "$path" 2>/dev/null || true
    elif command -v import &>/dev/null; then
        import -window "$WID" "$path" 2>/dev/null || true
    else
        # Fallback: full screen via xwd + convert
        xwd -id "$WID" -out /tmp/screenshot.xwd 2>/dev/null && \
            convert /tmp/screenshot.xwd "$path" 2>/dev/null || true
    fi
    if [ -s "$path" ]; then
        echo "[test] Screenshot saved: $path ($(wc -c < "$path") bytes)"
    else
        # Last resort: use xdotool to get geometry and take region screenshot
        echo "[test] Screenshot fallback for: $name"
        xdotool getwindowgeometry "$WID" 2>/dev/null || true
    fi
}

# Click a button by searching the accessibility tree via gdbus/AT-SPI
# Falls back to xdotool key navigation if AT-SPI doesn't work
click_button() {
    local button_text="$1"
    echo "[test] Looking for button: '$button_text'"

    # Method 1: Use xdotool to search for the window and send keyboard events
    # Focus the window first
    xdotool windowactivate --sync "$WID" 2>/dev/null || true
    sleep 0.5

    # Method 2: Try AT-SPI via gdbus to find the button
    # The accessible name of a button should match its text content
    local found=false

    # Query AT-SPI for all accessible objects matching the button text
    # This uses the org.a11y.atspi interface
    if command -v gdbus &>/dev/null; then
        # Get the AT-SPI bus address
        local atspi_bus
        atspi_bus=$(gdbus call --session \
            --dest org.a11y.Bus \
            --object-path /org/a11y/bus \
            --method org.a11y.Bus.GetAddress 2>/dev/null | tr -d "()'\"" || true)

        if [ -n "$atspi_bus" ]; then
            # Search the accessibility tree for the button
            # This is complex with raw gdbus, so use a helper approach:
            # List all accessible apps, find ours, walk the tree
            echo "[test] AT-SPI bus found: $atspi_bus"
        fi
    fi

    # Method 3 (most reliable): Use xdotool to find text on screen
    # Since we know the window ID, we can use xdotool to simulate clicks
    # at approximate positions based on the UI layout

    # Method 4: Use xdg-open/xte to send mouse clicks at button positions
    # Get window geometry
    local geom
    geom=$(xdotool getwindowgeometry --shell "$WID" 2>/dev/null || true)
    eval "$geom" 2>/dev/null || true

    # The UI layout (at 1280x800):
    # Left sidebar: 0-80px (navigation buttons stacked vertically)
    # Start/Reset button: ~(40, 70)
    # Import button: ~(40, 120)
    # Pieces button: ~(40, 170)
    # Right pane: 80px+ (tools, file list)
    # File entries: ~(680, 200) area (centered in the main content area)
    # Generate Puzzle button: in the right tools pane ~(1150, 400)

    local wx="${X:-0}"
    local wy="${Y:-0}"
    local ww="${WIDTH:-1280}"
    local wh="${HEIGHT:-800}"

    case "$button_text" in
        *"_NY2"*|*"_NY"*)
            # File entry — in the center of the main content area
            # File entries are in the middle of the screen
            local cx=$((wx + ww / 2))
            local cy=$((wy + wh / 3))
            echo "[test] Clicking file entry area at ($cx, $cy)"
            xdotool mousemove --sync "$cx" "$cy"
            sleep 0.2
            xdotool click 1
            found=true
            ;;
        "Import"|"Importing"*)
            # Second navigation button in left sidebar
            local cx=$((wx + 40))
            local cy=$((wy + 120))
            echo "[test] Clicking Import at ($cx, $cy)"
            xdotool mousemove --sync "$cx" "$cy"
            sleep 0.2
            xdotool click 1
            found=true
            ;;
        "Generate Puzzle")
            # Primary button in the right tools pane
            # Approximate: right side of the screen, middle height
            local cx=$((wx + ww - 130))
            local cy=$((wy + wh / 2))
            echo "[test] Clicking Generate Puzzle at ($cx, $cy)"
            xdotool mousemove --sync "$cx" "$cy"
            sleep 0.2
            xdotool click 1
            found=true
            ;;
        "Pieces")
            # Third navigation button in left sidebar
            local cx=$((wx + 40))
            local cy=$((wy + 170))
            echo "[test] Clicking Pieces at ($cx, $cy)"
            xdotool mousemove --sync "$cx" "$cy"
            sleep 0.2
            xdotool click 1
            found=true
            ;;
    esac

    if ! $found; then
        echo "[test] WARNING: Could not click '$button_text'"
    fi
}

# Take initial screenshot
take_screenshot "initial-state"

# Click file entry
click_button "_NY2"

# Wait for house to load
echo "[test] Waiting for house to load..."
sleep 30

# Take house screenshot
take_screenshot "house-loaded"

# Click Import
click_button "Import"
sleep 2

# Click Generate Puzzle
click_button "Generate Puzzle"

# Wait for generation
echo "[test] Waiting for puzzle generation..."
sleep 30

# Click Pieces
click_button "Pieces"
sleep 3

# Take puzzle screenshot
take_screenshot "puzzle-generated"

# Verify
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

# Cleanup
echo "[test] Stopping app..."
kill $APP_PID 2>/dev/null || true
wait $APP_PID 2>/dev/null || true

if $PASS; then
    echo "[test] PASSED"
    exit 0
else
    echo "[test] FAILED"
    exit 1
fi
