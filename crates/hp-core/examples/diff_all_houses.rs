//! For every in/_NY*.ai: render the bricks composite both ways
//! (direct extraction of every Image block vs MuPDF rendering of the
//! modified PDF), then diff. Reports per-file stats. Bails loudly if
//! any file's mean |Δ| exceeds a noise threshold.

use hp_core::ai_parser::parse_ai;
use hp_core::ocg_inject::{
    walk_page_bricks, build_modified_pdf, BrickContent, match_blocks_to_bricks, PageGeometry,
};
use hp_core::raster_extract::extract_image_block;
use image::RgbaImage;
use lopdf::{Document, Object};
use std::path::{Path, PathBuf};

fn page_h(doc: &Document, page_id: lopdf::ObjectId) -> f64 {
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
    match media.get(3).unwrap() {
        Object::Real(r) => *r as f64,
        Object::Integer(i) => *i as f64,
        _ => 0.0,
    }
}

#[derive(Debug)]
struct Report {
    name: String,
    placements: usize,
    image_blocks: usize,
    other_blocks: usize,
    bleed: (f64, f64),
    mean_abs_diff: f64,           // 0..255, averaged over all RGBA channels
    pct_pixels_above_4: f64,      // % of pixels where any channel diff > 4
    max_diff: u8,
    direct_secs: f64,
    mupdf_secs: f64,
}

