//! Full house export — runs the same pipeline the in-app export uses,
//! but driven from the CLI so we can A/B against existing exports.
//!
//!   cargo run --release -p hp-core --example export_house -- \
//!       in/_NY5.ai /tmp/ny5_export [target_pieces=60] [export_dpi=300] \
//!       [location=Rome] [house_name=NewHouse]
//!
//! Writes the unpacked export bundle (composite.png, background.png,
//! outlines.png, pieces/piece_*.png, house_data.json) into `out_dir`
//! AND a zipped `out_dir/export.zip` mirroring what the Tauri command
//! produces. Piece PNGs come from `render_piece_pngs_via_ocg_isolation`
//! — i.e. each piece is rendered with ONLY its own bricks' OCGs on,
//! at the same DPI / clip as the composite house layer.

use std::collections::HashMap;
use std::path::Path;

use hp_core::ai_parser::{self, parse_ai};
use hp_core::puzzle;
use hp_core::types::Brick;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = args.next().ok_or_else(|| anyhow::anyhow!("missing ai_path"))?;
    let out_dir = args.next().ok_or_else(|| anyhow::anyhow!("missing out_dir"))?;
    let target_pieces: usize = args.next().map(|s| s.parse().unwrap_or(60)).unwrap_or(60);
    let export_dpi: f64 = args.next().map(|s| s.parse().unwrap_or(300.0)).unwrap_or(300.0);
    let location = args.next().unwrap_or_else(|| "Rome".to_string());
    let house_name = args.next().unwrap_or_else(|| "NewHouse".to_string());

    let ai_path = Path::new(&ai_path);
    let out_dir = Path::new(&out_dir);
    std::fs::create_dir_all(out_dir)?;

    eprintln!("[export_house] parsing {} ...", ai_path.display());
    let (placements, meta, ai_data) = parse_ai(ai_path, hp_core::CANVAS_HEIGHT_PX as i32)?;
    eprintln!(
        "[export_house] {} bricks, canvas {}x{}, render_dpi {:.2}, screen_h_px={:.2}",
        placements.len(),
        meta.canvas_width,
        meta.canvas_height,
        meta.render_dpi,
        meta.screen_frame_height_px,
    );

    // Build bricks using AI layer names as IDs — same convention as
    // serve_pieces; matches the OCG-isolation lookup key.
    let mut bricks: Vec<Brick> = Vec::new();
    let mut brick_polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut brick_beziers: HashMap<String, Vec<hp_core::bezier::BezierPath>> = HashMap::new();
    for p in placements.iter() {
        let id = p.name.clone();
        bricks.push(Brick {
            id: id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            brick_type: p.layer_type.clone(),
        });
        if let Some(poly) = &p.polygon {
            brick_polygons.insert(id.clone(), poly.clone());
        }
        let block = ai_parser::LayerBlock {
            name: p.name.clone(),
            begin: p.block_begin,
            end: p.block_end,
            depth: 0,
            children: Vec::new(),
        };
        let beziers = ai_parser::extract_vector_path_bezier(
            &block,
            &ai_data.raw,
            meta.offset_x,
            meta.y_base,
        );
        brick_beziers.insert(id, beziers);
    }

    let adjacency = puzzle::build_adjacency_vector(&bricks, &brick_polygons, 15.0, 5.0, 2.0);
    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);

    eprintln!("[export_house] merging into ~{} pieces (seed=0) ...", target_pieces);
    let pieces = puzzle::merge_bricks(&bricks, target_pieces, 0, &adjacency, &brick_areas);
    eprintln!("[export_house] produced {} pieces", pieces.len());

    let bricks_by_id: HashMap<String, Brick> =
        bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();
    // Identity map: brick IDs are AI layer names here.
    let brick_layer_names: HashMap<String, String> =
        bricks.iter().map(|b| (b.id.clone(), b.id.clone())).collect();

    // Run the export render pipeline (OCG-isolated piece PNGs +
    // composite/background/outlines). pdf_offset is no longer detected
    // up-front — render_export_pieces derives a sub-pixel-precise
    // bleed from `build_modified_pdf` internally, so the old
    // compose-render-and-probe step (which was integer-pixel only) is
    // dead.
    eprintln!("[export_house] running render_export_pieces at {} DPI -> {} ...", export_dpi, out_dir.display());
    let trimmed_pieces = hp_core::render::render_export_pieces(
        ai_path,
        &placements,
        &meta,
        &pieces,
        &bricks_by_id,
        &brick_polygons,
        &brick_beziers,
        &brick_layer_names,
        export_dpi,
        out_dir,
    )?;
    eprintln!("[export_house] {} piece PNGs written", trimmed_pieces.len());

    // Sidecar JSON for the visual inspector — maps each piece id to
    // its canvas rect (export DPI) and the AI layer names of its
    // bricks. Consumed by /tmp/compare_view/export_diff.html.
    {
        let dump: Vec<_> = trimmed_pieces.iter().map(|p| {
            let layer_names: Vec<&String> = p.brick_ids.iter()
                .filter_map(|bid| brick_layer_names.get(bid)).collect();
            serde_json::json!({
                "id": p.id,
                "x": p.x, "y": p.y, "w": p.width, "h": p.height,
                "layers": layer_names,
                "brick_count": p.brick_ids.len(),
            })
        }).collect();
        let _ = std::fs::write(
            Path::new(&out_dir).join("_debug_pieces.json"),
            serde_json::to_string_pretty(&dump).unwrap(),
        );
    }

    // Down-scale trimmed pieces to loaded-DPI for generate_export_zip
    // (its contract — it re-applies export/loaded scale internally).
    let inv_scale = meta.render_dpi / export_dpi;
    let trimmed_loaded: Vec<hp_core::types::PuzzlePiece> = trimmed_pieces
        .iter()
        .map(|p| hp_core::types::PuzzlePiece {
            id: p.id.clone(),
            brick_ids: p.brick_ids.clone(),
            x: ((p.x as f64) * inv_scale).round() as i32,
            y: ((p.y as f64) * inv_scale).round() as i32,
            width: ((p.width as f64) * inv_scale).round().max(1.0) as i32,
            height: ((p.height as f64) * inv_scale).round().max(1.0) as i32,
        })
        .collect();

    // Match Rome_011's groups/waves: all pieces in wave 1, each piece in its own group.
    let waves = vec![serde_json::json!({
        "wave": 1,
        "pieceIds": trimmed_loaded.iter().map(|p| p.id.clone()).collect::<Vec<_>>(),
    })];
    let groups: Vec<serde_json::Value> = trimmed_loaded.iter().map(|p| {
        serde_json::json!({"pieceIds": [p.id.clone()]})
    }).collect();

    let zip_bytes = hp_core::export::generate_export_zip(
        &trimmed_loaded,
        &bricks_by_id,
        out_dir,
        meta.canvas_width,
        meta.canvas_height,
        meta.screen_frame_height_px,
        meta.render_dpi,
        export_dpi,
        &waves,
        &groups,
        &location,
        0,
        &house_name,
        12.0,
    )?;
    let zip_path = out_dir.join("export.zip");
    std::fs::write(&zip_path, &zip_bytes)?;
    eprintln!("[export_house] wrote {} ({} bytes)", zip_path.display(), zip_bytes.len());

    // Also dump house_data.json next to the bundle for quick inspection.
    let house_data_path = out_dir.join("house_data.json");
    // Decompress one file from the ZIP and write to disk for easy reading.
    let cursor = std::io::Cursor::new(&zip_bytes);
    let mut archive = zip::ZipArchive::new(cursor)?;
    {
        let mut f = archive.by_name("house_data.json")?;
        let mut buf = Vec::new();
        std::io::copy(&mut f, &mut buf)?;
        std::fs::write(&house_data_path, &buf)?;
    }
    // Same for piece PNGs into pieces/
    std::fs::create_dir_all(out_dir.join("pieces"))?;
    for i in 0..archive.len() {
        let mut f = archive.by_index(i)?;
        let name = f.name().to_string();
        if name.starts_with("pieces/") {
            let dst = out_dir.join(&name);
            if let Some(parent) = dst.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut buf = Vec::new();
            std::io::copy(&mut f, &mut buf)?;
            std::fs::write(&dst, &buf)?;
        }
    }
    eprintln!("[export_house] export complete -> {}", out_dir.display());
    Ok(())
}
