//! Image rendering pipeline — brick PNG extraction and compositing.

use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use std::path::Path;

use crate::ai_parser::BrickPlacement;

/// Extract a raster brick image from a block's raw byte range.
///
/// Parses the Xh matrix for image dimensions, reads raw RGB bytes,
/// converts white pixels to transparent. Returns native-resolution RGBA.
pub fn extract_raster_image(block_data: &[u8]) -> Option<RgbaImage> {
    let xh_re = regex::bytes::Regex::new(
        r"\[\s*-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s+-?\d+(?:\.\d+)?\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh"
    ).unwrap();
    let caps = xh_re.captures(block_data)?;
    let img_w: usize = std::str::from_utf8(&caps[1]).ok()?.parse().ok()?;
    let img_h: usize = std::str::from_utf8(&caps[2]).ok()?.parse().ok()?;
    if img_w == 0 || img_h == 0 {
        return None;
    }

    let xi_re = regex::bytes::Regex::new(r"%%BeginData:\s*\d+[^\n]*XI\n").unwrap();
    let xi_m = xi_re.find(block_data)?;
    let data_start = xi_m.end();
    let expected = img_w * img_h * 3;

    if data_start + expected > block_data.len() {
        return None;
    }

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

/// Render all brick PNGs to disk (full-canvas RGBA, brick at its position).
/// Parallelized with rayon for raster bricks.
/// Vector bricks are rendered via `bricks_layer_img` (MuPDF OCG render of bricks layer).
pub fn render_brick_pngs(
    raw: &[u8],
    bricks: &[(String, BrickPlacement)],
    canvas_width: u32,
    canvas_height: u32,
    out_dir: &Path,
    bricks_layer_img: Option<&RgbaImage>,
) {
    std::fs::create_dir_all(out_dir).ok();

    bricks.par_iter().for_each(|(id, bp)| {
        let out_path = out_dir.join(format!("brick_{id}.png"));
        let mut canvas = RgbaImage::new(canvas_width, canvas_height);

        if bp.layer_type == "brick" || bp.layer_type == "mixed_brick" {
            let block_data = &raw[bp.block_begin..bp.block_end];
            if let Some(raster) = extract_raster_image(block_data) {
                let w = bp.width.max(1) as u32;
                let h = bp.height.max(1) as u32;
                let resized = image::imageops::resize(&raster, w, h, image::imageops::Lanczos3);
                image::imageops::overlay(&mut canvas, &resized, bp.x as i64, bp.y as i64);
            }
        } else if bp.layer_type == "vector_brick" {
            // Vector brick: crop from the full bricks-layer MuPDF render
            if let Some(layer_img) = bricks_layer_img {
                // Copy the brick's region from the layer render
                for dy in 0..bp.height.max(0) {
                    for dx in 0..bp.width.max(0) {
                        let sx = (bp.x + dx) as u32;
                        let sy = (bp.y + dy) as u32;
                        if sx < layer_img.width() && sy < layer_img.height() {
                            let px = layer_img.get_pixel(sx, sy);
                            if px[3] > 0 {
                                canvas.put_pixel(sx, sy, *px);
                            }
                        }
                    }
                }
            }
        }

        canvas.save(&out_path).ok();
    });
}

/// Render the composite PNG (all bricks composited onto one canvas).
pub fn render_composite_png(
    raw: &[u8],
    bricks: &[(String, BrickPlacement)],
    canvas_width: u32,
    canvas_height: u32,
    out_path: &Path,
    bricks_layer_img: Option<&RgbaImage>,
) {
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);

    for (_, bp) in bricks {
        if bp.layer_type == "brick" || bp.layer_type == "mixed_brick" {
            let block_data = &raw[bp.block_begin..bp.block_end];
            if let Some(raster) = extract_raster_image(block_data) {
                let w = bp.width.max(1) as u32;
                let h = bp.height.max(1) as u32;
                let resized = image::imageops::resize(&raster, w, h, image::imageops::Lanczos3);
                image::imageops::overlay(&mut canvas, &resized, bp.x as i64, bp.y as i64);
            }
        } else if bp.layer_type == "vector_brick" {
            if let Some(layer_img) = bricks_layer_img {
                for dy in 0..bp.height.max(0) {
                    for dx in 0..bp.width.max(0) {
                        let sx = (bp.x + dx) as u32;
                        let sy = (bp.y + dy) as u32;
                        if sx < layer_img.width() && sy < layer_img.height() {
                            let px = layer_img.get_pixel(sx, sy);
                            if px[3] > 0 {
                                canvas.put_pixel(sx, sy, *px);
                            }
                        }
                    }
                }
            }
        }
    }

    canvas.save(out_path).ok();
}

