//! PDF page content-stream walker + Form-XObject vertex extraction.
//!
//! Foundation for the per-piece export-render pipeline: identifies the
//! `q…Q` blocks inside the page's `bricks` OCG region, captures each
//! block's CTM and Form-XObject (or inlined-path) content, and gives
//! callers enough information to match each block to one of the parser's
//! `BrickPlacement`s.
//!
//! Today this module is read-only — no PDF rewriting. The downstream
//! work (OCG marker injection) will use the same `BrickBlock` records
//! plus a parser→block matcher to know where to insert markers.

use anyhow::{anyhow, Context, Result};
use lopdf::{Dictionary, Document, Object, ObjectId, StringFormat};
use std::collections::BTreeMap;

/// 2×3 affine transform — PDF's `cm` operator stack composes these.
/// Stored row-major as `[a b c d e f]`, applied to a point `(x, y)` as
/// `(x·a + y·c + e, x·b + y·d + f)`.
#[derive(Debug, Clone, Copy)]
pub struct Affine {
    pub a: f64,
    pub b: f64,
    pub c: f64,
    pub d: f64,
    pub e: f64,
    pub f: f64,
}

impl Affine {
    pub const IDENTITY: Affine = Affine {
        a: 1.0,
        b: 0.0,
        c: 0.0,
        d: 1.0,
        e: 0.0,
        f: 0.0,
    };

    /// PDF semantics: `new_ctm = cm × old_ctm`.
    pub fn concat(self, cm: Affine) -> Affine {
        Affine {
            a: cm.a * self.a + cm.b * self.c,
            b: cm.a * self.b + cm.b * self.d,
            c: cm.c * self.a + cm.d * self.c,
            d: cm.c * self.b + cm.d * self.d,
            e: cm.e * self.a + cm.f * self.c + self.e,
            f: cm.e * self.b + cm.f * self.d + self.f,
        }
    }

    pub fn transform(self, x: f64, y: f64) -> (f64, f64) {
        (
            x * self.a + y * self.c + self.e,
            x * self.b + y * self.d + self.f,
        )
    }
}

/// What's drawn inside one brick `q…Q` block.
///
/// AI files in practice contain three brick shapes:
///   • An `Image` XObject placed via `Do`. This is the common case —
///     Illustrator rasterised the brick (with its 3-D effect baked into
///     the alpha SMask) and embedded it as a raster.
///   • A `Form` XObject placed via `Do`. Self-contained vector
///     sub-stream — rarer in our fixtures but possible for bricks whose
///     effect Illustrator kept as live vector.
///   • `Inlined` paths drawn straight onto the page, no XObject
///     indirection. Pure vector + gradient/solid fill.
///
/// All three are addressable via the same OCG-injection trick (wrap the
/// containing `q…Q` block in BDC/EMC markers).
#[derive(Debug, Clone)]
pub enum BrickContent {
    Form {
        name: String,
        object_id: ObjectId,
    },
    Image {
        name: String,
        object_id: ObjectId,
    },
    Inlined,
}

impl BrickContent {
    pub fn kind_str(&self) -> &'static str {
        match self {
            BrickContent::Form { .. } => "Form",
            BrickContent::Image { .. } => "Image",
            BrickContent::Inlined => "Inlined",
        }
    }
}

/// One identified brick block in a page's content stream.
#[derive(Debug, Clone)]
pub struct BrickBlock {
    /// Index of the opening `q` operator in `Content::operations`.
    pub q_idx: usize,
    /// Index of the matching `Q` operator (inclusive).
    pub end_q_idx: usize,
    /// CTM at the moment `q` opened — page-level CTM × any prior `cm`s.
    pub outer_ctm: Affine,
    /// CTM accumulated *inside* the block up to the point of `Do` (for
    /// Form blocks) — used to transform Form-internal vertices into
    /// page coords. For inlined blocks, this is the CTM at each path
    /// construction op; we capture the value at first path op for now.
    pub inner_ctm_at_content: Affine,
    pub content: BrickContent,
    /// Mean of all path-vertex coordinates encountered while the block
    /// was open, in raw PDF page coords (y-up). `None` if no path-
    /// construction operators ran inside the block (rare — typically
    /// implies a content-less block we can't position).
    pub path_centroid: Option<(f64, f64)>,
    /// Bounding box of ON-CURVE path endpoints (skip cubic control
    /// points), in PDF page coords (y-up). For Adobe-AI raster bricks
    /// the path-construction ops inside `q…Q` define the clip path
    /// that masks the bleed-padded image — i.e. the brick's geometric
    /// outline. So this bbox is the brick's geometric position in PDF
    /// page space, comparable directly to `placement.polygon` bbox in
    /// pymu space (after the constant bleed shift). Used for sub-pixel
    /// bleed detection, free of alpha-bleed asymmetry bias.
    /// Layout: `(min_x, min_y, max_x, max_y)`.
    pub path_endpoint_bbox: Option<(f64, f64, f64, f64)>,
    /// If the q…Q crossed an OCG boundary (e.g. /OC /bricks EMC'd
    /// inside the block and then /OC /lights BDC pushed before Q), the
    /// indices of those BDC/EMC operators so the rewrite layer can
    /// emit a split `hp_p_NNNN` wrap (one BDC pair per parent OCG
    /// scope) instead of one improperly-nested pair.
    /// Layout: `(emc_pop_idx, bdc_push_idx)` — the EMC that popped the
    /// bricks scope, and the next BDC that pushed a new scope before Q.
    pub straddle_split: Option<(usize, usize)>,
    /// Innermost non-"bricks" OCG layer name active at the q…Q open.
    /// `None` if no layer-specific BDC was active (e.g. inline-but-
    /// inside-bricks content). When present this is the AUTHORITATIVE
    /// brick assignment — bypasses geometric matching entirely.
    pub layer_ocg_name: Option<String>,
}

