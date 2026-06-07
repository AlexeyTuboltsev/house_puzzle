# House Puzzle Editor

Adobe Illustrator → Unity jigsaw-puzzle asset pipeline. A desktop app
that takes an Illustrator `.ai` file of a house, splits it into puzzle
pieces, and exports a ZIP of layered PNGs + placement metadata the
Unity game engine consumes directly.

![Pipeline](https://img.shields.io/badge/Tauri-v2-orange) ![Rust](https://img.shields.io/badge/Rust-edition_2021-blue) ![Elm](https://img.shields.io/badge/Frontend-Elm-1293D8)

## What it does

The artist draws a house in Illustrator with each visible "brick"
(wall segment, window, door, decoration) on its own layer. This tool:

1. **Parses the `.ai` file** as a PDF — Illustrator stores its
   document as a PDF with custom Optional Content Groups, one per
   layer. We read the layer tree, the vector polygon for each brick,
   and the raster image XObjects directly with `lopdf`.
2. **Merges bricks into puzzle pieces** — a greedy area-balanced
   algorithm grows pieces from the smallest brick until each piece
   hits the target area (canvas_area / target_piece_count). Pieces
   are connected through adjacency, never disjoint by accident.
3. **Renders the export bundle** — for every piece, a polygon-masked
   PNG; plus a composite, background, outlines layer, a soft white
   highlight halo, lights overlay, and `assets.json` with the
   placement metadata. See [EXPORT_BUNDLE.md](EXPORT_BUNDLE.md).
4. **Hands the ZIP to Unity** — the Unity importer reads
   `house_data.json` and instantiates each piece as a sprite with
   the correct polygon collider.

## Why it exists

Hand-cutting puzzle pieces in Illustrator and then re-tracing them in
Unity was eating ~half the level-creation budget per house. A single
artist now produces a ready-to-import puzzle bundle in one click,
with pixel-exact source rasters and bezier-precise piece outlines.

## Architecture

- **App**: Tauri v2 desktop app — native webview, no browser involved
- **Backend**: Rust workspace
  - `hp-core` — `.ai` parsing, puzzle generation, OCG injection,
    rendering, export. Pure library, tested in isolation.
  - `hp-tauri` — Tauri commands + session management; thin shell.
- **Frontend**: Elm SPA. Owns all UI state. Talks to the backend
  via Tauri commands.
- **Rendering**: MuPDF for vector content (Form XObjects, decorations,
  background, lights); direct `lopdf` extraction for the raster
  Image XObjects that make up the bricks themselves — bypasses
  re-rasterisation and gives ~10× the throughput at exact source
  pixels.

See [ARCHITECTURE.md](ARCHITECTURE.md) for the rendering pipeline,
coordinate systems, the OCG-injection trick that lets us toggle
individual bricks for per-piece renders, and the multi-component
piece polygon design.

## Quick start

```bash
# 1. Build the Elm frontend (writes crates/hp-tauri/dist/elm.js)
cd elm && npx elm make src/Main.elm --output ../crates/hp-tauri/dist/elm.js && cd ..

# 2. Run the Tauri app in dev mode
cargo tauri dev --manifest-path crates/hp-tauri/Cargo.toml
```

Drop `.ai` files into `./in/`, open one in the app, generate the
puzzle, and click **Export** to write the ZIP to your downloads
folder.

## Building a release

```bash
cd elm && npx elm make src/Main.elm --optimize --output ../crates/hp-tauri/dist/elm.js && cd ..
cargo tauri build --manifest-path crates/hp-tauri/Cargo.toml
```

Native installers land under `target/release/bundle/`.

## Testing

```bash
cargo test --release -p hp-core --lib --tests
```

The suite covers 59 unit tests (AI parsing, OCG injection,
puzzle merging, bezier merging, raster decoding) plus 2 end-to-end
**canary tests** that render real Illustrator files (`_NY5.ai`,
`_NY8.ai`) and fingerprint the resulting `composite.png` against
pinned alpha/RGB checksums. The canaries catch:

- decoder regressions (PNG predictor variants, Flate/Indexed
  palettes, soft-mask alpha)
- coordinate-system bugs in the pymu↔PDF projection
- broken OCG injection (missing bricks, mis-wrapped Image blocks)
- distance-transform / blur changes in `background_highlight.png`

The two canary AI files are checked into **git-LFS**; CI fetches them
on every push. Without LFS the canaries auto-skip — fork-friendly.

## Project structure

```
crates/
  hp-core/       — AI parsing, puzzle generation, rendering, export
    src/
      ai_parser.rs       parse Illustrator's PDF layers + polygons
      ocg_inject.rs      write a modified PDF with per-brick OCGs
      raster_extract.rs  direct Image XObject decode (Flate + PNG predictor)
      puzzle.rs          adjacency, area-balanced merge, piece polygons
      render.rs          composite, background, highlight, lights, outlines
      export.rs          ZIP bundling + house_data.json
    tests/
      canary_composite.rs   pixel-fingerprint canaries (NY5, NY8)
  hp-tauri/      — Tauri desktop app, commands, session management
elm/
  src/Main.elm   — Frontend application (Elm 0.19)
tests/
  accessibility/ — Cross-platform E2E tests (Rust, uses tauri-ui-test)
presets/         — Puzzle parameter presets (Coarse / Default / Fine)
in/              — drop .ai files here; gitignored except canaries
```

## Further reading

- [ARCHITECTURE.md](ARCHITECTURE.md) — rendering pipeline,
  coordinate systems, OCG injection, multi-component polygons
- [EXPORT_BUNDLE.md](EXPORT_BUNDLE.md) — ZIP contents, z-order,
  `assets.json` schema
