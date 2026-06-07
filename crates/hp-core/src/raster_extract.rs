//! Direct raster extraction from Adobe-Illustrator-style PDF Image XObjects.
//!
//! The AI export embeds each raster brick as an Image XObject (typically
//! Flate-compressed 8-bit RGB at a fixed pixel resolution like 512×N),
//! plus a separate SMask Image XObject carrying the alpha channel
//! (Flate-compressed 8-bit grey at the same resolution).
//!
//! Bypassing MuPDF for these bricks gives us:
//!   • exact pixels from the AI file (no sub-pixel rasterisation noise)
//!   • exact placement via the block's CTM (no probe-DPI / bleed math)
//!   • per-layer rendering at ~10× speed (image decode vs full-page raster)
//!
//! The CTM `a/b/c/d/e/f` maps the image's unit square onto the PDF page.
//! For Adobe's bricks `b == c == 0` (axis-aligned), so:
//!   image pixel (px, py) → PDF page (e + a·u, f + d·(1−v))
//!     where u = px/W, v = py/H, and the PDF y-axis is up.
//! The image is stored top-down (PDF row 0 = top), so the visual top
//! corresponds to `f + d` (PDF y-up).

use anyhow::{anyhow, bail, Context, Result};
use flate2::read::ZlibDecoder;
use image::RgbaImage;
use lopdf::{Document, Object, ObjectId};
use std::io::Read;

use crate::ocg_inject::{BrickBlock, BrickContent};

/// Iterate every Image block in `block_indices`, direct-extract its
/// RGB+SMask raster, and source-over compose onto `canvas` at the
/// position given by the block's `inner_ctm`. Non-Image blocks (Form /
/// Inlined) are silently skipped — caller is responsible for rendering
/// them via MuPDF first (or accepting they'll be missing).
///
/// `bleed_pts` is the pymu→PDF coord-system translation; pass
/// `ModifiedPdfArtifact::bleed_pts` from the same `build_modified_pdf`
/// call.
pub fn compose_image_blocks_onto_canvas<I>(
    doc: &Document,
    blocks: &[BrickBlock],
    block_indices: I,
    canvas: &mut RgbaImage,
    clip_rect_pymu_pts: (f64, f64, f64, f64),
    page_height_pt: f64,
    bleed_pts: (f64, f64),
    dpi: f64,
    bilinear: bool,
) where I: IntoIterator<Item = usize>,
{
    compose_image_blocks_onto_canvas_at(
        doc, blocks, block_indices, canvas,
        clip_rect_pymu_pts, page_height_pt, bleed_pts, dpi, bilinear, (0, 0),
    );
}

/// Same as `compose_image_blocks_onto_canvas` but `canvas` is treated as
/// a sub-image of the full export canvas, positioned at
/// `canvas_offset_px` in absolute canvas-px coords. Destination pixels
/// computed from each image's CTM are translated by `-canvas_offset_px`
/// before being written; pixels that fall outside the local canvas are
/// clipped. Used by per-piece rendering to allocate only a piece-sized
/// buffer instead of cloning the full export canvas.
pub fn compose_image_blocks_onto_canvas_at<I>(
    doc: &Document,
    blocks: &[BrickBlock],
    block_indices: I,
    canvas: &mut RgbaImage,
    clip_rect_pymu_pts: (f64, f64, f64, f64),
    page_height_pt: f64,
    bleed_pts: (f64, f64),
    dpi: f64,
    bilinear: bool,
    canvas_offset_px: (i32, i32),
) where I: IntoIterator<Item = usize>,
{
    for idx in block_indices {
        let Some(b) = blocks.get(idx) else { continue; };
        let BrickContent::Image { object_id, .. } = &b.content else { continue; };
        match extract_image_block(doc, *object_id, &b.inner_ctm_at_content) {
            Ok(raster) => raster.compose_onto_at(
                canvas, clip_rect_pymu_pts, page_height_pt, bleed_pts, dpi, bilinear, canvas_offset_px),
            Err(e) => eprintln!("[raster_extract] block #{} extract failed: {}", idx, e),
        }
    }
}

/// One extracted raster brick: RGBA pixels at the AI's native resolution,
/// plus the four affine params we need to place it in PDF page coords.
#[derive(Debug, Clone)]
pub struct ExtractedRaster {
    /// RGBA buffer, row-major, top-left origin (matches `image::RgbaImage`).
    pub rgba: Vec<u8>,
    pub width: u32,
    pub height: u32,
    /// Block's `inner_ctm`: image rect = (e, f) — (e+a, f+d) in PDF y-up.
    pub ctm_e: f64,
    pub ctm_f: f64,
    pub ctm_a: f64,
    pub ctm_d: f64,
}