/// Walk page 0's content stream and return every brick `q…Q` block we
/// find inside the `bricks` OCG region.
pub fn walk_page_bricks(doc: &Document, page_id: ObjectId) -> Result<Vec<BrickBlock>> {
    // Decode the page's combined content stream.
    let content = doc
        .get_and_decode_page_content(page_id)
        .context("decoding page content stream")?;

    // Resolve property names to OCG layer names so we can recognise
    // the bricks region precisely (not just any /OC BDC).
    let prop_to_ocg_name = build_property_to_ocg_name_map(doc, page_id)?;

    let mut blocks = Vec::new();

    let mut ctm_stack: Vec<Affine> = vec![Affine::IDENTITY];
    // For each active BDC: (is_bricks_ocg, layer_ocg_name).
    // `layer_ocg_name` is None unless the BDC referenced an OCG whose
    // name matches a parser placement ("Layer N").
    let mut bdc_stack: Vec<(bool, Option<String>)> = Vec::new();
    let mut in_bricks_depth = 0_u32;

    // Open-block state: when we hit a `q` at the top level inside the
    // bricks region, we open a candidate block here and close it on
    // the matching `Q`.
    let mut open: Option<BrickBlock> = None;
    let mut open_q_depth_at_start: u32 = 0;
    // BDC nesting state recorded when the block opened. If it differs
    // at Q close, the block straddles an OCG boundary and gets
    // recorded with `straddle_split` so injection can split-wrap.
    let mut open_bdc_stack_at_start: Vec<(bool, Option<String>)> = Vec::new();
    // First EMC/BDC operator indices observed inside the open block —
    // used for the split-wrap rewrite. We only need the first pair;
    // multi-segment straddles are rare and currently unsupported.
    let mut open_emc_pop_idx: Option<usize> = None;
    let mut open_bdc_push_idx: Option<usize> = None;
    // Path-op vertex accumulator (PDF page coords). On Q close we take
    // the mean → block centroid.
    let mut open_vertices: Vec<(f64, f64)> = Vec::new();
    // Separate ON-CURVE endpoint accumulator: skip cubic control
    // points so the bbox reflects the geometric outline. Used for
    // sub-pixel bleed detection.
    let mut open_endpoints: Vec<(f64, f64)> = Vec::new();

    // Apply the current CTM to (x, y) and push onto the open block's
    // vertex accumulator. Path ops give coords in the local frame; the
    // CTM maps them to PDF page space.
    let push_xy = |x: f64, y: f64, ctm: &Affine, verts: &mut Vec<(f64, f64)>| {
        let px = ctm.a * x + ctm.c * y + ctm.e;
        let py = ctm.b * x + ctm.d * y + ctm.f;
        verts.push((px, py));
    };

    for (idx, op) in content.operations.iter().enumerate() {
        let name = op.operator.as_str();

        match name {
            "q" => {
                let prev = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                ctm_stack.push(prev);
                if in_bricks_depth > 0 && open.is_none() {
                    // Innermost layer-OCG name = the topmost non-bricks
                    // entry on the BDC stack (search top-down).
                    let layer_ocg_name = bdc_stack
                        .iter()
                        .rev()
                        .find_map(|(_, name)| name.clone());
                    open = Some(BrickBlock {
                        q_idx: idx,
                        end_q_idx: idx, // overwritten on close
                        outer_ctm: prev,
                        inner_ctm_at_content: prev,
                        content: BrickContent::Inlined,
                        path_centroid: None,
                        path_endpoint_bbox: None,
                        straddle_split: None,
                        layer_ocg_name,
                    });
                    open_q_depth_at_start = ctm_stack.len() as u32 - 1;
                    open_bdc_stack_at_start = bdc_stack.clone();
                    open_emc_pop_idx = None;
                    open_bdc_push_idx = None;
                    open_vertices.clear();
                    open_endpoints.clear();
                }
            }
            "Q" => {
                if ctm_stack.len() > 1 {
                    ctm_stack.pop();
                }
                if let Some(mut block) = open.take() {
                    if (ctm_stack.len() as u32) == open_q_depth_at_start {
                        block.end_q_idx = idx;
                        // Centroid: prefer path-vertex mean for inlined/
                        // vector blocks. For Form/Image blocks (raster
                        // bricks drawn via `Do`), the inner CTM maps the
                        // unit square [0,1]² onto the page rect — so the
                        // centroid is CTM(0.5, 0.5).
                        if !open_vertices.is_empty() {
                            let n = open_vertices.len() as f64;
                            let (sx, sy) = open_vertices.iter().fold((0.0_f64, 0.0_f64),
                                |(ax, ay), (x, y)| (ax + x, ay + y));
                            block.path_centroid = Some((sx / n, sy / n));
                        } else if matches!(block.content,
                            BrickContent::Form { .. } | BrickContent::Image { .. })
                        {
                            let c = &block.inner_ctm_at_content;
                            let cx = 0.5 * c.a + 0.5 * c.c + c.e;
                            let cy = 0.5 * c.b + 0.5 * c.d + c.f;
                            block.path_centroid = Some((cx, cy));
                        }
                        // On-curve endpoint bbox: geometric outline of
                        // the brick (the clip path that masks the bleed
                        // out of the rendered raster). Used for precise
                        // bleed detection.
                        if !open_endpoints.is_empty() {
                            let (mut min_x, mut max_x) = (f64::INFINITY, f64::NEG_INFINITY);
                            let (mut min_y, mut max_y) = (f64::INFINITY, f64::NEG_INFINITY);
                            for (x, y) in &open_endpoints {
                                if *x < min_x { min_x = *x; }
                                if *x > max_x { max_x = *x; }
                                if *y < min_y { min_y = *y; }
                                if *y > max_y { max_y = *y; }
                            }
                            block.path_endpoint_bbox = Some((min_x, min_y, max_x, max_y));
                        }
                        // Straddle split if BDC nesting changed and we
                        // saw exactly one EMC + one BDC inside.
                        if bdc_stack != open_bdc_stack_at_start {
                            if let (Some(e), Some(b)) = (open_emc_pop_idx, open_bdc_push_idx) {
                                block.straddle_split = Some((e, b));
                            }
                        }
                        blocks.push(block);
                    } else {
                        // Nested Q didn't close our block yet — put it back.
                        open = Some(block);
                    }
                }
            }
            "cm" => {
                let nums = read_six_numbers(&op.operands);
                if let Some([a, b, c, d, e, f]) = nums {
                    let cm = Affine { a, b, c, d, e, f };
                    if let Some(top) = ctm_stack.last_mut() {
                        *top = top.concat(cm);
                    }
                    if let Some(block) = open.as_mut() {
                        block.inner_ctm_at_content = *ctm_stack.last().unwrap();
                    }
                }
            }
            "BDC" => {
                let is_oc = matches!(op.operands.first(), Some(Object::Name(n)) if n == b"OC");
                let (is_bricks, layer_name) = if is_oc {
                    match op.operands.get(1) {
                        Some(Object::Name(prop_name)) => {
                            let prop = String::from_utf8_lossy(prop_name).to_string();
                            let ocg_name = prop_to_ocg_name.get(&prop).cloned();
                            let is_bricks = ocg_name
                                .as_deref()
                                .map(|s| s.eq_ignore_ascii_case("bricks"))
                                .unwrap_or(false);
                            // Layer name: any non-"bricks" OCG (Layer N,
                            // lights, etc.). The matcher only cares
                            // about "Layer N" entries.
                            let layer_name = match ocg_name.as_deref() {
                                Some(s) if !s.eq_ignore_ascii_case("bricks") => {
                                    Some(s.to_string())
                                }
                                _ => None,
                            };
                            (is_bricks, layer_name)
                        }
                        _ => (false, None),
                    }
                } else {
                    (false, None)
                };
                bdc_stack.push((is_bricks, layer_name));
                if is_bricks {
                    in_bricks_depth += 1;
                }
                if open.is_some() && open_bdc_push_idx.is_none() && open_emc_pop_idx.is_some() {
                    open_bdc_push_idx = Some(idx);
                }
            }
            "EMC" => {
                if let Some((was_bricks, _name)) = bdc_stack.pop() {
                    if was_bricks && in_bricks_depth > 0 {
                        in_bricks_depth -= 1;
                    }
                }
                if open.is_some() && open_emc_pop_idx.is_none() {
                    open_emc_pop_idx = Some(idx);
                }
            }
            "Do" => {
                if let Some(block) = open.as_mut() {
                    if let Some(Object::Name(form_name)) = op.operands.first() {
                        let name = String::from_utf8_lossy(form_name).to_string();
                        if let Ok(object_id) = resolve_xobject_name(doc, page_id, &name) {
                            let is_image = doc
                                .get_object(object_id)
                                .ok()
                                .and_then(|o| o.as_stream().ok())
                                .and_then(|s| s.dict.get(b"Subtype").ok().cloned())
                                .map(|st| matches!(&st, Object::Name(n) if n == b"Image"))
                                .unwrap_or(false);
                            block.content = if is_image {
                                BrickContent::Image { name, object_id }
                            } else {
                                BrickContent::Form { name, object_id }
                            };
                            block.inner_ctm_at_content =
                                *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        }
                    }
                }
            }
            // ── Path-construction ops: accumulate vertices into the
            //    open block's centroid bucket (transformed to PDF page
            //    coords). PDF spec §8.5.2.
            "m" | "l" => {
                if open.is_some() {
                    if let Some([x, y]) = read_two_numbers(&op.operands) {
                        let ctm = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        push_xy(x, y, &ctm, &mut open_vertices);
                        push_xy(x, y, &ctm, &mut open_endpoints);
                    }
                }
            }
            "c" => {
                // 6 operands: x1 y1 x2 y2 x3 y3 — three control points.
                // x1,y1 and x2,y2 are off-curve control points; only x3,y3
                // is on the curve. We push all three into `open_vertices`
                // (for the centroid-matcher fallback) but ONLY x3,y3 into
                // `open_endpoints` (for the geometric bbox).
                if open.is_some() {
                    if let Some([x1, y1, x2, y2, x3, y3]) = read_six_numbers(&op.operands) {
                        let ctm = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        push_xy(x1, y1, &ctm, &mut open_vertices);
                        push_xy(x2, y2, &ctm, &mut open_vertices);
                        push_xy(x3, y3, &ctm, &mut open_vertices);
                        push_xy(x3, y3, &ctm, &mut open_endpoints);
                    }
                }
            }
            "v" | "y" => {
                // 4 operands. `v`: (x2 y2 x3 y3) — only x3,y3 on-curve.
                // `y`: (x1 y1 x3 y3) — only x3,y3 on-curve.
                if open.is_some() {
                    if let Some([a, b, c, d]) = read_four_numbers(&op.operands) {
                        let ctm = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        push_xy(a, b, &ctm, &mut open_vertices);
                        push_xy(c, d, &ctm, &mut open_vertices);
                        push_xy(c, d, &ctm, &mut open_endpoints);
                    }
                }
            }
            "re" => {
                // Rectangle: 4 operands x y w h. All four corners are
                // on-curve (axis-aligned linear segments).
                if open.is_some() {
                    if let Some([x, y, w, h]) = read_four_numbers(&op.operands) {
                        let ctm = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        push_xy(x, y, &ctm, &mut open_vertices);
                        push_xy(x + w, y, &ctm, &mut open_vertices);
                        push_xy(x + w, y + h, &ctm, &mut open_vertices);
                        push_xy(x, y + h, &ctm, &mut open_vertices);
                        push_xy(x, y, &ctm, &mut open_endpoints);
                        push_xy(x + w, y, &ctm, &mut open_endpoints);
                        push_xy(x + w, y + h, &ctm, &mut open_endpoints);
                        push_xy(x, y + h, &ctm, &mut open_endpoints);
                    }
                }
            }
            _ => {}
        }
    }

    Ok(blocks)
}


// ── Brick → PDF blocks mapping ──────────────────────────────────
//
// The export pipeline needs to know which PDF blocks make up each
// parser brick so it can wrap them in a per-brick OCG. We saw across
// all NY fixtures that:
//
//   1. Document-order alignment between parser bricks and PDF blocks
//      is ~exact (early matches in NY1 have Y residual < 1 pt and X
//      residual ≈ the 60-pt alpha-bleed offset).
//   2. Each parser brick may have 1+ associated PDF blocks — the main
//      raster placement, plus optional highlight/shadow overlays
//      placed at the same brick (within ~70 PDF pt by some anchor).
//   3. Some PDF blocks have no parser-brick correspondence — pure
//      decorations (ground shadows, frame ornaments, …) that share
//      the `bricks` OCG but aren't bricks.
//
// The matcher below codifies that:
//   • First pass: greedy 1:1 by document order. Walks PDF blocks and
//     parser bricks in lockstep, pairing each block to the next
//     unmatched parser brick whose Y is close enough (within 30 pt).
//   • Second pass: for each unmatched PDF block, attach to the nearest
//     parser brick by best of (bottom-left, top-left, centroid)
//     anchor — provided the residual is below `overlay_radius_pt`.
//   • Leftover unmatched blocks → decorations.

/// Geometry parameters needed to map a `BrickPlacement` (canvas px
/// coords from the parser) into the PDF-point space used by `BrickBlock`.
#[derive(Debug, Clone, Copy)]
pub struct PageGeometry {
    /// Min PDF y-down corner of the parser's clip rect (= `clip_x0`).
    pub clip_x0: f64,
    /// Min PDF y-down corner of the parser's clip rect (= `clip_y0`).
    pub clip_y0: f64,
    /// Render DPI the parser used to derive canvas dimensions.
    /// (canvas_px = pdf_pt × render_dpi / 72.)
    pub render_dpi: f64,
    /// The PDF page mediabox y1 (= page height in PDF points).
    pub page_height_pt: f64,
    /// Pymu → PDF-page X bleed in points. For AI exports the parser's
    /// pymu coords (derived from the AI's stored bbox metadata) and
    /// MuPDF's rendered position can differ by a constant X shift —
    /// the same one the load pipeline detects with `compute_pdf_offset`.
    /// `pdf_e = pymu_x_pt + bleed_x`. Set to 0 when there's no shift.
    pub bleed_x: f64,
    /// Pymu → PDF-page Y bleed in points (analogous to bleed_x; nearly
    /// always 0 in practice but plumbed through for symmetry).
    pub bleed_y: f64,
}

impl PageGeometry {
    fn scale(&self) -> f64 {
        self.render_dpi / 72.0
    }

    /// Project the placement's vector polygon into raw PDF page
    /// coords (the same frame block path centroids are in).
    /// Polygon vertices live in brick-local canvas-px coords (relative
    /// to `pymu_x/pymu_y`); we lift to page coords and apply the
    /// pymu→PDF X offset.
    pub fn placement_polygon_pdf(
        &self,
        brick: &crate::ai_parser::BrickPlacement,
    ) -> Option<Vec<(f64, f64)>> {
        let polygon = brick.polygon.as_ref()?;
        if polygon.len() < 3 { return None; }
        let s = self.scale();
        let bx = brick.pymu_x.max(0) as f64;
        let by = brick.pymu_y.max(0) as f64;
        let out: Vec<(f64, f64)> = polygon
            .iter()
            .map(|p| {
                let canvas_x = p[0] + bx;
                let canvas_y = p[1] + by;
                let pymu_x_pt = canvas_x / s + self.clip_x0;
                let pymu_y_pt = canvas_y / s + self.clip_y0;
                // Apply the pymu→PDF-page bleed so vertices end up in
                // the same coord frame as `BrickBlock::path_centroid`.
                let pdf_e = pymu_x_pt + self.bleed_x;
                let pdf_f = self.page_height_pt - (pymu_y_pt + self.bleed_y);
                (pdf_e, pdf_f)
            })
            .collect();
        Some(out)
    }

    /// Mean of placement polygon vertices in PDF page coords. Fast
    /// fallback when polygon-containment doesn't disambiguate (e.g.
    /// overlapping polygons, sparse paths).
    pub fn placement_polygon_centroid_pdf(
        &self,
        brick: &crate::ai_parser::BrickPlacement,
    ) -> Option<(f64, f64)> {
        let pts = self.placement_polygon_pdf(brick)?;
        let n = pts.len() as f64;
        let (sx, sy) = pts.iter().fold((0.0, 0.0), |(ax, ay), (x, y)| (ax + x, ay + y));
        Some((sx / n, sy / n))
    }
}

/// Result of the parser-brick ↔ PDF-block matcher.
#[derive(Debug, Clone)]
pub struct BrickBlockMap {
    /// `brick_to_blocks[i]` = PDF-block indices that belong to parser
    /// brick `i`. Typically 1 entry (main raster); 1+ if the brick has
    /// overlay rasters.
    pub brick_to_blocks: Vec<Vec<usize>>,
    /// PDF-block indices not attached to any parser brick — standalone
    /// decorations rendered inside the bricks OCG.
    pub decoration_blocks: Vec<usize>,
}

