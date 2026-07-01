//! Render every brick via direct AI Image XObject extraction (no MuPDF
//! per-layer raster). Form/Inlined bricks (the few that aren't pure
//! Image XObjects) fall back to MuPDF. Output is consumed by
//! `/tmp/compare_view/index.html` for visual inspection.
//!
//!   cargo run --release -p hp-core --example gen_all_layers -- \
//!       in/_NY5.ai /tmp/compare_view/layers 300

use hp_core::ai_parser::{parse_ai, extract_vector_path_bezier, LayerBlock};
use hp_core::ocg_inject::{walk_page_bricks, match_blocks_to_bricks, BrickContent, PageGeometry};
use hp_core::raster_extract::extract_image_block;
use image::RgbaImage;
use lopdf::{Document, Object};
use rayon::prelude::*;
use std::path::{Path, PathBuf};

fn sanitise(name: &str) -> String {
    name.chars().map(|c| if c.is_ascii_alphanumeric() { c } else { '_' }).collect()
}

fn page_mediabox_y1(doc: &Document, page_id: lopdf::ObjectId) -> f64 {
    let page = doc.get_object(page_id).unwrap().as_dict().unwrap();
    let media = page.get(b"MediaBox").unwrap().as_array().unwrap();
    match media.get(3).unwrap() {
        Object::Real(r) => *r as f64,
        Object::Integer(i) => *i as f64,
        _ => 0.0,
    }
}

fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = args.next().expect("ai path");
    let out_dir = args.next().expect("out dir");
    let dpi: f64 = args.next().and_then(|s| s.parse().ok()).unwrap_or(300.0);

    let ai_path = Path::new(&ai_path);
    let out_dir = PathBuf::from(out_dir);
    std::fs::create_dir_all(&out_dir)?;

    let t_total = std::time::Instant::now();
    let (placements, meta, _) = parse_ai(ai_path, hp_core::CANVAS_HEIGHT_PX as i32)?;
    eprintln!("[direct] {} placements @ {} dpi", placements.len(), dpi);

    // Load PDF, walk blocks, match.
    let doc = Document::load(ai_path)?;
    let page_id = doc.page_iter().next().unwrap();
    let blocks = walk_page_bricks(&doc, page_id)?;
    let page_h = page_mediabox_y1(&doc, page_id);

    // Build modified PDF — gives us the median-refined bleed in one
    // shot, plus the per-brick OCGs we need for the Form/Inlined
    // fallback. Costs ~1 walk + 1 PDF write (no rendering).
    let tmp = std::env::temp_dir().join("gen_all_layers_direct");
    std::fs::create_dir_all(&tmp)?;
    let modified_pdf = tmp.join("_modified.pdf");
    let artifact = hp_core::ocg_inject::build_modified_pdf(
        ai_path, &placements, &meta, &modified_pdf)?;
    let bleed = artifact.bleed_pts;
    eprintln!("[direct] bleed = ({:.3}, {:.3})", bleed.0, bleed.1);

    let geo = PageGeometry {
        clip_x0: meta.clip_rect.0, clip_y0: meta.clip_rect.1,
        render_dpi: meta.render_dpi, page_height_pt: page_h,
        bleed_x: bleed.0, bleed_y: bleed.1,
    };
    let map = match_blocks_to_bricks(&blocks, &placements, geo, 70.0);

    let scale = dpi / meta.render_dpi;
    let canvas_w = ((meta.canvas_width as f64) * scale).round() as u32;
    let canvas_h = ((meta.canvas_height as f64) * scale).round() as u32;

    // Pre-compute shifted clip for the MuPDF fallback path. Same trick
    // as gen_all_layers: shift clip_rect by `bleed` so MuPDF's pixmap
    // x=0 lands at the parser's pymu_x=0.
    let shifted_clip = (
        meta.clip_rect.0 + bleed.0, meta.clip_rect.1 + bleed.1,
        meta.clip_rect.2 + bleed.0, meta.clip_rect.3 + bleed.1,
    );

    // Per-layer entry for layers.json.
    let entries: Vec<Option<(String, String, i32, i32, i32, i32, Vec<[f64; 2]>)>> = placements
        .par_iter()
        .enumerate()
        .map(|(idx, p)| {
            let block_idxs = &map.brick_to_blocks[idx];
            let mut canvas = RgbaImage::new(canvas_w, canvas_h);
            let mut painted = false;
            // Z-order: blocks within a brick are listed in match-pass
            // order. For per-layer isolation each layer has at most a
            // few; order matters only when overlays exist.
            for &bi in block_idxs {
                let b = &blocks[bi];
                match &b.content {
                    BrickContent::Image { object_id, .. } => {
                        match extract_image_block(&doc, *object_id, &b.inner_ctm_at_content) {
                            Ok(raster) => {
                                raster.compose_onto(&mut canvas, meta.clip_rect, page_h, bleed, dpi, true);
                                painted = true;
                            }
                            Err(e) => eprintln!("[direct] {} block {}: image extract failed: {}", p.name, bi, e),
                        }
                    }
                    BrickContent::Form { .. } | BrickContent::Inlined => {
                        // MuPDF fallback for the small subset of non-Image bricks.
                        let target_ocg = match artifact.brick_ocg_names.get(idx) {
                            Some(s) => s.clone(),
                            None => continue,
                        };
                        let enabled = vec!["bricks", target_ocg.as_str()];
                        if let Some((rgba, w, h)) = hp_core::mupdf_ffi::render_page_with_ocg_set_clipped(
                            modified_pdf.to_str().unwrap(),
                            &enabled, dpi, Some(shifted_clip),
                        ) {
                            if let Some(rendered) = RgbaImage::from_raw(w, h, rgba) {
                                let composed = hp_core::render::compose_clipped_canvas(
                                    &rendered, "fallback", canvas_w, canvas_h, (0, 0));
                                image::imageops::overlay(&mut canvas, &composed, 0, 0);
                                painted = true;
                            }
                        }
                    }
                }
            }
            if !painted { /* still write an empty PNG so the viewer has files for every layer */ }

            let fname = format!("layer_{}.png", sanitise(&p.name));
            canvas.save(out_dir.join(&fname)).ok()?;
            let poly_scaled: Vec<[f64; 2]> = match p.polygon.as_ref() {
                Some(poly) => poly.iter().map(|v| [
                    (v[0] + p.pymu_x as f64) * scale,
                    (v[1] + p.pymu_y as f64) * scale,
                ]).collect(),
                None => Vec::new(),
            };
            Some((p.name.clone(), fname, p.pymu_x, p.pymu_y, p.pymu_w, p.pymu_h, poly_scaled))
        })
        .collect();

    let entries: Vec<_> = entries.into_iter().flatten().collect();
    eprintln!("[direct] {} layers in {:?}", entries.len(), t_total.elapsed());

    // ── Write layers.json ─────────────────────────────────────────
    let mut json = format!(
        "{{\n  \"canvas_w\": {},\n  \"canvas_h\": {},\n  \"layers\": [\n",
        canvas_w, canvas_h);
    for (i, (name, fname, x, y, w, h, poly)) in entries.iter().enumerate() {
        let comma = if i + 1 == entries.len() { "" } else { "," };
        let poly_str = if poly.is_empty() {
            String::from("[]")
        } else {
            let pts: Vec<String> = poly.iter().map(|v| format!("[{:.2},{:.2}]", v[0], v[1])).collect();
            format!("[{}]", pts.join(","))
        };
        json.push_str(&format!(
            "    {{\"name\": \"{}\", \"file\": \"{}\", \"x\": {}, \"y\": {}, \"w\": {}, \"h\": {}, \"polygon\": {}}}{}\n",
            name.replace('"', "\\\""), fname, x, y, w, h, poly_str, comma));
    }
    json.push_str("  ]\n}\n");
    std::fs::write(out_dir.join("layers.json"), json)?;

    // ── Full-house vector outline (unchanged from gen_all_layers) ─
    let ai_data = hp_core::ai_parser::decompress_ai_data(ai_path)?;
    let mut svg = format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 {} {}\" preserveAspectRatio=\"xMidYMid meet\">\n  <g fill=\"none\" stroke=\"white\" stroke-width=\"0.1\" vector-effect=\"non-scaling-stroke\">\n",
        canvas_w, canvas_h);
    let beziers: Vec<_> = placements.par_iter().map(|p| {
        let block = LayerBlock { name: p.name.clone(), begin: p.block_begin, end: p.block_end,
            depth: 0, children: Vec::new() };
        extract_vector_path_bezier(&block, &ai_data.raw, meta.offset_x, meta.y_base)
    }).collect();
    for (i, paths) in beziers.iter().enumerate() {
        for bp in paths {
            let t = bp.transform([-meta.clip_rect.0, -meta.clip_rect.1], dpi / 72.0);
            svg.push_str(&format!(
                "    <path data-brick=\"{}\" data-idx=\"{}\" d=\"{}\"/>\n",
                placements[i].name.replace('"', "&quot;"), i, t.to_svg_d()));
        }
    }
    svg.push_str("  </g>\n</svg>\n");
    std::fs::write(out_dir.join("full_outline.svg"), svg)?;

    eprintln!("[direct] DONE in {:?}", t_total.elapsed());
    Ok(())
}