/// Render piece PNGs by compositing brick PNGs from disk.
/// Each piece PNG is cropped to the piece's bounding box.
/// Parallelized with rayon.
pub fn render_piece_pngs(
    pieces: &[crate::types::PuzzlePiece],
    bricks_by_id: &std::collections::HashMap<String, crate::types::Brick>,
    extract_dir: &Path,
) {
    pieces.par_iter().for_each(|piece| {
        let pw = piece.width.max(1) as u32;
        let ph = piece.height.max(1) as u32;
        let mut piece_img = RgbaImage::new(pw, ph);

        for bid in &piece.brick_ids {
            let brick_path = extract_dir.join(format!("brick_{bid}.png"));
            if !brick_path.exists() {
                continue;
            }
            let brick_img = match image::open(&brick_path) {
                Ok(img) => img.to_rgba8(),
                Err(_) => continue,
            };
            // Brick PNG is full-canvas — crop to piece bbox
            image::imageops::overlay(
                &mut piece_img,
                &brick_img,
                -(piece.x as i64),
                -(piece.y as i64),
            );
        }

        let out_path = extract_dir.join(format!("piece_{}.png", piece.id));
        piece_img.save(&out_path).ok();

        // Also render outline: 2px white stroke of non-transparent edges
        render_piece_outline(&piece_img, &extract_dir.join(format!("piece_outline_{}.png", piece.id)));
    });
}