/// Match each PDF brick block to a parser brick (or to the decoration
/// bucket). Pure function — no I/O, no PDF mutation.
///
/// `overlay_radius_pt` controls how generous the second-pass attach is.
/// 70 pt accommodates the highlight/specular overlays we saw across NY
/// fixtures; tightening it pushes more blocks into the decoration set.
pub fn match_blocks_to_bricks(
    blocks: &[BrickBlock],
    placements: &[crate::ai_parser::BrickPlacement],
    geo: PageGeometry,
    overlay_radius_pt: f64,
) -> BrickBlockMap {
    // Pre-compute each placement's polygon and its centroid in raw
    // PDF page coords. We match blocks to placements by:
    //   (a) testing whether the block's path centroid sits inside the
    //       placement's polygon — geometric containment is the
    //       strongest possible signal for "this block belongs to that
    //       brick", and bricks rarely overlap so it's nearly unique
    //   (b) breaking ties (or filling in for missing polygons) by
    //       centroid-to-centroid distance.
    let polygons: Vec<Option<Vec<(f64, f64)>>> = placements
        .iter()
        .map(|p| geo.placement_polygon_pdf(p))
        .collect();
    let placement_centroids: Vec<Option<(f64, f64)>> = placements
        .iter()
        .map(|p| geo.placement_polygon_centroid_pdf(p))
        .collect();

    let in_poly = |x: f64, y: f64, poly: &[(f64, f64)]| -> bool {
        // Standard ray-cast inside test (PDF y-up doesn't change the
        // winding logic). Inline to avoid coupling to render.rs's
        // [[f64;2]] signature.
        let mut inside = false;
        let mut j = poly.len() - 1;
        for i in 0..poly.len() {
            let (xi, yi) = poly[i];
            let (xj, yj) = poly[j];
            if ((yi > y) != (yj > y))
                && (x < (xj - xi) * (y - yi) / (yj - yi + f64::EPSILON) + xi)
            {
                inside = !inside;
            }
            j = i;
        }
        inside
    };

    let mut brick_to_blocks: Vec<Vec<usize>> = vec![Vec::new(); placements.len()];
    let mut block_matched = vec![false; blocks.len()];
    let mut brick_used = vec![false; placements.len()];

    // ── Pass 0: AUTHORITATIVE direct match by Illustrator's own data ──
    // Each AI layer's `ai_rasters` is the list of `Xh` raster placements
    // Illustrator recorded under that layer (one per raster sub-object;
    // e.g. a porthole's 4 frame quarters give 4 entries). The same
    // images appear in the PDF content stream as Image XObjects called
    // via `/ImN Do`, with their position set by the surrounding `cm`s.
    // The PDF's CTM at the Do (`block.inner_ctm_at_content.e/.f`)
    // equals the AI tx/ty plus a SINGLE constant translation that
    // applies to every raster in the file (the AI→PDF reference shift).
    //
    // We find that translation empirically: for every (placement,
    // raster) and (block, ctm) pair we can compute a candidate `Δ`;
    // the right `Δ` is the one shared by most pairs. Then a second
    // pass uses `Δ` to pair every Image block with its AI raster.
    //
    // This sidesteps polygon-overlap matching entirely for raster
    // bricks — which is what Illustrator's own bookkeeping says is
    // correct, regardless of how the polygon happens to be shaped.
    let placement_idx_by_name: std::collections::HashMap<&str, usize> = placements
        .iter()
        .enumerate()
        .map(|(i, p)| (p.name.as_str(), i))
        .collect();
    let _ = placement_idx_by_name; // kept; might be useful later

    // 1. Pick the AI→PDF translation by finding a (raster, block) pair
    //    whose `(a, d, image_size)` signature is UNIQUE on both sides —
    //    a 1-of-1 match anchors `Δ` unambiguously. Voting across all
    //    pairs picks the wrong mode when many rasters share the same
    //    nearest-neighbour spacing (e.g. all the porthole quarters in
    //    one file vote for an "off-by-one-image" Δ).
    {
        // Helper: derive PDF-pt image size from an AI raster.
        let raster_size_pt = |r: &crate::ai_parser::AiRasterPlacement|
            (r.a.abs() * r.img_w, r.d.abs() * r.img_h);
        // Helper: PDF-pt image size from block ctm (assume axis-aligned).
        let block_size_pt = |b: &BrickBlock|
            (b.inner_ctm_at_content.a.abs(), b.inner_ctm_at_content.d.abs());

        // Bin to 0.1-pt buckets for size lookup so float noise doesn't
        // split otherwise-identical sizes into separate keys.
        let size_key = |w: f64, h: f64| -> (i64, i64) {
            ((w * 10.0).round() as i64, (h * 10.0).round() as i64)
        };
        let mut raster_by_size: std::collections::HashMap<(i64, i64), Vec<(usize, usize)>> =
            std::collections::HashMap::new(); // (placement_idx, raster_idx)
        for (i, p) in placements.iter().enumerate() {
            for (k, r) in p.ai_rasters.iter().enumerate() {
                let (w, h) = raster_size_pt(r);
                raster_by_size.entry(size_key(w, h)).or_default().push((i, k));
            }
        }
        let mut blocks_by_size: std::collections::HashMap<(i64, i64), Vec<usize>> =
            std::collections::HashMap::new();
        for (j, b) in blocks.iter().enumerate() {
            if !matches!(b.content, BrickContent::Image { .. }) { continue; }
            let (w, h) = block_size_pt(b);
            blocks_by_size.entry(size_key(w, h)).or_default().push(j);
        }

        // Convert AI raster → PDF bottom-left in PDF user-space.
        // AI `ty` is the TOP edge of the image in AI Y-up (this is what
        // the AI parser already assumes for pymu conversion). PDF `f`
        // in an Image XObject's CTM is the BOTTOM edge in PDF Y-up.
        // So for `d > 0`: PDF_f = (AI_ty - d*img_h) + Δy_bottom.
        // The same Δ applies file-wide.
        let raster_to_pdf_bl = |r: &crate::ai_parser::AiRasterPlacement| -> (f64, f64) {
            // AI bottom-left Y in AI Y-up:
            let ai_bl_y = r.ty - r.d.abs() * r.img_h;
            (r.tx, ai_bl_y)
        };

        // Find a size where exactly one raster and one block exist.
        let mut delta: Option<(f64, f64)> = None;
        // Prefer anchors with a distinctive size — sort sizes by
        // (uniqueness, area) descending so we deterministically pick
        // the LARGEST unique-size raster (less likely to be a sliver
        // matching at multiple aligns).
        let mut size_keys: Vec<(i64, i64)> = raster_by_size.keys().copied().collect();
        size_keys.sort_by(|a, b| (b.0 * b.1).cmp(&(a.0 * a.1)));
        for k in size_keys {
            let rs = &raster_by_size[&k];
            if rs.len() != 1 { continue; }
            let bs = match blocks_by_size.get(&k) {
                Some(v) if v.len() == 1 => v,
                _ => continue,
            };
            let (pi, ri) = rs[0];
            let r = &placements[pi].ai_rasters[ri];
            let b = &blocks[bs[0]];
            let (ai_x, ai_bl_y) = raster_to_pdf_bl(r);
            let dx = b.inner_ctm_at_content.e - ai_x;
            let dy = b.inner_ctm_at_content.f - ai_bl_y;
            eprintln!("[ocg_inject] anchor: '{}' raster #{} ({}×{} pt) → block #{} → Δ=({:.3}, {:.3})",
                placements[pi].name, ri, k.0 as f64 / 10.0, k.1 as f64 / 10.0, bs[0], dx, dy);
            delta = Some((dx, dy));
            break;
        }

        if delta.is_none() {
            eprintln!("[ocg_inject] WARNING: no unique-size anchor; falling back to mode-of-deltas");
            let mut votes: std::collections::HashMap<(i64, i64), u32> =
                std::collections::HashMap::new();
            for block in blocks.iter() {
                if !matches!(block.content, BrickContent::Image { .. }) { continue; }
                for p in placements.iter() {
                    for r in &p.ai_rasters {
                        let (ai_x, ai_bl_y) = raster_to_pdf_bl(r);
                        let dx = block.inner_ctm_at_content.e - ai_x;
                        let dy = block.inner_ctm_at_content.f - ai_bl_y;
                        let key = ((dx * 100.0).round() as i64, (dy * 100.0).round() as i64);
                        *votes.entry(key).or_insert(0) += 1;
                    }
                }
            }
            if let Some((&(dx_b, dy_b), _)) = votes.iter().max_by_key(|(_, c)| *c) {
                delta = Some(((dx_b as f64) / 100.0, (dy_b as f64) / 100.0));
            }
        }

        if let Some((dx, dy)) = delta {
            eprintln!("[ocg_inject] AI→PDF translation = ({:.3}, {:.3})", dx, dy);
            let tol = 0.5_f64;
            for (j, block) in blocks.iter().enumerate() {
                if !matches!(block.content, BrickContent::Image { .. }) { continue; }
                let target_e = block.inner_ctm_at_content.e - dx;
                let target_f = block.inner_ctm_at_content.f - dy;
                let mut hit: Option<usize> = None;
                for (i, p) in placements.iter().enumerate() {
                    for r in &p.ai_rasters {
                        let (ai_x, ai_bl_y) = raster_to_pdf_bl(r);
                        if (ai_x - target_e).abs() < tol && (ai_bl_y - target_f).abs() < tol {
                            hit = Some(i);
                            break;
                        }
                    }
                    if hit.is_some() { break; }
                }
                if let Some(i) = hit {
                    brick_to_blocks[i].push(j);
                    block_matched[j] = true;
                    brick_used[i] = true;
                }
            }
        }
    }

    // Precompute placement polygon BBOXes in PDF coords for the
    // bbox-based fallback. Polygons can be highly non-convex (e.g.
    // Layer 319's path has two arc sections with a "hole" between
    // them) so strict ray-cast containment misses points the brick
    // clearly owns. The bbox is a safe envelope: every part of the
    // brick is inside the bbox by definition.
    let polygon_bboxes: Vec<Option<(f64, f64, f64, f64)>> = polygons
        .iter()
        .map(|opt| opt.as_ref().and_then(|poly| {
            if poly.is_empty() { return None; }
            let (mut xmin, mut xmax) = (f64::INFINITY, f64::NEG_INFINITY);
            let (mut ymin, mut ymax) = (f64::INFINITY, f64::NEG_INFINITY);
            for &(x, y) in poly {
                if x < xmin { xmin = x; }
                if x > xmax { xmax = x; }
                if y < ymin { ymin = y; }
                if y > ymax { ymax = y; }
            }
            Some((xmin, ymin, xmax, ymax))
        }))
        .collect();
    let in_bbox = |x: f64, y: f64, bb: &(f64, f64, f64, f64)| -> bool {
        x >= bb.0 && x <= bb.2 && y >= bb.1 && y <= bb.3
    };

    // ── First pass: 1:1 by polygon containment of the block centroid ──
    // For each block, find the placement whose polygon CONTAINS the
    // block's path-vertex centroid. With non-overlapping bricks this is
    // unambiguous. Walk blocks in document order; first-come-first-
    // served keeps the matching deterministic.
    for (j, block) in blocks.iter().enumerate() {
        if block_matched[j] { continue; }
        let Some(centroid) = block.path_centroid else { continue; };
        let (bx, by) = centroid;
        let mut hit: Option<usize> = None;
        let mut best_dist = f64::INFINITY;
        for (i, poly_opt) in polygons.iter().enumerate() {
            if brick_used[i] { continue; }
            let Some(poly) = poly_opt else { continue; };
            if !in_poly(bx, by, poly) { continue; }
            // If multiple polygons contain the centroid (overlapping
            // bricks, which the parser should have de-duped — but
            // doesn't always), tie-break by centroid-to-centroid dist.
            let d = if let Some((cx, cy)) = placement_centroids[i] {
                ((bx - cx).powi(2) + (by - cy).powi(2)).sqrt()
            } else { f64::INFINITY };
            if d < best_dist {
                best_dist = d;
                hit = Some(i);
            }
        }
        if let Some(i) = hit {
            brick_to_blocks[i].push(j);
            brick_used[i] = true;
            block_matched[j] = true;
        }
    }

    // ── Pass 1.5: AUTHORITATIVE bbox-equality match for non-raster
    // blocks. The brick's polygon in AI = the same drawing's path in
    // PDF (Illustrator wrote both files). So the PDF block's path-
    // endpoint bbox should match the AI placement's polygon bbox
    // within a few points (≈ rounding + closure-vertex differences).
    // This is the same idea as the Xh tx/ty matching for rasters —
    // compare authoritative path data, not heuristic centroids.
    //
    // We use sum-of-corner-distances as the score; threshold 4.0 pt
    // total (≈1 pt per corner) lets honest matches through but
    // rejects neighbour-brick polygons that happen to overlap in bbox.
    let bbox_dist = |a: &(f64, f64, f64, f64), b: &(f64, f64, f64, f64)| -> f64 {
        (a.0 - b.0).abs() + (a.1 - b.1).abs() + (a.2 - b.2).abs() + (a.3 - b.3).abs()
    };
    for (j, block) in blocks.iter().enumerate() {
        if block_matched[j] { continue; }
        let Some(bb) = block.path_endpoint_bbox else { continue; };
        let mut hit: Option<usize> = None;
        let mut best_score = f64::INFINITY;
        for (i, poly_bb) in polygon_bboxes.iter().enumerate() {
            if brick_used[i] { continue; }
            let Some(poly_bb) = poly_bb else { continue; };
            let score = bbox_dist(&bb, poly_bb);
            if score < best_score { best_score = score; hit = Some(i); }
        }
        // Threshold: 4 pt sum-of-corners covers honest rounding +
        // closing-vertex variance, but rejects neighbours offset by
        // an image-size step (which would give ~340 pt for an 85-pt
        // raster's bbox).
        if let (Some(i), true) = (hit, best_score < 4.0) {
            brick_to_blocks[i].push(j);
            brick_used[i] = true;
            block_matched[j] = true;
        }
    }

    // ── Pass 2: fallback for leftover blocks. Use polygon-bbox
    //     containment with centroid-distance tie-break — same as
    //     1.5 but allows already-used bricks (for overlay strokes
    //     etc. that legitimately share a brick's area).
    for (j, block) in blocks.iter().enumerate() {
        if block_matched[j] { continue; }
        let Some((bx, by)) = block.path_centroid else { continue; };
        let mut hit: Option<usize> = None;
        let mut best_dist = f64::INFINITY;
        for (i, bb_opt) in polygon_bboxes.iter().enumerate() {
            let Some(bb) = bb_opt else { continue; };
            if !in_bbox(bx, by, bb) { continue; }
            let d = if let Some((cx, cy)) = placement_centroids[i] {
                ((bx - cx).powi(2) + (by - cy).powi(2)).sqrt()
            } else { f64::INFINITY };
            if d < best_dist { best_dist = d; hit = Some(i); }
        }
        let _ = overlay_radius_pt; // kept for API compat; not used now
        if let Some(i) = hit {
            brick_to_blocks[i].push(j);
            block_matched[j] = true;
        }
    }

    let decoration_blocks: Vec<usize> = (0..blocks.len())
        .filter(|&j| !block_matched[j])
        .collect();

    BrickBlockMap {
        brick_to_blocks,
        decoration_blocks,
    }
}