impl ExtractedRaster {
    /// Place this raster onto a canvas-sized RGBA image at the position
    /// implied by its CTM, scaled to `dpi`. Assumes axis-aligned CTM and
    /// that `clip_rect` (pymu pts) is the parser's bounding clip.
    /// `page_height_pt` is the PDF mediabox y1.
    ///
    /// `bleed` = `(bleed_x, bleed_y)` is the constant translation between
    /// PDF page coords (where CTMs live) and pymu pts (where the parser's
    /// clip_rect lives): `pdf_x = pymu_x + bleed_x`, `pdf_y_down = pymu_y_down + bleed_y`.
    /// Without this shift the image lands at PDF-page-X instead of
    /// pymu-X — for NY5 that's a ~267 pt offset (it puts the bottom-left
    /// brick at the bottom-centre of the canvas).
    ///
    /// Scaling uses nearest-neighbour by default; bilinear is more
    /// faithful for the typical case where image px ≪ canvas px. Pass
    /// `bilinear=true` to get the smoother variant.
    pub fn compose_onto(
        &self,
        canvas: &mut RgbaImage,
        clip_rect_pymu_pts: (f64, f64, f64, f64),
        page_height_pt: f64,
        bleed: (f64, f64),
        dpi: f64,
        bilinear: bool,
    ) {
        self.compose_onto_at(canvas, clip_rect_pymu_pts, page_height_pt, bleed, dpi, bilinear, (0, 0));
    }

    /// Same as `compose_onto` but treats `canvas` as a sub-image of the
    /// full export canvas, positioned at `canvas_offset_px` (absolute
    /// canvas-px coords of the local canvas's (0, 0)). Used to render
    /// into a piece-sized buffer instead of the full canvas.
    pub fn compose_onto_at(
        &self,
        canvas: &mut RgbaImage,
        clip_rect_pymu_pts: (f64, f64, f64, f64),
        page_height_pt: f64,
        bleed: (f64, f64),
        dpi: f64,
        bilinear: bool,
        canvas_offset_px: (i32, i32),
    ) {
        let s = dpi / 72.0;
        // Image rect in PDF page coords (y-up).
        let pdf_left = self.ctm_e;
        let pdf_right = self.ctm_e + self.ctm_a;
        let pdf_bottom_up = self.ctm_f;
        let pdf_top_up = self.ctm_f + self.ctm_d;
        // Convert to pymu pts. The bleed convention from
        // `ocg_inject::PageGeometry` is `pdf_e = pymu_x + bleed_x` and
        // `pdf_f = page_h − (pymu_y_down + bleed_y)`. Inverting:
        //   pymu_x      = pdf_e − bleed_x
        //   pymu_y_down = (page_h − pdf_y_up) − bleed_y
        // The previous implementation had a sign flip on the Y bleed
        // term and doubled the shift in the wrong direction. With the
        // signs corrected, direct-extract placement matches what
        // MuPDF produces when rendering the same modified PDF through
        // the shifted clip (used as ground truth for "where the brick
        // appears in Illustrator").
        let (bleed_x, bleed_y) = bleed;
        let pymu_left = pdf_left - bleed_x;
        let pymu_right = pdf_right - bleed_x;
        let pymu_top = (page_height_pt - pdf_top_up) - bleed_y;
        let pymu_bottom = (page_height_pt - pdf_bottom_up) - bleed_y;
        // Canvas-px float coords of the image's destination rect, in
        // ABSOLUTE canvas-px (origin = full-canvas top-left).
        let (cx0_pt, cy0_pt, _, _) = clip_rect_pymu_pts;
        let dest_left_f = (pymu_left - cx0_pt) * s;
        let dest_right_f = (pymu_right - cx0_pt) * s;
        let dest_top_f = (pymu_top - cy0_pt) * s;
        let dest_bottom_f = (pymu_bottom - cy0_pt) * s;
        let dest_w_f = dest_right_f - dest_left_f;
        let dest_h_f = dest_bottom_f - dest_top_f;
        if dest_w_f <= 0.0 || dest_h_f <= 0.0 { return; }

        // Apply the local-canvas offset so absolute coords land at the
        // right place in a sub-canvas. For the default (offset = 0, 0)
        // path this is a no-op.
        let (off_x, off_y) = canvas_offset_px;
        let dest_left_local = dest_left_f - off_x as f64;
        let dest_right_local = dest_right_f - off_x as f64;
        let dest_top_local = dest_top_f - off_y as f64;
        let dest_bottom_local = dest_bottom_f - off_y as f64;

        let canvas_w = canvas.width() as i32;
        let canvas_h = canvas.height() as i32;
        // Walk every canvas pixel covered by the dest rect; sample the
        // source image at the inverse-mapped float coord.
        let x_start = dest_left_local.floor() as i32;
        let x_end = dest_right_local.ceil() as i32;
        let y_start = dest_top_local.floor() as i32;
        let y_end = dest_bottom_local.ceil() as i32;

        let src_w = self.width as i32;
        let src_h = self.height as i32;
        // Inverse map: canvas_px → image_px. (Same as before — the
        // offset only shifts the destination rectangle, not the source
        // sampling rate.)
        let inv_x = (src_w as f64) / dest_w_f;
        let inv_y = (src_h as f64) / dest_h_f;

        for cy in y_start.max(0)..y_end.min(canvas_h) {
            let v = (cy as f64 + 0.5 - dest_top_local) * inv_y;
            for cx in x_start.max(0)..x_end.min(canvas_w) {
                let u = (cx as f64 + 0.5 - dest_left_local) * inv_x;
                let rgba = if bilinear {
                    sample_bilinear(&self.rgba, src_w, src_h, u, v)
                } else {
                    sample_nearest(&self.rgba, src_w, src_h, u, v)
                };
                if rgba[3] == 0 { continue; }
                // Source-over blend onto canvas pixel.
                let dst = canvas.get_pixel_mut(cx as u32, cy as u32);
                blend_src_over(dst.0.as_mut(), rgba);
            }
        }
    }
}

