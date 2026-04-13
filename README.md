# House Puzzle Editor

An automation pipeline for converting Adobe Illustrator `.ai` sketches into Unity game assets (jigsaw puzzle pieces).

## Architecture

- **Backend**: Rust (Axum web server + hp-core library)
- **Frontend**: Elm + JavaScript Canvas
- **Output**: Unity-compatible ZIP (piece sprite PNGs + house_data.json)

## Quick Start

```bash
# Run with Docker
docker compose up

# Or build and run directly (requires Rust + libclang)
cargo build --release
./target/release/hp-server
```

The editor opens at `http://localhost:5050`.

## Project Structure

```
crates/
  hp-core/       — AI parsing, puzzle generation, rendering, export
  hp-server/     — Axum web server, routes, session management
elm/
  src/Main.elm   — Frontend application
static/
  editor.js      — Canvas rendering and interaction
  elm.js         — Compiled Elm output
templates/
  elm.html       — Main HTML template
tests/
  canary.py      — Cross-platform integration tests
  baselines/     — Snapshot baselines for regression testing
presets/         — Puzzle parameter presets (Coarse/Default/Fine)
```

## Building

### Rust backend
```bash
cargo build --release
```

### Elm frontend
```bash
cd elm && npx elm make src/Main.elm --output ../static/elm.js
```

### Docker
```bash
docker compose up
```
