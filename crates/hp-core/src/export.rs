//! Unity export — generates house_data.json + piece sprite PNGs in a ZIP.

use anyhow::{Context, Result};
use serde_json::json;
use std::collections::HashMap;
use std::io::Write;
use std::path::Path;

use crate::types::{Brick, PuzzlePiece};

/// Convert pixel coords to Unity world position.
/// Unity: Y-up, origin at bottom. Canvas: Y-down, origin at top.
fn pixel_to_unity(px_x: f64, px_y: f64, canvas_h: f64, ppu: f64) -> (f64, f64) {
    let x = px_x / ppu;
    let y = (canvas_h - px_y) / ppu;
    (x, y)
}

/// Build the house_data.json structure for Unity import.
pub fn build_house_data(
    pieces: &[PuzzlePiece],
    _bricks_by_id: &HashMap<String, Brick>,
    canvas_width: i32,
    canvas_height: i32,
    waves: &[serde_json::Value],
    ppu: f64,
    scale: f64,
    location: &str,
    position_in_location: i32,
    house_name: &str,
    spacing: f64,
    groups: &[serde_json::Value],
) -> serde_json::Value {
    let scaled_w = (canvas_width as f64 * scale).round();
    let scaled_h = (canvas_height as f64 * scale).round();
    let canvas_center_x = scaled_w / 2.0 / ppu;

    // Blocks
    let mut blocks = Vec::new();
    let piece_id_to_idx: HashMap<&str, usize> = pieces.iter().enumerate()
        .map(|(i, p)| (p.id.as_str(), i))
        .collect();

    for piece in pieces {
        let name = format!("piece_{}", piece.id);
        let center_px_x = (piece.x as f64 + piece.width as f64 / 2.0) * scale;
        let center_px_y = (piece.y as f64 + piece.height as f64 / 2.0) * scale;
        let (ux, uy) = pixel_to_unity(center_px_x, center_px_y, scaled_h, ppu);

        blocks.push(json!({
            "name": name,
            "position": {
                "x": ((ux - canvas_center_x) * 1e6).round() / 1e6,
                "y": (uy * 1e6).round() / 1e6,
                "z": 0.0,
            },
            "orderInLayer": 0,
            "isChimney": false,
        }));
    }

    // Steps from waves
    let _all_piece_ids: std::collections::HashSet<&str> = pieces.iter().map(|p| p.id.as_str()).collect();
    let mut assigned_ids: std::collections::HashSet<String> = std::collections::HashSet::new();
    let mut steps = Vec::new();

    for w in waves {
        let wave_piece_ids: Vec<String> = w.get("pieceIds")
            .and_then(|v| v.as_array())
            .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
            .unwrap_or_default();
        assigned_ids.extend(wave_piece_ids.iter().cloned());
        let block_indices: Vec<usize> = wave_piece_ids.iter()
            .filter_map(|pid| piece_id_to_idx.get(pid.as_str()).copied())
            .collect();
        steps.push(json!({
            "wave": w.get("wave").and_then(|v| v.as_i64()).unwrap_or(steps.len() as i64 + 1),
            "blockIndices": block_indices,
        }));
    }

    // Unassigned pieces go into a final step
    let unassigned: Vec<usize> = pieces.iter()
        .filter(|p| !assigned_ids.contains(&p.id))
        .filter_map(|p| piece_id_to_idx.get(p.id.as_str()).copied())
        .collect();
    if !unassigned.is_empty() {
        steps.push(json!({
            "wave": steps.len() + 1,
            "blockIndices": unassigned,
        }));
    }

    // SameBlocksSettings from groups
    let mut piece_to_group: HashMap<String, Vec<String>> = HashMap::new();
    for g in groups {
        if let Some(ids) = g.get("pieceIds").and_then(|v| v.as_array()) {
            let pids: Vec<String> = ids.iter().filter_map(|v| v.as_str().map(String::from)).collect();
            for pid in &pids {
                piece_to_group.insert(pid.clone(), pids.clone());
            }
        }
    }

    let same_blocks_settings: Vec<Vec<usize>> = pieces.iter().enumerate().map(|(i, piece)| {
        if let Some(group_pids) = piece_to_group.get(&piece.id) {
            group_pids.iter()
                .filter_map(|pid| piece_id_to_idx.get(pid.as_str()).copied())
                .collect()
        } else {
            vec![i]
        }
    }).collect();

    let scaling_factor = 2.0;
    let ground_offset = json!({"x": 0.0, "y": 0.0, "z": 0.0});

    json!({
        "Name": house_name,
        "Location": location,
        "PositionInLocation": position_in_location,
        "Spacing": spacing,
        "ScalingFactor": scaling_factor,
        "GroundOffset": ground_offset,
        "Blocks": blocks,
        "Steps": steps,
        "SameBlocksSettings": same_blocks_settings,
    })
}

