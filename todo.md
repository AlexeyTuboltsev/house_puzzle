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

### Adobe Illustrator validation script
Create a standalone validation script that runs inside Adobe Illustrator
(ExtendScript / JSX) to check `.ai` files before export. Should detect:
- Missing required layers (bricks, background, screen)
- Empty required layers
- Unclosed paths in brick sub-layers
  Test case: NY9n has 4 unclosed brick sub-paths (b012 sp0 gap=50,
  b020 sp0 gap=111.8 diagonal, b022 sp0 gap=50, b026 sp0 gap=111.8
  diagonal) — the artist forgot the closing edge. No other NY file
  has any. Symptom: at certain target/seed combos, the piece
  containing one of these bricks ends up multi-path because the open
  chain doesn't close (NY9n p28/p51 at target=60). Currently
  worked-around by an auto-close pass in `ai_parser.rs` that appends
  an implicit line from the last vertex back to start; the validator
  should still flag these so the artist fixes them at source.
- Overlapping brick polygons (bricks must be adjacent, not overlapping)
- Brick containment (one brick fully inside another)
- Multi-object layers with independent sub-paths
- Degenerate paths (< 3 points, zero area)
- Intentional sub-pymu staircases — single sub-path edges < ~1 pymu long
  used to interlock with neighbour notches (NY5: b022/b027 etc have
  ~0.55-pymu vertical "step" edges). The bezier merge has to fuse
  cross-brick drift while preserving these intra-brick steps; flagging
  them in Illustrator lets the artist either snap the steps to
  meaningful values or remove them.
- Multi-grid drift across bricks — bricks within a single piece drawn on
  >1 y-grid (NY5 p21/p51: rows of bricks sit on `.85`, `.86`, and `.41`
  y-grids that are 0.55 pymu apart instead of one shared grid). The
  artist patches the seam with sub-pymu staircase edges, which then
  defeats the bezier merge: collapsing the staircase makes the outer
  bottom of one brick indistinguishable from the interior of an
  adjacent brick's notch. The validator should detect ≥3 distinct
  y-cluster centres within < 1 pymu of each other and warn.

When fixing a merge bug, always ask: "does this look like an AI-source
artifact that a future validator could catch?" If yes, file the case
here so we (a) keep an inventory of regression test grounds for the
validator and (b) recognise the pattern next time we see a related
issue. Each entry should describe the symptom, the AI pattern, and
which file/piece/brick reproduces it.

The Rust backend already validates these on load (see `ai_parser.rs`),
but catching errors in Illustrator is faster feedback for the artist.

## ~~Nice-to-have~~

### ~~Tauri desktop app~~
~~Wrap existing server+webview for native app bundle, Gatekeeper signing, dock icon.~~
Done: Tauri migration is now the mainline (PR #57).
