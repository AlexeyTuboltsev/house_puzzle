//! Image rendering pipeline — brick PNG extraction and compositing.

use image::{Rgba, RgbaImage};
use std::sync::Mutex;
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;

use crate::ai_parser::BrickPlacement;

/// Ray casting point-in-polygon test.
pub fn point_in_polygon(x: f64, y: f64, polygon: &[[f64; 2]]) -> bool {
    let n = polygon.len();
    if n < 3 { return false; }
    let mut inside = false;
    let mut j = n - 1;
    for i in 0..n {
        let yi = polygon[i][1];
        let yj = polygon[j][1];
        let xi = polygon[i][0];
        let xj = polygon[j][0];
        if ((yi > y) != (yj > y)) && (x < (xj - xi) * (y - yi) / (yj - yi) + xi) {
            inside = !inside;
        }
        j = i;
    }
    inside
}

/// Extract a raster brick's embedded image and place it at the correct
/// canvas position with polygon masking. Returns a canvas-sized RGBA image.
/// This bypasses MuPDF entirely for raster bricks — uses the original
/// resolution embedded pixel data.
pub fn render_raster_brick_direct(
    bp: &BrickPlacement,
    ai_data: &[u8],
    canvas_width: u32,
    canvas_height: u32,
) -> Option<RgbaImage> {
    let block_data = &ai_data[bp.block_begin..bp.block_end];
    let src_img = extract_raster_image(block_data)?;

    let src_w = src_img.width();
    let src_h = src_img.height();
    if src_w == 0 || src_h == 0 { return None; }

    let dst_w = bp.width as u32;
    let dst_h = bp.height as u32;
    if dst_w == 0 || dst_h == 0 { return None; }

    // Scale from source resolution to canvas pixel size
    let sx = src_w as f64 / dst_w as f64;
    let sy = src_h as f64 / dst_h as f64;

    let mut canvas = RgbaImage::new(canvas_width, canvas_height);

    // For raster bricks: no polygon masking — the embedded image IS the shape.
    // The polygon is slightly inset (~0.8px) from the bbox, causing gaps.
    // The raster image with white-to-alpha is the true authority.
    for dy in 0..dst_h {
        for dx in 0..dst_w {
            let src_x = ((dx as f64 + 0.5) * sx) as u32;
            let src_y = ((dy as f64 + 0.5) * sy) as u32;
            if src_x >= src_w || src_y >= src_h { continue; }

            let px = src_img.get_pixel(src_x, src_y);
            if px[3] == 0 { continue; }

            let cx = bp.x as u32 + dx;
            let cy = bp.y as u32 + dy;
            if cx < canvas_width && cy < canvas_height {
                canvas.put_pixel(cx, cy, *px);
            }
        }
    }
    Some(canvas)
}

/// Extract a raster brick image from a block's raw byte range.
pub fn extract_raster_image(block_data: &[u8]) -> Option<RgbaImage> {
    let xh_re = regex::bytes::Regex::new(
        r"\[\s*-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh"
    ).expect("static regex pattern is valid");
    let caps = xh_re.captures(block_data)?;
    let img_w: usize = std::str::from_utf8(&caps[1]).ok()?.parse().ok()?;
    let img_h: usize = std::str::from_utf8(&caps[2]).ok()?.parse().ok()?;
    if img_w == 0 || img_h == 0 { return None; }

    let xi_re = regex::bytes::Regex::new(r"%%BeginData:\s*\d+[^\r\n]*XI[\r\n]+")
        .expect("static regex pattern is valid");
    let xi_m = xi_re.find(block_data)?;
    let data_start = xi_m.end();
    let expected = img_w * img_h * 3;
    if data_start + expected > block_data.len() { return None; }
    let rgb_data = &block_data[data_start..data_start + expected];

    let mut img = RgbaImage::new(img_w as u32, img_h as u32);
    for y in 0..img_h {
        for x in 0..img_w {
            let idx = (y * img_w + x) * 3;
            let r = rgb_data[idx];
            let g = rgb_data[idx + 1];
            let b = rgb_data[idx + 2];
            let a = if r > 248 && g > 248 && b > 248 { 0 } else { 255 };
            img.put_pixel(x as u32, y as u32, Rgba([r, g, b, a]));
        }
    }
    Some(img)
}

/// Compute pdf_offset from an already-rendered bricks layer image.
/// Finds the first opaque pixel and compares to expected position.
pub fn compute_pdf_offset(
    bricks_layer: &RgbaImage,
    expected_min_x: i32,
    expected_min_y: i32,
) -> (i32, i32) {
    let w = bricks_layer.width();
    let h = bricks_layer.height();

    // Find first opaque column
    let mut first_col: Option<u32> = None;
    'outer_x: for x in 0..w {
        for y in 0..h {
            if bricks_layer.get_pixel(x, y)[3] > 30 {
                first_col = Some(x);
                break 'outer_x;
            }
        }
    }

    // Find first opaque row
    let mut first_row: Option<u32> = None;
    'outer_y: for y in 0..h {
        for x in 0..w {
            if bricks_layer.get_pixel(x, y)[3] > 30 {
                first_row = Some(y);
                break 'outer_y;
            }
        }
    }

    match (first_col, first_row) {
        (Some(col), Some(row)) => {
            let dx = expected_min_x - col as i32;
            let dy = expected_min_y - row as i32;
            if dx.abs() > 1 || dy.abs() > 1 {
                (dx, dy)
            } else {
                (0, 0)
            }
        }
        _ => (0, 0),
    }
}

/// Render all brick images using the hybrid approach:
/// - Raster bricks: extract embedded pixel data directly (original resolution)
/// - Vector bricks: crop from MuPDF OCG render (fallback)
/// Returns per-brick canvas-sized images.
pub fn render_brick_images_hybrid(
    bricks: &[(String, BrickPlacement)],
    ai_data: &[u8],
    canvas_width: u32,
    canvas_height: u32,
    ocg_fallback: &RgbaImage,
) -> HashMap<String, RgbaImage> {
    let result = Mutex::new(HashMap::new());
    let mut raster_count = 0u32;
    let mut vector_count = 0u32;

    bricks.par_iter().for_each(|(id, bp)| {
        // Try direct raster extraction first
        if bp.layer_type == "brick" || bp.layer_type == "mixed_brick" {
            if let Some(img) = render_raster_brick_direct(bp, ai_data, canvas_width, canvas_height) {
                result.lock().unwrap().insert(id.clone(), img);
                return;
            }
        }

        // Fallback: crop from OCG render (vector/gradient bricks)
        let mut canvas = RgbaImage::new(canvas_width, canvas_height);
        let poly = bp.polygon.as_ref();
        for dy in 0..bp.height.max(0) {
            for dx in 0..bp.width.max(0) {
                let sx = (bp.x + dx) as u32;
                let sy = (bp.y + dy) as u32;
                if sx < ocg_fallback.width() && sy < ocg_fallback.height() {
                    let px = ocg_fallback.get_pixel(sx, sy);
                    if px[3] > 0 {
                        let in_poly = match poly {
                            Some(pts) if pts.len() >= 3 => {
                                point_in_polygon(dx as f64 + 0.5, dy as f64 + 0.5, pts)
                            }
                            _ => true,
                        };
                        if in_poly {
                            canvas.put_pixel(sx, sy, *px);
                        }
                    }
                }
            }
        }
        result.lock().unwrap().insert(id.clone(), canvas);
    });

    let r = result.into_inner().unwrap();
    let rc = r.values().filter(|_| true).count(); // just to count
    eprintln!("[hybrid] {} bricks total ({} would be raster-direct)", r.len(), r.len());
    r
}

/// Save the OCG bricks layer render directly as the composite.
pub fn save_composite(bricks_layer_img: &RgbaImage, out_path: &Path) {
    bricks_layer_img.save(out_path).ok();
}

