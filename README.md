# House Puzzle Editor

An automation pipeline for converting Adobe Illustrator `.ai` sketches into Unity game assets (jigsaw puzzle pieces).

## Architecture

- **App**: Tauri v2 desktop app (native webview)
- **Backend**: Rust (hp-core library + Tauri commands)
- **Frontend**: Elm
- **Output**: Unity-compatible ZIP (piece sprite PNGs + house_data.json)

## Quick Start

```bash
# Build Elm frontend
cd elm && npx elm make src/Main.elm --output ../crates/hp-tauri/dist/elm.js && cd ..

# Build and run Tauri app
cargo tauri dev --manifest-path crates/hp-tauri/Cargo.toml
```

## Project Structure

```
crates/
  hp-core/       — AI parsing, puzzle generation, rendering, export
  hp-tauri/      — Tauri desktop app, commands, session management
elm/
  src/Main.elm   — Frontend application
tests/
  accessibility/ — Cross-platform E2E tests (Rust, uses tauri-ui-test)
presets/         — Puzzle parameter presets (Coarse/Default/Fine)
```

## Building

### Elm frontend
```bash
cd elm && npx elm make src/Main.elm --output ../crates/hp-tauri/dist/elm.js
```

### Tauri app (release)
```bash
cargo tauri build --manifest-path crates/hp-tauri/Cargo.toml
```
