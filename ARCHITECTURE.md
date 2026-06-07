# Architecture Notes

## Vector Shapes Are the Source of Truth

**The polygon shapes of bricks and pieces are the only correct representation of their geometry.**

Bounding boxes (`x`, `y`, `width`, `height`) exist only for spatial indexing and layout calculations. They must **never** be used for:
- Display or rendering masks
- Piece PNG generation
- Thumbnail rendering
- Any user-visible output

When rendering a brick or piece image, always mask through the polygon using `point_in_polygon`. The polygon may need 0.5px expansion (via `expand_polygon`) to ensure boundary pixels are included by both adjacent pieces.

### Why This Matters

Vector bricks (like arched windows, curved walls) have non-rectangular shapes defined by their PostScript path data. Using bbox masking makes these shapes appear rectangular, which is incorrect and confusing to users.

### Where Polygons Live

| Data | Coordinate System | Source |
|------|-------------------|--------|
| `BrickPlacement.polygon` | Brick-local pixels (origin = brick top-left) | AI parser |
| `brick_polygons` (session) | Brick-local pixels | AI parser |
| `piece_polygons` | Canvas pixels (y-down, origin = canvas top-left). **Multi-component**: `Vec<Vec<[f64; 2]>>` — one ring per disconnected component of the brick-polygon union | `puzzle::compute_piece_polygons` |

`piece_polygons` keeps every component (not just the largest). Bricks within the same piece can sit in separate rows with visible mortar gaps; previously the polygon dropped them and the rendered piece silently lost the bricks. The mask now treats any pixel inside *any* ring as "inside the piece" — matches what the bezier-merged outlines (`outlines.png`) show.

## Coordinate Systems

Two coord systems coexist in any AI export:

| System | Origin | Y direction | Used by |
|---|---|---|---|
| **PyMuPDF ("pymu") pts** | top-left of artbox | y-down | `parse_ai`, `clip_rect`, placement polygons |
| **PDF page pts** | bottom-left of mediabox | y-up | content-stream CTMs, MuPDF rendering |

They differ by a constant translation `bleed = (bleed_x, bleed_y)`:

```
pdf_e             = pymu_x + bleed_x
pdf_f (y-up)      = page_height - (pymu_y + bleed_y)
```

Adobe's AI export sets `bleed_x` to whatever the artwork's stored CTM places content at — for NY5 that's +267 pt, but the per-file value varies. `ocg_inject::build_modified_pdf` measures it by matching each Image block's clip-path centroid against the matched placement's polygon centroid, then takes the median. `ModifiedPdfArtifact::bleed_pts` carries the sub-pixel-precise value through to downstream code.

## Export Rendering Pipeline (`render_export_pieces`)

Up-front, once per export:

1. `parse_ai` — placements + metadata.
2. `build_modified_pdf` — write a copy of the AI PDF with per-block OCGs injected (one `hp_brick_NNNN` per parser brick, plus the globals `hp_image`, `hp_bricks_inline`, `hp_decoration`). Returns `ModifiedPdfArtifact` with `bleed_pts`, OCG name lookups, and stats.
3. `Document::load` + `walk_page_bricks` + `match_blocks_to_bricks` — re-walk the PDF blocks (path centroids, straddle-split data) and match each block to a parser brick via polygon containment.

Then for each output asset:

**`composite.png`** (canvas-sized, full house)
- MuPDF renders the bricks layer as the base (covers any non-Image content)
- `raster_extract::compose_image_blocks_onto_canvas` overlays every Image XObject extracted directly from the AI on top — exact source pixels, no MuPDF re-rasterisation

**`background.png`** (canvas-sized, blueprint backdrop)
- MuPDF render of the `background` OCG layer

**`background_highlight.png`** (padded canvas, +80 px each side)
- Soft white outline of the house silhouette + window/door cut-outs
- Algorithm: Euclidean distance transform on the silhouette mask, then `α = 255 · exp(-d²/2σ²)` for a Gaussian-falloff halo around every alpha boundary. Rotationally symmetric by construction — no corner artifacts.
- Padding lets the halo bleed past the canvas edges where the house silhouette touches them

**`lights.png`** (canvas-sized, warm window-pane glow)
- MuPDF render of the `lights` OCG layer (when present)

**Per-piece PNGs** (`piece_<id>.png`, alpha-bbox-trimmed)
- One MuPDF render of the whole-house non-Image content (every `hp_brick_NNNN` on, `hp_image` *off*) — produces the canvas-wide vector overlays (window panes, decorations) in a single call instead of 60.
- Per piece: copy the global non-Image canvas, direct-extract this piece's Image bricks, source-over compose on top, polygon-mask through the piece's multi-component polygon, alpha-bbox trim.

**`outlines.png`** (padded canvas, +16 px each side)
- For each piece: merge the brick beziers (`merge_piece_bezier`), tessellate cubics, stroke onto the canvas. Same code the live editor uses, so what gets exported is what the user sees on screen.
- Padding prevents the stroke from being clipped to half its width where a piece-boundary path coincides with the canvas edge.

**`assets.json`** — placement metadata for every canvas-aligned asset. See `EXPORT_BUNDLE.md`.

## Direct AI Raster Extraction (`raster_extract.rs`)

Adobe's `.ai` files are PDFs, and most bricks are stored as PDF Image XObjects (Flate-compressed RGB + a separate SMask Image XObject for the alpha channel). We read these directly with `lopdf` instead of asking MuPDF to re-rasterise them, which:

- preserves the AI's source pixels (no anti-aliasing noise from MuPDF re-rendering at our DPI)
- runs ~10× faster than MuPDF for per-layer extraction
- removes any sub-pixel clip-rect rounding error

The block's `inner_ctm_at_content` gives the image's PDF-page rect (`e/f` = lower-left, `a/d` = width/height). Place at `(pymu = pdf - bleed)` on the canvas, scale to export DPI.

Form XObjects and "Inlined" path-op blocks (window panes etc.) still go through MuPDF — those are vector or composite content the direct path can't represent.

## OCG Injection (`ocg_inject.rs`)

`build_modified_pdf` rewrites the page's content stream so each `q…Q` brick block is wrapped in a fresh per-brick OCG (`hp_brick_NNNN`). Image blocks additionally sit inside a global `hp_image` OCG. This lets MuPDF render specific subsets:

| To get… | Enable OCGs |
|---|---|
| every brick (default) | `bricks` + every `hp_brick_NNNN` |
| only this piece's bricks | `bricks` + only that piece's `hp_brick_NNNN`s + `hp_bricks_inline` |
| all non-Image vector content | `bricks` + every `hp_brick_NNNN` + `hp_bricks_inline` + `hp_decoration` (leave `hp_image` *off*) |

PDF OCG visibility is intersection: an Image block hides if *either* its `hp_brick_NNNN` *or* `hp_image` is off.

**Straddle split**: some `q…Q` blocks span the bricks→lights BDC boundary in the source content stream. Naïvely wrapping them in `hp_p_NNNN` BDC/EMC produces improperly-nested markup MuPDF mis-handles. `walk_page_bricks` records the EMC/BDC index pair for straddling blocks; the rewriter emits `hp_p_NNNN` BDC/EMC *twice* — once closed in the bricks scope, once reopened in the lights scope — so each pair sits cleanly inside one parent OCG.