/// Find covered bricks using in-memory images (no disk I/O).
/// Find bricks that are covered by another (>= 80% alpha overlap).
/// `protected_ids`: bricks that should never be removed (e.g., vector bricks).
pub fn find_covered_bricks(
    bricks: &[crate::types::Brick],
    brick_images: &HashMap<String, RgbaImage>,
    protected_ids: &std::collections::HashSet<String>,
) -> std::collections::HashSet<String> {
    let mut covered = std::collections::HashSet::new();

    for i in 0..bricks.len() {
        let a = &bricks[i];
        if covered.contains(&a.id) { continue; }

        for j in (i + 1)..bricks.len() {
            let b = &bricks[j];
            if covered.contains(&b.id) { continue; }

            // Bbox overlap check
            if a.x >= b.right() || b.x >= a.right() || a.y >= b.bottom() || b.y >= a.bottom() {
                continue;
            }

            let (small, big) = if a.area() <= b.area() { (a, b) } else { (b, a) };

            let img_s = match brick_images.get(&small.id) { Some(i) => i, None => continue };
            let img_b = match brick_images.get(&big.id) { Some(i) => i, None => continue };

            let mut overlap = 0u64;
            let mut total_s = 0u64;

            // Only check the overlap region
            let ox0 = small.x.max(big.x) as u32;
            let oy0 = small.y.max(big.y) as u32;
            let ox1 = small.right().min(big.right()) as u32;
            let oy1 = small.bottom().min(big.bottom()) as u32;

            // Also count total opaque in small
            for y in small.y as u32..small.bottom() as u32 {
                for x in small.x as u32..small.right() as u32 {
                    if x < img_s.width() && y < img_s.height() && img_s.get_pixel(x, y)[3] > 30 {
                        total_s += 1;
                        if x >= ox0 && x < ox1 && y >= oy0 && y < oy1 {
                            if x < img_b.width() && y < img_b.height() && img_b.get_pixel(x, y)[3] > 30 {
                                overlap += 1;
                            }
                        }
                    }
                }
            }

            if total_s > 0 {
                let pct = overlap as f64 / total_s as f64;
                // Skip small bricks — rendering differences between image crate
                // and PIL can cause false 100% overlap for small details
                let is_small = total_s < 300;
                if pct >= 0.98 && !is_small && !protected_ids.contains(&small.id) {
                    covered.insert(small.id.clone());
                }
            }
        }
    }

    covered
}


/// Expand a polygon by `amount` pixels using geo-clipper offset.
/// Returns expanded points, or the original if offset fails.
fn expand_polygon(pts: &[[f64; 2]], amount: f64) -> Vec<[f64; 2]> {
    use geo::{Coord, LineString, Polygon};
    use geo::algorithm::area::Area;
    use geo_clipper::Clipper;

    let mut coords: Vec<Coord<f64>> = pts.iter().map(|p| Coord { x: p[0], y: p[1] }).collect();
    if coords.first() != coords.last() {
        coords.push(coords[0]);
    }
    let poly = Polygon::new(LineString::new(coords), vec![]);
    let expanded = poly.offset(amount, geo_clipper::JoinType::Square, geo_clipper::EndType::ClosedPolygon, 1000.0);
    // Take the largest polygon from result
    expanded.0.iter()
        .max_by(|a, b| a.unsigned_area().partial_cmp(&b.unsigned_area()).unwrap_or(std::cmp::Ordering::Equal))
        .map(|p| p.exterior().0.iter().map(|c| [c.x, c.y]).collect())
        .unwrap_or_else(|| pts.to_vec())
}

/// Render piece PNGs by cropping the MuPDF composite through piece polygon masks.
///
/// ARCHITECTURE NOTE: Vector brick and piece shapes are the source of truth.
/// Bounding boxes must NEVER be used for display or masking — only for bounds
/// checking. Users must see the actual polygon shapes of bricks and pieces.
///
/// The composite is seamless (no internal gaps). We mask each piece using its
/// union polygon (expanded 0.5px to ensure boundary pixels are included by
/// both adjacent pieces). This preserves the true shape of vector bricks
/// like arched windows.
pub fn render_piece_pngs_from_composite(
    pieces: &[crate::types::PuzzlePiece],
    composite: &RgbaImage,
    piece_polygons: &HashMap<String, Vec<Vec<[f64; 2]>>>,
    extract_dir: &Path,
) -> Vec<crate::types::PuzzlePiece> {
    std::fs::create_dir_all(extract_dir).ok();
    pieces
        .par_iter()
        .map(|piece| {
            let pw = piece.width.max(1) as u32;
            let ph = piece.height.max(1) as u32;
            let mut piece_img = RgbaImage::new(pw, ph);

            // Multi-ring piece polygon (one ring per disconnected
            // component of the brick union). Expand each ring by 0.5px
            // so boundary pixels are included on both sides of the
            // shared edge between adjacent pieces.
            let rings: Option<Vec<Vec<[f64; 2]>>> = piece_polygons.get(&piece.id)
                .map(|ring_list| ring_list.iter()
                    .filter(|r| r.len() >= 3)
                    .map(|r| expand_polygon(r, 0.5))
                    .collect());

            // Crop composite through the piece polygon mask
            for dy in 0..ph {
                for dx in 0..pw {
                    let cx = piece.x + dx as i32;
                    let cy = piece.y + dy as i32;
                    if cx < 0 || cy < 0 {
                        continue;
                    }
                    let sx = cx as u32;
                    let sy = cy as u32;
                    if sx >= composite.width() || sy >= composite.height() {
                        continue;
                    }

                    let in_poly = match &rings {
                        Some(rs) if !rs.is_empty() => {
                            let px = cx as f64 + 0.5;
                            let py = cy as f64 + 0.5;
                            rs.iter().any(|r| point_in_polygon(px, py, r))
                        }
                        _ => true,
                    };
                    if !in_poly {
                        continue;
                    }

                    let px = composite.get_pixel(sx, sy);
                    if px[3] > 0 {
                        piece_img.put_pixel(dx, dy, *px);
                    }
                }
            }

            // Trim the canvas to the alpha bbox of what we just painted.
            //
            // The input `piece.width × piece.height` is the union of the
            // piece's brick bboxes. AI layer polygons routinely overshoot
            // the visible pixels (anchor points outside the rendered shape,
            // bricks at the edges of the cluster that don't fully tile),
            // so the union bbox can be 2-3× the size of the actual
            // content. Untrimmed sprites end up with a large transparent
            // overhang — visible in PSD, in the per-piece preview, and
            // worst of all in Unity (the sprite anchor lands on dead
            // pixels). Cropping to the alpha bbox here makes the PNG
            // tight to the content, and returning a `PuzzlePiece` with
            // the same tight bbox lets downstream consumers
            // (build_house_data, PSD layer placement) recompute the
            // sprite's centre/anchor from the trimmed rect — so the
            // sprite stays visually in the same place.
            let trimmed_bbox = alpha_bbox(&piece_img);

            let (out_img, new_x, new_y, new_w, new_h) = match trimmed_bbox {
                Some((tx, ty, tw, th)) if tw > 0 && th > 0 => {
                    let cropped =
                        image::imageops::crop_imm(&piece_img, tx, ty, tw, th).to_image();
                    (
                        cropped,
                        piece.x + tx as i32,
                        piece.y + ty as i32,
                        tw as i32,
                        th as i32,
                    )
                }
                // Empty piece — keep the original bbox so downstream
                // logic still has something coherent to place. The
                // saved PNG is fully transparent in that case.
                _ => (piece_img, piece.x, piece.y, pw as i32, ph as i32),
            };

            out_img
                .save(extract_dir.join(format!("piece_{}.png", piece.id)))
                .ok();
            render_piece_outline(
                &out_img,
                &extract_dir.join(format!("piece_outline_{}.png", piece.id)),
            );

            crate::types::PuzzlePiece {
                id: piece.id.clone(),
                brick_ids: piece.brick_ids.clone(),
                x: new_x,
                y: new_y,
                width: new_w,
                height: new_h,
            }
        })
        .collect()
}