/// Extract the first `n` distinct anchor-point vertices from a Form
/// XObject's content stream, transformed into PDF point coords on the
/// page. "Anchor points" = endpoints of `m`, `l`, `c`, `v`, `y`, `re`
/// (rectangle corners). Returns fewer than `n` if the Form's path is
/// shorter than that.
pub fn extract_form_anchor_vertices(
    doc: &Document,
    form_id: ObjectId,
    placement_ctm: Affine,
    n: usize,
) -> Result<Vec<(f64, f64)>> {
    let obj = doc.get_object(form_id).context("Form object lookup")?;
    let stream = obj.as_stream().context("Form must be a stream")?;
    let mut decoded = stream.clone();
    decoded.decompress();
    let content = lopdf::content::Content::decode(&decoded.content)
        .map_err(|e| anyhow!("Form content decode: {e}"))?;

    // Form XObjects can also have their own internal CTM (via cm), and
    // may have a /Matrix entry in the stream dict. PDF spec: the form's
    // /Matrix applies BEFORE its content stream's CTM.
    let mut ctm = placement_ctm;
    if let Ok(matrix) = stream.dict.get(b"Matrix").and_then(|m| m.as_array()) {
        if let Some([a, b, c, d, e, f]) = read_six_numbers_array(matrix) {
            ctm = ctm.concat(Affine { a, b, c, d, e, f });
        }
    }

    let mut ctm_stack: Vec<Affine> = vec![ctm];
    let mut out: Vec<(f64, f64)> = Vec::new();
    let push_xy = |x: f64, y: f64, ctm: Affine, out: &mut Vec<(f64, f64)>| {
        let (px, py) = ctm.transform(x, y);
        if out.last().map(|last| {
            (last.0 - px).abs() < 1e-3 && (last.1 - py).abs() < 1e-3
        }) != Some(true)
        {
            out.push((px, py));
        }
    };

    for op in &content.operations {
        if out.len() >= n {
            break;
        }
        let name = op.operator.as_str();
        match name {
            "q" => {
                let prev = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                ctm_stack.push(prev);
            }
            "Q" => {
                if ctm_stack.len() > 1 {
                    ctm_stack.pop();
                }
            }
            "cm" => {
                if let Some([a, b, c, d, e, f]) = read_six_numbers(&op.operands) {
                    let cm = Affine { a, b, c, d, e, f };
                    if let Some(top) = ctm_stack.last_mut() {
                        *top = top.concat(cm);
                    }
                }
            }
            "m" | "l" => {
                if let Some([x, y]) = read_two_numbers(&op.operands) {
                    let cur = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                    push_xy(x, y, cur, &mut out);
                }
            }
            "c" => {
                // x1 y1 x2 y2 x3 y3 — endpoint is x3 y3
                if let Some([_, _, _, _, x, y]) = read_six_numbers(&op.operands) {
                    let cur = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                    push_xy(x, y, cur, &mut out);
                }
            }
            "v" => {
                // current point used as control1, then x2 y2 x3 y3 — endpoint x3 y3
                if let Some([_, _, x, y]) = read_four_numbers(&op.operands) {
                    let cur = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                    push_xy(x, y, cur, &mut out);
                }
            }
            "y" => {
                // x1 y1 x3 y3 — endpoint x3 y3
                if let Some([_, _, x, y]) = read_four_numbers(&op.operands) {
                    let cur = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                    push_xy(x, y, cur, &mut out);
                }
            }
            "re" => {
                // rectangle: x y w h — emit corner points
                if let Some([x, y, w, h]) = read_four_numbers(&op.operands) {
                    let cur = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                    push_xy(x, y, cur, &mut out);
                    if out.len() >= n {
                        break;
                    }
                    push_xy(x + w, y, cur, &mut out);
                    if out.len() >= n {
                        break;
                    }
                    push_xy(x + w, y + h, cur, &mut out);
                    if out.len() >= n {
                        break;
                    }
                    push_xy(x, y + h, cur, &mut out);
                }
            }
            _ => {}
        }
    }

    Ok(out)
}

/// Walk the page's `/Resources /Properties` and resolve every property
/// name to an OCG layer name (i.e. the `/Name` field of an /OCG dict,
/// or — for an OCMC entry — the OCG it points to).
fn build_property_to_ocg_name_map(
    doc: &Document,
    page_id: ObjectId,
) -> Result<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    let page = doc.get_object(page_id)?.as_dict()?;
    let resources = match get_dict_via_inherit(doc, page, b"Resources") {
        Some(d) => d,
        None => return Ok(map),
    };
    let properties = match resources.get(b"Properties") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc.get_object(*id)?.as_dict()?.clone(),
        _ => return Ok(map),
    };
    for (k, v) in properties.iter() {
        let key = String::from_utf8_lossy(k).to_string();
        let dict = match v {
            Object::Dictionary(d) => d.clone(),
            Object::Reference(id) => match doc.get_object(*id) {
                Ok(o) => match o.as_dict() {
                    Ok(d) => d.clone(),
                    Err(_) => continue,
                },
                Err(_) => continue,
            },
            _ => continue,
        };
        if let Some(name) = resolve_property_to_ocg_name(doc, &dict) {
            map.insert(key, name);
        }
    }
    Ok(map)
}

/// For an OCG dict, return /Name; for an OCMC dict, follow the first
/// entry in /OCGs and return that OCG's /Name. Anything else → None.
fn resolve_property_to_ocg_name(doc: &Document, dict: &Dictionary) -> Option<String> {
    let type_name = dict.get(b"Type").ok().and_then(|t| match t {
        Object::Name(n) => Some(String::from_utf8_lossy(n).to_string()),
        _ => None,
    });
    match type_name.as_deref() {
        Some("OCG") => dict.get(b"Name").ok().and_then(|n| match n {
            Object::String(s, _) => Some(decode_pdf_string(s)),
            _ => None,
        }),
        Some("OCMD") => {
            let ocgs = dict.get(b"OCGs").ok()?;
            let arr = match ocgs {
                Object::Array(a) => a.clone(),
                Object::Reference(id) => doc.get_object(*id).ok()?.as_array().ok()?.clone(),
                _ => return None,
            };
            for entry in arr {
                if let Object::Reference(id) = entry {
                    if let Ok(obj) = doc.get_object(id) {
                        if let Ok(d) = obj.as_dict() {
                            if let Some(name) = resolve_property_to_ocg_name(doc, d) {
                                return Some(name);
                            }
                        }
                    }
                }
            }
            None
        }
        _ => None,
    }
}

fn resolve_xobject_name(
    doc: &Document,
    page_id: ObjectId,
    xobject_name: &str,
) -> Result<ObjectId> {
    let page = doc.get_object(page_id)?.as_dict()?;
    let resources = get_dict_via_inherit(doc, page, b"Resources")
        .ok_or_else(|| anyhow!("page has no /Resources"))?;
    let xobjects = match resources.get(b"XObject") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc.get_object(*id)?.as_dict()?.clone(),
        _ => return Err(anyhow!("no /XObject dict")),
    };
    match xobjects.get(xobject_name.as_bytes()) {
        Ok(Object::Reference(id)) => Ok(*id),
        Ok(_) => Err(anyhow!("XObject {xobject_name} not a reference")),
        Err(_) => Err(anyhow!("XObject {xobject_name} not found")),
    }
}

fn get_dict_via_inherit<'a>(
    _doc: &'a Document,
    dict: &'a Dictionary,
    key: &[u8],
) -> Option<Dictionary> {
    // For our AI files the page is a leaf with its own /Resources, so
    // we just look directly on the page. (Full /Parent walking can be
    // added if a fixture needs it.)
    match dict.get(key) {
        Ok(Object::Dictionary(d)) => Some(d.clone()),
        Ok(Object::Reference(id)) => _doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_dict().ok())
            .cloned(),
        _ => None,
    }
}

fn decode_pdf_string(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).to_string()
}

fn read_two_numbers(operands: &[Object]) -> Option<[f64; 2]> {
    let mut out = [0.0; 2];
    if operands.len() < 2 {
        return None;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = obj_to_f64(&operands[i])?;
    }
    Some(out)
}

