//! Unity export — generates house_data.json + piece sprite PNGs in a ZIP.

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
    bricks_by_id: &HashMap<String, Brick>,
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
    let all_piece_ids: std::collections::HashSet<&str> = pieces.iter().map(|p| p.id.as_str()).collect();
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
pub fn generate_export_zip(
    pieces: &[PuzzlePiece],
    bricks_by_id: &HashMap<String, Brick>,
    extract_dir: &Path,
    canvas_width: i32,
    canvas_height: i32,
    screen_frame_height_px: f64,
    waves: &[serde_json::Value],
    groups: &[serde_json::Value],
    location: &str,
    position: i32,
    house_name: &str,
    spacing: f64,
) -> Result<Vec<u8>, String> {
    let target_ppu = 50.0;
    let scale = if screen_frame_height_px > 0.0 {
        target_ppu * 15.5 / screen_frame_height_px
    } else {
        let target_canvas_h = target_ppu * 15.5;
        target_canvas_h / canvas_height as f64
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
    zip.start_file("house_data.json", options).map_err(|e| e.to_string())?;
    let json_bytes = serde_json::to_string_pretty(&house_data).map_err(|e| e.to_string())?;
    zip.write_all(json_bytes.as_bytes()).map_err(|e| e.to_string())?;

    // Write piece PNGs (read from extract_dir, scale for Unity PPU)
    for piece in pieces {
        let piece_path = extract_dir.join(format!("piece_{}.png", piece.id));
        if !piece_path.exists() {
            continue;
        }
        let img = match image::open(&piece_path) {
            Ok(img) => img.to_rgba8(),
            Err(_) => continue,
        };

        // Scale piece for Unity PPU
        let new_w = ((img.width() as f64 * scale).round() as u32).max(1);
        let new_h = ((img.height() as f64 * scale).round() as u32).max(1);
        let scaled = image::imageops::resize(&img, new_w, new_h, image::imageops::Lanczos3);

        let mut png_buf = Vec::new();
        {
            let mut cursor = std::io::Cursor::new(&mut png_buf);
            scaled.write_to(&mut cursor, image::ImageOutputFormat::Png)
                .map_err(|e| e.to_string())?;
        }

        let fname = format!("pieces/piece_{}.png", piece.id);
        zip.start_file(&fname, options).map_err(|e| e.to_string())?;
        zip.write_all(&png_buf).map_err(|e| e.to_string())?;
    }

    let cursor = zip.finish().map_err(|e| e.to_string())?;
    Ok(cursor.into_inner())
}
