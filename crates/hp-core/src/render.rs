//! Image rendering pipeline — brick PNG extraction and compositing.

use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use std::collections::HashMap;
use std::path::Path;
use std::sync::Mutex;

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

/// Extract a raster brick image from a block's raw byte range.
pub fn extract_raster_image(block_data: &[u8]) -> Option<RgbaImage> {
    let xh_re = regex::bytes::Regex::new(
        r"\[\s*-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh"
    ).unwrap();
    let caps = xh_re.captures(block_data)?;
    let img_w: usize = std::str::from_utf8(&caps[1]).ok()?.parse().ok()?;
    let img_h: usize = std::str::from_utf8(&caps[2]).ok()?.parse().ok()?;
    if img_w == 0 || img_h == 0 { return None; }

    let xi_re = regex::bytes::Regex::new(r"%%BeginData:\s*\d+[^\n]*XI\n").unwrap();
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

/// Render all brick images from the OCG bricks layer render.
/// For each brick: crop the layer image to the brick's polygon bbox,
/// then mask to the polygon outline. Unified pipeline for all brick types.
pub fn render_brick_images(
    bricks: &[(String, BrickPlacement)],
    canvas_width: u32,
    canvas_height: u32,
    bricks_layer_img: &RgbaImage,
) -> HashMap<String, RgbaImage> {
    let result = Mutex::new(HashMap::new());

    bricks.par_iter().for_each(|(id, bp)| {
        let mut canvas = RgbaImage::new(canvas_width, canvas_height);

        // Copy pixels from OCG render, masked to the polygon shape
        let poly = bp.polygon.as_ref();
        for dy in 0..bp.height.max(0) {
            for dx in 0..bp.width.max(0) {
                let sx = (bp.x + dx) as u32;
                let sy = (bp.y + dy) as u32;
                if sx < bricks_layer_img.width() && sy < bricks_layer_img.height() {
                    let px = bricks_layer_img.get_pixel(sx, sy);
                    if px[3] > 0 {
                        // Check if this pixel is inside the polygon
                        let in_poly = match poly {
                            Some(pts) if pts.len() >= 3 => {
                                point_in_polygon(dx as f64, dy as f64, pts)
                            }
                            _ => true, // no polygon = keep all pixels
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

    result.into_inner().unwrap()
}

/// Save brick images to disk as PNGs (for HTTP serving).
pub fn save_brick_pngs(brick_images: &HashMap<String, RgbaImage>, out_dir: &Path) {
    std::fs::create_dir_all(out_dir).ok();
    brick_images.par_iter().for_each(|(id, img)| {
        img.save(out_dir.join(format!("brick_{id}.png"))).ok();
    });
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

/// Render piece PNGs by reading brick PNGs from disk.
pub fn render_piece_pngs(
    pieces: &[crate::types::PuzzlePiece],
    extract_dir: &Path,
) {
    pieces.par_iter().for_each(|piece| {
        let pw = piece.width.max(1) as u32;
        let ph = piece.height.max(1) as u32;
        let mut piece_img = RgbaImage::new(pw, ph);

        for bid in &piece.brick_ids {
            let brick_path = extract_dir.join(format!("brick_{bid}.png"));
            if let Ok(brick_img) = image::open(&brick_path) {
                image::imageops::overlay(
                    &mut piece_img, &brick_img.to_rgba8(),
                    -(piece.x as i64), -(piece.y as i64),
                );
            }
        }

        piece_img.save(extract_dir.join(format!("piece_{}.png", piece.id))).ok();
        render_piece_outline(&piece_img, &extract_dir.join(format!("piece_outline_{}.png", piece.id)));
    });
}

/// Render piece PNGs directly from the in-memory OCG bricks layer image.
/// Each piece composites its bricks by overlaying the full bricks_layer_img
/// (which already has all bricks rendered at canvas positions).
/// The piece image is piece-sized, offset so piece.x/y maps to (0,0).
pub fn render_piece_pngs_from_layer(
    pieces: &[crate::types::PuzzlePiece],
    bricks_layer_img: &RgbaImage,
    extract_dir: &Path,
) {
    std::fs::create_dir_all(extract_dir).ok();
    pieces.par_iter().for_each(|piece| {
        let pw = piece.width.max(1) as u32;
        let ph = piece.height.max(1) as u32;
        let mut piece_img = RgbaImage::new(pw, ph);

        // Copy the region of bricks_layer_img that corresponds to this piece
        image::imageops::overlay(
            &mut piece_img, bricks_layer_img,
            -(piece.x as i64), -(piece.y as i64),
        );

        piece_img.save(extract_dir.join(format!("piece_{}.png", piece.id))).ok();
        render_piece_outline(&piece_img, &extract_dir.join(format!("piece_outline_{}.png", piece.id)));
    });
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

/// Render brick outlines as white strokes on transparent canvas.
pub fn render_outlines_png(
    bricks: &[(String, BrickPlacement)],
    canvas_width: u32,
    canvas_height: u32,
    out_path: &Path,
) {
    let ss = 2u32;
    let sw = canvas_width * ss;
    let sh = canvas_height * ss;
    let mut canvas = RgbaImage::new(sw, sh);

    for (_, bp) in bricks {
        let poly = match &bp.polygon {
            Some(p) if p.len() >= 3 => p,
            _ => continue,
        };
        let points: Vec<(f64, f64)> = poly.iter()
            .map(|pt| ((pt[0] + bp.x as f64) * ss as f64, (pt[1] + bp.y as f64) * ss as f64))
            .collect();
        let white = Rgba([255, 255, 255, 255]);
        for i in 0..points.len() {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            draw_line(&mut canvas, x0 as i32, y0 as i32, x1 as i32, y1 as i32, white, sw, sh);
        }
    }

    let result = image::imageops::resize(&canvas, canvas_width, canvas_height, image::imageops::Lanczos3);
    result.save(out_path).ok();
}

fn draw_line(img: &mut RgbaImage, x0: i32, y0: i32, x1: i32, y1: i32, color: Rgba<u8>, w: u32, h: u32) {
    let dx = (x1 - x0).abs();
    let dy = -(y1 - y0).abs();
    let sx = if x0 < x1 { 1 } else { -1 };
    let sy = if y0 < y1 { 1 } else { -1 };
    let mut err = dx + dy;
    let mut x = x0;
    let mut y = y0;
    loop {
        if x >= 0 && y >= 0 && (x as u32) < w && (y as u32) < h {
            img.put_pixel(x as u32, y as u32, color);
            for &(ox, oy) in &[(1, 0), (0, 1), (1, 1)] {
                let nx = x + ox;
                let ny = y + oy;
                if nx >= 0 && ny >= 0 && (nx as u32) < w && (ny as u32) < h {
                    img.put_pixel(nx as u32, ny as u32, color);
                }
            }
        }
        if x == x1 && y == y1 { break; }
        let e2 = 2 * err;
        if e2 >= dy { err += dy; x += sx; }
        if e2 <= dx { err += dx; y += sy; }
    }
}

/// Render a specific OCG layer to an RgbaImage (for in-memory use).
/// Uses pure FFI rendering to ensure OCG layer state is respected.
pub fn render_ocg_layer_image(
    ai_path: &Path,
    layer_name: &str,
    dpi: f64,
    clip_rect: (f64, f64, f64, f64),
    canvas_width: u32,
    canvas_height: u32,
    pdf_offset_px: (i32, i32),
) -> Option<RgbaImage> {
    use crate::mupdf_ffi;

    let (rgba, pw, ph) = mupdf_ffi::render_page_with_ocg(
        ai_path.to_str()?, layer_name, dpi,
    )?;

    let full_img = RgbaImage::from_raw(pw, ph, rgba)?;

    let scale = dpi / 72.0;
    let cx = (clip_rect.0 * scale).round() as i64;
    let cy = (clip_rect.1 * scale).round() as i64;
    let dx = pdf_offset_px.0 as i64;
    let dy = pdf_offset_px.1 as i64;

    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(&mut canvas, &full_img, -(cx - dx), -(cy - dy));
    Some(canvas)
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
}
