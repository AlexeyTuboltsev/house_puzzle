//! Diagnostic: walk the PDF page's content stream and reconcile the
//! brick `q…Q` blocks against the AI parser's brick list.
//!
//! Used to figure out why 198 PDF blocks correspond to only 183 parser
//! bricks (in `_NY1.ai`) and what the 15 extras are.
//!
//! Usage:
//!   cargo run -p hp-core --example dump_brick_blocks -- in/_NY1.ai

use lopdf::{Document, Object};
use std::collections::HashMap;

#[derive(Default, Debug)]
struct BlockStats {
    nesting_depth: u32,
    path_ops: u32,
    do_refs: u32,
    sh_refs: u32,
    gs_refs: u32,
    forms: Vec<String>,
}

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_brick_blocks <path>");
    let doc = Document::load(&path).expect("open PDF");

    let pages = doc.get_pages();
    let (_, page_id) = pages.iter().next().expect("at least one page");
    let content_data = doc
        .get_and_decode_page_content(*page_id)
        .expect("decode page content");

    println!("operator count: {}", content_data.operations.len());

    // ── First pass: stats over q…Q blocks in /OC regions ──────────
    let mut in_bricks = 0_i32;
    let mut q_stack = 0_u32;
    let mut current: Option<BlockStats> = None;
    let mut totals: Vec<BlockStats> = Vec::new();

    for op in &content_data.operations {
        let name = op.operator.as_str();
        match name {
            "BDC" => {
                let tag_is_oc = matches!(op.operands.get(0), Some(Object::Name(n)) if n == b"OC");
                if tag_is_oc {
                    in_bricks += 1;
                }
            }
            "EMC" => {
                if in_bricks > 0 {
                    in_bricks -= 1;
                }
            }
            "q" => {
                if in_bricks > 0 && q_stack == 0 {
                    current = Some(BlockStats::default());
                }
                if let Some(c) = current.as_mut() {
                    c.nesting_depth = c.nesting_depth.max(q_stack + 1);
                }
                q_stack += 1;
            }
            "Q" => {
                if q_stack > 0 {
                    q_stack -= 1;
                }
                if q_stack == 0 {
                    if let Some(c) = current.take() {
                        totals.push(c);
                    }
                }
            }
            "m" | "l" | "c" | "v" | "y" | "h" | "re" => {
                if let Some(c) = current.as_mut() {
                    c.path_ops += 1;
                }
            }
            "Do" => {
                if let Some(c) = current.as_mut() {
                    c.do_refs += 1;
                    if let Some(Object::Name(n)) = op.operands.get(0) {
                        c.forms.push(String::from_utf8_lossy(n).into_owned());
                    }
                }
            }
            "sh" => {
                if let Some(c) = current.as_mut() {
                    c.sh_refs += 1;
                }
            }
            "gs" => {
                if let Some(c) = current.as_mut() {
                    c.gs_refs += 1;
                }
            }
            _ => {}
        }
    }

    println!("\ntop-level q…Q blocks inside /OC BDC regions: {}", totals.len());

    let max_paths = totals.iter().map(|t| t.path_ops).max().unwrap_or(0);
    let blocks_with_paths = totals.iter().filter(|t| t.path_ops > 0).count();
    let blocks_with_sh = totals.iter().filter(|t| t.sh_refs > 0).count();
    println!("max path ops in any block: {}", max_paths);
    println!("blocks with any path ops:  {} / {}", blocks_with_paths, totals.len());
    println!("blocks with shading (sh):  {} / {}", blocks_with_sh, totals.len());

    let mut form_to_blocks: HashMap<String, u32> = HashMap::new();
    let mut blocks_with_zero_forms = 0;
    let mut blocks_with_one_form = 0;
    let mut blocks_with_many_forms = 0;
    for t in &totals {
        match t.forms.len() {
            0 => blocks_with_zero_forms += 1,
            1 => blocks_with_one_form += 1,
            _ => blocks_with_many_forms += 1,
        }
        for f in &t.forms {
            *form_to_blocks.entry(f.clone()).or_default() += 1;
        }
    }
    println!(
        "\nForm/Image XObject usage: zero={zero} one={one} many={many}",
        zero = blocks_with_zero_forms,
        one = blocks_with_one_form,
        many = blocks_with_many_forms,
    );

    // ── Per-block detail (via the real walker that resolves OCG names) ───
    let lopdf_doc = Document::load(&path).expect("re-open with lopdf");
    let pages = lopdf_doc.get_pages();
    let (_, page_id) = pages.iter().next().unwrap();
    let blocks = hp_core::ocg_inject::walk_page_bricks(&lopdf_doc, *page_id)
        .expect("walk_page_bricks");

    let n_image = blocks
        .iter()
        .filter(|b| matches!(b.content, hp_core::ocg_inject::BrickContent::Image { .. }))
        .count();
    let n_form = blocks
        .iter()
        .filter(|b| matches!(b.content, hp_core::ocg_inject::BrickContent::Form { .. }))
        .count();
    let n_inl = blocks
        .iter()
        .filter(|b| matches!(b.content, hp_core::ocg_inject::BrickContent::Inlined))
        .count();
    println!(
        "\nocg_inject walker found {} blocks inside /OC bricks",
        blocks.len()
    );
    println!("  by kind: Image={n_image}  Form={n_form}  Inlined={n_inl}");

    // ── Extract min/max x from all drawing ops in content stream ─
    let mut cs_xmin = f64::INFINITY;
    let mut cs_xmax = f64::NEG_INFINITY;
    let mut in_bg_layer = false;
    let mut bg_x_min = f64::INFINITY;
    let mut bg_x_max = f64::NEG_INFINITY;
    let mut in_any_oc = 0i32;
    for op in &content_data.operations {
        match op.operator.as_str() {
            "BDC" => {
                if in_any_oc == 0 { in_bg_layer = true; }
                in_any_oc += 1;
            }
            "EMC" => {
                if in_any_oc > 0 { in_any_oc -= 1; }
                if in_any_oc == 0 { in_bg_layer = false; }
            }
            "m" | "l" | "re" => {
                let x_val = op.operands.first().and_then(|o| match o {
                    lopdf::Object::Real(r) => Some(*r as f64),
                    lopdf::Object::Integer(i) => Some(*i as f64),
                    _ => None,
                });
                if let Some(x) = x_val {
                    cs_xmin = cs_xmin.min(x);
                    cs_xmax = cs_xmax.max(x);
                    if in_bg_layer {
                        bg_x_min = bg_x_min.min(x);
                        bg_x_max = bg_x_max.max(x);
                    }
                }
            }
            _ => {}
        }
    }
    println!("\n== Content stream x extents ==");
    println!("  All ops:        x=[{:.3}..{:.3}]", cs_xmin, cs_xmax);
    println!("  Background BDC: x=[{:.3}..{:.3}]", bg_x_min, bg_x_max);
    let cm_ops: Vec<_> = content_data.operations.iter().enumerate()
        .filter(|(_, op)| op.operator == "cm")
        .collect();
    let cm_e_min = cm_ops.iter().filter_map(|(_, op)| {
        let nums: Vec<f64> = op.operands.iter().filter_map(|o| match o {
            lopdf::Object::Real(r) => Some(*r as f64),
            lopdf::Object::Integer(i) => Some(*i as f64),
            _ => None,
        }).collect();
        if nums.len() >= 6 { Some(nums[4]) } else { None }
    }).fold(f64::INFINITY, f64::min);
    let cm_e_max = cm_ops.iter().filter_map(|(_, op)| {
        let nums: Vec<f64> = op.operands.iter().filter_map(|o| match o {
            lopdf::Object::Real(r) => Some(*r as f64),
            lopdf::Object::Integer(i) => Some(*i as f64),
            _ => None,
        }).collect();
        if nums.len() >= 6 { Some(nums[4]) } else { None }
    }).fold(f64::NEG_INFINITY, f64::max);
    println!("  cm e-values:    [{:.3}..{:.3}]  ({} cm ops)", cm_e_min, cm_e_max, cm_ops.len());
    println!("\n== First 20 cm ops ==");
    for (i, op) in cm_ops.iter().take(20) {
        println!("  [{}] cm {:?}", i, op.operands);
    }

    // ── PDF blocks dump ─────────────────────────────────────────
    println!("\n== PDF blocks in document order ==");
    println!(
        "{:<6} {:<8} {:<10} {:>10} {:>10} {:>12} {:>12}",
        "idx", "kind", "name", "inner_e", "inner_f", "outer_e", "outer_f"
    );
    for (i, b) in blocks.iter().enumerate() {
        let (kind, name) = match &b.content {
            hp_core::ocg_inject::BrickContent::Image { name, .. } => ("Image", name.clone()),
            hp_core::ocg_inject::BrickContent::Form { name, .. } => ("Form", name.clone()),
            hp_core::ocg_inject::BrickContent::Inlined => ("Inlined", "-".to_string()),
        };
        println!(
            "{:<6} {:<8} {:<10} {:>10.2} {:>10.2} {:>12.2} {:>12.2}",
            i, kind, name, b.inner_ctm_at_content.e, b.inner_ctm_at_content.f,
            b.outer_ctm.e, b.outer_ctm.f
        );
    }

    // ── Parser bricks dump + cross-reference ────────────────────
    let Ok((placements, meta, _ai_data)) =
        hp_core::ai_parser::parse_ai(std::path::Path::new(&path), 900)
    else {
        eprintln!("parse_ai failed");
        return;
    };
    let (clip_x0, clip_y0, _clip_x1, _clip_y1) = meta.clip_rect;
    // Page height (mediabox y1) varies per file — read it from lopdf.
    let pdf_page_height: f64 = {
        let page = lopdf_doc.get_object(*page_id).expect("page object").as_dict().expect("page dict");
        let mediabox = match page.get(b"MediaBox") {
            Ok(Object::Array(a)) => a.clone(),
            Ok(Object::Reference(id)) => lopdf_doc.get_object(*id).expect("mb ref").as_array().expect("mb arr").clone(),
            _ => panic!("no MediaBox"),
        };
        let to_f = |o: &Object| match o {
            Object::Real(r) => *r as f64,
            Object::Integer(i) => *i as f64,
            _ => 0.0,
        };
        to_f(&mediabox[3]) // y1
    };
    let scale = meta.render_dpi / 72.0; // canvas_px per pdf_pt
    println!("page height (mediabox y1): {pdf_page_height:.2}");

    println!(
        "\nparser brick count: {}  ({} PDF blocks)",
        placements.len(),
        blocks.len()
    );
    println!(
        "meta: render_dpi={:.2} scale={:.4} px/pt  clip_x0={:.2} clip_y0={:.2}",
        meta.render_dpi, scale, clip_x0, clip_y0
    );
    println!(
        "\n== Parser bricks in placement order (with PDF-pt placement) =="
    );
    println!(
        "{:<6} {:<14} {:<14} {:>8} {:>8} {:>8} {:>8} {:>10} {:>10}",
        "idx", "name", "layer_type", "bbox_x", "bbox_y", "w", "h", "pdf_e", "pdf_f"
    );
    for (i, p) in placements.iter().enumerate() {
        // Convert canvas bottom-left → PDF y-up (e, f) for placement.
        // canvas-y of the brick's bottom edge:
        let canvas_y_bottom = (p.y + p.height) as f64;
        // pymu y-down at bottom edge:
        let pymu_y_bottom = canvas_y_bottom / scale + clip_y0;
        // PDF y-up:
        let pdf_f = pdf_page_height - pymu_y_bottom;
        // PDF x:
        let pdf_e = p.x as f64 / scale + clip_x0;
        println!(
            "{:<6} {:<14} {:<14} {:>8} {:>8} {:>8} {:>8} {:>10.2} {:>10.2}",
            i, p.name, p.layer_type, p.x, p.y, p.width, p.height, pdf_e, pdf_f
        );
    }

    // ── Nearest-neighbour matcher ─────────────────────────────────
    // For each parser brick (in document order), compute its PDF (e, f)
    // and find the PDF block whose (place_x, place_y) is closest. Report
    // residual distance. Then dump unmatched blocks.
    let parser_pts: Vec<(f64, f64)> = placements
        .iter()
        .map(|p| {
            let canvas_y_bottom = (p.y + p.height) as f64;
            let pymu_y_bottom = canvas_y_bottom / scale + clip_y0;
            let pdf_f = pdf_page_height - pymu_y_bottom;
            let pdf_e = p.x as f64 / scale + clip_x0;
            (pdf_e, pdf_f)
        })
        .collect();

    let block_pts: Vec<(f64, f64)> = blocks
        .iter()
        .map(|b| (b.inner_ctm_at_content.e, b.inner_ctm_at_content.f))
        .collect();

    // Per parser brick, nearest block.
    let mut block_used = vec![false; blocks.len()];
    println!("\n== Parser → PDF nearest match ==");
    println!(
        "{:<6} {:<14} {:>10} {:>10}   {:>10} {:>10}   {:>8}  {:<8} {:<10}",
        "idx", "name", "pdf_e", "pdf_f", "best_e", "best_f", "dist", "kind", "block_idx"
    );
    for (i, (pe, pf)) in parser_pts.iter().enumerate() {
        let mut best = (f64::INFINITY, usize::MAX);
        for (j, (be, bf)) in block_pts.iter().enumerate() {
            if block_used[j] {
                continue;
            }
            let d = ((pe - be).powi(2) + (pf - bf).powi(2)).sqrt();
            if d < best.0 {
                best = (d, j);
            }
        }
        if best.1 != usize::MAX {
            block_used[best.1] = true;
            let (be, bf) = block_pts[best.1];
            let kind = match &blocks[best.1].content {
                hp_core::ocg_inject::BrickContent::Image { .. } => "Image",
                hp_core::ocg_inject::BrickContent::Form { .. } => "Form",
                hp_core::ocg_inject::BrickContent::Inlined => "Inlined",
            };
            println!(
                "{:<6} {:<14} {:>10.2} {:>10.2}   {:>10.2} {:>10.2}   {:>8.2}  {:<8} {:<10}",
                i, placements[i].name, pe, pf, be, bf, best.0, kind, best.1
            );
        }
    }

    println!("\n== Unmatched PDF blocks (extras) ==");
    for (j, used) in block_used.iter().enumerate() {
        if !*used {
            let kind = match &blocks[j].content {
                hp_core::ocg_inject::BrickContent::Image { name, .. } => format!("Image {name}"),
                hp_core::ocg_inject::BrickContent::Form { name, .. } => format!("Form {name}"),
                hp_core::ocg_inject::BrickContent::Inlined => "Inlined".to_string(),
            };
            let (e, f) = block_pts[j];
            println!("  block {j:3}  {kind:<14}  ({e:.2}, {f:.2})");
        }
    }

    // ── Reverse match: are extras shadow/highlight overlays? ─────
    // For each "extra" PDF block, find the nearest parser brick
    // (without exclusion). If the distance is small, the extra is
    // likely an overlay of that brick. The first matcher used
    // canvas-bottom-left for the parser brick; here we also probe
    // bottom-LEFT, top-LEFT, and CENTROID to see which alignment
    // works for these overlay blocks.
    println!("\n== Extras → nearest parser brick (anchor probing) ==");
    println!(
        "{:<11} {:>10} {:>10}   {:<14} {:>8} {:>8} {:>8}",
        "block", "place_e", "place_f", "nearest", "d_bl", "d_tl", "d_cent"
    );
    let mut extras_min_dist: Vec<f64> = Vec::new();
    for (j, used) in block_used.iter().enumerate() {
        if *used {
            continue;
        }
        let (be, bf) = block_pts[j];
        let mut best = (f64::INFINITY, usize::MAX, 0.0, 0.0, 0.0);
        for (i, p) in placements.iter().enumerate() {
            let pdf_e_bl = p.x as f64 / scale + clip_x0;
            let pymu_y_bot = (p.y + p.height) as f64 / scale + clip_y0;
            let pdf_f_bl = pdf_page_height - pymu_y_bot;
            let pymu_y_top = p.y as f64 / scale + clip_y0;
            let pdf_f_tl = pdf_page_height - pymu_y_top;
            let pdf_e_c = (p.x as f64 + p.width as f64 / 2.0) / scale + clip_x0;
            let pymu_y_c = (p.y as f64 + p.height as f64 / 2.0) / scale + clip_y0;
            let pdf_f_c = pdf_page_height - pymu_y_c;

            let d_bl = ((be - pdf_e_bl).powi(2) + (bf - pdf_f_bl).powi(2)).sqrt();
            let d_tl = ((be - pdf_e_bl).powi(2) + (bf - pdf_f_tl).powi(2)).sqrt();
            let d_c = ((be - pdf_e_c).powi(2) + (bf - pdf_f_c).powi(2)).sqrt();
            let d_min = d_bl.min(d_tl).min(d_c);
            if d_min < best.0 {
                best = (d_min, i, d_bl, d_tl, d_c);
            }
        }
        let nearest = if best.1 != usize::MAX {
            &placements[best.1].name
        } else {
            "(none)"
        };
        let kind = blocks[j].content.kind_str();
        println!(
            "{:<5} {:<5} {:>10.2} {:>10.2}   {:<14} {:>8.2} {:>8.2} {:>8.2}",
            j, kind, be, bf, nearest, best.2, best.3, best.4
        );
        extras_min_dist.push(best.0);
    }

    // ── One-line summary (for multi-file runs) ────────────────────
    let extras_total = extras_min_dist.len();
    let extras_within_25 = extras_min_dist.iter().filter(|&&d| d < 25.0).count();
    let extras_within_70 = extras_min_dist.iter().filter(|&&d| d < 70.0).count();
    let extras_max = extras_min_dist.iter().cloned().fold(0.0_f64, f64::max);
    println!(
        "\nSUMMARY {path} pdf_blocks={pb} image={img} form={form} inlined={inl} parser={parser} extras={ext} ext_under25={u25} ext_under70={u70} ext_max_dist={max:.1}",
        path = path,
        pb = blocks.len(),
        img = n_image,
        form = n_form,
        inl = n_inl,
        parser = placements.len(),
        ext = extras_total,
        u25 = extras_within_25,
        u70 = extras_within_70,
        max = extras_max,
    );
}

