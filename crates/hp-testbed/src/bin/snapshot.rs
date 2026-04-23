//! One-time snapshot: parse the supplied AI file, run the puzzle merge at
//! fixed seed/target, and dump the data the testbed needs.
//!
//! Usage: `hp-snapshot <path-to-ai> [--out ./testbed] [--target 120] [--seed 42]`.

use anyhow::{Context, Result, bail};
use hp_core::ai_parser::{self, LayerBlock};
use hp_core::bezier::BezierPath;
use hp_core::puzzle;
use hp_core::types::{Brick, PuzzlePiece};
use serde::Serialize;
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Serialize)]
struct Transform {
    clip_x0: f64,
    clip_y0: f64,
    scale: f64,
    canvas_width: u32,
    canvas_height: u32,
}

#[derive(Debug, Serialize)]
struct BrickOut {
    id: String,
    name: String,
    layer_type: String,
    /// bezier sub-paths in AI pymu coords (curves preserved)
    beziers: Vec<BezierPath>,
}

#[derive(Debug, Serialize)]
struct Snapshot {
    source_file: String,
    transform: Transform,
    bricks: Vec<BrickOut>,
    pieces: Vec<PuzzlePiece>,
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = PathBuf::from(args.next().context("missing AI file argument")?);
    let mut out_dir = PathBuf::from("testbed");
    let mut target: usize = 120;
    let mut seed: u64 = 42;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => out_dir = PathBuf::from(args.next().context("--out needs a value")?),
            "--target" => target = args.next().context("--target needs value")?.parse()?,
            "--seed" => seed = args.next().context("--seed needs value")?.parse()?,
            other => bail!("unknown arg: {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).ok();

    eprintln!("[snapshot] parsing {:?}", ai_path);
    let canvas_height = 900;
    let (placements, metadata, ai_data) = ai_parser::parse_ai(&ai_path, canvas_height)?;
    eprintln!(
        "[snapshot] {} placements, canvas {}x{}, clip {:?}, dpi {:.2}",
        placements.len(),
        metadata.canvas_width,
        metadata.canvas_height,
        metadata.clip_rect,
        metadata.render_dpi
    );

    // Build Brick list + both flavours of polygons (tessellated for adjacency,
    // bezier for the testbed).
    let (clip_x0, clip_y0, _, _) = metadata.clip_rect;
    let scale = metadata.render_dpi / 72.0;

    let mut bricks: Vec<Brick> = Vec::new();
    let mut brick_polys_canvas: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut bricks_out: Vec<BrickOut> = Vec::new();

    for (i, p) in placements.iter().enumerate() {
        let id = format!("b{i:03}");
        bricks.push(Brick {
            id: id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            brick_type: p.layer_type.clone(),
        });

        // `p.polygon` is already in brick-local canvas pixel coords —
        // exactly what `build_adjacency_vector` expects.
        if let Some(poly) = &p.polygon {
            brick_polys_canvas.insert(id.clone(), poly.clone());
        }

        // Bezier paths in AI pymu coords — re-parse the block bytes preserving cubics.
        let dummy_block = LayerBlock {
            name: p.name.clone(),
            begin: p.block_begin,
            end: p.block_end,
            depth: 0,
            children: Vec::new(),
        };
        let beziers = ai_parser::extract_vector_path_bezier(
            &dummy_block,
            &ai_data.raw,
            metadata.offset_x,
            metadata.y_base,
        );
        bricks_out.push(BrickOut {
            id,
            name: p.name.clone(),
            layer_type: p.layer_type.clone(),
            beziers,
        });
    }

    eprintln!("[snapshot] bricks with bezier data: {}",
        bricks_out.iter().filter(|b| !b.beziers.is_empty()).count());

    // Run the merge to get 120 pieces.
    let adj = puzzle::build_adjacency_vector(&bricks, &brick_polys_canvas, 15.0, 5.0, 2.0);
    let areas = puzzle::compute_polygon_areas(&bricks, &brick_polys_canvas);
    let pieces: Vec<PuzzlePiece> = puzzle::merge_bricks(&bricks, target, seed, &adj, &areas);
    eprintln!("[snapshot] merged into {} pieces", pieces.len());

    let snap = Snapshot {
        source_file: ai_path.to_string_lossy().into_owned(),
        transform: Transform {
            clip_x0,
            clip_y0,
            scale,
            canvas_width: metadata.canvas_width as u32,
            canvas_height: metadata.canvas_height as u32,
        },
        bricks: bricks_out,
        pieces,
    };

    let json_path = out_dir.join("snapshot.json");
    std::fs::write(&json_path, serde_json::to_vec_pretty(&snap)?)?;
    eprintln!("[snapshot] wrote {}", json_path.display());
    Ok(())
}
