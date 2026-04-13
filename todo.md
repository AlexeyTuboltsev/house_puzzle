## Open

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

### macOS double-click binary (PR #44)
Binary name with dots (`house-puzzle-0.3.7`) breaks Finder double-click.
Fix: use dashes (`house-puzzle-0-3-7`). PR #44 open, not merged.

## Features

### Programmatic export API
Full /api/export without browser — server-side outline path generation.

### Extensive logging + remote error reporting
Add structured logging throughout the pipeline (parse, render, merge, export).
Eventually: opt-in home-calling that sends error logs to a remote endpoint
so client-reported issues can be diagnosed without access to their machine.

### Update checker
On startup, check GitHub releases API for a newer version.
If found, show a non-blocking banner in the UI: "Version X.Y.Z available".

## Nice-to-have

### Tauri desktop app
Wrap existing server+webview for native app bundle, Gatekeeper signing, dock icon.
