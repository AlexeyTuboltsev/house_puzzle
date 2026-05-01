//! Shared data model + snapshot-generation logic for hp-testbed binaries.

use anyhow::Result;
use hp_core::ai_parser::{self, LayerBlock};
use hp_core::bezier::BezierPath;
use hp_core::puzzle;
use hp_core::types::{Brick, PuzzlePiece};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Transform {
    pub clip_x0: f64,
    pub clip_y0: f64,
    pub scale: f64,
    pub canvas_width: u32,
    pub canvas_height: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrickOut {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub layer_type: String,
    pub beziers: Vec<BezierPath>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Snapshot {
    pub source_file: String,
    pub transform: Transform,
    pub bricks: Vec<BrickOut>,
    pub pieces: Vec<PuzzlePiece>,
}

/// Build the snapshot for an AI file in-process (no file IO). Performs the
/// full parse + bezier adjacency + merge pipeline. Slow on first call
/// because `parse_ai` takes ~90s for a typical NY house.
pub fn build_snapshot(ai_path: &Path, target: usize, seed: u64) -> Result<Snapshot> {
    let canvas_height = hp_core::CANVAS_HEIGHT_PX as i32;
    let (placements, metadata, ai_data) = ai_parser::parse_ai(ai_path, canvas_height)?;
    let (clip_x0, clip_y0, _, _) = metadata.clip_rect;
    let scale = metadata.render_dpi / 72.0;

    // Bezier extraction is the per-brick bottleneck; do it in parallel.
    use rayon::prelude::*;
    let bezier_per_brick: Vec<Vec<BezierPath>> = placements
        .par_iter()
        .map(|p| {
            let block = LayerBlock {
                name: p.name.clone(),
                begin: p.block_begin,
                end: p.block_end,
                depth: 0,
                children: Vec::new(),
            };
            ai_parser::extract_vector_path_bezier(
                &block,
                &ai_data.raw,
                metadata.offset_x,
                metadata.y_base,
            )
        })
        .collect();

    let mut bricks: Vec<Brick> = Vec::with_capacity(placements.len());
    let mut brick_polys_canvas: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut bricks_out: Vec<BrickOut> = Vec::with_capacity(placements.len());

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
        if let Some(poly) = &p.polygon {
            brick_polys_canvas.insert(id.clone(), poly.clone());
        }
        bricks_out.push(BrickOut {
            id,
            name: p.name.clone(),
            layer_type: p.layer_type.clone(),
            beziers: bezier_per_brick[i].clone(),
        });
    }

    let min_border_pymu = 5.0 / scale;
    let brick_beziers_map: HashMap<String, Vec<BezierPath>> =
        bricks_out.iter().map(|b| (b.id.clone(), b.beziers.clone())).collect();
    let adj = puzzle::build_adjacency_bezier(&bricks, &brick_beziers_map, min_border_pymu);
    let areas = puzzle::compute_bezier_areas(&bricks, &brick_beziers_map);
    let pieces: Vec<PuzzlePiece> =
        puzzle::merge_bricks(&bricks, target, seed, &adj, &areas);

    Ok(Snapshot {
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
    })
}

/// Always rebuild the snapshot from the AI file. Disk caching is
/// intentionally disabled here — stale snapshots have shadowed real
/// bugs (e.g. the NY8 'Layer 320' regression: the JSON on disk had
/// been written before the parser validation tightened, so the
/// testbed kept showing a brick the editor was correctly dropping).
/// Always reparse so every request reflects the current parser, at
/// the cost of a few seconds per first hit.
pub fn load_or_build(
    ai_path: &Path,
    _out_dir: &Path,
    target: usize,
    seed: u64,
) -> Result<Snapshot> {
    build_snapshot(ai_path, target, seed)
}

/// List every `.ai` file in `in_dir` sorted by file name.
pub fn scan_ai_dir(in_dir: &Path) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(in_dir)? {
        let entry = entry?;
        let p = entry.path();
        if p.extension().and_then(|s| s.to_str()).unwrap_or("").eq_ignore_ascii_case("ai") {
            out.push(p);
        }
    }
    out.sort();
    Ok(out)
}
