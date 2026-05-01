## Open

### AI unscaled vector data as source of truth
Current pipeline runs vector operations (adjacency, unions, spike removal,
piece outlines) partly in canvas-pixel coords after scaling. Scaling is lossy
— shared endpoints that were bit-identical in the AI file stop matching, so
we pile on workarounds: epsilon expand/erode, vertex snapping, spike removal.

Refactor: keep AI native coords (pymu units) and bezier curves as the single
source of truth for every vector op. Tessellate and scale to canvas only at
the last possible moment — ideally only for raster masks. Blue/white outlines
should go to SVG as bezier path commands and be tessellated by the browser
after the CSS transform, not by us before.

Raster pipeline review (same direction): we may not need per-brick raster
layers at all. Alternative shape:

1. Render ONE high-res raster of the full artwork via MuPDF (bricks layer,
   plus lights/background/etc. as separate hi-res layers).
2. Clip that hi-res raster by vector outlines — at full AI resolution, so
   edges land exactly on vector boundaries.
3. Downsample per piece/brick to the target canvas size only at the end.

That removes the per-brick OCG render roundtrips, keeps edges sharp, and
avoids all the polygon-mask artefacts we chase today.

### Piece 1px gaps/bleeding — rasterization pipeline redesign
**Root cause**: We rasterize the full page at ~30 DPI via MuPDF, then split
the low-res raster into bricks/pieces using vector polygon masks. At 30 DPI,
1 pixel = ~2.4 PDF points — polygon edges don't align with pixel boundaries,
causing gaps between adjacent pieces and bleeding from neighbors.

**Current pipeline**:
1. MuPDF renders full page (mediabox) at DPI → single large raster
2. Overlay onto canvas-sized image (implicit clip to brick bounding box)
3. Per-brick: mask raster with point-in-polygon → canvas-sized brick PNG
4. Per-piece: scanline-rasterize brick polygons → mask → piece PNG

**The problem**: step 1 is lossy. We have perfect vector data in the AI file
but discard it by rasterizing the entire page into one image.

**Possible fixes (increasing effort)**:
1. **Higher DPI** — render at 4x DPI (~120), clip per brick. Gaps become
   sub-pixel. Memory: 16x, time: ~4x. Quick win.
2. **MuPDF clip-rect** — pass clip rect to MuPDF render call so it only
   rasterizes the visible area, not the full mediabox. Prerequisite for #1.
3. **Per-brick OCG render** — toggle individual brick sub-layers in MuPDF,
   render each independently. Correct pixels, no cross-brick bleeding.
   ~200 MuPDF calls per house, possibly parallelizable.
4. **Hybrid vector+raster** — raster bricks: extract embedded pixel data
   directly (already parsed via `extract_raster_image`). Vector/gradient
   bricks: render via tiny-skia from parsed PostScript paths. No MuPDF
   for brick images at all, only for page geometry.
5. **Full vector pipeline** — parse all PostScript fills, gradients, compound
   paths. Render everything via tiny-skia. MuPDF only for decompression
   and page geometry. Most work but highest fidelity.

### ~~macOS double-click binary (PR #44)~~
~~Binary name with dots breaks Finder double-click.~~
Resolved: Tauri app bundle handles this natively.

## Features

### Programmatic export API
Full /api/export without browser — server-side outline path generation.

### Extensive logging + remote error reporting
Add structured logging throughout the pipeline (parse, render, merge, export).
Eventually: opt-in home-calling that sends error logs to a remote endpoint
so client-reported issues can be diagnosed without access to their machine.