fn read_four_numbers(operands: &[Object]) -> Option<[f64; 4]> {
    let mut out = [0.0; 4];
    if operands.len() < 4 {
        return None;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = obj_to_f64(&operands[i])?;
    }
    Some(out)
}

fn read_six_numbers(operands: &[Object]) -> Option<[f64; 6]> {
    let mut out = [0.0; 6];
    if operands.len() < 6 {
        return None;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = obj_to_f64(&operands[i])?;
    }
    Some(out)
}

fn read_six_numbers_array(arr: &[Object]) -> Option<[f64; 6]> {
    let mut out = [0.0; 6];
    if arr.len() < 6 {
        return None;
    }
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = obj_to_f64(&arr[i])?;
    }
    Some(out)
}

/// Resolve an Object to a `Vec<Object>` if it's either a direct Array
/// or a Reference pointing to one. Returns `None` for anything else.
/// Used when reading PDF entries that AI files sometimes write inline
/// and sometimes as indirect references.
fn read_array_resolving_ref(doc: &Document, obj: Option<&Object>) -> Option<Vec<Object>> {
    match obj? {
        Object::Array(a) => Some(a.clone()),
        Object::Reference(id) => doc
            .get_object(*id)
            .ok()
            .and_then(|o| o.as_array().ok())
            .map(|a| a.clone()),
        _ => None,
    }
}

fn obj_to_f64(o: &Object) -> Option<f64> {
    match o {
        Object::Real(r) => Some(*r as f64),
        Object::Integer(i) => Some(*i as f64),
        _ => None,
    }
}

// ── PDF rewriter: inject per-brick OCG markers ────────────────────
//
// Produces a modified PDF where every brick `q…Q` block is wrapped in
// `/OC /<prop> BDC … EMC` referring to a fresh per-brick OCG. The
// caller (typically the per-piece export renderer) then turns the new
// OCGs ON or OFF to control which bricks render. Default state: every
// new OCG is ON, so the modified PDF visually matches the original
// until a renderer mutates the OCG state.

/// Names of the OCGs minted by `inject_per_brick_ocgs`. `brick_ocgs[i]`
/// corresponds to the parser brick at index `i`; `decoration_ocg` is the
/// single shared OCG for blocks that didn't match any parser brick;
/// `inline_ocg` wraps drawing ops that live directly under `/OC /bricks`
/// but outside any q…Q block (the "leak" content — yellow window-pane
/// gradients that aren't part of any specific brick).
#[derive(Debug, Clone)]
pub struct InjectedOcgs {
    pub brick_ocgs: Vec<String>,
    pub decoration_ocg: String,
    pub inline_ocg: String,
    /// Global OCG covering every Image XObject block. Useful for
    /// rendering "all non-Image content of the whole house" in a
    /// single MuPDF call: enable everything else, leave this OFF.
    /// Image blocks then hide because PDF OCG visibility is the
    /// intersection of their enclosing OCG states (Image blocks live
    /// inside both their `hp_brick_NNNN` and `hp_image`; with
    /// `hp_image` off the block stays hidden).
    pub image_ocg: String,
}

/// Rewrite the PDF so each brick block is wrapped in a per-brick OCG.
///
/// Mutations on `doc`:
///   1. Add one new OCG object per parser brick + one for decorations.
///      Each is a `<< /Type /OCG /Name (...) >>` dict.
///   2. Append every new OCG reference to `/OCProperties /OCGs` and to
///      `/OCProperties /D /Order` so MuPDF surfaces them in its layer
///      config (the API we use to toggle them at render time).
///   3. Add one `/Properties` entry per PDF block on the page's
///      `/Resources` dict, mapping a fresh property name → the OCG ref
///      for whichever brick (or decoration) that block belongs to.
///   4. Rewrite page 0's content stream: insert `/<prop> /OC BDC` just
///      before each block's `q` and `EMC` just after its `Q`.
///
/// Returns the names of the new OCGs so callers can match them against
/// MuPDF's layer-config-UI enumeration when toggling visibility.
pub fn inject_per_brick_ocgs(
    doc: &mut Document,
    page_id: ObjectId,
    blocks: &[BrickBlock],
    map: &BrickBlockMap,
) -> Result<InjectedOcgs> {
    use lopdf::{content::Content, Stream};

    // 1. Build per-block → OCG-name assignment.
    let brick_ocg_names: Vec<String> = (0..map.brick_to_blocks.len())
        .map(|i| format!("hp_brick_{:04}", i))
        .collect();
    let decoration_ocg_name = "hp_decoration".to_string();
    let inline_ocg_name = "hp_bricks_inline".to_string();
    let image_ocg_name = "hp_image".to_string();

    let mut block_to_ocg: std::collections::HashMap<usize, String> =
        std::collections::HashMap::with_capacity(blocks.len());
    for (brick_idx, block_idxs) in map.brick_to_blocks.iter().enumerate() {
        for &bi in block_idxs {
            block_to_ocg.insert(bi, brick_ocg_names[brick_idx].clone());
        }
    }
    for &bi in &map.decoration_blocks {
        block_to_ocg.insert(bi, decoration_ocg_name.clone());
    }

    // 2. Create OCG objects. Each unique OCG name gets one PDF object.
    let mut ocg_object_id: std::collections::HashMap<String, ObjectId> =
        std::collections::HashMap::new();
    for name in brick_ocg_names
        .iter()
        .chain(std::iter::once(&decoration_ocg_name))
        .chain(std::iter::once(&inline_ocg_name))
        .chain(std::iter::once(&image_ocg_name))
    {
        let mut dict = Dictionary::new();
        dict.set("Type", Object::Name(b"OCG".to_vec()));
        dict.set("Name", Object::String(name.as_bytes().to_vec(), StringFormat::Literal));
        let id = doc.add_object(Object::Dictionary(dict));
        ocg_object_id.insert(name.clone(), id);
    }

    // 3. Add /Properties entries on the page's /Resources. Property
    //    names are stable per block index so the content-stream rewrite
    //    can reference them by name.
    let mut block_to_prop_name: std::collections::HashMap<usize, String> =
        std::collections::HashMap::with_capacity(block_to_ocg.len());
    {
        // /Resources may be inline on the page or referenced via /Resources <id>.
        // Read it (and any /Properties sub-dict) without holding a mutable borrow,
        // mutate locally, then write back inline.
        let page_dict = doc
            .get_object(page_id)
            .context("page lookup")?
            .as_dict()
            .context("page dict")?
            .clone();
        let mut resources_dict = match page_dict.get(b"Resources") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        let mut properties = match resources_dict.get(b"Properties") {
            Ok(Object::Dictionary(d)) => d.clone(),
            Ok(Object::Reference(id)) => doc
                .get_object(*id)
                .ok()
                .and_then(|o| o.as_dict().ok())
                .cloned()
                .unwrap_or_default(),
            _ => Dictionary::new(),
        };
        for (block_idx, ocg_name) in &block_to_ocg {
            let prop_name = format!("hp_p_{:04}", block_idx);
            let oid = ocg_object_id[ocg_name];
            properties.set(prop_name.as_str(), Object::Reference(oid));
            block_to_prop_name.insert(*block_idx, prop_name);
        }
        // Add a stable property name for the inline OCG. Used by every
        // injected inline-wrapper BDC.
        properties.set(
            "hp_p_inline",
            Object::Reference(ocg_object_id[&inline_ocg_name]),
        );
        // Property name for the global Image OCG (one BDC reference
        // around every Image block).
        properties.set(
            "hp_p_image",
            Object::Reference(ocg_object_id[&image_ocg_name]),
        );
        resources_dict.set("Properties", Object::Dictionary(properties));
        let page = doc
            .get_object_mut(page_id)
            .context("page lookup #2")?
            .as_dict_mut()
            .context("page dict #2")?;
        page.set("Resources", Object::Dictionary(resources_dict));
    }

    // 4. Rewrite the page content stream. We walk the decoded operations
    //    and emit a fresh stream with BDC/EMC injected at block boundaries.
    //    We ALSO wrap content that sits directly inside /OC /bricks but
    //    outside any q…Q brick block — these inline draw ops are the
    //    "leak" (yellow window-pane gradients etc.) that previously
    //    rendered uncontrollably whenever /OC /bricks was on. Wrapping
    //    them in /OC /hp_bricks_inline lets piece renders disable them.
    let content = doc
        .get_and_decode_page_content(page_id)
        .context("decode page content")?;
    // Fast lookup: op-index → block-index for openers and closers.
    let mut q_at: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::with_capacity(blocks.len());
    let mut q_end_at: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::with_capacity(blocks.len());
    // For straddling blocks, additional split-wrap brackets. The EMC
    // that pops the bricks BDC inside the q…Q needs an EMC(hp_p_X)
    // INSERTED RIGHT BEFORE it (so hp_p_X closes within the bricks
    // scope). The BDC that pushes the next scope (lights) needs a
    // BDC(hp_p_X) INSERTED RIGHT AFTER it (so hp_p_X reopens within
    // the lights scope, fully nested). Without that split, our
    // hp_p_NNNN BDC/EMC would straddle a parent BDC boundary and
    // MuPDF would mis-pair the EMCs — the very bug that earlier made
    // us reject these blocks.
    let mut split_emc_before: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    let mut split_bdc_after: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::new();
    // Image-block flag per block: drives the extra `BDC /hp_p_image`
    // wrap around Image blocks only. Non-Image (Form / Inlined)
    // blocks skip this wrap.
    let mut is_image_block = vec![false; blocks.len()];
    for (i, b) in blocks.iter().enumerate() {
        q_at.insert(b.q_idx, i);
        q_end_at.insert(b.end_q_idx, i);
        if let Some((emc_idx, bdc_idx)) = b.straddle_split {
            split_emc_before.insert(emc_idx, i);
            split_bdc_after.insert(bdc_idx, i);
        }
        if matches!(b.content, BrickContent::Image { .. }) {
            is_image_block[i] = true;
        }
    }

    // Property-name → OCG-name map, so we can detect which BDCs delimit
    // the /OC /bricks region during the rewrite (same logic walk_page_bricks
    // uses).
    let prop_to_ocg_name = build_property_to_ocg_name_map(doc, page_id)?;
    let is_bricks_bdc = |op: &lopdf::content::Operation| -> bool {
        if op.operator != "BDC" { return false; }
        let is_oc = matches!(op.operands.first(), Some(Object::Name(n)) if n == b"OC");
        if !is_oc { return false; }
        match op.operands.get(1) {
            Some(Object::Name(prop)) => {
                let p = String::from_utf8_lossy(prop).to_string();
                prop_to_ocg_name
                    .get(&p)
                    .map(|s| s.eq_ignore_ascii_case("bricks"))
                    .unwrap_or(false)
            }
            _ => false,
        }
    };

    let mut new_ops: Vec<lopdf::content::Operation> =
        Vec::with_capacity(content.operations.len() + blocks.len() * 2);

    // State machine for inline wrapping:
    //   in_bricks_depth: how deep we are inside /OC /bricks BDC/EMC pairs.
    //   bdc_is_bricks_stack: tracks whether each open BDC is the bricks one.
    //   block_open: are we currently inside a known q…Q brick block?
    //   inline_open: have we emitted a /OC /hp_p_inline BDC that's not yet closed?
    // When the "should be inline" state changes between ops, emit BDC/EMC.
    let mut in_bricks_depth: u32 = 0;
    let mut bdc_is_bricks_stack: Vec<bool> = Vec::new();
    let mut block_open: bool = false;
    let mut inline_open: bool = false;
    let open_inline = |new_ops: &mut Vec<lopdf::content::Operation>| {
        new_ops.push(lopdf::content::Operation::new(
            "BDC",
            vec![
                Object::Name(b"OC".to_vec()),
                Object::Name(b"hp_p_inline".to_vec()),
            ],
        ));
    };
    let close_inline = |new_ops: &mut Vec<lopdf::content::Operation>| {
        new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
    };

    for (idx, op) in content.operations.iter().enumerate() {
        let name = op.operator.as_str();

        // ── Update state BEFORE deciding what to emit, so that for this
        //    iteration we can answer "are we currently in inline state?"
        //    The BDC/EMC ops themselves are the brackets — they don't
        //    count as "inside" their own scope.

        // Detect entering a brick block (q at a tracked index).
        let entering_block = name == "q" && q_at.contains_key(&idx);
        // Detect leaving a brick block (Q at a tracked index).
        let leaving_block = name == "Q" && q_end_at.contains_key(&idx);

        // The conditions under which the CURRENT op is inline content
        // (inside /OC /bricks, outside any brick block, not a bricks
        // BDC/EMC bracket itself).
        let bdc_is_bricks_here = is_bricks_bdc(op);
        let emc_closes_bricks_here = name == "EMC"
            && bdc_is_bricks_stack.last().copied().unwrap_or(false);
        // "is the op inside /OC /bricks RIGHT NOW (before any state
        //  change this op causes)?" — bdc_is_bricks_here is the OPENING
        // bracket, so it's still at the outer depth.
        let in_bricks_now = in_bricks_depth > 0 && !bdc_is_bricks_here;
        // When we hit a Q that ends a tracked block, the block is still
        // open AT this op — close happens AFTER. Same for q starts.
        let in_block_now = block_open || entering_block;
        let should_be_inline =
            in_bricks_now && !in_block_now && !emc_closes_bricks_here;

        // Open / close inline wrapping based on transition.
        if should_be_inline && !inline_open {
            open_inline(&mut new_ops);
            inline_open = true;
        } else if !should_be_inline && inline_open {
            close_inline(&mut new_ops);
            inline_open = false;
        }

        // Emit the existing brick-block opener BDC right before the q,
        // and (for Image blocks only) a second nested BDC for the
        // global `hp_image` OCG. That nested wrap is what lets callers
        // toggle the entire raster set independently of per-brick
        // OCGs — e.g. "render every non-Image block" by leaving
        // hp_image off.
        if let Some(&block_idx) = q_at.get(&idx) {
            if let Some(prop_name) = block_to_prop_name.get(&block_idx) {
                new_ops.push(lopdf::content::Operation::new(
                    "BDC",
                    vec![
                        Object::Name(b"OC".to_vec()),
                        Object::Name(prop_name.as_bytes().to_vec()),
                    ],
                ));
                if is_image_block.get(block_idx).copied().unwrap_or(false) {
                    new_ops.push(lopdf::content::Operation::new(
                        "BDC",
                        vec![
                            Object::Name(b"OC".to_vec()),
                            Object::Name(b"hp_p_image".to_vec()),
                        ],
                    ));
                }
            }
        }
        // Straddling: emit EMC(hp_p_X) right BEFORE the EMC that pops
        // the bricks BDC inside the block. Closes the first half of
        // the hp_p_X scope while we're still inside bricks. For Image
        // blocks the inner hp_p_image scope must close first.
        if let Some(&block_idx) = split_emc_before.get(&idx) {
            if block_to_prop_name.contains_key(&block_idx) {
                if is_image_block.get(block_idx).copied().unwrap_or(false) {
                    new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
                }
                new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
            }
        }

        new_ops.push(op.clone());

        // Straddling: emit BDC(hp_p_X) right AFTER the BDC that pushes
        // the next scope (lights). Reopens hp_p_X inside that scope,
        // so the block's remaining content stays toggleable. For
        // Image blocks the inner hp_p_image scope re-opens too.
        if let Some(&block_idx) = split_bdc_after.get(&idx) {
            if let Some(prop_name) = block_to_prop_name.get(&block_idx) {
                new_ops.push(lopdf::content::Operation::new(
                    "BDC",
                    vec![
                        Object::Name(b"OC".to_vec()),
                        Object::Name(prop_name.as_bytes().to_vec()),
                    ],
                ));
                if is_image_block.get(block_idx).copied().unwrap_or(false) {
                    new_ops.push(lopdf::content::Operation::new(
                        "BDC",
                        vec![
                            Object::Name(b"OC".to_vec()),
                            Object::Name(b"hp_p_image".to_vec()),
                        ],
                    ));
                }
            }
        }
        // Emit the brick-block closer EMC right after the Q. For
        // Image blocks emit two EMCs (the inner hp_p_image first,
        // then the outer hp_brick_NNNN).
        if let Some(&block_idx) = q_end_at.get(&idx) {
            if block_to_prop_name.contains_key(&block_idx) {
                if is_image_block.get(block_idx).copied().unwrap_or(false) {
                    new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
                }
                new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
            }
        }

        // ── Update state AFTER this op for future iterations.
        match name {
            "BDC" => {
                let is_b = bdc_is_bricks_here;
                bdc_is_bricks_stack.push(is_b);
                if is_b { in_bricks_depth += 1; }
            }
            "EMC" => {
                if let Some(was_b) = bdc_is_bricks_stack.pop() {
                    if was_b && in_bricks_depth > 0 {
                        in_bricks_depth -= 1;
                    }
                }
            }
            _ => {}
        }
        if entering_block { block_open = true; }
        if leaving_block { block_open = false; }
    }
    // Safety: close any open inline at end (shouldn't trigger if the
    // bricks region is well-formed, but be defensive).
    if inline_open {
        close_inline(&mut new_ops);
    }

    let new_content = Content { operations: new_ops };
    let encoded = new_content
        .encode()
        .map_err(|e| anyhow!("encode content stream: {e}"))?;

    // Replace the page's /Contents with a single new stream.
    let new_stream = Stream::new(Dictionary::new(), encoded);
    let new_stream_id = doc.add_object(Object::Stream(new_stream));
    {
        let page = doc
            .get_object_mut(page_id)
            .context("page lookup #3")?
            .as_dict_mut()
            .context("page dict #3")?;
        page.set("Contents", Object::Reference(new_stream_id));
    }

    // 5. Register the new OCGs in /OCProperties so MuPDF discovers them.
    //    Append to /OCGs (array of refs) and to /D /Order (so they show
    //    up in the default layer-config UI we toggle through FFI).
    register_ocgs_in_catalog(
        doc,
        brick_ocg_names
            .iter()
            .map(|n| ocg_object_id[n])
            .chain(std::iter::once(ocg_object_id[&decoration_ocg_name]))
            .chain(std::iter::once(ocg_object_id[&inline_ocg_name]))
            .chain(std::iter::once(ocg_object_id[&image_ocg_name]))
            .collect(),
    )?;

    Ok(InjectedOcgs {
        brick_ocgs: brick_ocg_names,
        decoration_ocg: decoration_ocg_name,
        inline_ocg: inline_ocg_name,
        image_ocg: image_ocg_name,
    })
}

