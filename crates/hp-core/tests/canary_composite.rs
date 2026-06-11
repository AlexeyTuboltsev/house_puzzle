//! Canary tests for the export pipeline.
//!
//! Each test runs the full export pipeline on a real NY AI file and
//! fingerprints the resulting `composite.png`. The fingerprint is a
//! handful of pixel-derived summaries (dimensions, alpha bbox, alpha
//! sum, per-third alpha sums, RGB checksum) that drift if anything
//! along the pipeline misbehaves — wrong brick set, missing OCG,
//! mis-aligned overlay, broken decode — without storing a large
//! golden PNG in the repo.
//!
//! Tests are skipped if the input AI file isn't present (the `in/`
//! directory is gitignored; CI must populate it before running). The
//! check-in workflow is: render once, copy the printed numbers from
//! the test's diagnostic output into the `expected` blocks here.
//!
//! Tolerance: alpha/RGB sums can drift by ~0.1 % between runs because
//! of MuPDF anti-aliasing changes and direct-extract bilinear
//! sampling rounding. We bound at 1 % for stability across machines.

use image::GenericImageView;
use std::collections::HashMap;
use std::path::PathBuf;

use hp_core::ai_parser::{self, parse_ai};
use hp_core::puzzle;
use hp_core::types::Brick;

/// Per-house expected fingerprint. Update with `cargo test
/// canary_composite -- --nocapture` and copy the printed numbers
/// when the pipeline output legitimately changes.
struct Canary {
    file_stem: &'static str,
    width: u32,
    height: u32,
    alpha_sum: u64,
    alpha_top_third: u64,
    alpha_mid_third: u64,
    alpha_bot_third: u64,
    rgb_sum: u64,
}

// Dimensions are at 100 DPI (the canary's render DPI) padded by
// `pad_px = 80·100/300 ≈ 27` on each side — the same unified canvas
// size every non-piece asset is saved at. Alpha sums are invariant
// vs the un-padded version (padding adds transparent pixels with
// alpha=0); per-third sums shifted because the bucket boundaries
// cut at different y-positions when the content is offset by pad_px.
const NY5: Canary = Canary {
    file_stem: "_NY5",
    width: 1096, height: 2729,
    alpha_sum: 710_729_426,
    alpha_top_third: 234_355_241,
    alpha_mid_third: 241_529_309,
    alpha_bot_third: 234_844_876,
    rgb_sum: 1_070_587_028,
};

const NY8: Canary = Canary {
    file_stem: "_NY8",
    width: 1514, height: 8980,
    alpha_sum: 2_735_416_329,
    alpha_top_third: 518_689_439,
    alpha_mid_third: 1_113_608_196,
    alpha_bot_third: 1_103_118_694,
    rgb_sum: 4_776_880_109,
};

const TOLERANCE: f64 = 0.01;

fn ai_path(stem: &str) -> Option<PathBuf> {
    let candidates = [
        format!("in/{}.ai", stem),
        format!("../../in/{}.ai", stem),
    ];
    candidates.iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
}

fn close_enough(actual: u64, expected: u64, tol: f64, label: &str) {
    let diff = (actual as i128 - expected as i128).abs() as u64;
    let limit = ((expected as f64) * tol).round() as u64;
    assert!(
        diff <= limit,
        "{label} drifted: actual={actual} expected={expected} diff={diff} > {limit} ({:.2}% > {:.0}%)",
        100.0 * diff as f64 / expected as f64,
        tol * 100.0,
    );
}