### Update checker — doesn't fire (confirmed broken v0.4.0 → v0.4.1)
tauri-plugin-updater integrated (PR #56), `check_for_updates` command
wired, but the banner never appears even when a newer release exists.

**Three root causes:**

1. **No `latest.json` manifest.** The updater endpoint
   `https://github.com/AlexeyTuboltsev/house_puzzle/releases/latest/download/latest.json`
   returns HTTP 404. Tauri v2 updater needs this JSON to compare versions
   and locate the per-platform update bundle.

2. **No signing set up.** `tauri.conf.json` has `"pubkey": ""`; CI
   (`build-tauri.yml`) has no `TAURI_SIGNING_PRIVATE_KEY` /
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env vars passed to `tauri-action`,
   so the bundler never emits `.sig` files. Tauri v2 updater rejects
   unsigned updates by design.

3. **Platform-prefixed artifact renames.** Release assets end up with names
   like `linux-House.Puzzle_0.4.1_amd64.AppImage` (done in a later CI step).
   Even if a `latest.json` existed, the URLs baked into it by `tauri-action`
   would point at the original names and 404 on download.

**To fix end-to-end:**

1. Generate a Tauri signing keypair: `cargo tauri signer generate`.
2. Paste the public key into `tauri.conf.json` → `plugins.updater.pubkey`.
3. Add the private key + password to repo secrets:
   `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. Wire both env vars into the `tauri-apps/tauri-action` step so the
   bundler signs each artifact and produces `latest.json`.
5. Either stop renaming artifacts with the platform prefix, or regenerate
   `latest.json` after renames so its URLs match the uploaded filenames.

### Extract test harness behind feature flag
The `--test-mode` file watcher and JS eval for clicks live in production
`main.rs`. Move behind `#[cfg(feature = "e2e-test")]` so they're only
compiled in test builds. Also move `save_screenshot` command.

### Evaluate tauri-webdriver for e2e testing
Replace our custom test harness with proper WebDriver-based testing:
- `tauri-plugin-webdriver-automation` (danielraffel) — JS bridge plugin
  for macOS WKWebView, speaks W3C WebDriver. Solves the context isolation
  issue we hit. https://github.com/danielraffel/tauri-webdriver
- `tauri-plugin-webdriver` (Choochmeque) — embedded WebDriver server,
  cross-platform. The one we tried but had issues with.
- `tauri-plugin-screenshots` — Tauri v2 plugin for window/monitor
  screenshots. Could replace our WKWebView.takeSnapshot code.
- CrabNebula WebDriver — commercial hosted service with macOS support.

### Piece editor adjacency check
When combining pieces in the editor, verify they are adjacent before
allowing the merge. Currently not enforced.

### ~~OS file picker — remember last location~~
~~The native open dialog should reopen at the last directory the user
picked from.~~ Done — picker persists last directory in app data.

### ~~Waves — "Last wave" button~~
~~Create a new wave and assign every currently unassigned piece.~~
Done — "Last wave" button next to "New wave".

### ~~"Big wave" needs a scrollbar~~
Done — horizontal scrollbar on the bottom tray is now 12px and
visually prominent.

### ~~Stronger selected-piece highlight (canvas + strips)~~
Done — selected piece gets a glowing yellow stroke + bright fill on
the canvas and a glowing border on every matching strip thumb.

### ~~Selected piece auto-scrolls into view in every strip~~
Done — `scrollPieceIntoView` port calls `el.scrollIntoView` on every
`[data-piece-id]` match, including the canvas overlay.

### ~~Wave number badge on each wave~~
Done — 1-based ordinal badge in the wave row header.

### ~~Groups + waves: "Show only blueprint" checkbox~~
Done — per-group and per-wave "BP" checkbox swaps thumbnails to
`piece.outlineUrl`.

### ~~Numeric input next to "Pieces" and "Min border" sliders~~
Done — paired number inputs share the slider's handler/value.

### Parse cache — cache busting, cleanup, versioning
The load-speedup PR added a parse cache under `<temp_dir>/house_puzzle_parse_cache/`
keyed by (PARSE_CACHE_VERSION, file size, file mtime, file path,
canvas_height). It writes two artifacts per cache key — a `.bin`
(bincoded `CachedParse`) and a `_bricks.png` (full MuPDF pixmap of the
bricks layer). Together they save ~6 s on re-opens of the same AI file.
But the cache grows monotonically and never reclaims space.

Need:
1. **Startup sweep** that deletes files whose name doesn't match the
   current `PARSE_CACHE_VERSION` schema, so old-version blobs go away
   automatically after a version bump.
2. **LRU / max-age policy** — e.g. delete files not accessed in 30
   days, or cap total cache size at N MB. Power users will accumulate
   one entry per (AI file × canvas height) combination over time.
3. **"Clear cache" action** in the UI for manual nuke.
4. **Move out of `temp_dir`** into the OS cache dir (`app_cache_dir()`
   — survives reboots but really needs cleanup or it'll snowball).

### Parse cache — versioning is fragile
Right now any shape change to `CachedParse` requires bumping
`PARSE_CACHE_VERSION` by hand. Bincode silently accepts mismatched
schemas in some cases. Consider:
- A versioned header byte at the start of each `.bin` so reads can
  reject mismatches loudly even within the same cache key.
- A more forward-compatible format (e.g. CBOR with serde flatten
  defaults) so adding optional fields doesn't require invalidation.
- Document the bump rule in code: "any new field in any cached type
  → bump PARSE_CACHE_VERSION".

### Adobe Illustrator validation script
Create a standalone validation script that runs inside Adobe Illustrator
(ExtendScript / JSX) to check `.ai` files before export. Should detect:
- Missing required layers (bricks, background, screen)
- Empty required layers
- Unclosed paths in brick sub-layers
- Overlapping brick polygons (bricks must be adjacent, not overlapping)
- Brick containment (one brick fully inside another)
- Multi-object layers with independent sub-paths
- Degenerate paths (< 3 points, zero area)

The Rust backend already validates these on load (see `ai_parser.rs`),
but catching errors in Illustrator is faster feedback for the artist.

### Illustrator script — release / distribution pipeline
The validator above only helps if the artist actually has it installed.
Today the script (when written) lives in the repo and the artist would
have to clone the repo, copy the `.jsx`, and update by hand on every
change. Need a real distribution path. Options to evaluate:

- **Bundle it into the Tauri app**: ship the `.jsx` next to the binary
  and add an "Install Illustrator script" action that copies it into
  the user's `~/Library/Application Support/Adobe/Adobe Illustrator
  <ver>/<lang>/Scripts/` (macOS) / `%APPDATA%\Adobe\Adobe Illustrator
  <ver>\<lang>\Scripts\` (Windows) directory. Could also bundle a
  small "Update script" button that re-copies the latest version.
- **GitHub-Actions release artefact**: the existing `build-tauri.yml`
  release job already publishes `.AppImage` / `.dmg` / `.msi` to a
  tagged release. Add the `.jsx` to that release with a stable URL
  (`releases/latest/download/hp-validator.jsx`) so the artist (and
  the in-app installer) can fetch the current version.
- **Versioning**: tag the script with the same `vX.Y.Z` as the
  editor, so the validator and parser stay in lock-step (whenever
  the parser tightens, both move). Update the tauri-action step to
  pick up `crates/illustrator-validator/dist/hp-validator.jsx`.
- **Editor compatibility check**: have the script print its own
  version into a layer-comment that the Rust parser reads at load.
  Mismatch → editor warns "your AI was validated by an older script
  — re-run validation". Catches the "did the artist run the new
  validator" question without asking.

### Tauri warning / error UX — currently a raw dump
`load_pdf` returns `metadata.warnings` as `Vec<String>` and the Elm
side displays them as a flat list under the canvas. Some cases:

- `Layer 'X': N unclosed path(s) — discarded` (now auto-closed but
  still flagged so the artist can fix the source)
- `Layer 'A' is fully contained within Layer 'B' (95% overlap) — discarded`
- `Layer 'A' overlaps Layer 'B' (45% of smaller area) — Layer 'A' discarded`
- `MULTI_OBJECT: layer 'X' has 3 polygons, discarded 1 independent objects`
- `SKIPPED: 'Y' has no vector polygon`
- `COVERED: 'Z' removed (hidden under another brick)`

Today the user gets a wall of these on every load, can't sort, can't
filter, can't open the offending layer in Illustrator with one click,
can't tell "this is a real problem" from "the parser handled it". We
should:

1. **Structured warnings on the wire** — replace `Vec<String>` with
   `Vec<{severity, kind, layer_name, related_layer, message}>` so
   the frontend can group / sort / filter by severity (info /
   warning / error) and kind (unclosed / overlap / containment /
   skipped / covered).
2. **Collapsible panel in Elm** — group by severity, default
   collapse "info" (auto-fixed), expand "warning" / "error". Show a
   count badge per group. Click a row → highlight that layer on the
   canvas.
3. **"Open in Illustrator" link** per warning row — copy a
   `aiFile://...?layer=X` URL to clipboard (or open it via a Tauri
   command if Illustrator's URL scheme works) so the artist can
   jump straight to the broken layer.
4. **Suppress duplicates** — many warnings repeat the same layer
   name; collapse identical messages with a count.
5. **Persist dismissals** — once the artist acknowledges "I know
   about Layer 320", don't surface it on every reload of the same
   AI file. Tied to the AI file's content hash; a re-export by
   the artist invalidates dismissals.

## ~~Nice-to-have~~

### ~~Tauri desktop app~~
~~Wrap existing server+webview for native app bundle, Gatekeeper signing, dock icon.~~
Done: Tauri migration is now the mainline (PR #57).