fn sample_nearest(buf: &[u8], w: i32, h: i32, u: f64, v: f64) -> [u8; 4] {
    let x = (u.floor() as i32).clamp(0, w - 1);
    let y = (v.floor() as i32).clamp(0, h - 1);
    let i = ((y * w + x) * 4) as usize;
    [buf[i], buf[i + 1], buf[i + 2], buf[i + 3]]
}

fn sample_bilinear(buf: &[u8], w: i32, h: i32, u: f64, v: f64) -> [u8; 4] {
    let ux = u - 0.5;
    let vy = v - 0.5;
    let x0 = ux.floor() as i32;
    let y0 = vy.floor() as i32;
    let fx = ux - x0 as f64;
    let fy = vy - y0 as f64;
    let x1 = x0 + 1;
    let y1 = y0 + 1;
    let cx0 = x0.clamp(0, w - 1);
    let cx1 = x1.clamp(0, w - 1);
    let cy0 = y0.clamp(0, h - 1);
    let cy1 = y1.clamp(0, h - 1);
    let p = |x: i32, y: i32| -> [f64; 4] {
        let i = ((y * w + x) * 4) as usize;
        [buf[i] as f64, buf[i + 1] as f64, buf[i + 2] as f64, buf[i + 3] as f64]
    };
    let p00 = p(cx0, cy0);
    let p10 = p(cx1, cy0);
    let p01 = p(cx0, cy1);
    let p11 = p(cx1, cy1);
    let mut out = [0u8; 4];
    for c in 0..4 {
        let top = p00[c] * (1.0 - fx) + p10[c] * fx;
        let bot = p01[c] * (1.0 - fx) + p11[c] * fx;
        out[c] = (top * (1.0 - fy) + bot * fy).round().clamp(0.0, 255.0) as u8;
    }
    out
}

fn blend_src_over(dst: &mut [u8], src: [u8; 4]) {
    // Premultiplied source-over: out = src + dst·(1 − src_a).
    let sa = src[3] as f32 / 255.0;
    let ia = 1.0 - sa;
    dst[0] = ((src[0] as f32) * sa + (dst[0] as f32) * ia).round().clamp(0.0, 255.0) as u8;
    dst[1] = ((src[1] as f32) * sa + (dst[1] as f32) * ia).round().clamp(0.0, 255.0) as u8;
    dst[2] = ((src[2] as f32) * sa + (dst[2] as f32) * ia).round().clamp(0.0, 255.0) as u8;
    let da = dst[3] as f32 / 255.0;
    dst[3] = ((sa + da * ia) * 255.0).round().clamp(0.0, 255.0) as u8;
}