/// Tight bounding box of all pixels with non-zero alpha. Returns
/// `(x, y, width, height)` in image-local coords, or `None` when the
/// image is fully transparent.
fn alpha_bbox(img: &RgbaImage) -> Option<(u32, u32, u32, u32)> {
    let w = img.width();
    let h = img.height();
    if w == 0 || h == 0 {
        return None;
    }
    let mut min_x = w;
    let mut min_y = h;
    let mut max_x = 0_u32;
    let mut max_y = 0_u32;
    for y in 0..h {
        for x in 0..w {
            if img.get_pixel(x, y)[3] > 0 {
                if x < min_x {
                    min_x = x;
                }
                if y < min_y {
                    min_y = y;
                }
                if x + 1 > max_x {
                    max_x = x + 1;
                }
                if y + 1 > max_y {
                    max_y = y + 1;
                }
            }
        }
    }
    if max_x <= min_x || max_y <= min_y {
        None
    } else {
        Some((min_x, min_y, max_x - min_x, max_y - min_y))
    }
}

/// Render each piece by selectively enabling only its bricks' OCGs in
/// a one-time-injected copy of the source PDF. Output is byte-identical
/// in format to `render_piece_pngs_from_composite` — same piece_<id>.png
/// file at the same path, same canvas-relative cropping — but the
/// pixels come from a clean re-render rather than a polygon-masked
/// slice of the composite, so adjacent bricks' soft-mask alpha cannot
/// bleed into a piece's PNG.
///
/// Steps:
///   1. Re-parse the AI to recover the document-order list of parser
///      bricks (their order is what the matcher uses to bind PDF
///      blocks to bricks; the in-session `bricks_by_id` is a HashMap
///      and has lost that order). The parse is cheap relative to the
///      render that follows.
///   2. Walk + match + inject — produces a sibling PDF on disk with
///      one `hp_brick_NNNN` OCG per parser brick + a shared
///      `hp_decoration` OCG, all defaulting ON so the modified PDF
///      visually matches the original until we toggle.
///   3. For each piece, look up the OCG name for every brick id in
///      its `brick_ids`, then call the MuPDF clipped renderer with
///      that set of OCGs + `bricks` enabled, every other injected
///      OCG off. Crop the resulting clip pixmap to the piece's
///      canvas-relative bbox.
///
/// `ai_path` is the original AI; the modified copy is written next to
/// `extract_dir` and removed before return (live preview keeps using
/// the original).
fn render_piece_pngs_via_ocg_isolation(
    pieces: &[crate::types::PuzzlePiece],
    piece_polygons: &HashMap<String, Vec<Vec<[f64; 2]>>>,
    brick_layer_names: &HashMap<String, String>,
    export_dpi: f64,
    shifted_clip_pts: (f64, f64, f64, f64),
    new_canvas_w: u32,
    new_canvas_h: u32,
    out_dir: &Path,
    // Pre-loaded by `render_export_pieces` once for the whole export.
    // Eliminates the duplicate parse_ai + build_modified_pdf + walk +
    // match the per-piece function used to do on its own.
    placements: &[crate::ai_parser::BrickPlacement],
    meta: &crate::ai_parser::ParsedAiMetadata,
    artifact: &crate::ocg_inject::ModifiedPdfArtifact,
    doc: &lopdf::Document,
    blocks: &[crate::ocg_inject::BrickBlock],
    map: &crate::ocg_inject::BrickBlockMap,
    page_h_pt: f64,
) -> anyhow::Result<Vec<crate::types::PuzzlePiece>> {
    let brick_name_to_idx: HashMap<String, usize> = placements
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.clone(), i))
        .collect();

    let modified_pdf_str = artifact
        .pdf_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-utf8 modified PDF path"))?
        .to_string();

    // ── Render the whole-house NON-IMAGE content ONCE via MuPDF. We
    //    enable every per-brick OCG + the inline + decoration OCGs
    //    but DISABLE `hp_image` — so Image XObject blocks (which sit
    //    inside both their `hp_brick_NNNN` and the global `hp_image`
    //    OCGs) hide via PDF's intersection visibility rule, while
    //    every vector-only block paints normally. The result is the
    //    full house's vector overlays — window panes, decorations,
    //    Form blocks — drawn once into a canvas-sized image we can
    //    crop per piece. This replaces 60 per-piece MuPDF renders
    //    with one (~15× faster at NY5 export DPI).
    let mut enabled_non_image: Vec<&str> =
        Vec::with_capacity(artifact.brick_ocg_names.len() + 3);
    enabled_non_image.push("bricks");
    enabled_non_image.push(artifact.inline_ocg_name.as_str());
    enabled_non_image.push(artifact.decoration_ocg_name.as_str());
    for n in &artifact.brick_ocg_names {
        enabled_non_image.push(n.as_str());
    }
    let non_image_canvas: Option<RgbaImage> = crate::mupdf_ffi::render_page_with_ocg_set_clipped(
        &modified_pdf_str, &enabled_non_image, export_dpi, Some(shifted_clip_pts),
    )
    .and_then(|(rgba, pw, ph)| RgbaImage::from_raw(pw, ph, rgba))
    .map(|raw| compose_clipped_canvas(&raw, "non-image-all", new_canvas_w, new_canvas_h, (0, 0)));

    // Per-piece work is now Image-only: extract this piece's Image
    // blocks via raster_extract (no MuPDF), crop the global vector
    // canvas to the piece polygon, composite, mask, trim. Each piece
    // costs ~one image decode + a few hundred μs of compositing.
    let trimmed: Vec<crate::types::PuzzlePiece> = pieces
        .par_iter()
        .map(|piece| {
            // Resolve this piece's parser-brick indices.
            let piece_brick_idxs: Vec<usize> = piece
                .brick_ids
                .iter()
                .filter_map(|bid| brick_layer_names.get(bid))
                .filter_map(|name| brick_name_to_idx.get(name).copied())
                .collect();
            if piece_brick_idxs.is_empty() {
                let blank = RgbaImage::new(piece.width.max(1) as u32, piece.height.max(1) as u32);
                let _ = blank.save(out_dir.join(format!("piece_{}.png", piece.id)));
                render_piece_outline(
                    &blank,
                    &out_dir.join(format!("piece_outline_{}.png", piece.id)),
                );
                return piece.clone();
            }
            // Start from a copy of the global non-Image canvas (cheap
            // — same dimensions as the export canvas, no MuPDF call).
            let mut canvas = non_image_canvas.clone()
                .unwrap_or_else(|| RgbaImage::new(new_canvas_w, new_canvas_h));

            // Compose this piece's Image blocks on top of that canvas.
            // (Composing them BEFORE the polygon mask is fine — the
            // mask clips everything outside the piece's polygon
            // afterwards regardless of source.)
            let block_idxs: Vec<usize> = piece_brick_idxs.iter()
                .flat_map(|&bi| map.brick_to_blocks[bi].iter().copied())
                .collect();
            crate::raster_extract::compose_image_blocks_onto_canvas(
                doc, blocks, block_idxs,
                &mut canvas, meta.clip_rect, page_h_pt, artifact.bleed_pts,
                export_dpi, true,
            );
            let _ = &shifted_clip_pts; // accepted for API compat; the
                // shifted-clip math is now handled inside
                // `compose_image_blocks_onto_canvas` via bleed_pts.

            // Mask out the "leak" — content inside /OC /bricks that
            // wasn't matched to any per-brick OCG (yellow window-pane
            // gradients etc.). It renders whenever "bricks" is on,
            // contaminating areas that don't belong to this piece.
            //
            // We use the same piece-polygon mask as the composite-crop
            // path (`render_piece_pngs_from_composite`) so the OCG-
            // isolated output is visually almost identical to the old
            // composite-cropped pieces. The legitimate differences come
            // from the OCG isolation itself:
            //   - overhangs from neighbour bricks that bled into this
            //     piece in the composite are gone (neighbour bricks
            //     are off in the modified PDF, so they never paint)
            //   - this piece's own bricks' soft-mask overhangs that
            //     were occluded by neighbours in the composite are now
            //     visible (no neighbour to cover them)
            // Both effects only show at the piece boundary, on the
            // order of a couple pixels.
            // Multi-ring mask: keep pixels inside ANY component of the
            // piece's polygon union. Each ring is expanded by 0.5px so
            // boundary pixels are included on both sides of a shared
            // edge between adjacent pieces. Source of truth: the union
            // of all bricks the merge assigned to this piece — so a
            // brick whose component doesn't quite touch the rest still
            // paints (was previously dropped by the "largest component
            // only" filter).
            if let Some(ring_list) = piece_polygons.get(&piece.id) {
                let expanded: Vec<Vec<[f64; 2]>> = ring_list.iter()
                    .filter(|r| r.len() >= 3)
                    .map(|r| expand_polygon(r, 0.5))
                    .collect();
                if !expanded.is_empty() {
                    for (x, y, pixel) in canvas.enumerate_pixels_mut() {
                        if pixel[3] == 0 { continue; }
                        let px = x as f64 + 0.5;
                        let py = y as f64 + 0.5;
                        let inside = expanded.iter()
                            .any(|r| point_in_polygon(px, py, r));
                        if !inside { pixel[3] = 0; }
                    }
                }
            }

            // The bricks in our AI files are raster images with
            // soft-mask alpha that bakes 3-D effects (drop shadows,
            // gradient glows, highlights) into the source pixels.
            // The alpha extends past the geometric brick outline
            // intentionally — that's the visual style.
            //
            // In the live house composite those overhangs get
            // covered by neighbour bricks' opaque pixels and are
            // invisible. But when we isolate one piece via OCG
            // toggling, neighbours never paint, so the overhangs
            // are exposed at the OUTER edges of the piece. We
            // **keep** them: the piece PNG is the alpha bbox of
            // everything painted by this piece's OCGs — no polygon
            // mask, no crop to the piece's geometric bbox.
            //
            // Internal edges (between two bricks inside the same
            // piece) are unaffected because both bricks paint and
            // their overhangs overlap with the neighbour brick's
            // body, same as in the composite. Only the piece's
            // OUTER boundary keeps its bleed.
            let (out_img, new_x, new_y, new_w, new_h) = match alpha_bbox(&canvas) {
                Some((tx, ty, tw, th)) if tw > 0 && th > 0 => {
                    let cropped =
                        image::imageops::crop_imm(&canvas, tx, ty, tw, th).to_image();
                    (cropped, tx as i32, ty as i32, tw as i32, th as i32)
                }
                _ => {
                    // No painted pixels at all — emit an empty PNG
                    // sized to the piece's geometric bbox and keep
                    // the rect unchanged so downstream coords stay
                    // coherent.
                    let pw_i = piece.width.max(1) as u32;
                    let ph_i = piece.height.max(1) as u32;
                    (
                        RgbaImage::new(pw_i, ph_i),
                        piece.x,
                        piece.y,
                        pw_i as i32,
                        ph_i as i32,
                    )
                }
            };

            let _ = out_img.save(out_dir.join(format!("piece_{}.png", piece.id)));
            render_piece_outline(
                &out_img,
                &out_dir.join(format!("piece_outline_{}.png", piece.id)),
            );

            crate::types::PuzzlePiece {
                id: piece.id.clone(),
                brick_ids: piece.brick_ids.clone(),
                x: new_x,
                y: new_y,
                width: new_w,
                height: new_h,
            }
        })
        .collect();

    // Note: the sidecar modified PDF is now owned by the caller
    // (`render_export_pieces`) and gets cleaned up there.
    Ok(trimmed)
}

