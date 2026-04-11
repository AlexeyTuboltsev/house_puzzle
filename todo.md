## Open

### Piece 1px gaps/bleeding between pieces
Scanline rasterization of brick polygons didn't fully eliminate gaps.
Root cause: shared polygon edges produce inconsistent pixel assignment.
Needs investigation — possibly use anti-aliased mask or SVG-based piece rendering.

### macOS double-click binary (PR #44)
Binary name with dots (`house-puzzle-0.3.7`) breaks Finder double-click.
Fix: use dashes (`house-puzzle-0-3-7`). PR #44 open, not merged.

### Piece shape deduplication (#4 from original list)
Detect duplicate piece shapes, name them `same_1`/`same_2`, place maximally apart.

### Min/max piece size constraints (#3 from original list)
Min 50×150 or 100×100, max 400×300. Needs merge algorithm changes.

### Gravity validation (#5 from level design list)
Highlight pieces that would fall (support piece in later wave).

### Selection performance
Lasso/click selection is laggy. Needs profiling — likely re-rendering on every mouse move.

### Dragged piece z-order
Dragged piece renders under already-placed pieces. Should be topmost.

### Programmatic export API
Full /api/export without browser — server-side outline path generation.

### Unity export: bottom piece collider vectorization
Clamp bottom contour to y=0 for ground-flush colliders per piece.

### Unity export: ScalingFactor formula
Replace heuristic `round(220 / avg_sprite_width)` with:
`ScalingFactor = round(refHeight / (PPU × 2 × orthoSize))`

### Tauri desktop app (future)
Wrap existing server+webview for native app bundle, Gatekeeper signing, dock icon.