/// Append a set of new OCG object refs to `/Root /OCProperties /OCGs`
/// and `/Root /OCProperties /D /Order`. If `/OCProperties` is missing
/// (the AI files always have one, but be defensive), create it.
fn register_ocgs_in_catalog(doc: &mut Document, new_ocgs: Vec<ObjectId>) -> Result<()> {
    // Catalog id is the trailer's /Root.
    let root_id = doc
        .trailer
        .get(b"Root")
        .ok()
        .and_then(|o| if let Object::Reference(id) = o { Some(*id) } else { None })
        .ok_or_else(|| anyhow!("no /Root in trailer"))?;

    // Get or create /OCProperties on the catalog.
    let mut catalog = doc
        .get_object(root_id)?
        .as_dict()
        .context("catalog must be a dict")?
        .clone();
    let mut ocprops = match catalog.get(b"OCProperties") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };

    // Update /OCGs (array of refs). May be either a direct Array or a
    // Reference to an Array object.
    let mut ocgs_array: Vec<Object> = read_array_resolving_ref(doc, ocprops.get(b"OCGs").ok())
        .unwrap_or_default();
    for id in &new_ocgs {
        ocgs_array.push(Object::Reference(*id));
    }
    ocprops.set("OCGs", Object::Array(ocgs_array));

    // Update /D /Order so MuPDF lists them in its layer-config UI.
    let mut d_dict = match ocprops.get(b"D") {
        Ok(Object::Dictionary(d)) => d.clone(),
        Ok(Object::Reference(id)) => doc
            .get_object(*id)
            .and_then(|o| o.as_dict())
            .map(|d| d.clone())
            .unwrap_or_default(),
        _ => Dictionary::new(),
    };
    // The same resolve-ref-or-array trick: /Order is *commonly* an
    // indirect reference in Illustrator output. Reading the dict by
    // value via lopdf doesn't auto-follow it, so a naive
    // `match d_dict.get(b"Order") { Array(a) => a }` returned empty
    // and wiped the existing layer order.
    let mut order_array: Vec<Object> =
        read_array_resolving_ref(doc, d_dict.get(b"Order").ok()).unwrap_or_default();
    for id in &new_ocgs {
        order_array.push(Object::Reference(*id));
    }
    d_dict.set("Order", Object::Array(order_array));
    // Force /BaseState to be /ON so default rendering matches original.
    if d_dict.get(b"BaseState").is_err() {
        d_dict.set("BaseState", Object::Name(b"ON".to_vec()));
    }
    ocprops.set("D", Object::Dictionary(d_dict));

    catalog.set("OCProperties", Object::Dictionary(ocprops));
    // Persist catalog back.
    doc.objects.insert(root_id, Object::Dictionary(catalog));
    Ok(())
}

// ── End-to-end orchestration: build the modified PDF on disk ─────
//
// Wraps walk → match → inject → save into one call so the CLI
// example (and later, the session loader) doesn't have to know the
// individual steps. Writes the rewritten PDF to a fresh temp path so
// MuPDF can open it via its existing path-based FFI.

/// Output of `build_modified_pdf` — the path the rewritten PDF was
/// written to, plus the metadata callers need to render per piece.
#[derive(Debug, Clone)]
pub struct ModifiedPdfArtifact {
    /// Path to the rewritten PDF on disk.
    pub pdf_path: std::path::PathBuf,
    /// One OCG name per parser brick, same order as the input
    /// `placements` vector.
    pub brick_ocg_names: Vec<String>,
    /// Single OCG name covering all leftover decoration blocks.
    pub decoration_ocg_name: String,
    /// OCG wrapping inline draw ops that sit inside /OC /bricks but
    /// outside any q…Q block (the "leak" content — yellow window-pane
    /// gradients etc.). Leave it disabled in piece renders.
    pub inline_ocg_name: String,
    /// Global OCG covering every Image XObject block. Toggle this OFF
    /// (with every other OCG ON) to render "all non-Image content"
    /// in a single MuPDF call — useful for Option-B piece rendering
    /// where direct extraction handles the Image rasters and MuPDF
    /// only needs to paint the vector overlays once.
    pub image_ocg_name: String,
    /// Counts (mostly for diagnostics + logging).
    pub stats: ModifiedPdfStats,
    /// Pymu → PDF-page X bleed in points, refined from the actual
    /// matched (block, placement) geometry. Sub-pixel precise — use
    /// this when computing the render's `shifted_clip` so the raster
    /// pixmap and any vector overlay derived from the parser's pymu
    /// coords line up exactly. `pdf_e_actual = pymu_e + bleed_pts.0`.
    pub bleed_pts: (f64, f64),
    /// Per-brick PDF-page bbox in PYMU/raster Y-down coords (origin
    /// top-left of mediabox). Derived from the matched block's
    /// path-endpoint bbox (which is the brick's geometric outline,
    /// not the bleed-padded image bbox). Use this directly as the
    /// brick's clip rect for per-brick rendering — avoids per-brick
    /// variance in image-CTM-vs-polygon offset that breaks a global
    /// median-bleed approach. `None` if the brick had no matched
    /// block or the matched block had no path-endpoint info.
    /// Layout: `(min_x, min_y, max_x, max_y)`.
    pub brick_pdf_bboxes: Vec<Option<(f64, f64, f64, f64)>>,
}

