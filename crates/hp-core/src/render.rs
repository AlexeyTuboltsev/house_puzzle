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
