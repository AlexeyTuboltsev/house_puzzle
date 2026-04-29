#!/usr/bin/env bash
# Dev runner for the Tauri editor.
#
# Two background watchers + the Tauri dev shell run together; ^C tears
# them all down.
#
#   1. elm watcher — `cargo-watch` rebuilds `crates/hp-tauri/dist/elm.js`
#      whenever `elm/src/**/*.elm` changes (~0.5 s).
#   2. `cargo tauri dev` — boots the editor window, hot-restarts the
#      Rust side on `crates/**/*.rs` changes.
#
# After elm rebuilds, the Tauri webview does NOT auto-reload (we don't
# run a dev server, just a static frontendDist). Press ⌘R / Ctrl+R in
# the open window to pick up new elm.js bytes. CSS / `dist/index.html`
# tweaks: same — save the file, hit Ctrl+R.
#
# Prereqs (already on this host): `cargo`, `cargo-watch`, `node`, `elm`,
# `webkit2gtk-4.1`, `gtk+-3.0`. Tauri CLI is in `~/.cargo/bin`.
#
# Run from the repo root:
#   ./dev.sh

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")" && pwd)"
ELM_SRC="$REPO_ROOT/elm"
ELM_OUT="$REPO_ROOT/crates/hp-tauri/dist/elm.js"
TAURI_DIR="$REPO_ROOT/crates/hp-tauri"

# Use the elm binary that ships in `elm/node_modules/.bin/` if present
# (matches what CI builds with), else PATH `elm`.
if [ -x "$ELM_SRC/node_modules/.bin/elm" ]; then
    ELM_BIN="$ELM_SRC/node_modules/.bin/elm"
else
    ELM_BIN="$(command -v elm)"
fi
echo "[dev] using elm: $ELM_BIN"

# Initial build of the Elm bundle so the first `cargo tauri dev` boot
# doesn't show a stale (or missing) frontend. The bundle is unoptimised
# in dev — `elm make --optimize` rejects any `Debug.*` use, and Main.elm
# has `Debug.log` calls. CI rebuilds with `--optimize` for releases.
echo "[dev] building elm.js once before starting watchers..."
(
    cd "$ELM_SRC"
    "$ELM_BIN" make src/Main.elm --output "$ELM_OUT" 2>&1
)
echo "[dev] elm.js built: $(du -h "$ELM_OUT" | cut -f1)"

# Cleanup helper — kills child watchers when the script exits.
PIDS=()
cleanup() {
    echo
    echo "[dev] shutting down ${#PIDS[@]} bg job(s)..."
    for pid in "${PIDS[@]}"; do
        kill "$pid" 2>/dev/null || true
    done
    wait 2>/dev/null || true
}
trap cleanup EXIT INT TERM

# Background: elm watcher. cargo-watch resolves --watch paths against
# the cargo workspace root (it walks up to Cargo.toml), so we have to
# pass an absolute path, not `src`. Run cargo-watch from the workspace
# root and `cd elm` inside the rebuild shell.
echo "[dev] starting elm watcher (cargo-watch on $ELM_SRC/src)..."
(
    cd "$REPO_ROOT"
    cargo-watch \
        --watch "$ELM_SRC/src" \
        --shell "(cd \"$ELM_SRC\" && \"$ELM_BIN\" make src/Main.elm --output \"$ELM_OUT\" 2>&1) && echo \"[elm] rebuilt \$(date +%H:%M:%S)\""
) &
PIDS+=("$!")

# Foreground: Tauri dev. ^C here triggers `cleanup` above.
echo "[dev] starting cargo tauri dev (Ctrl+C to stop everything)..."
cd "$TAURI_DIR"
exec cargo tauri dev