/// Generate export ZIP with piece sprites + house_data.json.
///
/// The caller is expected to have ALREADY rendered piece PNGs at
/// `export_dpi` under `extract_dir`. We don't rescale here — bytes
/// from disk go straight into the ZIP. `loaded_dpi` + `export_dpi`
/// are still needed for two things:
///   - deriving `target_ppu` so house_data.json's coordinates match
///     the sprite resolution Unity will import;
///   - the `scale` factor build_house_data uses to convert
///     loaded-canvas piece coordinates to output-canvas Unity coords
///     (it's a pure number, no images involved).
pub fn generate_export_zip(
    pieces: &[PuzzlePiece],
    bricks_by_id: &HashMap<String, Brick>,
    extract_dir: &Path,
    canvas_width: i32,
    canvas_height: i32,
    screen_frame_height_px: f64,
    loaded_dpi: f64,
    export_dpi: f64,
    waves: &[serde_json::Value],
    groups: &[serde_json::Value],
    location: &str,
    position: i32,
    house_name: &str,
    spacing: f64,
) -> Result<Vec<u8>> {
    // target_ppu = export_dpi × screen_frame_h_pts / (72 × HOUSE_UNITS_HIGH).
    // We have screen_frame_height_px (loaded pixels), convert to PDF
    // points via loaded_dpi: h_pts = h_px × 72 / loaded_dpi.
    let (target_ppu, scale) = if screen_frame_height_px > 0.0 && loaded_dpi > 0.0 {
        let screen_frame_h_pts = screen_frame_height_px * 72.0 / loaded_dpi;
        let target_ppu = export_dpi * screen_frame_h_pts / (72.0 * 15.5);
        let scale = export_dpi / loaded_dpi;
        (target_ppu, scale)
    } else {
        // Degenerate AI (no `screen` layer) — fall back to the legacy
        // 50 PPU default. Better than divide-by-zero.
        let target_ppu = 50.0;
        let scale = target_ppu * 15.5 / canvas_height as f64;
        (target_ppu, scale)
    };

    let house_data = build_house_data(
        pieces, bricks_by_id, canvas_width, canvas_height,
        waves, target_ppu, scale, location, position, house_name, spacing, groups,
    );

    let buf = Vec::new();
    let cursor = std::io::Cursor::new(buf);
    let mut zip = zip::ZipWriter::new(cursor);
    let options = zip::write::FileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    // Write house_data.json
    zip.start_file("house_data.json", options).context("starting house_data.json in ZIP")?;
    let json_bytes = serde_json::to_string_pretty(&house_data).context("serialising house_data to JSON")?;
    zip.write_all(json_bytes.as_bytes()).context("writing house_data.json bytes")?;

    // Helper — copy a file from `extract_dir` straight into the ZIP
    // at the given archive path. By contract the caller has already
    // produced everything at `export_dpi` (via
    // `render_export_pieces`), so we don't rescale here.
    let mut put_file = |zip_path: &str, src: &Path| -> Result<()> {
        if !src.exists() {
            return Ok(());
        }
        let bytes = std::fs::read(src).with_context(|| format!("reading {}", src.display()))?;
        zip.start_file(zip_path, options)
            .with_context(|| format!("starting {zip_path} in ZIP"))?;
        zip.write_all(&bytes)
            .with_context(|| format!("writing {zip_path} bytes"))?;
        Ok(())
    };

    // Full-house composite + blueprint background + composite vector
    // outlines sit at the archive root next to house_data.json. All
    // three come from `render_export_pieces`.
    put_file("composite.png", &extract_dir.join("composite.png"))?;
    put_file("background.png", &extract_dir.join("background.png"))?;
    put_file("outlines.png", &extract_dir.join("outlines.png"))?;
    // Soft white house-silhouette glow. Optional — only present when
    // the AI declared a `background` OCG layer (same as
    // `background.png`). The sidecar JSON carries its placement
    // offset (padded to let the blur halo bleed past the canvas).
    let bg_highlight = extract_dir.join("background_highlight.png");
    if bg_highlight.exists() {
        put_file("background_highlight.png", &bg_highlight)?;
        let bg_highlight_meta = extract_dir.join("background_highlight.json");
        if bg_highlight_meta.exists() {
            put_file("background_highlight.json", &bg_highlight_meta)?;
        }
    }
    // Lights overlay (warm window-pane glow). Optional.
    let lights = extract_dir.join("lights.png");
    if lights.exists() {
        put_file("lights.png", &lights)?;
    }
    // Compact placement metadata for every canvas-aligned asset.
    let assets_json = extract_dir.join("assets.json");
    if assets_json.exists() {
        put_file("assets.json", &assets_json)?;
    }

    // Per-piece sprite only — outlines are bundled into a single
    // `outlines.png` overlay above instead of per-piece files.
    for piece in pieces {
        put_file(
            &format!("pieces/piece_{}.png", piece.id),
            &extract_dir.join(format!("piece_{}.png", piece.id)),
        )?;
    }

    drop(put_file);
    let cursor = zip.finish().context("finalising ZIP archive")?;
    Ok(cursor.into_inner())
}

