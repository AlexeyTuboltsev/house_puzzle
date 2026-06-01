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
    let mut bdc_stack: Vec<bool> = Vec::new(); // true = current BDC is the bricks OCG
    let mut in_bricks_depth = 0_u32;

    // Open-block state: when we hit a `q` at the top level inside the
    // bricks region, we open a candidate block here and close it on
    // the matching `Q`.
    let mut open: Option<BrickBlock> = None;
    let mut open_q_depth_at_start: u32 = 0;

    for (idx, op) in content.operations.iter().enumerate() {
        let name = op.operator.as_str();

        match name {
            "q" => {
                let prev = *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                ctm_stack.push(prev);
                if in_bricks_depth > 0 && open.is_none() {
                    open = Some(BrickBlock {
                        q_idx: idx,
                        end_q_idx: idx, // overwritten on close
                        outer_ctm: prev,
                        inner_ctm_at_content: prev,
                        content: BrickContent::Inlined,
                    });
                    open_q_depth_at_start = ctm_stack.len() as u32 - 1;
                }
            }
            "Q" => {
                if ctm_stack.len() > 1 {
                    ctm_stack.pop();
                }
                if let Some(mut block) = open.take() {
                    if (ctm_stack.len() as u32) == open_q_depth_at_start {
                        block.end_q_idx = idx;
                        blocks.push(block);
                    } else {
                        // Nested Q didn't close our block yet — put it back.
                        open = Some(block);
                    }
                }
            }
            "cm" => {
                // 6 operands: a b c d e f
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
                // operands: [tag, properties_name_or_dict]
                let is_oc = matches!(op.operands.first(), Some(Object::Name(n)) if n == b"OC");
                let is_bricks = if is_oc {
                    match op.operands.get(1) {
                        Some(Object::Name(prop_name)) => {
                            let prop = String::from_utf8_lossy(prop_name).to_string();
                            prop_to_ocg_name
                                .get(&prop)
                                .map(|s| s.eq_ignore_ascii_case("bricks"))
                                .unwrap_or(false)
                        }
                        _ => false,
                    }
                } else {
                    false
                };
                bdc_stack.push(is_bricks);
                if is_bricks {
                    in_bricks_depth += 1;
                }
            }
            "EMC" => {
                if let Some(was_bricks) = bdc_stack.pop() {
                    if was_bricks && in_bricks_depth > 0 {
                        in_bricks_depth -= 1;
                    }
                }
            }
            "Do" => {
                if let Some(block) = open.as_mut() {
                    if let Some(Object::Name(form_name)) = op.operands.first() {
                        let name = String::from_utf8_lossy(form_name).to_string();
                        if let Ok(object_id) = resolve_xobject_name(doc, page_id, &name) {
                            // Distinguish Form vs Image XObject via the
                            // referenced stream's /Subtype.
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
                            // Record the CTM at the moment of the Do — the
                            // placement transform that maps the XObject's
                            // internal coords onto the page.
                            block.inner_ctm_at_content =
                                *ctm_stack.last().unwrap_or(&Affine::IDENTITY);
                        }
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
}

impl PageGeometry {
    fn scale(&self) -> f64 {
        self.render_dpi / 72.0
    }

    /// Project a parser brick into three PDF y-up anchor points
    /// (bottom-left, top-left, centroid) so the overlay-matcher can
    /// pick whichever the AI file's overlay aligned to.
    fn brick_anchors(&self, brick: &crate::ai_parser::BrickPlacement) -> [(f64, f64); 3] {
        let s = self.scale();
        let pdf_e_left = brick.x as f64 / s + self.clip_x0;
        let pdf_e_cent = (brick.x as f64 + brick.width as f64 / 2.0) / s + self.clip_x0;
        let pymu_y_top = brick.y as f64 / s + self.clip_y0;
        let pymu_y_bot = (brick.y + brick.height) as f64 / s + self.clip_y0;
        let pymu_y_cent = pymu_y_top + (brick.height as f64 / 2.0) / s;
        let pdf_f_top = self.page_height_pt - pymu_y_top;
        let pdf_f_bot = self.page_height_pt - pymu_y_bot;
        let pdf_f_cent = self.page_height_pt - pymu_y_cent;
        [
            (pdf_e_left, pdf_f_bot),  // bottom-left
            (pdf_e_left, pdf_f_top),  // top-left
            (pdf_e_cent, pdf_f_cent), // centroid
        ]
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
    // Per-brick three-anchor cache.
    let anchors: Vec<[(f64, f64); 3]> = placements.iter().map(|p| geo.brick_anchors(p)).collect();

    let mut brick_to_blocks: Vec<Vec<usize>> = vec![Vec::new(); placements.len()];
    let mut block_matched = vec![false; blocks.len()];
    let mut brick_used = vec![false; placements.len()];

    // ── First pass: 1:1 greedy by Y-residual, walking in document order ──
    // The PDF emits blocks in roughly the same order as the AI tree.
    // The X coordinate has a consistent ~60 pt alpha-bleed offset; Y is
    // ~exact. So we match on Y alone within a tight 30 pt tolerance.
    const Y_TOL_PT: f64 = 30.0;
    for (j, block) in blocks.iter().enumerate() {
        let (_be, bf) = (block.inner_ctm_at_content.e, block.inner_ctm_at_content.f);
        // Scan parser bricks in order, picking the first unused one
        // whose any-anchor Y matches.
        let best = placements
            .iter()
            .enumerate()
            .filter(|(i, _)| !brick_used[*i])
            .map(|(i, _)| {
                let dy = anchors[i]
                    .iter()
                    .map(|(_, fy)| (bf - fy).abs())
                    .fold(f64::INFINITY, f64::min);
                (dy, i)
            })
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        if let Some((dy, i)) = best {
            if dy < Y_TOL_PT {
                brick_to_blocks[i].push(j);
                brick_used[i] = true;
                block_matched[j] = true;
            }
        }
    }

    // ── Second pass: attach remaining blocks to nearest brick ─────
    // For each unmatched PDF block, find the parser brick with the
    // smallest min-anchor distance. If within `overlay_radius_pt`,
    // attach as an overlay; otherwise it's a decoration.
    for (j, block) in blocks.iter().enumerate() {
        if block_matched[j] {
            continue;
        }
        let (be, bf) = (block.inner_ctm_at_content.e, block.inner_ctm_at_content.f);
        let mut best: (f64, usize) = (f64::INFINITY, usize::MAX);
        for (i, anchs) in anchors.iter().enumerate() {
            let d = anchs
                .iter()
                .map(|(ax, ay)| ((be - ax).powi(2) + (bf - ay).powi(2)).sqrt())
                .fold(f64::INFINITY, f64::min);
            if d < best.0 {
                best = (d, i);
            }
        }
        if best.1 != usize::MAX && best.0 < overlay_radius_pt {
            brick_to_blocks[best.1].push(j);
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
/// single shared OCG for blocks that didn't match any parser brick.
#[derive(Debug, Clone)]
pub struct InjectedOcgs {
    pub brick_ocgs: Vec<String>,
    pub decoration_ocg: String,
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
    for name in brick_ocg_names.iter().chain(std::iter::once(&decoration_ocg_name)) {
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
    let content = doc
        .get_and_decode_page_content(page_id)
        .context("decode page content")?;
    // Fast lookup: op-index → block-index for openers and closers.
    let mut q_at: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::with_capacity(blocks.len());
    let mut q_end_at: std::collections::HashMap<usize, usize> =
        std::collections::HashMap::with_capacity(blocks.len());
    for (i, b) in blocks.iter().enumerate() {
        q_at.insert(b.q_idx, i);
        q_end_at.insert(b.end_q_idx, i);
    }

    let mut new_ops: Vec<lopdf::content::Operation> =
        Vec::with_capacity(content.operations.len() + blocks.len() * 2);
    for (idx, op) in content.operations.iter().enumerate() {
        if let Some(&block_idx) = q_at.get(&idx) {
            if let Some(prop_name) = block_to_prop_name.get(&block_idx) {
                new_ops.push(lopdf::content::Operation::new(
                    "BDC",
                    vec![
                        Object::Name(b"OC".to_vec()),
                        Object::Name(prop_name.as_bytes().to_vec()),
                    ],
                ));
            }
        }
        new_ops.push(op.clone());
        if let Some(&block_idx) = q_end_at.get(&idx) {
            if block_to_prop_name.contains_key(&block_idx) {
                new_ops.push(lopdf::content::Operation::new("EMC", Vec::new()));
            }
        }
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
            .collect(),
    )?;

    Ok(InjectedOcgs {
        brick_ocgs: brick_ocg_names,
        decoration_ocg: decoration_ocg_name,
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
    /// Counts (mostly for diagnostics + logging).
    pub stats: ModifiedPdfStats,
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
    let geo = PageGeometry {
        clip_x0,
        clip_y0,
        render_dpi: meta.render_dpi,
        page_height_pt,
    };

    // Match.
    let map = match_blocks_to_bricks(&blocks, placements, geo, DEFAULT_OVERLAY_RADIUS_PT);
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

    Ok(ModifiedPdfArtifact {
        pdf_path: out_path.to_path_buf(),
        brick_ocg_names: injected.brick_ocgs,
        decoration_ocg_name: injected.decoration_ocg,
        stats,
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
