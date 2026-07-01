# Export Bundle Format

The export ZIP produced by `render_export_pieces` + `export::generate_export_zip` contains a fixed set of rendering assets plus `house_data.json`, ready for the Unity importer (or any downstream consumer). This file documents what's in the bundle, what each file is for, and how to position it.

All assets are PNGs in sRGB with a straight alpha channel. Pixel dimensions are at the export DPI requested at render time (default 300).

## Top-level files

```
composite.png              full house, all bricks rendered (no background)
background.png             blueprint backdrop (purple silhouette w/ window cut-outs)
background_highlight.png   soft white halo around the silhouette boundary
outlines.png               piece-outline strokes (white, ~3 px wide)
lights.png                 warm window-pane glow overlay
assets.json                placement metadata for the above (see schema)
house_data.json            puzzle structure for the game engine
pieces/piece_<id>.png      one per puzzle piece, trimmed to alpha bbox
```

Optional files are omitted when the AI doesn't define the corresponding OCG layer:
- `background.png`, `background_highlight.png` — present only when the AI declares a `background` OCG
- `lights.png` — present only when the AI declares a `lights` OCG

## Z-order

Bottom to top, when composited the way the game engine renders it:

```
1.  background_highlight.png   — soft white halo (sits behind the house)
2.  background.png             — blueprint silhouette
3.  outlines.png               — piece-edge strokes (drawn under the pieces so
                                  edges show only in inter-piece gaps)
4.  pieces/piece_*.png         — individual brick sprites at their (x, y)
5.  lights.png                 — warm window glow on top of everything
```

## Placement: `assets.json`

```jsonc
{
  "canvas_w": 3127,            // base canvas width  in export-DPI pixels
  "canvas_h": 8026,            // base canvas height in export-DPI pixels
  "assets": {
    "composite.png":            { "file": "composite.png",
                                  "x":   0, "y":   0, "w": 3127, "h": 8026 },
    "background.png":           { "file": "background.png",
                                  "x":   0, "y":   0, "w": 3127, "h": 8026 },
    "background_highlight.png": { "file": "background_highlight.png",
                                  "x": -80, "y": -80, "w": 3287, "h": 8186 },
    "outlines.png":             { "file": "outlines.png",
                                  "x": -16, "y": -16, "w": 3159, "h": 8058 },
    "lights.png":               { "file": "lights.png",
                                  "x":   0, "y":   0, "w": 3127, "h": 8026 }
  }
}
```

- `(x, y)` is the asset's top-left position in **canvas coordinates** (origin = top-left of the base canvas, y-down).
- `(w, h)` is the asset's pixel dimensions.
- Two assets have **negative offsets**:
  - `background_highlight.png` is padded by 80 px on every side so the Gaussian blur halo can extend past the house silhouette wherever it touches the canvas edge. Place its top-left at `(-80, -80)`.
  - `outlines.png` is padded by 16 px so strokes that coincide with a canvas edge aren't clipped to half stroke width. Place its top-left at `(-16, -16)`.
- Every other asset sits at `(0, 0)` and matches the base canvas dimensions exactly.

## Pieces: `house_data.json`

Each puzzle piece is a `Block` entry in the JSON. The key fields:

| Field | Meaning |
|---|---|
| `name` | `piece_<id>` — matches the PNG file `piece_<id>.png` |
| `position.x` / `.y` / `.z` | World position in Unity units (game-coord system, ScalingFactor already applied; see `UNITY_INTEGRATION.md`) |
| `orderInLayer` | Z-order index used by the game's sprite renderer |
| `isChimney` | true for chimney blocks (interactable separately) |

The piece PNG itself is alpha-bbox-trimmed; its pixels carry both the brick body and any Illustrator-baked alpha bleed (soft-mask drop-shadows / glow) on the outside edges. The bezier-merged outline (`outlines.png`) traces only the geometric piece boundary, so the soft alpha overhangs the outline at piece edges intentionally.

## Notes for consumers

- The base canvas (`canvas_w` × `canvas_h`) is the union of every brick's polygon at export DPI, plus any inline content. Use it as the world bounds when laying out the assets in a scene.
- `composite.png` is **the entire bricks layer baked in one image** — it's redundant with the per-piece PNGs and exists for debugging / quick previews. Game runtime should consume `pieces/piece_*.png` for any interactive puzzle behaviour and ignore composite.
- `background_highlight.png` uses solid `(255,255,255)` for its lit pixels; only alpha varies. If you need a different tint, multiply the alpha by your tint colour at render time.
- `assets.json` was added in 2026-06; older exports may not have it. Consumers should treat it as optional and fall back to hardcoded `(0, 0)` placement for assets that aren't listed.