/// Render a piece outline PNG: white border of opaque pixels.
fn render_piece_outline(piece_img: &RgbaImage, out_path: &Path) {
    let w = piece_img.width();
    let h = piece_img.height();
    let mut outline = RgbaImage::new(w, h);

    for y in 0..h {
        for x in 0..w {
            let px = piece_img.get_pixel(x, y);
            if px[3] < 30 {
                continue;
            }
            // Check if any neighbor is transparent → this is a border pixel
            let is_border = [(0i32, -1), (0, 1), (-1, 0), (1, 0)]
                .iter()
                .any(|&(dx, dy)| {
                    let nx = x as i32 + dx;
                    let ny = y as i32 + dy;
                    if nx < 0 || ny < 0 || nx >= w as i32 || ny >= h as i32 {
                        return true;
                    }
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
/// Uses the vector polygons from parsing (brick-local coords → canvas coords).
pub fn render_outlines_png(
    bricks: &[(String, crate::ai_parser::BrickPlacement)],
    canvas_width: u32,
    canvas_height: u32,
    out_path: &Path,
) {
    // 2x supersampling for smoother strokes
    let ss = 2u32;
    let sw = canvas_width * ss;
    let sh = canvas_height * ss;
    let mut canvas = RgbaImage::new(sw, sh);

    for (_, bp) in bricks {
        let poly = match &bp.polygon {
            Some(p) if p.len() >= 3 => p,
            _ => continue,
        };

        // Convert brick-local → canvas coords, scaled by supersampling factor
        let points: Vec<(f64, f64)> = poly.iter()
            .map(|pt| (
                (pt[0] + bp.x as f64) * ss as f64,
                (pt[1] + bp.y as f64) * ss as f64,
            ))
            .collect();

        // Draw polygon edges as white lines (Bresenham)
        let white = Rgba([255, 255, 255, 255]);
        for i in 0..points.len() {
            let (x0, y0) = points[i];
            let (x1, y1) = points[(i + 1) % points.len()];
            draw_line(&mut canvas, x0 as i32, y0 as i32, x1 as i32, y1 as i32, white, sw, sh);
        }
    }

    // Downscale 2x → 1x
    let result = image::imageops::resize(&canvas, canvas_width, canvas_height, image::imageops::Lanczos3);
    result.save(out_path).ok();
}

/// Bresenham line drawing.
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
            // Thicken: draw adjacent pixels for ~2px stroke at 2x
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

    let doc = mupdf::pdf::PdfDocument::open(ai_path.to_str()?).ok()?;

    let layer_count = mupdf_ffi::count_layer_ui(&doc);
    let mut found = false;
    for i in 0..layer_count {
        mupdf_ffi::deselect_layer_ui(&doc, i);
    }
    for i in 0..layer_count {
        let info = mupdf_ffi::layer_ui_info(&doc, i);
        if info.text == layer_name {
            mupdf_ffi::select_layer_ui(&doc, i);
            found = true;
        }
    }
    if !found {
        return None;
    }

    let scale = dpi as f32 / 72.0;
    let matrix = mupdf::Matrix::new_scale(scale, scale);
    let page = doc.load_page(0).ok()?;
    let cs = mupdf::Colorspace::device_rgb();
    let pixmap = page.to_pixmap(&matrix, &cs, true, false).ok()?;

    let pw = pixmap.width() as u32;
    let ph = pixmap.height() as u32;
    let n = pixmap.n() as u32;
    let samples = pixmap.samples();

    let mut full_img = RgbaImage::new(pw, ph);
    for y in 0..ph {
        for x in 0..pw {
            let idx = ((y * pw + x) * n) as usize;
            if idx + 3 < samples.len() {
                let r = samples[idx];
                let g = samples[idx + 1];
                let b = samples[idx + 2];
                let a = if n >= 4 { samples[idx + 3] } else { 255 };
                full_img.put_pixel(x, y, Rgba([r, g, b, a]));
            }
        }
    }

    // Crop to clip rect
    let cx = (clip_rect.0 * scale as f64).round() as i64;
    let cy = (clip_rect.1 * scale as f64).round() as i64;
    let dx = pdf_offset_px.0 as i64;
    let dy = pdf_offset_px.1 as i64;

    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(&mut canvas, &full_img, -(cx - dx), -(cy - dy));
    Some(canvas)
}

/// Render a specific OCG layer (e.g., "lights", "background") to a PNG.
///
/// Opens the AI file, toggles OCG layers (disable all, enable target),
/// renders the page to a pixmap, crops to the clip rect, and saves.
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
    use crate::mupdf_ffi;

    let doc = match mupdf::pdf::PdfDocument::open(ai_path.to_str().unwrap_or("")) {
        Ok(d) => d,
        Err(_) => return false,
    };

    // Find and toggle OCG layers
    let layer_count = mupdf_ffi::count_layer_ui(&doc);
    let mut target_idx: Option<i32> = None;

    // Disable all layers first
    for i in 0..layer_count {
        mupdf_ffi::deselect_layer_ui(&doc, i);
    }

    // Enable only the target layer
    for i in 0..layer_count {
        let info = mupdf_ffi::layer_ui_info(&doc, i);
        if info.text == layer_name {
            mupdf_ffi::select_layer_ui(&doc, i);
            target_idx = Some(i);
        }
    }

    if target_idx.is_none() {
        return false;
    }

    // Render page at DPI scale
    let scale = dpi as f32 / 72.0;
    let matrix = mupdf::Matrix::new_scale(scale, scale);
    let page = match doc.load_page(0) {
        Ok(p) => p,
        Err(_) => return false,
    };
    let cs = mupdf::Colorspace::device_rgb();
    let pixmap = match page.to_pixmap(&matrix, &cs, true, false) {
        Ok(p) => p,
        Err(_) => return false,
    };

    // Convert pixmap to RgbaImage
    let pw = pixmap.width() as u32;
    let ph = pixmap.height() as u32;
    let n = pixmap.n() as u32;
    let samples = pixmap.samples();

    let mut full_img = RgbaImage::new(pw, ph);
    for y in 0..ph {
        for x in 0..pw {
            let idx = (y * pw + x) * n;
            let idx = idx as usize;
            if idx + 3 < samples.len() {
                let r = samples[idx];
                let g = samples[idx + 1];
                let b = samples[idx + 2];
                let a = if n >= 4 { samples[idx + 3] } else { 255 };
                full_img.put_pixel(x, y, Rgba([r, g, b, a]));
            }
        }
    }

    // Crop to clip rect (in scaled pixel coords)
    let cx = (clip_rect.0 * scale as f64).round() as i64;
    let cy = (clip_rect.1 * scale as f64).round() as i64;

    // Apply PDF offset
    let dx = pdf_offset_px.0 as i64;
    let dy = pdf_offset_px.1 as i64;

    // Create canvas-sized output with the layer cropped and shifted
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);
    image::imageops::overlay(&mut canvas, &full_img, -(cx - dx), -(cy - dy));

    canvas.save(out_path).ok();
    true
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