fn run_canary(c: &Canary) {
    let Some(ai) = ai_path(c.file_stem) else {
        eprintln!("[canary {}] in/{}.ai not present — skipping", c.file_stem, c.file_stem);
        return;
    };
    let out_dir = std::env::temp_dir().join(format!("canary_{}", c.file_stem));
    let _ = std::fs::remove_dir_all(&out_dir);
    std::fs::create_dir_all(&out_dir).expect("create out_dir");

    // Mini export driver: parse, merge, render.
    let (placements, meta, _) =
        parse_ai(&ai, hp_core::CANVAS_HEIGHT_PX as i32).expect("parse_ai");

    let bricks: Vec<Brick> = placements.iter().enumerate().map(|(i, p)| Brick {
        id: format!("b{}", i),
        x: p.x, y: p.y, width: p.width, height: p.height,
        brick_type: p.layer_type.clone(),
    }).collect();
    let bricks_by_id: HashMap<String, Brick> =
        bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    let mut brick_polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut brick_beziers: HashMap<String, Vec<hp_core::bezier::BezierPath>> = HashMap::new();
    let mut brick_layer_names: HashMap<String, String> = HashMap::new();
    let ai_data = ai_parser::decompress_ai_data(&ai).expect("decompress_ai_data");
    for (i, p) in placements.iter().enumerate() {
        let id = format!("b{}", i);
        if let Some(poly) = &p.polygon { brick_polygons.insert(id.clone(), poly.clone()); }
        let block = ai_parser::LayerBlock {
            name: p.name.clone(),
            begin: p.block_begin, end: p.block_end,
            depth: 0, children: Vec::new(),
        };
        let bezier = ai_parser::extract_vector_path_bezier(
            &block, &ai_data.raw, meta.offset_x, meta.y_base,
        );
        brick_beziers.insert(id.clone(), bezier);
        brick_layer_names.insert(id, p.name.clone());
    }

    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);
    let adjacency = puzzle::build_adjacency_vector(
        &bricks, &brick_polygons, 15.0, 5.0, 2.0,
    );
    let pieces = puzzle::merge_bricks(&bricks, 60, 0, &adjacency, &brick_areas);

    // 100 DPI keeps the canary fast (~10 s per file) while still
    // exercising every stage. Pin to load_dpi = render_dpi so we
    // don't introduce scale rounding into the fingerprint.
    let export_dpi = 100.0_f64;
    hp_core::render::render_export_pieces(
        &ai, &placements, &meta,
        &pieces, &bricks_by_id, &brick_polygons,
        &brick_beziers, &brick_layer_names,
        export_dpi, export_dpi, &out_dir,
    ).expect("render_export_pieces");

    // Fingerprint composite.png.
    let composite_path = out_dir.join("composite.png");
    let img = image::open(&composite_path)
        .unwrap_or_else(|e| panic!("open composite.png: {e}"));
    let (w, h) = img.dimensions();
    let rgba = img.to_rgba8();
    let h_usize = h as usize;
    let third = h_usize / 3;

    let mut alpha_sum = 0u64;
    let mut alpha_top = 0u64;
    let mut alpha_mid = 0u64;
    let mut alpha_bot = 0u64;
    let mut rgb_sum = 0u64;
    for y in 0..h_usize {
        let bucket = if y < third { 0 } else if y < 2 * third { 1 } else { 2 };
        for x in 0..(w as usize) {
            let p = rgba.get_pixel(x as u32, y as u32);
            let a = p[3] as u64;
            alpha_sum += a;
            match bucket {
                0 => alpha_top += a,
                1 => alpha_mid += a,
                _ => alpha_bot += a,
            }
            rgb_sum += (p[0] as u64) + (p[1] as u64) + (p[2] as u64);
        }
    }

    // Print the fingerprint so an intentional baseline refresh just
    // means copying these numbers into the const above.
    eprintln!(
        "[canary {}] w={} h={} alpha_sum={} alpha_thirds=({},{},{}) rgb_sum={}",
        c.file_stem, w, h, alpha_sum, alpha_top, alpha_mid, alpha_bot, rgb_sum,
    );

    assert_eq!(w, c.width, "canvas width changed");
    assert_eq!(h, c.height, "canvas height changed");
    close_enough(alpha_sum, c.alpha_sum, TOLERANCE, "alpha_sum");
    close_enough(alpha_top, c.alpha_top_third, TOLERANCE, "alpha_top_third");
    close_enough(alpha_mid, c.alpha_mid_third, TOLERANCE, "alpha_mid_third");
    close_enough(alpha_bot, c.alpha_bot_third, TOLERANCE, "alpha_bot_third");
    close_enough(rgb_sum, c.rgb_sum, TOLERANCE, "rgb_sum");
}

#[test] fn canary_ny5_composite() { run_canary(&NY5); }
#[test] fn canary_ny8_composite() { run_canary(&NY8); }
