//! CLI diagnostic: render each parser brick to its own PNG via the
//! injected per-brick OCG pipeline. The output PNGs are the visual
//! validation surface for the no-bleed property — a per-brick render
//! that came from selectively enabling one OCG cannot contain pixels
//! from any adjacent brick's image, by construction.
//!
//! Usage:
//!   cargo run --release -p hp-core --example render_per_brick -- \
//!       in/_NY1.ai out_dir/

use std::path::Path;

use hp_core::ai_parser::parse_ai;
use hp_core::mupdf_ffi;
use hp_core::ocg_inject::{self, ModifiedPdfArtifact};

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: render_per_brick <ai_path> <out_dir>"))?;
    let out_dir = args
        .next()
        .ok_or_else(|| anyhow::anyhow!("usage: render_per_brick <ai_path> <out_dir>"))?;
    let ai_path = Path::new(&ai_path);
    let out_dir = Path::new(&out_dir);
    std::fs::create_dir_all(out_dir)?;

    // 1. Parse AI → brick placements + parsing metadata.
    let (placements, meta, _ai_data) = parse_ai(ai_path, hp_core::CANVAS_HEIGHT_PX as i32)?;
    eprintln!(
        "parsed {} bricks, canvas {}x{} px, render_dpi {:.2}",
        placements.len(),
        meta.canvas_width,
        meta.canvas_height,
        meta.render_dpi,
    );

    // 2. Build the rewritten PDF beside the input, suffixed `.ocg.pdf`.
    let modified_path = out_dir.join("modified.ocg.pdf");
    let artifact: ModifiedPdfArtifact =
        ocg_inject::build_modified_pdf(ai_path, &placements, &meta, &modified_path)?;
    eprintln!(
        "rewrote PDF → {}\n  pdf_blocks: {} (matched={} decoration={})\n  bricks_with_blocks: {} (orphans={})",
        modified_path.display(),
        artifact.stats.pdf_blocks_total,
        artifact.stats.pdf_blocks_matched,
        artifact.stats.pdf_blocks_decoration,
        artifact.stats.bricks_with_at_least_one_block,
        artifact.stats.bricks_orphaned,
    );

    // 3. Sanity-check MuPDF can enumerate our injected OCGs.
    mupdf_ffi::init_ffi_context();
    {
        let modified_str = modified_path
            .to_str()
            .ok_or_else(|| anyhow::anyhow!("non-utf8 path"))?;
        let doc = mupdf::pdf::PdfDocument::open(modified_str)?;
        let count = mupdf_ffi::count_layer_ui(&doc);
        let names: Vec<String> = (0..count)
            .map(|i| mupdf_ffi::layer_ui_info(&doc, i).text)
            .collect();
        let injected_seen = names
            .iter()
            .filter(|n| n.starts_with("hp_brick_"))
            .count();
        eprintln!(
            "MuPDF sees {} layer-config-UI entries; {} are injected hp_brick_*",
            count, injected_seen
        );
        if injected_seen == 0 {
            anyhow::bail!(
                "MuPDF did not surface any injected OCGs — check that /D /Order \
                 was updated and that /OCProperties /OCGs contains the new refs"
            );
        }
    }

    // 4. Render each brick to its own PNG. We enable:
    //      - `bricks`            (so brick blocks render at all)
    //      - `background`        (kept for visual context — comment out
    //                             to render bricks-only on transparent)
    //      - the target `hp_brick_NNNN`
    //    All other injected OCGs (other bricks + decoration) are off.
    let modified_str = modified_path
        .to_str()
        .ok_or_else(|| anyhow::anyhow!("non-utf8 path"))?;
    let dpi = meta.render_dpi;
    let mut rendered = 0_usize;
    let mut empty = 0_usize;
    for (brick_idx, brick_ocg) in artifact.brick_ocg_names.iter().enumerate() {
        let enabled = ["bricks", brick_ocg.as_str()];
        let render = mupdf_ffi::render_page_with_ocg_set_clipped(
            modified_str,
            &enabled,
            dpi,
            None,
        );
        let Some((rgba, w, h)) = render else {
            eprintln!("  brick {brick_idx:4}: render returned None — skipping");
            continue;
        };
        // Drop fully-transparent renders to make manual review easier.
        let any_visible = rgba.chunks_exact(4).any(|px| px[3] > 0);
        if !any_visible {
            empty += 1;
            continue;
        }
        let png_path = out_dir.join(format!("brick_{:04}.png", brick_idx));
        save_rgba_png(&png_path, w, h, &rgba)?;
        rendered += 1;
    }
    eprintln!("rendered {rendered} brick PNGs ({empty} empty)");

    // 5. Also render a "composite no-decoration" PNG for sanity-checking
    //    against the existing render — should look like the AI house but
    //    without any decorations layer (if it exists).
    let mut composite_enabled: Vec<&str> = vec!["bricks"];
    composite_enabled.extend(artifact.brick_ocg_names.iter().map(|s| s.as_str()));
    if let Some((rgba, w, h)) = mupdf_ffi::render_page_with_ocg_set_clipped(
        modified_str,
        &composite_enabled,
        dpi,
        None,
    ) {
        save_rgba_png(&out_dir.join("composite_no_decoration.png"), w, h, &rgba)?;
    }

    Ok(())
}

fn save_rgba_png(path: &Path, w: u32, h: u32, rgba: &[u8]) -> anyhow::Result<()> {
    use image::{ImageBuffer, Rgba};
    let buf: ImageBuffer<Rgba<u8>, _> =
        ImageBuffer::from_raw(w, h, rgba.to_vec()).ok_or_else(|| {
            anyhow::anyhow!("RGBA buffer length {} doesn't match {}x{}", rgba.len(), w, h)
        })?;
    buf.save(path)?;
    Ok(())
}
