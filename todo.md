## Open

### Piece 1px gaps/bleeding between pieces
Scanline rasterization of brick polygons didn't fully eliminate gaps.
Root cause: shared polygon edges produce inconsistent pixel assignment.
Needs investigation — possibly use anti-aliased mask or SVG-based piece rendering.

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