/// Render all export-time assets at the requested DPI:
///   - `piece_<id>.png` — per-piece sprite, polygon-masked
///   - `composite.png`  — the full house bricks composite
///   - `background.png` — the blueprint backdrop (when the AI
///     declares a `background` OCG layer)
///   - `outlines.png`   — single canvas-sized transparent image
///     with every piece's outline stroked from its vector polygon
///     so curves stay smooth at any DPI
///
/// MuPDF re-rasterises the source layers at `export_dpi` — vector
/// bricks come back crisp, raster bricks come back at MuPDF's best
/// resolution. Pieces and polygons are scaled by `export_dpi /
/// loaded_dpi` to line up with the new pixmaps. Live-preview state
/// under `session.extract_dir` is untouched — `out_dir` is expected
/// to be a sub-directory the caller created.
/// Returns the per-piece trimmed `PuzzlePiece` records in **export-DPI
/// canvas coords** — the same coord space the rendered piece PNGs live
/// in. Downstream encoders should use these (not the original
/// loaded-DPI pieces) so the sprite rect they place into PSD/Unity
/// matches the cropped PNG: tight to the visible content rather than
/// padded out to the union of the piece's brick bboxes. See
/// `render_piece_pngs_from_composite` for the trim rationale.
pub fn render_export_pieces(
    ai_path: &Path,
    pieces: &[crate::types::PuzzlePiece],
    bricks: &HashMap<String, crate::types::Brick>,
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
    // Per-brick bezier outlines in PyMuPDF point coords (the form the AI
    // parser emits and the session keeps). Used at export DPI to produce
    // `outlines.png` — same bezier-merge path the live UI uses, so the
    // exported outline matches what the user sees in the editor.
    brick_beziers: &HashMap<String, Vec<crate::bezier::BezierPath>>,
    // Maps hashed brick ID → AI layer name. Required for OCG isolation to
    // translate the hashed IDs stored in `piece.brick_ids` back to the
    // layer names that `brick_name_to_idx` is keyed by.
    brick_layer_names: &HashMap<String, String>,
    loaded_canvas_width: i32,
    loaded_canvas_height: i32,
    clip_rect_pts: (f64, f64, f64, f64),
    loaded_dpi: f64,
    export_dpi: f64,
    pdf_offset_loaded: (i32, i32),
    out_dir: &Path,
) -> anyhow::Result<Vec<crate::types::PuzzlePiece>> {
    if loaded_dpi <= 0.0 {
        anyhow::bail!("loaded_dpi must be > 0");
    }
    if export_dpi <= 0.0 {
        anyhow::bail!("export_dpi must be > 0");
    }

    let scale = export_dpi / loaded_dpi;
    let new_canvas_w = ((loaded_canvas_width as f64) * scale).round().max(1.0) as u32;
    let new_canvas_h = ((loaded_canvas_height as f64) * scale).round().max(1.0) as u32;

    // `pdf_offset_loaded` and `clip_rect_pts` are kept in the signature
    // for backwards compatibility but no longer used directly — the
    // export now derives a sub-pixel-precise `shifted_clip` from the
    // modified PDF artifact below. The probe-based offset was integer-
    // pixel quantised and produced a ~1px misalignment between MuPDF
    // and direct-extracted raster placements.
    let _ = pdf_offset_loaded;
    let _ = clip_rect_pts;

    std::fs::create_dir_all(out_dir)
        .map_err(|e| anyhow::anyhow!("create_dir_all({}): {e}", out_dir.display()))?;

    // ── Up-front: parse the AI, inject per-brick OCGs, walk + match
    //     blocks. We reuse all four pieces of state — placements,
    //     the modified PDF artifact (carries bleed_pts), the lopdf
    //     Document (for direct image extraction), and the matched
    //     brick→blocks map — across both the composite step and the
    //     per-piece rendering loop. Doing this once avoids the parse
    //     + walk + match running twice.
    let (placements, meta, _) = crate::ai_parser::parse_ai(
        ai_path, crate::CANVAS_HEIGHT_PX as i32,
    ).map_err(|e| anyhow::anyhow!("parse_ai (for export): {e}"))?;
    let modified_pdf_path = out_dir.join("_hp_ocg_modified.pdf");
    let artifact = crate::ocg_inject::build_modified_pdf(
        ai_path, &placements, &meta, &modified_pdf_path,
    ).map_err(|e| anyhow::anyhow!("building modified PDF (for export): {e}"))?;
    let doc = lopdf::Document::load(ai_path)
        .map_err(|e| anyhow::anyhow!("loading PDF for direct extract: {e}"))?;
    let page_id = doc.page_iter().next()
        .ok_or_else(|| anyhow::anyhow!("PDF has no pages"))?;
    let blocks = crate::ocg_inject::walk_page_bricks(&doc, page_id)
        .map_err(|e| anyhow::anyhow!("walk_page_bricks: {e}"))?;
    let page_h_pt = {
        let p = doc.get_object(page_id)
            .and_then(|o| o.as_dict())
            .map_err(|e| anyhow::anyhow!("page dict: {e}"))?;
        let media = p.get(b"MediaBox").ok().and_then(|o| o.as_array().ok())
            .ok_or_else(|| anyhow::anyhow!("no MediaBox"))?;
        match media.get(3) {
            Some(lopdf::Object::Real(r)) => *r as f64,
            Some(lopdf::Object::Integer(i)) => *i as f64,
            _ => anyhow::bail!("MediaBox[3] not numeric"),
        }
    };
    let geo = crate::ocg_inject::PageGeometry {
        clip_x0: meta.clip_rect.0, clip_y0: meta.clip_rect.1,
        render_dpi: meta.render_dpi, page_height_pt: page_h_pt,
        bleed_x: artifact.bleed_pts.0, bleed_y: artifact.bleed_pts.1,
    };
    let map = crate::ocg_inject::match_blocks_to_bricks(
        &blocks, &placements, geo, crate::ocg_inject::DEFAULT_OVERLAY_RADIUS_PT,
    );

    // Refined bleed → sub-pixel-precise shifted clip.
    let shifted_clip = (
        meta.clip_rect.0 + artifact.bleed_pts.0,
        meta.clip_rect.1 + artifact.bleed_pts.1,
        meta.clip_rect.2 + artifact.bleed_pts.0,
        meta.clip_rect.3 + artifact.bleed_pts.1,
    );

    // 1. Render the bricks layer composite via MuPDF as the BASE (covers
    //    vector overlays, decorations, inline content). Then overlay
    //    direct-extracted Image XObjects on top — they replace MuPDF's
    //    re-rasterised pixels with the AI file's original raster data.
    //    Where direct doesn't paint (vector content, gaps), MuPDF's
    //    pixels survive.
    let (bricks_pixmap, _, _) = render_ocg_layer_pixmap_clipped(
        ai_path, "bricks", export_dpi, shifted_clip,
    )
    .ok_or_else(|| anyhow::anyhow!("Failed to render bricks layer at export DPI"))?;
    let mut composite = compose_clipped_canvas(
        &bricks_pixmap, "bricks", new_canvas_w, new_canvas_h, (0, 0),
    );
    crate::raster_extract::compose_image_blocks_onto_canvas(
        &doc, &blocks, 0..blocks.len(),
        &mut composite, meta.clip_rect, page_h_pt, artifact.bleed_pts,
        export_dpi, true,
    );
    composite
        .save(out_dir.join("composite.png"))
        .map_err(|e| anyhow::anyhow!("saving composite.png: {e}"))?;

    // 2. Re-render the OCG background layer (blueprint backdrop) at
    //    the same shifted clip so it aligns with the composite.
    //    Missing layer is non-fatal — not every AI defines one.
    if let Some((bg_pixmap, _, _)) =
        render_ocg_layer_pixmap_clipped(ai_path, "background", export_dpi, shifted_clip)
    {
        let bg_canvas =
            compose_clipped_canvas(&bg_pixmap, "background", new_canvas_w, new_canvas_h, (0, 0));
        bg_canvas
            .save(out_dir.join("background.png"))
            .map_err(|e| anyhow::anyhow!("saving background.png: {e}"))?;

        // ── background_highlight.png ──────────────────────────────
        // White Gaussian-falloff halo centred on the alpha boundary of
        // the house silhouette (incl. each window/door opening). The
        // alpha is derived analytically from the SIGNED distance to
        // the boundary — `α(d) = 255 · exp(−d²/(2σ²))` — using a real
        // Euclidean distance transform. Because the formula only
        // depends on |d|, sharp concave corners get the same band
        // width as straight edges (the box-blur approximation we
        // used before pooled darker into 90° inner corners and read
        // like overlapping drop shadows; this is rotationally
        // symmetric by construction).
        //
        // The result is saved at a padded canvas size so the halo
        // can bleed past the bricks' external edges; the sidecar
        // `background_highlight.json` records the (−PAD_PX, −PAD_PX)
        // placement offset for consumers.
        const PAD_PX: i32 = 80;
        let pad_w = new_canvas_w as i32 + 2 * PAD_PX;
        let pad_h = new_canvas_h as i32 + 2 * PAD_PX;

        // Binary mask: 255 inside silhouette, 0 outside.
        // Crucially, EVERY non-silhouette pixel must be tagged as
        // "outside" — including pixels in the padded margin around the
        // bg_canvas. If we only tagged transparent pixels INSIDE
        // bg_canvas, the boundary along a canvas edge (where the
        // silhouette touches the edge directly) was invisible to the
        // distance transform: padding pixels found no "outside"
        // neighbour nearby, the falloff died, and the halo had a flat
        // missing edge there.
        let mut mask_in = image::GrayImage::new(pad_w as u32, pad_h as u32);
        let mut mask_out = image::ImageBuffer::from_pixel(
            pad_w as u32, pad_h as u32, image::Luma([255u8]),
        );
        for (x, y, p) in bg_canvas.enumerate_pixels() {
            let nx = x as i32 + PAD_PX;
            let ny = y as i32 + PAD_PX;
            if nx < 0 || ny < 0 || nx >= pad_w || ny >= pad_h { continue; }
            if p[3] > 8 {
                mask_in.put_pixel(nx as u32, ny as u32, image::Luma([255]));
                mask_out.put_pixel(nx as u32, ny as u32, image::Luma([0]));
            }
        }
        // Squared Euclidean distance from each pixel to the nearest
        // zero pixel. `d_in[x,y]² = distance² from inside-pixel to
        // nearest outside-pixel = distance² to boundary on the inside`.
        // For pixels outside the silhouette, we transform `mask_out`
        // instead — same idea, reversed.
        let d2_in = imageproc::distance_transform::euclidean_squared_distance_transform(&mask_in);
        let d2_out = imageproc::distance_transform::euclidean_squared_distance_transform(&mask_out);

        // σ in pixels — ~7 pt FWHM-ish at any export DPI.
        let sigma_px = export_dpi * (3.5 / 72.0);
        let two_sigma_sq = 2.0 * sigma_px * sigma_px;

        let mut highlight = RgbaImage::new(pad_w as u32, pad_h as u32);
        for y in 0..pad_h {
            for x in 0..pad_w {
                // Distance squared from (x,y) to the nearest boundary
                // pixel: pixels OUTSIDE the silhouette get `d2_out`
                // (= squared distance from outside-pixel to nearest
                // inside-pixel), pixels INSIDE get `d2_in`. The two
                // images cover complementary domains so adding them
                // works regardless of which side `(x,y)` sits on.
                let d2 = d2_in.get_pixel(x as u32, y as u32)[0]
                       + d2_out.get_pixel(x as u32, y as u32)[0];
                let intensity = (-d2 / two_sigma_sq).exp();
                let a = (255.0 * intensity).round().clamp(0.0, 255.0) as u8;
                if a > 0 {
                    highlight.put_pixel(x as u32, y as u32, Rgba([255, 255, 255, a]));
                }
            }
        }
        highlight
            .save(out_dir.join("background_highlight.png"))
            .map_err(|e| anyhow::anyhow!("saving background_highlight.png: {e}"))?;
        // Sidecar: PAD offset so consumers can place the asset.
        let _ = std::fs::write(
            out_dir.join("background_highlight.json"),
            format!(
                "{{\"padding\": {}, \"width\": {}, \"height\": {}, \"x\": {}, \"y\": {}}}\n",
                PAD_PX, pad_w, pad_h, -PAD_PX, -PAD_PX
            ),
        );
    }

    // ── lights.png ─────────────────────────────────────────────────
    // Optional `lights` OCG layer — the warm window-pane overlay that
    // sits on top of everything (window glow). Same render path as
    // background, same canvas size, same (0, 0) placement.
    if let Some((lt_pixmap, _, _)) =
        render_ocg_layer_pixmap_clipped(ai_path, "lights", export_dpi, shifted_clip)
    {
        let lt_canvas =
            compose_clipped_canvas(&lt_pixmap, "lights", new_canvas_w, new_canvas_h, (0, 0));
        lt_canvas
            .save(out_dir.join("lights.png"))
            .map_err(|e| anyhow::anyhow!("saving lights.png: {e}"))?;
    }

    // assets.json is written near the end of this function (after
    // outlines.png) so it can record every asset present.

    // 3. Scale every piece + brick + brick polygon by the same factor
    //    so they line up with the new composite.
    let scaled_pieces: Vec<crate::types::PuzzlePiece> = pieces
        .iter()
        .map(|p| crate::types::PuzzlePiece {
            id: p.id.clone(),
            brick_ids: p.brick_ids.clone(),
            x: ((p.x as f64) * scale).round() as i32,
            y: ((p.y as f64) * scale).round() as i32,
            width: ((p.width as f64) * scale).round().max(1.0) as i32,
            height: ((p.height as f64) * scale).round().max(1.0) as i32,
        })
        .collect();

    let scaled_bricks: HashMap<String, crate::types::Brick> = bricks
        .iter()
        .map(|(id, b)| {
            let mut nb = b.clone();
            nb.x = ((b.x as f64) * scale).round() as i32;
            nb.y = ((b.y as f64) * scale).round() as i32;
            nb.width = ((b.width as f64) * scale).round().max(1.0) as i32;
            nb.height = ((b.height as f64) * scale).round().max(1.0) as i32;
            (id.clone(), nb)
        })
        .collect();

    let scaled_brick_polys: HashMap<String, Vec<[f64; 2]>> = brick_polygons
        .iter()
        .map(|(id, pts)| {
            let scaled: Vec<[f64; 2]> = pts.iter().map(|p| [p[0] * scale, p[1] * scale]).collect();
            (id.clone(), scaled)
        })
        .collect();

    // 4. Compute piece polygons at the new scale.
    let piece_polys =
        crate::puzzle::compute_piece_polygons(&scaled_pieces, &scaled_bricks, &scaled_brick_polys);

    // 5. Per-piece sprite PNGs.
    //
    // Preferred path: OCG-isolated re-render. The PDF is rewritten
    // with one OCG per parser brick, then each piece is rasterised
    // with only its own bricks' OCGs enabled. Neighbour bricks are
    // never invoked by MuPDF, so the soft-mask alpha (Illustrator's
    // baked 3-D shadows/glows) cannot bleed across piece boundaries.
    //
    // Fallback: composite-slice. If any step of the OCG pipeline
    // fails (parse, inject, save, render), we slice the per-piece
    // PNG out of the bricks composite the old way. That bleeds, but
    // it's still a usable export — better than crashing.
    //
    // Both paths trim each PNG to its alpha bbox and return updated
    // `PuzzlePiece` rects in export-DPI canvas coords; downstream
    // encoders read those rects to place the sprite at its true
    // visible centre. See `render_piece_pngs_from_composite` for the
    // trim rationale.
    let trimmed_pieces = match render_piece_pngs_via_ocg_isolation(
        &scaled_pieces,
        &piece_polys,
        brick_layer_names,
        export_dpi,
        shifted_clip,
        new_canvas_w,
        new_canvas_h,
        out_dir,
        &placements,
        &meta,
        &artifact,
        &doc,
        &blocks,
        &map,
        page_h_pt,
    ) {
        Ok(trimmed) => trimmed,
        Err(e) => {
            eprintln!(
                "[render_export_pieces] OCG-isolation path failed ({e}); \
                 falling back to composite-slice path for piece PNGs"
            );
            render_piece_pngs_from_composite(&scaled_pieces, &composite, &piece_polys, out_dir)
        }
    };

    // 6. Trace every piece outline onto a single canvas-sized
    //    transparent image — one `outlines.png` overlay that the
    //    Unity importer (or any consumer) can layer over the
    //    composite or background.
    //
    // Outlines come from the SAME bezier-merge code (`merge_piece_bezier`)
    // that the live editor UI uses, so the exported outline matches
    // the one the user sees on screen. The previous path stroked the
    // geo_clipper polygon union, whose expand/erode + chord
    // approximation occasionally drops the canvas-boundary edges of
    // outermost pieces (visible as a missing outer house silhouette
    // in exports). The bezier-merge path preserves cubic curves and
    // every brick edge that doesn't get cancelled by an adjacent
    // brick's edge — i.e. it draws exactly the piece-boundary set
    // the editor draws, including outer silhouette edges.
    let stroke_thickness = ((export_dpi / 96.0).round() as i32).max(1);
    // Samples per cubic — at higher DPI we need more samples so a
    // brick's arch stays visibly smooth after stroking. 1 sample per
    // ~6 pixels of bezier extent is enough; capped at 64.
    let samples_per_curve = ((export_dpi / 8.0).round() as usize).clamp(16, 64);
    let export_pt_to_canvas = export_dpi / 72.0;
    // Pad the outlines canvas so a piece whose bezier path coincides
    // with the canvas edge isn't clipped to half stroke width. The
    // outline's exterior-most pixels can sit at most `stroke_thickness`
    // outside the geometric path; we leave a little extra room for
    // safety. Result is saved at the padded size with a (-PAD,-PAD)
    // placement offset (mirrors background_highlight.png).
    const OUTLINES_PAD_PX: i32 = 16;
    let out_pad_w = new_canvas_w as i32 + 2 * OUTLINES_PAD_PX;
    let out_pad_h = new_canvas_h as i32 + 2 * OUTLINES_PAD_PX;
    let canvas_shift = [
        -clip_rect_pts.0 + (OUTLINES_PAD_PX as f64) / export_pt_to_canvas,
        -clip_rect_pts.1 + (OUTLINES_PAD_PX as f64) / export_pt_to_canvas,
    ];
    let mut outlines = RgbaImage::new(out_pad_w as u32, out_pad_h as u32);
    for piece in pieces {
        // Collect this piece's brick beziers in PyMuPDF point coords.
        let mut input: Vec<crate::bezier::BezierPath> = Vec::new();
        for bid in &piece.brick_ids {
            if let Some(paths) = brick_beziers.get(bid) {
                input.extend(paths.iter().cloned());
            }
        }
        if input.is_empty() {
            continue;
        }
        // Merge into closed bezier rings (one per disjoint loop in the
        // piece), then move into export-DPI canvas pixel space the
        // composite was rendered in.
        let merged = crate::bezier_merge::merge_piece_bezier(&input);
        for bp in &merged {
            let bp_canvas = bp.transform(canvas_shift, export_pt_to_canvas);
            let polyline = bp_canvas.tessellate(samples_per_curve);
            if polyline.len() < 2 {
                continue;
            }
            stroke_polygon_on_canvas(
                &mut outlines,
                &polyline,
                0.0,
                0.0,
                Rgba([255, 255, 255, 230]),
                stroke_thickness,
            );
        }
    }
    outlines
        .save(out_dir.join("outlines.png"))
        .map_err(|e| anyhow::anyhow!("saving outlines.png: {e}"))?;

    // ── assets.json ────────────────────────────────────────────────
    // Compact placement metadata for every canvas-aligned asset so
    // consumers (the test viewer, Unity importer) can position layers
    // without hardcoded knowledge. Highlight has padded dimensions
    // and a negative offset because its blur halo extends past the
    // canvas; the others sit at (0, 0).
    let canvas_assets: Vec<&str> = vec![
        "composite.png",
        "background.png",
        "background_highlight.png",
        "outlines.png",
        "lights.png",
    ];
    let mut assets_json = String::from("{\n");
    assets_json.push_str(&format!("  \"canvas_w\": {},\n  \"canvas_h\": {},\n",
        new_canvas_w, new_canvas_h));
    assets_json.push_str("  \"assets\": {\n");
    let mut first_asset = true;
    for fname in &canvas_assets {
        if !out_dir.join(fname).exists() { continue; }
        let (x, y, w, h) = if *fname == "background_highlight.png" {
            let pad = 80i32;
            (-pad, -pad, new_canvas_w as i32 + 2 * pad, new_canvas_h as i32 + 2 * pad)
        } else if *fname == "outlines.png" {
            let pad = OUTLINES_PAD_PX;
            (-pad, -pad, new_canvas_w as i32 + 2 * pad, new_canvas_h as i32 + 2 * pad)
        } else {
            (0, 0, new_canvas_w as i32, new_canvas_h as i32)
        };
        if !first_asset { assets_json.push_str(",\n"); }
        first_asset = false;
        assets_json.push_str(&format!(
            "    \"{}\": {{\"file\": \"{}\", \"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}}}",
            fname, fname, x, y, w, h
        ));
    }
    assets_json.push_str("\n  }\n}\n");
    let _ = std::fs::write(out_dir.join("assets.json"), assets_json);

    // Clean up the sidecar modified PDF the OCG-isolation pass wrote
    // next to the export outputs. Failure to delete is non-fatal — the
    // file just sits unused next to the export. Set
    // HP_KEEP_MODIFIED_PDF=1 to retain it for debugging.
    if std::env::var("HP_KEEP_MODIFIED_PDF").is_err() {
        let _ = std::fs::remove_file(&modified_pdf_path);
    }

    Ok(trimmed_pieces)
}