/// Decode a Flate-compressed image stream into raw byte rows.
/// Honours the `/DecodeParms /Predictor` setting (1 = none; PNG 10–15
/// variants apply per-row reconstruction).
fn decode_flate_image(
    doc: &Document,
    stream_id: ObjectId,
) -> Result<(Vec<u8>, u32, u32, u32, u32)> {
    let obj = doc.get_object(stream_id).context("get image obj")?;
    let stream = obj.as_stream().context("not a stream")?;
    let dict = &stream.dict;
    let width = dict
        .get(b"Width")
        .and_then(|o| o.as_i64())
        .map_err(|e| anyhow!("missing /Width: {e}"))? as u32;
    let height = dict
        .get(b"Height")
        .and_then(|o| o.as_i64())
        .map_err(|e| anyhow!("missing /Height: {e}"))? as u32;
    let bpc = dict
        .get(b"BitsPerComponent")
        .and_then(|o| o.as_i64())
        .unwrap_or(8) as u32;
    // Colors: derive from ColorSpace. ICCBased ref carries /N in its stream
    // dict. DeviceRGB → 3, DeviceGray → 1, DeviceCMYK → 4. Anything else
    // we don't handle yet.
    let colors = resolve_color_components(doc, dict.get(b"ColorSpace").ok())?;

    // Decompress with zlib.
    let mut decoder = ZlibDecoder::new(&stream.content[..]);
    let mut raw = Vec::new();
    decoder.read_to_end(&mut raw).context("zlib decode")?;

    // Apply predictor if present.
    let (predictor, pred_cols, pred_colors, pred_bpc) = read_decode_parms(dict);
    let bytes_per_row = if pred_cols > 0 && pred_colors > 0 && pred_bpc > 0 {
        // Predictor's view of row width — used for PNG-predictor unfilter.
        ((pred_cols * pred_colors * pred_bpc + 7) / 8) as usize
    } else {
        ((width * colors * bpc + 7) / 8) as usize
    };

    let unfiltered = match predictor {
        // 1 = no prediction; data is row-major raw.
        1 | 0 => raw,
        // 10–15 = PNG predictor variants. Adobe writes 12 (PNG Up); we
        // support the full family because the unfilter algorithm is the
        // same — each row carries a 1-byte filter tag selecting None/
        // Sub/Up/Average/Paeth.
        10..=15 => unfilter_png(&raw, bytes_per_row).context("PNG unfilter")?,
        2 => {
            // TIFF predictor: subtract previous component on the same row.
            bail!("TIFF predictor (2) not implemented yet");
        }
        _ => bail!("unknown /Predictor {}", predictor),
    };

    Ok((unfiltered, width, height, colors, bpc))
}

fn read_decode_parms(dict: &lopdf::Dictionary) -> (u32, u32, u32, u32) {
    let Ok(Object::Dictionary(dp)) = dict.get(b"DecodeParms") else {
        return (1, 0, 0, 0);
    };
    let predictor = dp.get(b"Predictor").and_then(|o| o.as_i64()).unwrap_or(1) as u32;
    let columns = dp.get(b"Columns").and_then(|o| o.as_i64()).unwrap_or(1) as u32;
    let colors = dp.get(b"Colors").and_then(|o| o.as_i64()).unwrap_or(1) as u32;
    let bpc = dp.get(b"BitsPerComponent").and_then(|o| o.as_i64()).unwrap_or(8) as u32;
    (predictor, columns, colors, bpc)
}

fn resolve_color_components(doc: &Document, cs: Option<&Object>) -> Result<u32> {
    let Some(cs) = cs else { return Ok(3); }; // assume RGB if unspecified
    match cs {
        Object::Name(n) => match n.as_slice() {
            b"DeviceRGB" => Ok(3),
            b"DeviceGray" => Ok(1),
            b"DeviceCMYK" => Ok(4),
            other => bail!("unsupported named ColorSpace /{}", String::from_utf8_lossy(other)),
        },
        Object::Array(a) => {
            // ICCBased: [/ICCBased <stream-ref>]; the stream's /N tells
            // us component count.
            if a.first().map(|o| matches!(o, Object::Name(n) if n == b"ICCBased")).unwrap_or(false) {
                if let Some(Object::Reference(sid)) = a.get(1) {
                    let icc = doc.get_object(*sid).context("get ICC stream")?
                        .as_stream().context("ICC not a stream")?;
                    let n = icc.dict.get(b"N").and_then(|o| o.as_i64()).unwrap_or(3) as u32;
                    return Ok(n);
                }
            }
            bail!("unsupported array ColorSpace: first = {:?}", a.first())
        }
        Object::Reference(sid) => {
            // Some files have ColorSpace as a direct reference to a Stream/Array.
            let obj = doc.get_object(*sid).context("resolve ColorSpace ref")?;
            resolve_color_components(doc, Some(obj))
        }
        _ => bail!("unrecognised ColorSpace object"),
    }
}