#[derive(Debug, Clone, Copy)]
pub struct ModifiedPdfStats {
    pub pdf_blocks_total: usize,
    pub pdf_blocks_matched: usize,
    pub pdf_blocks_decoration: usize,
    pub bricks_with_at_least_one_block: usize,
    pub bricks_orphaned: usize,
}

/// Default overlay attach radius — see `match_blocks_to_bricks`.
pub const DEFAULT_OVERLAY_RADIUS_PT: f64 = 70.0;

/// Walk the AI/PDF, match blocks to parser bricks, inject per-brick
/// OCGs, and write the modified PDF to `out_path`. Returns a struct
/// carrying the path and the OCG names a renderer needs.
pub fn build_modified_pdf(
    ai_path: &std::path::Path,
    placements: &[crate::ai_parser::BrickPlacement],
    meta: &crate::ai_parser::ParsedAiMetadata,
    out_path: &std::path::Path,
) -> Result<ModifiedPdfArtifact> {
    let mut doc = Document::load(ai_path).context("loading PDF with lopdf")?;
    let pages = doc.get_pages();
    let (_, page_id) = pages
        .iter()
        .next()
        .ok_or_else(|| anyhow!("PDF has no pages"))?;
    let page_id = *page_id;

    // Walk blocks.
    let blocks = walk_page_bricks(&doc, page_id).context("walking page bricks")?;

    // Pull PDF page height from the page's mediabox.
    let page_height_pt = page_mediabox_y1(&doc, page_id)?;
    let (clip_x0, clip_y0, _, _) = meta.clip_rect;

    // Detect the pymu → PDF-page X/Y bleed by rasterising the bricks
    // layer through the parser's clip rect and comparing the first
    // opaque pixel column/row against `meta.expected_brick_min`.
    // Same trick the load pipeline uses (commands.rs:329).
    //
    // Without this correction, placement polygons land in a different
    // frame from `BrickBlock::path_centroid` (PDF page coords from the
    // content stream's CTM) and no containment test ever succeeds.
    let probe_dpi = 100.0_f64;
    let (bleed_x, bleed_y) = match crate::mupdf_ffi::render_page_with_ocg_set_clipped(
        ai_path.to_str().unwrap_or(""),
        &["bricks"],
        probe_dpi,
        Some(meta.clip_rect),
    ) {
        Some((rgba, w, h)) => match image::RgbaImage::from_raw(w, h, rgba) {
            Some(img) => {
                let scale = probe_dpi / meta.render_dpi;
                let exp_x = ((meta.expected_brick_min.0 as f64) * scale).round() as i32;
                let exp_y = ((meta.expected_brick_min.1 as f64) * scale).round() as i32;
                let (dx_px, dy_px) = crate::render::compute_pdf_offset(&img, exp_x, exp_y);
                // dx_px > 0 means actual first-opaque-col is to the LEFT
                // of expected (artwork lives at smaller pymu_x than the
                // parser thinks). We want `pdf_e = pymu_e + bleed`, with
                // bleed = (-dx_px) in pixels → pts.
                let pts_per_px = 72.0 / probe_dpi;
                ((-dx_px as f64) * pts_per_px, (-dy_px as f64) * pts_per_px)
            }
            None => (0.0, 0.0),
        },
        None => (0.0, 0.0),
    };
    eprintln!("[ocg_inject] detected bleed_x={:.2}pt bleed_y={:.2}pt", bleed_x, bleed_y);
    if let Ok(target) = std::env::var("HP_DEBUG_BRICK") {
        if let Some((i, p)) = placements.iter().enumerate().find(|(_, p)| p.name == target) {
            let g = PageGeometry { clip_x0, clip_y0, render_dpi: meta.render_dpi, page_height_pt, bleed_x, bleed_y };
            let cent = g.placement_polygon_centroid_pdf(p);
            eprintln!("[ocg_inject:debug] {} (idx={}) pymu_xywh=({},{},{},{}) centroid_pdf={:?}",
                target, i, p.pymu_x, p.pymu_y, p.pymu_w, p.pymu_h, cent);
            if let Some(poly) = p.polygon.as_ref() {
                eprintln!("  raw polygon ({} verts):", poly.len());
                for (k, v) in poly.iter().enumerate() {
                    eprintln!("    [{}] = ({:.2}, {:.2})", k, v[0], v[1]);
                }
            }
            if let Some(poly_pdf) = g.placement_polygon_pdf(p) {
                let xs: Vec<f64> = poly_pdf.iter().map(|p| p.0).collect();
                let ys: Vec<f64> = poly_pdf.iter().map(|p| p.1).collect();
                eprintln!("  polygon in PDF: x range [{:.1}..{:.1}], y range [{:.1}..{:.1}]",
                    xs.iter().cloned().fold(f64::INFINITY, f64::min),
                    xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
                    ys.iter().cloned().fold(f64::INFINITY, f64::min),
                    ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max));
            }
        }
    }

    let geo = PageGeometry {
        clip_x0,
        clip_y0,
        render_dpi: meta.render_dpi,
        page_height_pt,
        bleed_x,
        bleed_y,
    };

    // Match.
    let map = match_blocks_to_bricks(&blocks, placements, geo, DEFAULT_OVERLAY_RADIUS_PT);

    // ── Refine bleed from matched (Image block, polygon) centres ──
    //
    // The constant coordinate-system shift between pymu pts (parser
    // frame, used by `polygon`) and PDF page pts (content stream
    // frame, used by `inner_ctm`) is what we call `bleed_pts`. To
    // measure it precisely, take one sample per Image block:
    //
    //   image_centre_pdf = ctm(0.5, 0.5)         ← centre of image rect
    //   polygon_centre_pymu = bbox centre        ← centre of geometric outline
    //   sample = image_centre_pdf − polygon_centre_pymu  ← coord-system shift
    //
    // For each brick the AI file embeds the raster's image rect with
    // some alpha-bleed extent on each side. If those bleeds are
    // *symmetric* (uniform per-brick — which user-confirmed for NY5),
    // the image rect's centre equals the geometric outline's centre,
    // so each sample is exactly the global shift. Median across all
    // Image bricks → the global bleed, free of per-brick bleed-extent
    // variance and free of the dual-cluster bias the previous
    // corner-based version produced.
    let refined_bleed_pts: (f64, f64) = {
        let s = meta.render_dpi / 72.0;
        let mut dxs: Vec<f64> = Vec::new();
        let mut dys_pdf_up: Vec<f64> = Vec::new();
        for (i, blks) in map.brick_to_blocks.iter().enumerate() {
            if blks.is_empty() { continue; }
            let block_opt = blks.iter().copied().find_map(|bi| {
                let b = &blocks[bi];
                if matches!(b.content, BrickContent::Image { .. }) { Some(b) } else { None }
            });
            let Some(block) = block_opt else { continue; };
            let Some(poly) = placements[i].polygon.as_ref() else { continue; };
            if poly.len() < 3 { continue; }
            let ctm = block.inner_ctm_at_content;
            // Axis-aligned positive-scale check so `a`/`d` mean width/height.
            if ctm.b.abs() > 1e-3 || ctm.c.abs() > 1e-3 { continue; }
            if ctm.a <= 0.0 || ctm.d <= 0.0 { continue; }
            let img_cx_pdf = ctm.e + ctm.a / 2.0;
            let img_cy_pdf_up = ctm.f + ctm.d / 2.0;
            let bx_local = placements[i].pymu_x.max(0) as f64;
            let by_local = placements[i].pymu_y.max(0) as f64;
            let min_x = poly.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min);
            let max_x = poly.iter().map(|v| v[0]).fold(f64::NEG_INFINITY, f64::max);
            let min_y = poly.iter().map(|v| v[1]).fold(f64::INFINITY, f64::min);
            let max_y = poly.iter().map(|v| v[1]).fold(f64::NEG_INFINITY, f64::max);
            let cx_canvas = (min_x + max_x) / 2.0 + bx_local;
            let cy_canvas = (min_y + max_y) / 2.0 + by_local;
            let cx_pymu_pt = cx_canvas / s + clip_x0;
            let cy_pymu_pt_down = cy_canvas / s + clip_y0;
            let cy_pdf_up = page_height_pt - cy_pymu_pt_down;
            dxs.push(img_cx_pdf - cx_pymu_pt);
            dys_pdf_up.push(img_cy_pdf_up - cy_pdf_up);
        }
        // Median that averages the two middle values for even counts —
        // matters because dx samples can cluster bimodally if bricks
        // have any systematic left/right bleed asymmetry, and an
        // off-by-one median would pick one cluster's edge.
        fn median(mut xs: Vec<f64>) -> Option<f64> {
            if xs.is_empty() { return None; }
            xs.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
            let n = xs.len();
            Some(if n % 2 == 0 { (xs[n / 2 - 1] + xs[n / 2]) / 2.0 } else { xs[n / 2] })
        }
        let bx = median(dxs).unwrap_or(bleed_x);
        let by_pdf_up = median(dys_pdf_up).unwrap_or(-bleed_y);
        // pymu y-down convention: pdf_y_down = pymu_y_down + bleed_y_down.
        // dy_sample_pdf_up = image_y_up − polygon_y_up = −(pdf_y_down −
        // pymu_y_down) = −bleed_y_down, so flip the sign.
        (bx, -by_pdf_up)
    };
    let sample_count = {
        let mut c = 0;
        for (i, blks) in map.brick_to_blocks.iter().enumerate() {
            if blks.is_empty() { continue; }
            if placements[i].polygon.as_ref().map(|p| p.len() >= 3).unwrap_or(false)
                && blks.iter().any(|&bi| matches!(blocks[bi].content, BrickContent::Image { .. }))
            { c += 1; }
        }
        c
    };
    eprintln!(
        "[ocg_inject] refined bleed from {} Image centres: ({:.3}pt, {:.3}pt) (probe was {:.2}, {:.2})",
        sample_count, refined_bleed_pts.0, refined_bleed_pts.1, bleed_x, bleed_y,
    );
    if let Ok(target) = std::env::var("HP_DEBUG_BRICK") {
        if let Some((i, p)) = placements.iter().enumerate().find(|(_, p)| p.name == target) {
            eprintln!("[ocg_inject:debug] {} brick_to_blocks[{}] = {:?}", target, i, map.brick_to_blocks[i]);
            let g = PageGeometry { clip_x0, clip_y0, render_dpi: meta.render_dpi, page_height_pt, bleed_x, bleed_y };
            let poly = g.placement_polygon_pdf(p);
            for &bi in &map.brick_to_blocks[i] {
                let b = &blocks[bi];
                eprintln!("  block #{:3}: inner.e={:.3} f={:.3} a={:.3} d={:.3} content={} straddle={:?}",
                    bi, b.inner_ctm_at_content.e, b.inner_ctm_at_content.f,
                    b.inner_ctm_at_content.a, b.inner_ctm_at_content.d,
                    b.content.kind_str(), b.straddle_split);
                if let Some((mnx, mny, mxx, mxy)) = b.path_endpoint_bbox {
                    eprintln!("    path_endpoint_bbox: x=[{:.3}..{:.3}] y_up=[{:.3}..{:.3}] (w={:.3} h={:.3})",
                        mnx, mxx, mny, mxy, mxx - mnx, mxy - mny);
                    let img_right = b.inner_ctm_at_content.e + b.inner_ctm_at_content.a;
                    let img_top = b.inner_ctm_at_content.f + b.inner_ctm_at_content.d;
                    eprintln!("    image-rect (Do CTM):    x=[{:.3}..{:.3}] y_up=[{:.3}..{:.3}]",
                        b.inner_ctm_at_content.e, img_right, b.inner_ctm_at_content.f, img_top);
                }
                if let (Some((cx, cy)), Some(poly)) = (b.path_centroid, poly.as_ref()) {
                    // Run the same ray-cast manually.
                    let mut inside = false;
                    let mut j = poly.len() - 1;
                    for k in 0..poly.len() {
                        let (xi, yi) = poly[k];
                        let (xj, yj) = poly[j];
                        if ((yi > cy) != (yj > cy))
                            && (cx < (xj - xi) * (cy - yi) / (yj - yi + f64::EPSILON) + xi)
                        { inside = !inside; }
                        j = k;
                    }
                    eprintln!("    in_polygon? {} for centroid ({:.1},{:.1})", inside, cx, cy);
                }
            }
        }
    }
    let bricks_with_blocks = map
        .brick_to_blocks
        .iter()
        .filter(|v| !v.is_empty())
        .count();
    let stats = ModifiedPdfStats {
        pdf_blocks_total: blocks.len(),
        pdf_blocks_matched: blocks.len() - map.decoration_blocks.len(),
        pdf_blocks_decoration: map.decoration_blocks.len(),
        bricks_with_at_least_one_block: bricks_with_blocks,
        bricks_orphaned: placements.len() - bricks_with_blocks,
    };

    // Inject.
    let injected = inject_per_brick_ocgs(&mut doc, page_id, &blocks, &map)
        .context("injecting per-brick OCGs")?;

    // Save.
    doc.save(out_path).context("saving modified PDF")?;

    // Per-brick PDF bbox in PYMU Y-down. We want the brick's *image
    // clip path* bbox — i.e. the path inside the Image block's q…Q
    // that masks the bleed-padded image. So we look only at the
    // matched Image blocks; Inlined/Form blocks may have paths that
    // span much larger areas (full-page clips, overlay shapes).
    //
    // If no Image block is matched, fall back to the IMAGE block's
    // inner_ctm-derived bbox (the image's PDF user-space rect from
    // ctm.e/f and ctm.a/d).
    let brick_pdf_bboxes: Vec<Option<(f64, f64, f64, f64)>> = (0..placements.len())
        .map(|i| {
            let mut acc: Option<(f64, f64, f64, f64)> = None;
            for &bi in &map.brick_to_blocks[i] {
                let block = &blocks[bi];
                // Restrict to Image-content blocks.
                if !matches!(block.content, BrickContent::Image { .. }) {
                    continue;
                }
                // Prefer the path-endpoint bbox (geometric clip).
                let block_bbox_up = match block.path_endpoint_bbox {
                    Some(b) => b,
                    None => {
                        // Image with no clip path — use ctm.e/f and
                        // ctm.a/d to derive the image's PDF rect.
                        let c = &block.inner_ctm_at_content;
                        let (x0, x1) = if c.a >= 0.0 { (c.e, c.e + c.a) } else { (c.e + c.a, c.e) };
                        let (y0, y1) = if c.d >= 0.0 { (c.f, c.f + c.d) } else { (c.f + c.d, c.f) };
                        (x0, y0, x1, y1)
                    }
                };
                let (x0, y0_up, x1, y1_up) = block_bbox_up;
                // PDF Y-up → pymu Y-down: y_down = page_h - y_up. Note
                // that max_y_up < min_y_down so min/max swap on Y.
                let (y0_down, y1_down) = (page_height_pt - y1_up, page_height_pt - y0_up);
                acc = Some(match acc {
                    None => (x0, y0_down, x1, y1_down),
                    Some((ax0, ay0, ax1, ay1)) => (
                        ax0.min(x0), ay0.min(y0_down),
                        ax1.max(x1), ay1.max(y1_down),
                    ),
                });
            }
            acc
        })
        .collect();

    Ok(ModifiedPdfArtifact {
        pdf_path: out_path.to_path_buf(),
        brick_ocg_names: injected.brick_ocgs,
        decoration_ocg_name: injected.decoration_ocg,
        inline_ocg_name: injected.inline_ocg,
        image_ocg_name: injected.image_ocg,
        stats,
        bleed_pts: refined_bleed_pts,
        brick_pdf_bboxes,
    })
}