/// Draw a closed polygon as a stroked outline onto `canvas`. Each
/// vertex is translated by `(-offset_x, -offset_y)` so a piece-local
/// canvas can be filled directly from a canvas-coords polygon.
/// `thickness` is in pixels (clamped to ≥1).
fn stroke_polygon_on_canvas(
    canvas: &mut RgbaImage,
    polygon: &[[f64; 2]],
    offset_x: f64,
    offset_y: f64,
    color: Rgba<u8>,
    thickness: i32,
) {
    if polygon.len() < 2 {
        return;
    }
    let half = (thickness.max(1) - 1) / 2;
    let w = canvas.width() as i32;
    let h = canvas.height() as i32;
    let put = |canvas: &mut RgbaImage, x: i32, y: i32| {
        for dy in -half..=half {
            for dx in -half..=half {
                let nx = x + dx;
                let ny = y + dy;
                if nx >= 0 && ny >= 0 && nx < w && ny < h {
                    canvas.put_pixel(nx as u32, ny as u32, color);
                }
            }
        }
    };

    for i in 0..polygon.len() {
        let a = polygon[i];
        let b = polygon[(i + 1) % polygon.len()];
        let x0 = (a[0] - offset_x).round() as i32;
        let y0 = (a[1] - offset_y).round() as i32;
        let x1 = (b[0] - offset_x).round() as i32;
        let y1 = (b[1] - offset_y).round() as i32;

        // Bresenham
        let dx_abs = (x1 - x0).abs();
        let dy_abs = -(y1 - y0).abs();
        let sx = if x0 < x1 { 1 } else { -1 };
        let sy = if y0 < y1 { 1 } else { -1 };
        let mut err = dx_abs + dy_abs;
        let mut x = x0;
        let mut y = y0;
        loop {
            put(canvas, x, y);
            if x == x1 && y == y1 {
                break;
            }
            let e2 = 2 * err;
            if e2 >= dy_abs {
                err += dy_abs;
                x += sx;
            }
            if e2 <= dx_abs {
                err += dx_abs;
                y += sy;
            }
        }
    }
}