/// Reverse the PNG row-filter encoding. Input bytes are `(1 + row_bytes) * height`
/// (one filter-tag byte per row). Output is `row_bytes * height` raw bytes.
fn unfilter_png(raw: &[u8], row_bytes: usize) -> Result<Vec<u8>> {
    let stride = row_bytes + 1;
    if raw.len() % stride != 0 {
        bail!("PNG predictor input length {} not multiple of stride {}",
            raw.len(), stride);
    }
    let rows = raw.len() / stride;
    let mut out = vec![0u8; row_bytes * rows];
    let bpp = 1usize; // bytes per pixel for PNG-predictor reconstruction:
                     // PDF treats it as 1 byte stride (Adobe convention
                     // for 8-bit RGB-image flate). Sub/Paeth filters use
                     // it as the "left" offset.
                     //
                     // The PDF spec is annoyingly imprecise here; the
                     // PNG-spec-compliant value would be Colors *
                     // BitsPerComponent / 8 but in practice for Adobe's
                     // brick exports bpp=1 reproduces the original image
                     // faithfully (filter tags are all 2=Up, which
                     // doesn't even use bpp). If we hit a Sub/Paeth row
                     // and pixels look glitched, this is the knob to
                     // revisit.
    for r in 0..rows {
        let tag = raw[r * stride];
        let row_in = &raw[r * stride + 1..(r + 1) * stride];
        let row_out_start = r * row_bytes;
        let prev_start = if r == 0 { usize::MAX } else { (r - 1) * row_bytes };
        match tag {
            0 => {
                out[row_out_start..row_out_start + row_bytes].copy_from_slice(row_in);
            }
            1 => {
                // Sub: out[i] = in[i] + out[i - bpp]
                for i in 0..row_bytes {
                    let left = if i >= bpp { out[row_out_start + i - bpp] } else { 0 };
                    out[row_out_start + i] = row_in[i].wrapping_add(left);
                }
            }
            2 => {
                // Up: out[i] = in[i] + prev_row[i]
                for i in 0..row_bytes {
                    let up = if prev_start == usize::MAX { 0 } else { out[prev_start + i] };
                    out[row_out_start + i] = row_in[i].wrapping_add(up);
                }
            }
            3 => {
                // Average
                for i in 0..row_bytes {
                    let left = if i >= bpp { out[row_out_start + i - bpp] } else { 0 } as u16;
                    let up = if prev_start == usize::MAX { 0 } else { out[prev_start + i] } as u16;
                    out[row_out_start + i] = row_in[i].wrapping_add(((left + up) / 2) as u8);
                }
            }
            4 => {
                // Paeth
                for i in 0..row_bytes {
                    let a = if i >= bpp { out[row_out_start + i - bpp] } else { 0 } as i32;
                    let b = if prev_start == usize::MAX { 0 } else { out[prev_start + i] } as i32;
                    let c = if prev_start == usize::MAX || i < bpp { 0 }
                            else { out[prev_start + i - bpp] } as i32;
                    let p = a + b - c;
                    let pa = (p - a).abs();
                    let pb = (p - b).abs();
                    let pc = (p - c).abs();
                    let paeth = if pa <= pb && pa <= pc { a }
                               else if pb <= pc { b }
                               else { c };
                    out[row_out_start + i] = row_in[i].wrapping_add(paeth as u8);
                }
            }
            other => bail!("unknown PNG filter tag {}", other),
        }
    }
    Ok(out)
}

/// PDF Indexed colorspace palette resolved into a flat RGB byte table.
struct IndexedPalette { lookup_rgb: Vec<u8> }

