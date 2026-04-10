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
/// Parallelized with rayon.
pub fn render_brick_pngs(
    raw: &[u8],
    bricks: &[(String, BrickPlacement)], // (id, placement)
    canvas_width: u32,
    canvas_height: u32,
    out_dir: &Path,
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
        }
        // TODO: vector_brick rendering (gradients + compound paths)

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