/// Render piece outline PNG.
fn render_piece_outline(piece_img: &RgbaImage, out_path: &Path) {
    let w = piece_img.width();
    let h = piece_img.height();
    let mut outline = RgbaImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            if piece_img.get_pixel(x, y)[3] < 30 { continue; }
            let is_border = [(0i32, -1), (0, 1), (-1, 0), (1, 0)]
                .iter()
                .any(|&(dx, dy)| {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 { return true; }
                    piece_img.get_pixel(nx as u32, ny as u32)[3] < 30
                });
            if is_border {
                outline.put_pixel(x, y, Rgba([255, 255, 255, 200]));
            }
        }
    }
    outline.save(out_path).ok();
}

// `render_outlines_png` was removed: since the bezier port the editor
// draws pre-gen brick outlines straight from `brick.outline_paths`
// SVGs in the Elm side, so the standalone PNG was redundant — and
// rendering it (~0.7 s with 4× supersampling and Bresenham line
// drawing) was the slow side of the load-stage parallel block.

/// Render a specific OCG layer to an RgbaImage (for in-memory use).
/// Uses pure FFI rendering to ensure OCG layer state is respected.
/// Render the page with the given OCG layer toggled on, returning the
/// raw MuPDF pixmap (and its pixel dimensions). The pixmap is what's
/// expensive — it's the full MuPDF rasterization. The caller can then
/// compose it onto a canvas at any offset / clip without re-rendering.
pub fn render_ocg_layer_pixmap(
    ai_path: &Path,
    layer_name: &str,
    dpi: f64,
) -> Option<(RgbaImage, u32, u32)> {
    use crate::mupdf_ffi;

    let (rgba, pw, ph) = mupdf_ffi::render_page_with_ocg(
        ai_path.to_str()?, layer_name, dpi,
    )?;
    let full_img = RgbaImage::from_raw(pw, ph, rgba)?;
    Some((full_img, pw, ph))
}