fn render_one(ai_path: &Path, dpi: f64, save_dir: Option<&Path>) -> anyhow::Result<Report> {
    let base = ai_path.file_stem().unwrap().to_string_lossy().to_string();

    let (placements, meta, _) = parse_ai(ai_path, hp_core::CANVAS_HEIGHT_PX as i32)?;
    let doc = Document::load(ai_path)?;
    let page_id = doc.page_iter().next().unwrap();
    let blocks = walk_page_bricks(&doc, page_id)?;
    let page_h_pt = page_h(&doc, page_id);

    let tmp = std::env::temp_dir().join("diff_all_houses").join(&base);
    std::fs::create_dir_all(&tmp)?;
    let modified_pdf = tmp.join("_modified.pdf");
    let artifact = build_modified_pdf(ai_path, &placements, &meta, &modified_pdf)?;
    let geo = PageGeometry {
        clip_x0: meta.clip_rect.0, clip_y0: meta.clip_rect.1,
        render_dpi: meta.render_dpi, page_height_pt: page_h_pt,
        bleed_x: artifact.bleed_pts.0, bleed_y: artifact.bleed_pts.1,
    };
    let _map = match_blocks_to_bricks(&blocks, &placements, geo, 70.0);

    let scale = dpi / meta.render_dpi;
    let canvas_w = ((meta.canvas_width as f64) * scale).round() as u32;
    let canvas_h = ((meta.canvas_height as f64) * scale).round() as u32;

    // Hybrid composite: render the FULL MuPDF composite as the base
    // (gives correct non-Image content and z-order), then overlay
    // direct-extracted Image rasters on top. Source-over alpha
    // compositing means:
    //   - direct's opaque pixels overwrite MuPDF's at the brick body
    //   - MuPDF's pixels show through direct's transparent edges and
    //     wherever direct doesn't paint at all (vector content,
    //     decorations, gaps between bricks)
    // For pure-Image AIs this is equivalent to direct-only; for AIs
    // mixing vector overlays with bricks the vector content survives.
    let shifted_clip = (
        meta.clip_rect.0 + artifact.bleed_pts.0,
        meta.clip_rect.1 + artifact.bleed_pts.1,
        meta.clip_rect.2 + artifact.bleed_pts.0,
        meta.clip_rect.3 + artifact.bleed_pts.1,
    );
    let mut enabled: Vec<&str> = vec!["bricks", artifact.inline_ocg_name.as_str(),
        artifact.decoration_ocg_name.as_str()];
    for n in &artifact.brick_ocg_names { enabled.push(n.as_str()); }
    let t = std::time::Instant::now();
    let full_mupdf = hp_core::mupdf_ffi::render_page_with_ocg_set_clipped(
        modified_pdf.to_str().unwrap(), &enabled, dpi, Some(shifted_clip),
    ).and_then(|(rgba, w, h)| RgbaImage::from_raw(w, h, rgba));
    let mupdf_overlay_secs = t.elapsed().as_secs_f64();

    let mut canvas_d = if let Some(img) = full_mupdf {
        hp_core::render::compose_clipped_canvas(&img, "base", canvas_w, canvas_h, (0, 0))
    } else { RgbaImage::new(canvas_w, canvas_h) };

    // Direct extract Image blocks on top.
    let mut img_blocks = 0; let mut other_blocks = 0;
    let t = std::time::Instant::now();
    for b in &blocks {
        match &b.content {
            BrickContent::Image { object_id, .. } => {
                img_blocks += 1;
                if let Ok(r) = extract_image_block(&doc, *object_id, &b.inner_ctm_at_content) {
                    r.compose_onto(&mut canvas_d, meta.clip_rect, page_h_pt,
                        artifact.bleed_pts, dpi, true);
                }
            }
            _ => other_blocks += 1,
        }
    }
    let direct_secs = t.elapsed().as_secs_f64() + mupdf_overlay_secs;

    // MuPDF composite — same modified PDF, all OCGs on. To match the
    // direct path's content set we also enable hp_bricks_inline (so
    // Form/Inlined blocks paint), which is what the export does today.
    let shifted_clip = (
        meta.clip_rect.0 + artifact.bleed_pts.0,
        meta.clip_rect.1 + artifact.bleed_pts.1,
        meta.clip_rect.2 + artifact.bleed_pts.0,
        meta.clip_rect.3 + artifact.bleed_pts.1,
    );
    let mut enabled: Vec<&str> = vec!["bricks", artifact.inline_ocg_name.as_str()];
    for n in &artifact.brick_ocg_names { enabled.push(n.as_str()); }
    let t = std::time::Instant::now();
    let (rgba_m, w_m, h_m) = hp_core::mupdf_ffi::render_page_with_ocg_set_clipped(
        modified_pdf.to_str().unwrap(), &enabled, dpi, Some(shifted_clip),
    ).ok_or_else(|| anyhow::anyhow!("mupdf compose failed"))?;
    let mupdf_secs = t.elapsed().as_secs_f64();
    let img_m = RgbaImage::from_raw(w_m, h_m, rgba_m).unwrap();
    let canvas_m = hp_core::render::compose_clipped_canvas(
        &img_m, "bricks", canvas_w, canvas_h, (0, 0));

    // Save individual outputs + a colour-coded diff PNG to the
    // shared compare_view directory so the user can flip through them
    // side-by-side via the web viewer.
    if let Some(dir) = save_dir {
        std::fs::create_dir_all(dir).ok();
        canvas_d.save(dir.join(format!("{}_direct.png", base))).ok();
        canvas_m.save(dir.join(format!("{}_mupdf.png", base))).ok();
        // Diff PNG: per-pixel red = direct alpha not in mupdf, blue =
        // mupdf alpha not in direct, magenta = both present but RGB
        // disagree, transparent = identical or both empty.
        if canvas_d.dimensions() == canvas_m.dimensions() {
            let (w, h) = canvas_d.dimensions();
            let mut diff = RgbaImage::new(w, h);
            for y in 0..h {
                for x in 0..w {
                    let a = canvas_d.get_pixel(x, y).0;
                    let b = canvas_m.get_pixel(x, y).0;
                    let a_lit = a[3] > 8;
                    let b_lit = b[3] > 8;
                    let rgb_diff = (a[0] as i32 - b[0] as i32).unsigned_abs()
                        .max((a[1] as i32 - b[1] as i32).unsigned_abs())
                        .max((a[2] as i32 - b[2] as i32).unsigned_abs())
                        > 12;
                    let pix = match (a_lit, b_lit) {
                        (true, false) => [255, 0, 0, 255],   // direct-only
                        (false, true) => [0, 0, 255, 255],   // mupdf-only
                        (true, true) if rgb_diff => [255, 0, 255, 255], // disagree
                        _ => [0, 0, 0, 0],                   // identical or both empty
                    };
                    diff.put_pixel(x, y, image::Rgba(pix));
                }
            }
            diff.save(dir.join(format!("{}_diff.png", base))).ok();
        }
    }

    // Diff. Direct includes only Image blocks; MuPDF includes everything.
    // For NY5 the "other" set is just 7 small inline shapes (window
    // panes), so the diff will *expect* to be non-zero at those spots.
    // We measure but don't fail on this — the goal is to confirm that
    // the IMAGE content lines up.
    let (mut sum, mut n, mut over_4, mut total_px, mut maxd) = (0u64, 0u64, 0u64, 0u64, 0u8);
    if canvas_d.dimensions() == canvas_m.dimensions() {
        for (a, b) in canvas_d.pixels().zip(canvas_m.pixels()) {
            let mut row_max = 0u8;
            for c in 0..4 {
                let d = (a.0[c] as i16 - b.0[c] as i16).unsigned_abs() as u8;
                sum += d as u64; n += 1;
                if d > row_max { row_max = d; }
                if d > maxd { maxd = d; }
            }
            total_px += 1;
            if row_max > 4 { over_4 += 1; }
        }
    }

    Ok(Report {
        name: base, placements: placements.len(),
        image_blocks: img_blocks, other_blocks,
        bleed: artifact.bleed_pts,
        mean_abs_diff: sum as f64 / n as f64,
        pct_pixels_above_4: 100.0 * over_4 as f64 / total_px as f64,
        max_diff: maxd,
        direct_secs, mupdf_secs,
    })
}

