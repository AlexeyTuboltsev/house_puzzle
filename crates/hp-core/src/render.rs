//! Image rendering pipeline — brick PNG extraction and compositing.
//!
//! Extracts individual brick images from AI private data and composites them
//! into full-canvas PNGs. Parallelized with rayon for per-brick rendering.

use image::{Rgba, RgbaImage};
use rayon::prelude::*;
use std::path::Path;

use crate::ai_parser::{AiPrivateData, LayerBlock};

/// Extract a raster brick image from AI private data.
///
/// Parses the Xh matrix to find the image dimensions, reads raw RGB bytes
/// from the %%BeginData section, and converts white pixels to transparent.
/// Returns an RGBA image at the native resolution.
pub fn extract_raster_image(
    block: &LayerBlock,
    raw: &[u8],
) -> Option<RgbaImage> {
    let block_data = &raw[block.begin..block.end];

    // Parse Xh matrix to get image dimensions
    let num = r"-?\d+(?:\.\d+)?";
    let pattern = format!(
        r"\[\s*{n}\s+{n}\s+{n}\s+{n}\s+{n}\s+{n}\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh",
        n = num
    );
    let xh_re = regex::bytes::Regex::new(&pattern).unwrap();
    let caps = xh_re.captures(block_data)?;
    let img_w: usize = std::str::from_utf8(&caps[1]).ok()?.parse().ok()?;
    let img_h: usize = std::str::from_utf8(&caps[2]).ok()?.parse().ok()?;
    if img_w == 0 || img_h == 0 {
        return None;
    }

    // Find %%BeginData marker
    let xi_re = regex::bytes::Regex::new(r"%%BeginData:\s*\d+[^\n]*XI\n").unwrap();
    let xi_m = xi_re.find(block_data)?;
    let data_start = xi_m.end();
    let expected = img_w * img_h * 3;

    if data_start + expected > block_data.len() {
        return None;
    }

    let rgb_data = &block_data[data_start..data_start + expected];

    // Build RGBA image with white-to-alpha conversion
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

/// Extract all brick PNGs as full-canvas images and save to disk.
///
/// Each brick is rendered as a canvas-sized RGBA PNG with the brick
/// at its correct position. Raster bricks are extracted from AI data;
/// vector/gradient bricks are stubbed as empty for now.
pub fn extract_brick_pngs(
    ai_data: &AiPrivateData,
    bricks: &[(String, String, i32, i32, i32, i32, &LayerBlock)],
    // (id, layer_type, x, y, w, h, layer_block)
    canvas_width: u32,
    canvas_height: u32,
    out_dir: &Path,
) {
    std::fs::create_dir_all(out_dir).ok();
    let raw = &ai_data.raw;

    bricks.par_iter().for_each(|(id, layer_type, bx, by, bw, bh, block)| {
        let out_path = out_dir.join(format!("brick_{id}.png"));
        let mut canvas = RgbaImage::new(canvas_width, canvas_height);

        if layer_type == "brick" {
            if let Some(raster) = extract_raster_image(block, raw) {
                let w = (*bw).max(1) as u32;
                let h = (*bh).max(1) as u32;
                let resized = image::imageops::resize(&raster, w, h, image::imageops::Lanczos3);
                image::imageops::overlay(&mut canvas, &resized, *bx as i64, *by as i64);
            }
        }
        // TODO: vector_brick and mixed_brick rendering

        canvas.save(&out_path).ok();
    });
}

/// Composite all brick PNGs into a single canvas image.
pub fn compose_bricks_png(
    ai_data: &AiPrivateData,
    bricks: &[(String, String, i32, i32, i32, i32, &LayerBlock)],
    canvas_width: u32,
    canvas_height: u32,
    out_path: &Path,
) {
    let raw = &ai_data.raw;
    let mut canvas = RgbaImage::new(canvas_width, canvas_height);

    for (_, layer_type, bx, by, bw, bh, block) in bricks {
        if *layer_type == "brick" {
            if let Some(raster) = extract_raster_image(block, raw) {
                let w = (*bw).max(1) as u32;
                let h = (*bh).max(1) as u32;
                let resized = image::imageops::resize(&raster, w, h, image::imageops::Lanczos3);
                image::imageops::overlay(&mut canvas, &resized, *bx as i64, *by as i64);
            }
        }
        // TODO: vector_brick and mixed_brick rendering
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

        // Try extracting the first brick child
        let first_child = &bricks_node.children[0];
        let img = extract_raster_image(first_child, &ai_data.raw);

        if let Some(img) = img {
            eprintln!("First brick raster: {}x{}", img.width(), img.height());
            assert!(img.width() > 0 && img.height() > 0);
        } else {
            eprintln!("First brick has no raster (may be vector-only)");
        }

        // Count how many bricks have rasters
        let raster_count = bricks_node.children.iter()
            .filter(|c| extract_raster_image(c, &ai_data.raw).is_some())
            .count();
        eprintln!("Bricks with rasters: {}/{}", raster_count, bricks_node.children.len());
    }
}