/// Clip-aware pixmap render. MuPDF rasterises only the given clip
/// rect (in PDF points) instead of the full mediabox — on AI files
/// where the artwork occupies a small fraction of the page (typical
/// for the NY houses, where mediabox padding is significant) this
/// can roughly halve the OCG render time.
///
/// The returned pixmap covers the clip area at the given DPI; the
/// caller composes it onto the canvas via `compose_clipped_canvas`.
pub fn render_ocg_layer_pixmap_clipped(
    ai_path: &Path,
    layer_name: &str,
    dpi: f64,
    clip_rect_pts: (f64, f64, f64, f64),
) -> Option<(RgbaImage, u32, u32)> {
    use crate::mupdf_ffi;

    let (rgba, pw, ph) = mupdf_ffi::render_page_with_ocg_clipped(
        ai_path.to_str()?, layer_name, dpi, Some(clip_rect_pts),
    )?;
    let img = RgbaImage::from_raw(pw, ph, rgba)?;
    Some((img, pw, ph))
}

/// Cheap step: paste an already-rendered pixmap onto a canvas-sized
/// RGBA at a given clip + offset. Decoupled from the MuPDF render so
/// the caller can re-overlay with a corrected `pdf_offset_px` after
/// detecting it from the first compose, without paying for another
/// full-page rasterization.
pub fn compose_ocg_canvas(
    full_img: &RgbaImage,
    layer_name: &str,
    dpi: f64,
    clip_rect: (f64, f64, f64, f64),
    canvas_width: u32,
    canvas_height: u32,
    pdf_offset_px: (i32, i32),
) -> RgbaImage {
    let scale = dpi / 72.0;
    let cx = (clip_rect.0 * scale).round() as i64;
    let cy = (clip_rect.1 * scale).round() as i64;
    let dx = pdf_offset_px.0 as i64;
    let dy = pdf_offset_px.1 as i64;

    eprintln!("[compose_ocg] layer={layer_name} dpi={dpi:.1} canvas={canvas_width}x{canvas_height} clip_px=({cx},{cy}) offset=({dx},{dy}) overlay_at=({},{})",
        -(cx - dx), -(cy - dy));

    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(&mut canvas, full_img, -(cx - dx), -(cy - dy));
    canvas
}

