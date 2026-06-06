//! Render the full house canvas with ONLY one brick visible (everything
//! else transparent). Used to inspect what a single brick's content
//! actually looks like through the OCG-isolation path.
//!
//!   cargo run --release -p hp-core --example show_one_brick -- \
//!       in/_NY5.ai "Layer 25" /tmp/layer25.png

use hp_core::ai_parser::parse_ai;
use image::RgbaImage;
use std::path::Path;

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = args.next().ok_or_else(|| anyhow::anyhow!("missing ai path"))?;
    let layer_name = args.next().ok_or_else(|| anyhow::anyhow!("missing layer name"))?;
    let out_path = args.next().ok_or_else(|| anyhow::anyhow!("missing out path"))?;

    let ai_path = Path::new(&ai_path);
    let out_path = Path::new(&out_path);

    let dpi: f64 = args.next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300.0);

    let (placements, meta, _) = parse_ai(ai_path, hp_core::CANVAS_HEIGHT_PX as i32)?;
    eprintln!("[show] {} bricks, canvas {}x{}, render_dpi={:.2}, target_dpi={}",
        placements.len(), meta.canvas_width, meta.canvas_height, meta.render_dpi, dpi);
    eprintln!("[show] clip_rect (pymu pts): {:?}", meta.clip_rect);

    // Find the placement matching the requested layer name.
    let idx = placements.iter().position(|p| p.name == layer_name)
        .ok_or_else(|| anyhow::anyhow!("layer '{}' not found", layer_name))?;
    let p = &placements[idx];
    eprintln!("[show] match: idx={} name={} pymu_bbox=({},{},{},{})",
        idx, p.name, p.pymu_x, p.pymu_y, p.pymu_w, p.pymu_h);

    // Compute the pdf_offset by rendering the bricks layer of the ORIGINAL
    // AI and comparing where its first opaque pixel sits vs the parser's
    // expected leftmost-brick coord. Some AIs have a coord-origin disagreement
    // between the parser's bbox math and MuPDF's page-space rendering; the
    // load_pdf path fixes this with a shifted clip rect — we do the same here.
    let offset_dpi = 100.0_f64;
    let (rgba0, w0, h0) = hp_core::mupdf_ffi::render_page_with_ocg_set_clipped(
        ai_path.to_str().unwrap(),
        &["bricks"],
        offset_dpi,
        Some(meta.clip_rect),
    ).ok_or_else(|| anyhow::anyhow!("offset-probe render failed"))?;
    let probe = RgbaImage::from_raw(w0, h0, rgba0).unwrap();
    let s = offset_dpi / meta.render_dpi;
    let expected_x = ((meta.expected_brick_min.0 as f64) * s).round() as i32;
    let expected_y = ((meta.expected_brick_min.1 as f64) * s).round() as i32;
    let pdf_offset = hp_core::render::compute_pdf_offset(&probe, expected_x, expected_y);
    eprintln!("[show] pdf_offset (at {} dpi) = {:?}, expected_min=({},{})",
        offset_dpi, pdf_offset, expected_x, expected_y);

    // Convert pixel offset to PDF pts and shift the clip rect on the same side.
    let dx_pts = pdf_offset.0 as f64 * 72.0 / offset_dpi;
    let dy_pts = pdf_offset.1 as f64 * 72.0 / offset_dpi;
    let shifted_clip = (
        meta.clip_rect.0 - dx_pts,
        meta.clip_rect.1 - dy_pts,
        meta.clip_rect.2 - dx_pts,
        meta.clip_rect.3 - dy_pts,
    );
    eprintln!("[show] shifted_clip = {:?}", shifted_clip);

    // Build the OCG-modified PDF — same path the export uses.
    let tmp_dir = std::env::temp_dir().join("show_one_brick");
    std::fs::create_dir_all(&tmp_dir)?;
    let modified_pdf = tmp_dir.join("_modified.pdf");
    let artifact = hp_core::ocg_inject::build_modified_pdf(
        ai_path, &placements, &meta, &modified_pdf,
    )?;
    eprintln!("[show] injected {} brick OCGs", artifact.brick_ocg_names.len());

    // Render full canvas at target DPI with enabled = ["bricks", hp_bricks_inline, hp_brick_NNNN_for_layer].
    let target_ocg = artifact.brick_ocg_names.get(idx)
        .ok_or_else(|| anyhow::anyhow!("no OCG for idx {}", idx))?;
    eprintln!("[show] enabling bricks + {} + {}", artifact.inline_ocg_name, target_ocg);

    // Enable hp_bricks_inline too — it wraps the inline content
    // inside /OC /bricks that isn't part of any per-brick block
    // (window panes etc. that the injection couldn't match to a brick).
    // Default: do NOT enable hp_bricks_inline. That OCG carries content
    // that lives inside /OC /bricks but wasn't tagged into any per-brick
    // q…Q block (chiefly: blocks whose q…Q straddles the bricks/lights
    // OCG boundary, which we reject in walk_page_bricks to avoid
    // improper BDC/EMC nesting). Showing it would mix in other bricks'
    // content (e.g. Layer 4's arch) — not what "isolate Layer N" means.
    // Set INCLUDE_INLINE=1 to see it anyway.
    let inline_name = artifact.inline_ocg_name.clone();
    let include_inline = std::env::var("INCLUDE_INLINE")
        .map(|v| v == "1" || v == "true")
        .unwrap_or(false);
    let mut enabled: Vec<&str> = vec!["bricks", target_ocg.as_str()];
    if include_inline { enabled.push(inline_name.as_str()); }
    eprintln!("[show] include_inline={}", include_inline);
    let (rgba, w, h) = hp_core::mupdf_ffi::render_page_with_ocg_set_clipped(
        modified_pdf.to_str().unwrap(),
        &enabled,
        dpi,
        Some(shifted_clip),
    )
    .ok_or_else(|| anyhow::anyhow!("render failed"))?;
    eprintln!("[show] rendered {}x{}", w, h);

    // Compose to canvas at (0, 0) scaled to the target DPI.
    let scale = dpi / meta.render_dpi;
    let canvas_w = ((meta.canvas_width as f64) * scale).round() as u32;
    let canvas_h = ((meta.canvas_height as f64) * scale).round() as u32;
    let raw = RgbaImage::from_raw(w, h, rgba).unwrap();
    let canvas = hp_core::render::compose_clipped_canvas(
        &raw, "show-one-brick", canvas_w, canvas_h, (0, 0),
    );
    canvas.save(out_path)?;
    eprintln!("[show] wrote {} ({}x{})", out_path.display(), canvas_w, canvas_h);

    // Keep modified PDF for inspection.
    eprintln!("[show] modified PDF kept at: {}", modified_pdf.display());
    Ok(())
}