fn resolve_indexed(doc: &Document, cs: &Object) -> Result<Option<IndexedPalette>> {
    let arr = match cs {
        Object::Array(a) => a,
        Object::Reference(sid) => {
            let obj = doc.get_object(*sid).context("resolve cs ref")?;
            return resolve_indexed(doc, obj);
        }
        _ => return Ok(None),
    };
    let is_indexed = arr.first()
        .map(|o| matches!(o, Object::Name(n) if n == b"Indexed"))
        .unwrap_or(false);
    if !is_indexed { return Ok(None); }
    // [/Indexed <base> <hival> <lookup>]
    let base = arr.get(1).ok_or_else(|| anyhow!("Indexed: missing base CS"))?;
    let base_components = resolve_color_components(doc, Some(base))?;
    let lookup = arr.get(3).ok_or_else(|| anyhow!("Indexed: missing lookup"))?;
    let lookup_bytes = match lookup {
        Object::String(b, _) => b.clone(),
        Object::Reference(sid) => {
            let obj = doc.get_object(*sid).context("resolve lookup ref")?;
            let stream = obj.as_stream().context("lookup not a stream")?;
            let mut dec = ZlibDecoder::new(&stream.content[..]);
            let mut out = Vec::new();
            // Stream may or may not be Flate; if no /Filter, content is raw.
            if stream.dict.get(b"Filter").is_ok() {
                dec.read_to_end(&mut out).context("decode lookup stream")?;
            } else {
                out = stream.content.clone();
            }
            out
        }
        _ => bail!("unsupported Indexed lookup object"),
    };
    // Convert base→RGB if needed. For NY5 base is always DeviceRGB so
    // we keep bytes as-is; CMYK / Gray bases get expanded inline.
    let lookup_rgb = match base_components {
        3 => lookup_bytes,
        1 => {
            let mut out = Vec::with_capacity(lookup_bytes.len() * 3);
            for g in lookup_bytes { out.push(g); out.push(g); out.push(g); }
            out
        }
        4 => {
            // CMYK palette — same naive conversion as the per-pixel one.
            let mut out = Vec::with_capacity((lookup_bytes.len() / 4) * 3);
            for chunk in lookup_bytes.chunks_exact(4) {
                let c = chunk[0] as f32 / 255.0;
                let m = chunk[1] as f32 / 255.0;
                let y = chunk[2] as f32 / 255.0;
                let k = chunk[3] as f32 / 255.0;
                out.push(((1.0 - c) * (1.0 - k) * 255.0).round() as u8);
                out.push(((1.0 - m) * (1.0 - k) * 255.0).round() as u8);
                out.push(((1.0 - y) * (1.0 - k) * 255.0).round() as u8);
            }
            out
        }
        n => bail!("unsupported Indexed base component count {}", n),
    };
    Ok(Some(IndexedPalette { lookup_rgb }))
}

