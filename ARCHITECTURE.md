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
| `piece_polygons` | Canvas pixels (y-down, origin = content top-left) | `puzzle::compute_piece_polygons` (union of brick polygons) |

### Rendering Pipeline

1. **Brick PNGs** (`/api/s/{key}/brick/{id}.png`): Mask through brick polygon, output brick-sized image
2. **Piece PNGs** (`render_piece_pngs_from_composite`): Mask through piece polygon (expanded 0.5px), output piece-sized image
3. **Wave tray thumbnails** (Elm `viewWaveTrayThumb`): Display piece PNG via `<img>` tag — shape comes from piece PNG alpha
4. **Main canvas** (Elm SVG): Draws piece polygons directly as SVG paths — always correct
5. **Export** (`generate_export_zip`): Uses piece PNGs scaled for Unity