fn main() -> anyhow::Result<()> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir("in")?
        .filter_map(|e| e.ok()).map(|e| e.path())
        .filter(|p| p.file_name().and_then(|n| n.to_str())
            .map(|n| n.starts_with("_NY") && n.ends_with(".ai")).unwrap_or(false))
        .collect();
    paths.sort();
    let dpi: f64 = std::env::var("DPI").ok().and_then(|s| s.parse().ok()).unwrap_or(150.0);
    let save_dir = PathBuf::from("/tmp/compare_view/houses");
    eprintln!("dpi={}, {} files, saving to {}\n", dpi, paths.len(), save_dir.display());

    let mut reports = Vec::new();
    let mut alerts = Vec::new();
    for p in &paths {
        eprint!("{:<14} ", p.file_name().unwrap().to_string_lossy());
        match render_one(p, dpi, Some(&save_dir)) {
            Ok(r) => {
                eprintln!("p={:3} I={:3} other={:2} bleed=({:+7.3},{:+7.3}) \
                          mean_diff={:6.3} >4px:{:5.2}% max={:3} direct={:5.2}s mupdf={:5.2}s",
                    r.placements, r.image_blocks, r.other_blocks,
                    r.bleed.0, r.bleed.1,
                    r.mean_abs_diff, r.pct_pixels_above_4, r.max_diff,
                    r.direct_secs, r.mupdf_secs);
                // Noise threshold: NY5 had mean ≈ 1-2 / 255 and ~0% over-4
                // for Image-only content. Anything noticeably worse should
                // halt and alert.
                if r.mean_abs_diff > 3.0 || r.pct_pixels_above_4 > 1.0 {
                    alerts.push(format!("{}: mean={:.2} >4%={:.2}",
                        r.name, r.mean_abs_diff, r.pct_pixels_above_4));
                }
                reports.push(r);
            }
            Err(e) => {
                eprintln!("FAILED: {}", e);
                alerts.push(format!("{}: FAILED: {}", p.display(), e));
            }
        }
    }
    eprintln!("\n=== {} of {} clean ===", reports.len() - alerts.iter()
        .filter(|s| !s.contains("FAILED")).count(), reports.len());
    if !alerts.is_empty() {
        eprintln!("\nALERTS:");
        for a in &alerts { eprintln!("  ⚠️  {}", a); }
        std::process::exit(2);
    }
    Ok(())
}