fn page_mediabox_y1(doc: &Document, page_id: ObjectId) -> Result<f64> {
    let page = doc.get_object(page_id)?.as_dict()?;
    // /MediaBox can be inherited from a parent /Pages node.
    let mb = match page.get(b"MediaBox") {
        Ok(Object::Array(a)) => a.clone(),
        Ok(Object::Reference(id)) => doc.get_object(*id)?.as_array()?.clone(),
        _ => {
            // Walk /Parent chain.
            let mut current_id = page_id;
            let mut found = None;
            for _ in 0..16 {
                let dict = match doc.get_object(current_id).and_then(|o| o.as_dict()) {
                    Ok(d) => d,
                    Err(_) => break,
                };
                match dict.get(b"MediaBox") {
                    Ok(Object::Array(a)) => {
                        found = Some(a.clone());
                        break;
                    }
                    Ok(Object::Reference(rid)) => {
                        if let Ok(arr) = doc.get_object(*rid).and_then(|o| o.as_array()) {
                            found = Some(arr.clone());
                            break;
                        }
                    }
                    _ => {}
                }
                match dict.get(b"Parent") {
                    Ok(Object::Reference(pid)) => current_id = *pid,
                    _ => break,
                }
            }
            found.ok_or_else(|| anyhow!("no /MediaBox found"))?
        }
    };
    mb.get(3)
        .and_then(|o| match o {
            Object::Real(r) => Some(*r as f64),
            Object::Integer(i) => Some(*i as f64),
            _ => None,
        })
        .ok_or_else(|| anyhow!("/MediaBox y1 not numeric"))
}

// Quiet the unused-import warnings when this module is built but its
// crate-level consumers haven't wired in yet.
#[allow(dead_code)]
const _USES_STRING_FORMAT: Option<StringFormat> = None;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    /// Locate an NY AI file if present (the `in/` directory is
    /// gitignored — tests skip when the file isn't there).
    fn ai_path(stem: &str) -> Option<PathBuf> {
        let candidates = [
            format!("in/{}.ai", stem),
            format!("../../in/{}.ai", stem),
        ];
        candidates.iter().map(PathBuf::from).find(|p| p.exists())
    }

    #[test]
    fn build_modified_pdf_ny5_smoke() {
        let Some(ai) = ai_path("_NY5") else {
            eprintln!("in/_NY5.ai not present — skipping");
            return;
        };
        let (placements, meta, _) =
            crate::ai_parser::parse_ai(&ai, crate::CANVAS_HEIGHT_PX as i32).unwrap();
        let tmp = std::env::temp_dir().join("ocg_inject_test_ny5");
        std::fs::create_dir_all(&tmp).unwrap();
        let out_pdf = tmp.join("_modified.pdf");
        let artifact = build_modified_pdf(&ai, &placements, &meta, &out_pdf).unwrap();

        // One OCG name per parser brick.
        assert_eq!(artifact.brick_ocg_names.len(), placements.len(),
            "brick_ocg_names length mismatch");
        // Names follow the documented hp_brick_NNNN convention.
        for (i, n) in artifact.brick_ocg_names.iter().enumerate() {
            assert_eq!(n, &format!("hp_brick_{:04}", i),
                "OCG name doesn't follow convention");
        }
        // Globals are populated.
        assert_eq!(artifact.image_ocg_name, "hp_image");
        assert_eq!(artifact.inline_ocg_name, "hp_bricks_inline");
        assert_eq!(artifact.decoration_ocg_name, "hp_decoration");

        // Bleed for NY5 is roughly +267 pt on X (the artwork's
        // baked-in coord shift). Anywhere in [200, 350] is sane;
        // outside that range means our matching or coord math has
        // drifted.
        assert!(
            (200.0..=350.0).contains(&artifact.bleed_pts.0),
            "bleed_x out of expected range: {}", artifact.bleed_pts.0,
        );
        assert!(
            artifact.bleed_pts.1.abs() < 10.0,
            "bleed_y unexpectedly large: {}", artifact.bleed_pts.1,
        );

        // The rewritten PDF file exists, is non-empty, and parses.
        assert!(out_pdf.exists());
        let md = std::fs::metadata(&out_pdf).unwrap();
        assert!(md.len() > 10_000, "modified PDF suspiciously small: {} bytes", md.len());
        let _ = lopdf::Document::load(&out_pdf)
            .expect("modified PDF must round-trip through lopdf");

        // Block-level matching produced sensible coverage.
        assert!(
            artifact.stats.pdf_blocks_matched + artifact.stats.pdf_blocks_decoration
                == artifact.stats.pdf_blocks_total,
            "matched+decoration must sum to total: {:?}", artifact.stats,
        );
        assert!(
            artifact.stats.bricks_with_at_least_one_block as f64
                / placements.len() as f64
                > 0.95,
            ">5% of bricks ended up with no matched block — matcher regression. \
             stats={:?}", artifact.stats,
        );
    }

    /// `walk_page_bricks` followed by `match_blocks_to_bricks` should
    /// place every Image block on top of exactly one polygon (or, in
    /// rare overlay cases, attach via the second pass). Run on NY1
    /// because the test_parse_ai assertion already pins it as a
    /// stable input.
    #[test]
    fn match_blocks_to_bricks_ny1_covers_most_images() {
        let Some(ai) = ai_path("_NY1") else {
            eprintln!("in/_NY1.ai not present — skipping");
            return;
        };
        let (placements, meta, _) =
            crate::ai_parser::parse_ai(&ai, crate::CANVAS_HEIGHT_PX as i32).unwrap();
        let doc = lopdf::Document::load(&ai).unwrap();
        let page_id = doc.page_iter().next().unwrap();
        let blocks = walk_page_bricks(&doc, page_id).unwrap();
        let page_h = page_mediabox_y1(&doc, page_id).unwrap();

        // Use a probe bleed of (0, 0); the matcher's polygon-
        // containment test is what we exercise. The numerical bleed
        // value affects whether centroids land inside polygons, but
        // for NY1 the polygon centroids are far enough from any
        // boundary that the matcher succeeds even with a coarse
        // bleed estimate.
        let geo = PageGeometry {
            clip_x0: meta.clip_rect.0, clip_y0: meta.clip_rect.1,
            render_dpi: meta.render_dpi, page_height_pt: page_h,
            bleed_x: 0.0, bleed_y: 0.0,
        };
        let map = match_blocks_to_bricks(&blocks, &placements, geo, DEFAULT_OVERLAY_RADIUS_PT);
        let matched_bricks = map.brick_to_blocks.iter().filter(|v| !v.is_empty()).count();
        let total_bricks = placements.len();
        assert!(
            matched_bricks as f64 / total_bricks as f64 > 0.85,
            "matcher coverage regressed: {}/{} bricks matched",
            matched_bricks, total_bricks,
        );
    }
}