/// Generate an Adobe Photoshop file from the export-DPI assets that
/// `render_export_pieces` has already written to `extract_dir`.
///
/// Layer order (bottom-up — PSD convention):
///   1. background.png  — blueprint backdrop (when present)
///   2. composite.png   — full house bricks
///   3. piece_<id>.png  — one layer per piece, at piece bounds
///   4. outlines.png    — vector-traced outline overlay on top
///
/// The merged preview baked into the file is `composite.png` — that's
/// what tools without layer support (and OS file pickers) render.
pub fn generate_export_psd(
    pieces: &[PuzzlePiece],
    extract_dir: &Path,
    canvas_width: i32,
    canvas_height: i32,
) -> Result<Vec<u8>> {
    use crate::psd::{write_psd, PsdLayer};

    let cw = canvas_width.max(1) as u32;
    let ch = canvas_height.max(1) as u32;

    // Load a canvas-sized RGBA layer from `extract_dir/<name>` if it
    // exists. Returns `None` so missing optional assets (e.g. no
    // background layer in the AI) are silently skipped.
    let load_canvas_layer = |file: &str, layer_name: &str| -> Result<Option<PsdLayer>> {
        let path = extract_dir.join(file);
        if !path.exists() {
            return Ok(None);
        }
        let img = image::open(&path)
            .with_context(|| format!("opening {}", path.display()))?
            .to_rgba8();
        if img.width() != cw || img.height() != ch {
            // Better to fail loud than ship a misaligned PSD.
            anyhow::bail!(
                "{} is {}×{} but canvas is {}×{} — render_export_pieces should have produced canvas-sized assets",
                file, img.width(), img.height(), cw, ch
            );
        }
        Ok(Some(PsdLayer {
            name: layer_name.to_string(),
            rect: (0, 0, ch, cw),
            image: img,
        }))
    };

    let mut layers: Vec<PsdLayer> = Vec::new();

    if let Some(l) = load_canvas_layer("background.png", "background")? {
        layers.push(l);
    }
    let composite = load_canvas_layer("composite.png", "composite")?
        .ok_or_else(|| anyhow::anyhow!("composite.png missing under {}", extract_dir.display()))?;
    let merged_preview = composite.image.clone();
    layers.push(composite);

    // One layer per piece, named "piece_<id>", placed at the piece's
    // canvas-coord rect (already at export DPI thanks to
    // render_export_pieces scaling).
    for piece in pieces {
        let path = extract_dir.join(format!("piece_{}.png", piece.id));
        if !path.exists() {
            continue;
        }
        let img = image::open(&path)
            .with_context(|| format!("opening {}", path.display()))?
            .to_rgba8();
        let top = piece.y.max(0) as u32;
        let left = piece.x.max(0) as u32;
        // Clamp the rect to the canvas so an off-by-one in piece
        // dimensions doesn't trip the bounds check in `write_psd`.
        let bottom = (top + img.height()).min(ch);
        let right = (left + img.width()).min(cw);
        if bottom <= top || right <= left {
            continue;
        }
        // If the rect was clamped, crop the image to match.
        let final_img = if right - left == img.width() && bottom - top == img.height() {
            img
        } else {
            image::imageops::crop_imm(&img, 0, 0, right - left, bottom - top).to_image()
        };
        layers.push(PsdLayer {
            name: format!("piece_{}", piece.id),
            rect: (top, left, bottom, right),
            image: final_img,
        });
    }

    if let Some(l) = load_canvas_layer("outlines.png", "outlines")? {
        layers.push(l);
    }

    let mut buf = Vec::new();
    write_psd(&mut buf, cw, ch, &layers, &merged_preview)?;
    Ok(buf)
}