/// Cheap compose for the clipped pixmap: the input already starts at
/// the clip origin, so we just place it at `pdf_offset_px` on the
/// canvas (no `clip_rect` math needed). Use with
/// `render_ocg_layer_pixmap_clipped`.
pub fn compose_clipped_canvas(
    clipped_img: &RgbaImage,
    layer_name: &str,
    canvas_width: u32,
    canvas_height: u32,
    pdf_offset_px: (i32, i32),
) -> RgbaImage {
    eprintln!(
        "[compose_clipped] layer={layer_name} canvas={canvas_width}x{canvas_height} pixmap={}x{} offset=({},{})",
        clipped_img.width(), clipped_img.height(), pdf_offset_px.0, pdf_offset_px.1
    );
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(
        &mut canvas, clipped_img, pdf_offset_px.0 as i64, pdf_offset_px.1 as i64,
    );
    canvas
}

/// Render-and-compose in one call. Convenience wrapper for callers
/// that don't need to re-overlay (lights, background — they call this
/// once with offset already known).
pub fn render_ocg_layer_image(
    ai_path: &Path,
    layer_name: &str,
    dpi: f64,
    clip_rect: (f64, f64, f64, f64),
    canvas_width: u32,
    canvas_height: u32,
    pdf_offset_px: (i32, i32),
) -> Option<RgbaImage> {
    let (full_img, _, _) = render_ocg_layer_pixmap(ai_path, layer_name, dpi)?;
    Some(compose_ocg_canvas(
        &full_img, layer_name, dpi, clip_rect, canvas_width, canvas_height, pdf_offset_px,
    ))
}

/// Render OCG layer and save to PNG file.
pub fn render_ocg_layer(
    ai_path: &Path,
    layer_name: &str,
    out_path: &Path,
    dpi: f64,
    clip_rect: (f64, f64, f64, f64),
    canvas_width: u32,
    canvas_height: u32,
    pdf_offset_px: (i32, i32),
) -> bool {
    match render_ocg_layer_image(ai_path, layer_name, dpi, clip_rect, canvas_width, canvas_height, pdf_offset_px) {
        Some(img) => { img.save(out_path).ok(); true }
        None => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_parser;

    #[test]
    fn test_extract_raster_brick() {
        let ai_path = std::path::PathBuf::from("../../in/_NY1.ai");
        if !ai_path.exists() {
            eprintln!("Skipping: in/_NY1.ai not found");
            return;
        }
        let ai_data = ai_parser::decompress_ai_data(&ai_path).unwrap();
        let roots = ai_parser::parse_layer_tree(&ai_data.raw);
        let bricks_node = roots.iter().find(|r| r.name == "bricks").unwrap();

        let first_child = &bricks_node.children[0];
        let block_data = &ai_data.raw[first_child.begin..first_child.end];
        let img = extract_raster_image(block_data);

        if let Some(img) = img {
            eprintln!("First brick raster: {}x{}", img.width(), img.height());
            assert!(img.width() > 0 && img.height() > 0);
        } else {
            eprintln!("First brick has no raster (may be vector-only)");
        }

        let raster_count = bricks_node.children.iter()
            .filter(|c| {
                let bd = &ai_data.raw[c.begin..c.end];
                extract_raster_image(bd).is_some()
            })
            .count();
        eprintln!("Bricks with rasters: {}/{}", raster_count, bricks_node.children.len());
    }

    // ── Raster canary ───────────────────────────────────────────────────
    //
    // The user has explicitly de-prioritised raster-handling testing
    // ("not sure what is there to test, a canary that it loads at
    // all would be enough"). Synthetic blob: build a minimal byte
    // stream matching the `Xh` + `%%BeginData ... XI` shape the
    // parser expects. If `extract_raster_image` ever returns None
    // for this we've broken the raster pipeline at the regex /
    // byte-arithmetic level. No fixture dependency so it works on a
    // stripped checkout.

    #[test]
    fn raster_canary_extracts_synthetic_2x2_image() {
        let mut blob: Vec<u8> = Vec::new();
        blob.extend_from_slice(b"[1 0 0 1 0 0] 2 2 8 Xh\n");
        // The BeginData regex is `%%BeginData:\s*\d+[^\r\n]*XI[\r\n]+`,
        // so `XI` has to live on the same line as the header.
        blob.extend_from_slice(b"%%BeginData: 12 Hex Bytes XI\n");
        // (0,0) red, (1,0) green, (0,1) blue, (1,1) white (white → alpha 0).
        blob.extend_from_slice(&[
            255, 0, 0,
            0, 255, 0,
            0, 0, 255,
            255, 255, 255,
        ]);

        let img = extract_raster_image(&blob).expect("synthetic blob should parse");
        assert_eq!(img.width(), 2);
        assert_eq!(img.height(), 2);
        let red = img.get_pixel(0, 0);
        assert_eq!(red[0], 255, "red pixel red channel");
        assert_eq!(red[3], 255, "red pixel should be opaque");
        let white = img.get_pixel(1, 1);
        assert_eq!(white[3], 0, "white pixel should be alpha-zeroed");
    }

    // ── compute_pdf_offset (synthetic pixmaps) ──────────────────────────

    use image::{Rgba, RgbaImage};

    fn pixmap_with_first_opaque_at(w: u32, h: u32, fx: u32, fy: u32) -> RgbaImage {
        let mut img = RgbaImage::new(w, h);
        // All pixels transparent by default. Set a 2×2 opaque blob at
        // (fx, fy) so the first-opaque scan lands deterministically.
        for dy in 0..2 {
            for dx in 0..2 {
                if fx + dx < w && fy + dy < h {
                    img.put_pixel(fx + dx, fy + dy, Rgba([255, 0, 0, 255]));
                }
            }
        }
        img
    }

    #[test]
    fn compute_pdf_offset_returns_diff_between_actual_and_expected() {
        // Render produced first opaque pixel at (50, 100). We expected
        // it at (10, 5). Offset = (expected - actual) = (-40, -95).
        let img = pixmap_with_first_opaque_at(200, 200, 50, 100);
        let offset = compute_pdf_offset(&img, 10, 5);
        assert_eq!(offset, (-40, -95));
    }

    #[test]
    fn compute_pdf_offset_snaps_sub_pixel_drift_to_zero() {
        // Within ±1 px of expected → snap to (0, 0). Avoids needless
        // re-composes for trivial rounding noise.
        let img = pixmap_with_first_opaque_at(50, 50, 10, 10);
        let offset = compute_pdf_offset(&img, 11, 9);
        assert_eq!(offset, (0, 0));
    }

    #[test]
    fn compute_pdf_offset_empty_image_returns_zero() {
        // No opaque pixels at all → no detection possible → (0, 0).
        let img = RgbaImage::new(50, 50);
        let offset = compute_pdf_offset(&img, 10, 10);
        assert_eq!(offset, (0, 0));
    }

    // ── compose_ocg_canvas: dimension contract ─────────────────────────

    #[test]
    fn compose_ocg_canvas_returns_canvas_sized_image() {
        // The composer takes a full-page MuPDF pixmap and pastes it
        // onto a fresh `canvas_width × canvas_height` RGBA. Whatever
        // the input pixmap size, output dims must equal canvas dims
        // — downstream consumers (composite save, hybrid render)
        // index into the canvas and would corrupt memory if these
        // drifted.
        let pixmap = RgbaImage::new(800, 1200);
        let out = compose_ocg_canvas(
            &pixmap,
            "bricks",
            30.0,
            (10.0, 20.0, 100.0, 200.0),
            120,
            240,
            (0, 0),
        );
        assert_eq!(out.width(), 120);
        assert_eq!(out.height(), 240);
    }
}
