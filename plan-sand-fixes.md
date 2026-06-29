# Sand-file fixes — work plan

Triggered by client reports against `in/snd/Sand{3,9,10}.ai`. Each
file exposes a different gap in the editor / parser / AI validator
chain. Plan groups the work by where the change lives, then by
dependencies.

## Concrete bug evidence

| File | Symptom in the editor | Root cause |
|---|---|---|
| Sand9  | Only piece outlines render — composite is blank | All AI artwork is detached from the `bricks` OCG. MuPDF returns 0 pixels when only that OCG is selected. Parser still finds 158 bricks via the AI's private layer data. |
| Sand10 | Composite shifted ~1.5 Unity units vs piece outlines | `compute_pdf_offset` detects a non-zero Y shift (-66 px) caused by phantom parser polygons covering parts of the canvas where no actual brick renders. The shifted re-render moves the composite content but not the polygons. |
| Sand3  | 3 visible missing bricks → holes after puzzle generation | (1) Orphan raster placed outside any `bricks/Layer NNN/` sub-layer. (2-3) Two clusters where the parser's overlap validator dropped 50–61 %-overlapping siblings; the dropped bricks' raster content still composites onto the surviving brick by centroid match, but the survivor's polygon mask clips it → hole. |

## Tracks

### E1 — Editor live preview uses the export's pipeline

The export's composite renderer already tolerates broken AI OCG
metadata (MuPDF bricks + direct-extract Image XObject overlay using
sub-pixel-precise `bleed_pts`). Port the same approach into the
editor's `load_pdf`. Drops the legacy `compute_pdf_offset` +
shifted re-render path.

Fixes Sand9 (rasters paint via direct extract even with empty OCG)
and Sand10 (no more shifted re-render → composite aligns with
polygons).

Cost: ~+200–700 ms on file load (`compute_bleed_pts` derived
from `build_modified_pdf`'s centroid-match math, skipping the
content-stream rewrite). Zero runtime hit afterwards.

### E2 — Phantom polygon drop

After E1 has the bricks render in hand, walk the parser's
placement list and drop any whose polygon-bbox region has ≤ N %
rendered alpha. Removes ghost polygons that produce floating
outlines + empty puzzle-piece slots.

Fixes Sand10 visually even when E1 alone doesn't quite line things
up. Also catches a class of "phantom brick" the script-side check
can't see.

Cost: < 100 ms — one polygon-vs-pixmap pass.

### E3 — Surface findings to the artist

E1 + E2 produce data that the editor currently has no way to
display. Push structured entries into `metadata.warnings` so the
warning panel shows them. Schema is already
`Vec<{severity, kind, layer_name, ...}>` per the existing
"Tauri warning / error UX" todo.

Examples:
- `"bricks layer OCG empty: 158 parser bricks have no MuPDF render"`
- `"phantom polygons dropped: 73 of 285 placements"`

Cost: zero — falls out of E1 + E2 naturally.

### S1 — AI validator catches orphan rasters

`tools/ai-validate/lib/walk_paths.jsx::collectBricks` filters to
"leaf layers with ≥ 1 PathItem". Rasters placed outside that
structure are invisible to every existing check. Add:

1. Extend `walk_paths.jsx` to also enumerate raster items
   individually: `{ id, parent_layer_path, bbox }`.
2. New `checkOrphanRaster(snapshot, findings)` in `lib/checks.jsx`
   that warns on any raster whose parent layer path doesn't match
   `bricks/<leaf-with-PathItem>/`.

Output: a `findings` entry like
```json
{ "severity": "warning", "kind": "orphan_raster",
  "raster_id": "...", "parent_layer_path": "bricks/",
  "bbox_pymu": [x0, y0, x1, y1],
  "message": "raster sits outside any bricks/Layer NNN/ sub-layer
              — Rust parser will miss it" }
```

ExtendScript-only change, no Rust side. Cost: dev loop on the
artist's macOS Illustrator host (`run.sh`).

## Dependencies + recommended order

```
S1 ──── independent
E1 ──── prerequisite for E2
E2 ──── prerequisite for E3
E3 ──── after E1 + E2 land
```

Order: **E1 → E2 → E3 → S1**. E1 is useful standalone; E2 and E3
are easier to land on top of E1; S1 doesn't touch Rust at all and
can land any time but is shipped via the separate `sv*` release
channel.

## Out of scope (explicitly deferred)

- Artist-side "diagnose brick problems in Illustrator" documentation.
  Skipped at request — the warning text emitted by E3 + S1 should
  be enough for now; if not, revisit.