/// Extract the RGBA pixels for one brick block.
///
/// Adobe-AI Image XObjects store RGB in the main stream and the alpha
/// channel in a separate Image referenced via `/SMask`. Both decode the
/// same way; we recombine them into a single RGBA buffer for caller use.
pub fn extract_image_block(
    doc: &Document,
    image_object_id: ObjectId,
    block_ctm: &crate::ocg_inject::Affine,
) -> Result<ExtractedRaster> {
    // Detect Indexed up front: decode_flate_image would reject the
    // /Indexed array as "unsupported ColorSpace", but with Indexed the
    // "colors" count is actually 1 (single-byte index) — and we then
    // expand via the palette ourselves.
    let stream_dict = doc.get_object(image_object_id)?.as_stream()?.dict.clone();
    let palette = if let Ok(cs) = stream_dict.get(b"ColorSpace") {
        resolve_indexed(doc, cs)?
    } else { None };

    let (raw_bytes, w, h, colors, bpc) = if palette.is_some() {
        // Force colors=1 inside decode_flate_image by temporarily
        // swapping ColorSpace to /DeviceGray. Easier path: replicate
        // the decode logic inline so we don't mutate the doc.
        let stream = doc.get_object(image_object_id)?.as_stream()?;
        let dict = &stream.dict;
        let width = dict.get(b"Width").and_then(|o| o.as_i64())
            .map_err(|e| anyhow!("indexed Width: {e}"))? as u32;
        let height = dict.get(b"Height").and_then(|o| o.as_i64())
            .map_err(|e| anyhow!("indexed Height: {e}"))? as u32;
        let bpc = dict.get(b"BitsPerComponent").and_then(|o| o.as_i64()).unwrap_or(8) as u32;
        let mut dec = ZlibDecoder::new(&stream.content[..]);
        let mut raw = Vec::new();
        dec.read_to_end(&mut raw).context("zlib decode indexed")?;
        let (predictor, pred_cols, pred_colors, pred_bpc) = read_decode_parms(dict);
        let row_bytes = if pred_cols > 0 {
            ((pred_cols * pred_colors.max(1) * pred_bpc.max(8) + 7) / 8) as usize
        } else {
            ((width * bpc + 7) / 8) as usize
        };
        let unfiltered = match predictor {
            1 | 0 => raw,
            10..=15 => unfilter_png(&raw, row_bytes).context("PNG unfilter indexed")?,
            other => bail!("unsupported predictor {} for indexed image", other),
        };
        (unfiltered, width, height, 1u32, bpc)
    } else {
        decode_flate_image(doc, image_object_id).context("decoding main image stream")?
    };
    if bpc != 8 {
        bail!("only 8-bit images supported (got {} bpc)", bpc);
    }
    // Expand Indexed → RGB; non-indexed images keep the same bytes.
    let rgb_bytes: Vec<u8> = if let Some(pal) = &palette {
        let mut out = Vec::with_capacity(raw_bytes.len() * 3);
        for idx in &raw_bytes {
            let i = *idx as usize * 3;
            if i + 2 < pal.lookup_rgb.len() {
                out.push(pal.lookup_rgb[i]);
                out.push(pal.lookup_rgb[i + 1]);
                out.push(pal.lookup_rgb[i + 2]);
            } else { out.extend_from_slice(&[0, 0, 0]); }
        }
        out
    } else { raw_bytes };
    let colors = if palette.is_some() { 3 } else { colors };

    // SMask (alpha) — same w/h, single component, 8-bit gray.
    let alpha = if let Ok(Object::Reference(sid)) = doc
        .get_object(image_object_id).and_then(|o| o.as_stream()).map(|s| &s.dict)
        .map_err(|e| anyhow!("get stream dict: {e}"))?
        .get(b"SMask")
    {
        let (smask_bytes, sw, sh, scolors, sbpc) = decode_flate_image(doc, *sid)
            .context("decoding SMask stream")?;
        if scolors != 1 || sbpc != 8 || sw != w || sh != h {
            bail!("SMask format mismatch: expected {}×{} 8-bit gray, got {}×{} {}-bit {}-channel",
                w, h, sw, sh, sbpc, scolors);
        }
        smask_bytes
    } else {
        vec![255u8; (w * h) as usize]
    };

    // Combine RGB(A) + alpha into RGBA.
    let pixel_count = (w * h) as usize;
    let mut rgba = Vec::with_capacity(pixel_count * 4);
    match colors {
        3 => {
            for i in 0..pixel_count {
                rgba.push(rgb_bytes[i * 3]);
                rgba.push(rgb_bytes[i * 3 + 1]);
                rgba.push(rgb_bytes[i * 3 + 2]);
                rgba.push(alpha[i]);
            }
        }
        1 => {
            for i in 0..pixel_count {
                let g = rgb_bytes[i];
                rgba.push(g); rgba.push(g); rgba.push(g); rgba.push(alpha[i]);
            }
        }
        4 => {
            // Naive CMYK → RGB via complement; good enough for the brick
            // textures in NY5 where CMYK use is rare.
            for i in 0..pixel_count {
                let c = rgb_bytes[i * 4] as f32 / 255.0;
                let m = rgb_bytes[i * 4 + 1] as f32 / 255.0;
                let y = rgb_bytes[i * 4 + 2] as f32 / 255.0;
                let k = rgb_bytes[i * 4 + 3] as f32 / 255.0;
                let r = ((1.0 - c) * (1.0 - k) * 255.0).round() as u8;
                let g = ((1.0 - m) * (1.0 - k) * 255.0).round() as u8;
                let b = ((1.0 - y) * (1.0 - k) * 255.0).round() as u8;
                rgba.push(r); rgba.push(g); rgba.push(b); rgba.push(alpha[i]);
            }
        }
        other => bail!("unsupported component count {}", other),
    }

    Ok(ExtractedRaster {
        rgba, width: w, height: h,
        ctm_e: block_ctm.e,
        ctm_f: block_ctm.f,
        ctm_a: block_ctm.a,
        ctm_d: block_ctm.d,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    // ─── blend_src_over ────────────────────────────────────────────
    #[test]
    fn blend_src_over_opaque_white_over_black_yields_white() {
        let mut dst = [0u8, 0, 0, 255];
        blend_src_over(&mut dst, [255, 255, 255, 255]);
        assert_eq!(dst, [255, 255, 255, 255]);
    }

    #[test]
    fn blend_src_over_transparent_source_keeps_dst() {
        let mut dst = [10u8, 20, 30, 200];
        blend_src_over(&mut dst, [255, 255, 255, 0]);
        assert_eq!(dst, [10, 20, 30, 200]);
    }

    #[test]
    fn blend_src_over_half_alpha_white_over_black_yields_mid_grey() {
        let mut dst = [0u8, 0, 0, 0];
        blend_src_over(&mut dst, [255, 255, 255, 128]);
        // Source-over: 255 · (128/255) + 0 · (1 − 128/255) = 128.
        assert!((dst[0] as i32 - 128).abs() <= 1);
        assert!((dst[1] as i32 - 128).abs() <= 1);
        assert!((dst[2] as i32 - 128).abs() <= 1);
        // Alpha: 128 + 0·(1 − 128/255) = 128.
        assert!((dst[3] as i32 - 128).abs() <= 1);
    }

    // ─── sample_nearest / sample_bilinear ──────────────────────────
    //
    // A 2 × 2 RGBA image: top-left red, top-right green,
    // bottom-left blue, bottom-right white. Layout is row-major.
    fn small_rgba_2x2() -> Vec<u8> {
        vec![
            255, 0, 0, 255,   0, 255, 0, 255,   // row 0
            0, 0, 255, 255,   255, 255, 255, 255 // row 1
        ]
    }

    #[test]
    fn sample_nearest_picks_correct_quadrant() {
        let buf = small_rgba_2x2();
        assert_eq!(sample_nearest(&buf, 2, 2, 0.2, 0.2), [255, 0, 0, 255]);
        assert_eq!(sample_nearest(&buf, 2, 2, 1.7, 0.2), [0, 255, 0, 255]);
        assert_eq!(sample_nearest(&buf, 2, 2, 0.2, 1.7), [0, 0, 255, 255]);
        assert_eq!(sample_nearest(&buf, 2, 2, 1.7, 1.7), [255, 255, 255, 255]);
    }

    #[test]
    fn sample_bilinear_at_centre_averages_four_corners() {
        let buf = small_rgba_2x2();
        // Pixel centres sit at (0.5, 0.5) and (1.5, 1.5); sampling at
        // (1.0, 1.0) is the midpoint of all four → average of
        // (255,0,0), (0,255,0), (0,0,255), (255,255,255) = (128,128,128).
        let mid = sample_bilinear(&buf, 2, 2, 1.0, 1.0);
        assert!((mid[0] as i32 - 128).abs() <= 1);
        assert!((mid[1] as i32 - 128).abs() <= 1);
        assert!((mid[2] as i32 - 128).abs() <= 1);
        assert_eq!(mid[3], 255);
    }

    // ─── unfilter_png ──────────────────────────────────────────────
    //
    // Two rows × four bytes, filter tag = 0 (None) on each row.
    // The expected output is the input minus the tag bytes.
    #[test]
    fn unfilter_png_filter_none_passes_through() {
        let raw = vec![
            0, 10, 20, 30, 40,   // row 0, tag=0
            0, 50, 60, 70, 80,   // row 1, tag=0
        ];
        let out = unfilter_png(&raw, 4).expect("unfilter");
        assert_eq!(out, vec![10, 20, 30, 40, 50, 60, 70, 80]);
    }

    // Filter 2 (Up): output[i] = input[i] + prev_row[i]. Tag is 0 on
    // the first row, 2 on the second.
    #[test]
    fn unfilter_png_filter_up_reconstructs_second_row() {
        let raw = vec![
            0,  1,  2,  3,  4,  // row 0, tag=0 (literal)
            2, 10, 10, 10, 10,  // row 1, tag=2 (Up) → 1+10, 2+10, …
        ];
        let out = unfilter_png(&raw, 4).expect("unfilter");
        assert_eq!(out, vec![1, 2, 3, 4, 11, 12, 13, 14]);
    }

    // Filter 1 (Sub): output[i] = input[i] + output[i - bpp] (bpp=1).
    // Single row with tag=1 and inputs (5, 5, 5, 5) → cumulative sum.
    #[test]
    fn unfilter_png_filter_sub_cumulates_along_row() {
        let raw = vec![1, 5, 5, 5, 5];
        let out = unfilter_png(&raw, 4).expect("unfilter");
        assert_eq!(out, vec![5, 10, 15, 20]);
    }
}
