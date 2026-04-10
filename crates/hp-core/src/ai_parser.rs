//! AI file parser — extracts brick layers, geometry, and vector polygons.
//!
//! AI files are PDF-based with embedded PostScript data (AIPrivateData streams).
//! This module:
//! 1. Extracts and decompresses AIPrivateData via MuPDF FFI
//! 2. Parses the PostScript layer tree
//! 3. (TODO) Extracts brick placement and vector polygon data

use regex::bytes::Regex;
use std::path::Path;

use crate::mupdf_ffi;

/// Helper: convert a byte slice to &str (ASCII portion).
fn bstr(b: &[u8]) -> &str {
    std::str::from_utf8(b).unwrap_or("")
}

/// A parsed layer block from the AI PostScript data.
/// `begin` and `end` are byte offsets into the raw decompressed data.
#[derive(Debug, Clone)]
pub struct LayerBlock {
    pub name: String,
    pub begin: usize,
    pub end: usize,
    pub depth: usize,
    pub children: Vec<LayerBlock>,
}

/// Raw AI data: the decompressed bytes.
/// All offsets (LayerBlock.begin/end) are byte positions into `raw`.
/// Use `text_slice()` to get a &str for a range (ASCII portions only).
pub struct AiPrivateData {
    pub raw: Vec<u8>,
}

impl AiPrivateData {
    /// Get a text slice from the raw data (lossy UTF-8).
    /// Safe for ASCII/latin-1 content like PostScript operators.
    pub fn text_slice(&self, begin: usize, end: usize) -> String {
        String::from_utf8_lossy(&self.raw[begin..end]).to_string()
    }
}

/// Extract and decompress all AIPrivateData streams from an AI file.
///
/// AI files are PDFs with `/AIPrivateData1`, `/AIPrivateData2`, ... entries
/// pointing to streams containing zstd-compressed PostScript data.
pub fn decompress_ai_data(ai_path: &Path) -> Result<AiPrivateData, String> {
    let doc = mupdf::pdf::PdfDocument::open(ai_path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to open AI file: {e}"))?;

    // Find AIPrivateData references using dictionary-level access
    let pairs = mupdf_ffi::find_ai_private_data(&doc);
    if pairs.is_empty() {
        return Err("No AIPrivateData found in .ai file".to_string());
    }

    // Concatenate all stream data
    let mut raw = Vec::new();
    for (_, xref) in &pairs {
        if let Some(data) = mupdf_ffi::xref_stream(&doc, *xref) {
            raw.extend_from_slice(&data);
        }
    }

    if raw.is_empty() {
        return Err("AIPrivateData streams are empty".to_string());
    }

    // Find ZStandard frame magic: 0x28 0xB5 0x2F 0xFD
    let magic = [0x28u8, 0xB5, 0x2F, 0xFD];
    let pos = raw.windows(4)
        .position(|w| w == magic)
        .ok_or("ZStandard magic not found in AIPrivateData")?;

    // Decompress
    let compressed = &raw[pos..];
    let decompressed = zstd::decode_all(std::io::Cursor::new(compressed))
        .map_err(|e| format!("ZStd decompression failed: {e}"))?;

    Ok(AiPrivateData {
        raw: decompressed,
    })
}

/// Parse `%AI5_BeginLayer` / `%AI5_EndLayer` pairs into a nested tree.
/// Operates on raw bytes — all offsets are byte positions.
pub fn parse_layer_tree(data: &[u8]) -> Vec<LayerBlock> {
    let begin_re = Regex::new(r"%AI5_BeginLayer").unwrap();
    let end_re = Regex::new(r"%AI5_EndLayer").unwrap();
    let name_re = Regex::new(r"Lb\r\(([^)]*)\)").unwrap();

    let mut events: Vec<(char, usize)> = Vec::new();
    for m in begin_re.find_iter(data) {
        events.push(('B', m.start()));
    }
    for m in end_re.find_iter(data) {
        events.push(('E', m.start()));
    }
    events.sort_by_key(|e| e.1);

    let mut stack: Vec<LayerBlock> = Vec::new();
    let mut roots: Vec<LayerBlock> = Vec::new();

    for (typ, pos) in events {
        if typ == 'B' {
            let snippet_end = (pos + 300).min(data.len());
            let snippet = &data[pos..snippet_end];
            let name = name_re.captures(snippet)
                .map(|c| String::from_utf8_lossy(&c[1]).to_string())
                .unwrap_or_default();
            let depth = stack.len();
            let block = LayerBlock {
                name,
                begin: pos,
                end: pos,
                depth,
                children: Vec::new(),
            };
            stack.push(block);
        } else {
            if let Some(mut block) = stack.pop() {
                block.end = pos + b"%AI5_EndLayer".len();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(block);
                } else {
                    roots.push(block);
                }
            }
        }
    }

    roots
}

// ---------------------------------------------------------------------------
// Coordinate transform: AI native → PyMuPDF page coords (y-down)
// ---------------------------------------------------------------------------

/// Derive (offset_x, y_base) from the background layer's path coords vs page artbox.
/// pymu_x = ai_x + offset_x; pymu_y = y_base - ai_y
pub fn compute_ai_transform(
    background: &LayerBlock,
    data: &[u8],
    page: &mupdf::Page,
    artbox: Option<(f64, f64, f64, f64)>,
) -> (f64, f64) {
    let block_data = &data[background.begin..background.end];
    let coord_re = Regex::new(r"(-?\d+\.?\d*)\s+(-?\d+\.?\d*)\s+[mLCl]\b").unwrap();

    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();
    for cap in coord_re.captures_iter(block_data) {
        let x_str = std::str::from_utf8(&cap[1]).unwrap_or("0");
        let y_str = std::str::from_utf8(&cap[2]).unwrap_or("0");
        if let (Ok(x), Ok(y)) = (x_str.parse::<f64>(), y_str.parse::<f64>()) {
            xs.push(x);
            ys.push(y);
        }
    }

    // Use artbox if available, else fall back to page bounds (mediabox)
    let (art_x0, _art_y0, _art_x1, art_y1) = artbox.unwrap_or_else(|| {
        let b = mupdf_ffi::page_artbox(page);
        (b.0 as f64, b.1 as f64, b.2 as f64, b.3 as f64)
    });

    if xs.is_empty() {
        return (art_x0, art_y1);
    }

    let ai_xmin = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    let ai_ymin = ys.iter().cloned().fold(f64::INFINITY, f64::min);

    let offset_x = art_x0 - ai_xmin;
    let y_base = art_y1 + ai_ymin;
    (offset_x, y_base)
}

// ---------------------------------------------------------------------------
// Brick placement helpers
// ---------------------------------------------------------------------------

/// Check if a block contains a gradient fill (Bg operator).
fn has_gradient(block_data: &[u8]) -> bool {
    if !block_data.windows(2).any(|w| w == b"Bg") {
        return false;
    }
    for line in block_data.split(|&b| b == b'\r') {
        let s = bstr(line).trim();
        if s.ends_with("Bg") && s.contains('(') {
            return true;
        }
    }
    false
}

/// Extract the raster placement matrix from an Xh operator.
/// Returns (tx, ty, w_pts, h_pts) in AI coordinate space.
fn extract_raster_matrix(block_data: &[u8]) -> Option<(f64, f64, f64, f64)> {
    let num = r"-?\d+(?:\.\d+)?";
    let pattern = format!(
        r"\[\s*({n})\s+{n}\s+{n}\s+({n})\s+({n})\s+({n})\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh",
        n = num
    );
    let re = Regex::new(&pattern).unwrap();
    let m = re.captures(block_data)?;

    let a: f64 = bstr(&m[1]).parse().ok()?;
    let d: f64 = bstr(&m[2]).parse().ok()?;
    let tx: f64 = bstr(&m[3]).parse().ok()?;
    let ty: f64 = bstr(&m[4]).parse().ok()?;
    let img_w: f64 = bstr(&m[5]).parse().ok()?;
    let img_h: f64 = bstr(&m[6]).parse().ok()?;

    if img_w <= 0.0 || img_h <= 0.0 {
        return None;
    }

    let w_pts = a.abs() * img_w;
    let h_pts = d.abs() * img_h;
    Some((tx, ty, w_pts, h_pts))
}

/// Extract bounding box from plain (non-%_) path operators.
/// Returns (ai_xmin, ai_ymin, ai_xmax, ai_ymax) in AI y-up coords.
fn extract_plain_path_bbox(block: &LayerBlock, data: &[u8]) -> Option<(f64, f64, f64, f64)> {
    let block_data = &data[block.begin..block.end];
    let mut xs: Vec<f64> = Vec::new();
    let mut ys: Vec<f64> = Vec::new();

    for line in block_data.split(|&b| b == b'\r') {
        let line = bstr(line).trim().to_string();
        if line.starts_with('%') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let op = *parts.last().unwrap();
        match op {
            "m" | "L" | "l" if parts.len() >= 3 => {
                if let (Ok(x), Ok(y)) = (parts[0].parse::<f64>(), parts[1].parse::<f64>()) {
                    xs.push(x);
                    ys.push(y);
                }
            }
            "C" | "c" if parts.len() >= 7 => {
                for i in (0..6).step_by(2) {
                    if let (Ok(x), Ok(y)) = (parts[i].parse::<f64>(), parts[i + 1].parse::<f64>()) {
                        xs.push(x);
                        ys.push(y);
                    }
                }
            }
            _ => {}
        }
    }

    if xs.len() < 2 {
        return None;
    }
    Some((
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        ys.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
        ys.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    ))
}

// ---------------------------------------------------------------------------
// Vector polygon extraction
// ---------------------------------------------------------------------------

const PATH_OPS: &[&str] = &["m", "L", "l", "C", "c", "n", "N", "f", "F", "s", "S", "b", "B"];

/// Parse path operator lines into polygons (PyMuPDF y-down coords).
fn parse_path_lines(
    lines: &[Vec<&str>],
    offset_x: f64,
    y_base: f64,
) -> Vec<Vec<[f64; 2]>> {
    let to_pymu = |ax: f64, ay: f64| -> [f64; 2] {
        [ax + offset_x, y_base - ay]
    };

    let mut pts: Vec<[f64; 2]> = Vec::new();
    let mut polygons: Vec<Vec<[f64; 2]>> = Vec::new();

    for parts in lines {
        if parts.is_empty() {
            continue;
        }
        let op = *parts.last().unwrap();

        match op {
            "m" if parts.len() >= 3 => {
                if pts.len() >= 3 {
                    polygons.push(pts.clone());
                }
                let x: f64 = parts[0].parse().unwrap_or(0.0);
                let y: f64 = parts[1].parse().unwrap_or(0.0);
                pts = vec![to_pymu(x, y)];
            }
            "L" | "l" if parts.len() >= 3 => {
                let x: f64 = parts[0].parse().unwrap_or(0.0);
                let y: f64 = parts[1].parse().unwrap_or(0.0);
                pts.push(to_pymu(x, y));
            }
            "C" | "c" if parts.len() >= 7 => {
                if pts.is_empty() {
                    continue;
                }
                let p1 = *pts.last().unwrap();
                let cp1 = to_pymu(parts[0].parse().unwrap_or(0.0), parts[1].parse().unwrap_or(0.0));
                let cp2 = to_pymu(parts[2].parse().unwrap_or(0.0), parts[3].parse().unwrap_or(0.0));
                let p4 = to_pymu(parts[4].parse().unwrap_or(0.0), parts[5].parse().unwrap_or(0.0));
                // Tessellate cubic bezier with 9 intermediate points
                for i in 1..9 {
                    let t = i as f64 / 9.0;
                    let u = 1.0 - t;
                    let x = u.powi(3) * p1[0] + 3.0 * u.powi(2) * t * cp1[0]
                        + 3.0 * u * t.powi(2) * cp2[0] + t.powi(3) * p4[0];
                    let y = u.powi(3) * p1[1] + 3.0 * u.powi(2) * t * cp1[1]
                        + 3.0 * u * t.powi(2) * cp2[1] + t.powi(3) * p4[1];
                    pts.push([x, y]);
                }
                pts.push(p4);
            }
            "n" | "N" | "f" | "F" | "s" | "S" | "b" | "B" => {
                if pts.len() >= 3 {
                    polygons.push(pts.clone());
                }
                pts.clear();
            }
            _ => {}
        }
    }

    if pts.len() >= 3 {
        polygons.push(pts);
    }
    polygons
}

/// Extract the vector polygon for a brick from %_ prefixed PostScript path lines.
fn extract_vector_path(
    block: &LayerBlock,
    data: &[u8],
    offset_x: f64,
    y_base: f64,
) -> Vec<[f64; 2]> {
    let block_data = &data[block.begin..block.end];

    // Parse all lines into owned strings first (avoids lifetime issues)
    let lines: Vec<String> = block_data.split(|&b| b == b'\r')
        .map(|l| bstr(l).trim().to_string())
        .collect();

    // Primary: %_ prefixed lines
    let mut parsed_lines: Vec<Vec<String>> = Vec::new();
    for line in &lines {
        if !line.starts_with("%_") {
            continue;
        }
        let parts: Vec<String> = line[2..].split_whitespace().map(|s| s.to_string()).collect();
        if !parts.is_empty() && PATH_OPS.contains(&parts.last().unwrap().as_str()) {
            parsed_lines.push(parts);
        }
    }

    if parsed_lines.is_empty() {
        for line in &lines {
            if line.starts_with('%') {
                continue;
            }
            let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            if !parts.is_empty()
                && PATH_OPS.contains(&parts.last().unwrap().as_str())
                && (parts.len() == 3 || parts.len() == 7)
            {
                let all_numeric = parts[..parts.len() - 1]
                    .iter()
                    .all(|p| p.parse::<f64>().is_ok());
                if all_numeric {
                    parsed_lines.push(parts);
                }
            }
        }
    }

    // Convert to &str slices for parse_path_lines
    let refs: Vec<Vec<&str>> = parsed_lines.iter()
        .map(|parts| parts.iter().map(|s| s.as_str()).collect())
        .collect();
    let polygons = parse_path_lines(&refs, offset_x, y_base);
    polygons.into_iter().max_by_key(|p| p.len()).unwrap_or_default()
}

// ---------------------------------------------------------------------------
// Main parse function: collect brick placements + polygons
// ---------------------------------------------------------------------------

/// A brick placement extracted from the AI file.
#[derive(Debug, Clone)]
pub struct BrickPlacement {
    pub name: String,
    pub layer_type: String,
    /// Bounding box in PyMuPDF y-down pixel coords.
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    /// Vector polygon in brick-local pixel coords.
    pub polygon: Option<Vec<[f64; 2]>>,
    /// Byte range in raw AI data for this brick's layer block.
    pub block_begin: usize,
    pub block_end: usize,
}

/// Parse an AI file: extract brick placements with positions and vector polygons.
/// Returns (placements, metadata, raw_ai_data) — the raw data is needed by the renderer.
pub fn parse_ai(
    ai_path: &Path,
    canvas_height: i32,
) -> Result<(Vec<BrickPlacement>, ParsedAiMetadata, AiPrivateData), String> {
    // Step 1: decompress AI private data
    let ai_data = decompress_ai_data(ai_path)?;
    let data = &ai_data.raw;

    // Step 2: parse layer tree
    let roots = parse_layer_tree(data);
    let bg = roots.iter().find(|r| r.name == "background")
        .ok_or("No 'background' layer found")?;
    let bricks_node = roots.iter().find(|r| r.name == "bricks")
        .ok_or("No 'bricks' layer found")?;

    // Step 3: open as PDF for page geometry
    let doc = mupdf::pdf::PdfDocument::open(ai_path.to_str().unwrap_or(""))
        .map_err(|e| format!("Failed to open AI as PDF: {e}"))?;
    let page = doc.load_page(0)
        .map_err(|e| format!("Failed to load page: {e}"))?;

    // Get artbox via FFI (more accurate than page.bounds() which returns mediabox)
    let artbox = mupdf_ffi::pdf_page_artbox(ai_path.to_str().unwrap_or(""));
    eprintln!("[parse] artbox via FFI: {:?}", artbox);
    eprintln!("[parse] page.bounds(): {:?}", page.bounds());

    let (offset_x, y_base) = compute_ai_transform(bg, data, &page, artbox);

    // Step 4: collect brick placements (PyMuPDF y-down coords)
    struct RawPlacement<'a> {
        child: &'a LayerBlock,
        pymu_bbox: (f64, f64, f64, f64), // x0, y_top, x1, y_bottom
        layer_type: String,
    }

    let mut placements: Vec<RawPlacement> = Vec::new();
    for child in &bricks_node.children {
        let block_data = &data[child.begin..child.end];
        let is_gradient = has_gradient(block_data);
        let mat = extract_raster_matrix(block_data);

        if let Some((tx, ty, w_pts, h_pts)) = mat {
            if !is_gradient {
                let pymu_x0 = tx + offset_x;
                let pymu_y_top = y_base - ty;
                let pymu_x1 = tx + w_pts + offset_x;
                let pymu_y_bottom = y_base - ty + h_pts;
                let has_vector = extract_plain_path_bbox(child, data).is_some();
                let ltype = if has_vector { "mixed_brick" } else { "brick" };
                placements.push(RawPlacement {
                    child,
                    pymu_bbox: (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom),
                    layer_type: ltype.to_string(),
                });
                continue;
            }
        }

        // Gradient/vector-only brick
        if let Some((ai_xmin, ai_ymin, ai_xmax, ai_ymax)) = extract_plain_path_bbox(child, data) {
            let pymu_x0 = ai_xmin + offset_x;
            let pymu_x1 = ai_xmax + offset_x;
            let pymu_y_top = y_base - ai_ymax;
            let pymu_y_bottom = y_base - ai_ymin;
            placements.push(RawPlacement {
                child,
                pymu_bbox: (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom),
                layer_type: "vector_brick".to_string(),
            });
        }
    }

    if placements.is_empty() {
        return Err("No brick rasters found in 'bricks' layer".to_string());
    }

    // Compute clip rect and scale
    let all_x0: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.0).collect();
    let all_y0: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.1).collect();
    let all_x1: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.2).collect();
    let all_y1: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.3).collect();

    let clip_x0 = all_x0.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let clip_y0 = all_y0.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let page_rect = mupdf_ffi::page_artbox(&page);
    let clip_x1 = all_x1.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(page_rect.2 as f64);
    let clip_y1 = *all_y1.iter().max_by(|a, b| a.partial_cmp(b).unwrap()).unwrap();

    let clip_h_pts = clip_y1 - clip_y0;
    let clip_w_pts = clip_x1 - clip_x0;
    let dpi = if clip_h_pts > 0.0 { canvas_height as f64 / clip_h_pts * 72.0 } else { 72.0 };
    let scale = dpi / 72.0;
    eprintln!("[parse] clip_rect: ({:.1}, {:.1}, {:.1}, {:.1})", clip_x0, clip_y0, clip_x1, clip_y1);
    eprintln!("[parse] clip_size: {:.1} x {:.1} pts, DPI={:.2}, scale={:.4}", clip_w_pts, clip_h_pts, dpi, scale);
    eprintln!("[parse] page_rect: {:?}", page_rect);
    let canvas_w_px = (clip_w_pts * scale).round() as i32;
    let canvas_h_px = (clip_h_pts * scale).round() as i32;

    // Screen frame height
    let screen_node = roots.iter().find(|r| r.name.eq_ignore_ascii_case("screen"));
    let mut screen_frame_height_px: f64 = 0.0;
    if let Some(sn) = screen_node {
        let targets = if sn.children.is_empty() { vec![sn] } else { sn.children.iter().collect() };
        for t in targets {
            if let Some((_, ai_ymin, _, ai_ymax)) = extract_plain_path_bbox(t, data) {
                let screen_h_pts = (y_base - ai_ymin) - (y_base - ai_ymax);
                screen_frame_height_px = screen_h_pts * scale;
                break;
            }
        }
    }

    // Build BrickPlacements — deduplicate by bbox, extract polygons
    let mut seen_bbox = std::collections::HashSet::new();
    let mut results: Vec<BrickPlacement> = Vec::new();
    let mut skipped_bricks: Vec<String> = Vec::new();

    for p in &placements {
        let px = ((p.pymu_bbox.0 - clip_x0) * scale).round() as i32;
        let py = ((p.pymu_bbox.1 - clip_y0) * scale).round() as i32;
        let pw = ((p.pymu_bbox.2 - p.pymu_bbox.0) * scale).round().max(1.0) as i32;
        let ph = ((p.pymu_bbox.3 - p.pymu_bbox.1) * scale).round().max(1.0) as i32;

        let bbox_key = (px, py, pw, ph);
        if seen_bbox.contains(&bbox_key) {
            continue;
        }
        seen_bbox.insert(bbox_key);

        // Extract vector polygon
        let poly_pymu = extract_vector_path(p.child, data, offset_x, y_base);
        let polygon = if poly_pymu.len() >= 3 {
            Some(
                poly_pymu.iter().map(|pt| {
                    [(pt[0] - clip_x0) * scale - px as f64,
                     (pt[1] - clip_y0) * scale - py as f64]
                }).collect()
            )
        } else {
            skipped_bricks.push(p.child.name.clone());
            None
        };

        results.push(BrickPlacement {
            name: p.child.name.clone(),
            layer_type: p.layer_type.clone(),
            x: px,
            y: py,
            width: pw,
            height: ph,
            polygon,
            block_begin: p.child.begin,
            block_end: p.child.end,
        });
    }

    // Filter out bricks without polygons
    results.retain(|b| b.polygon.is_some());

    let metadata = ParsedAiMetadata {
        canvas_width: canvas_w_px,
        canvas_height: canvas_h_px,
        render_dpi: dpi,
        clip_rect: (clip_x0, clip_y0, clip_x1, clip_y1),
        screen_frame_height_px,
        skipped_bricks,
    };

    Ok((results, metadata, ai_data))
}

/// Metadata from AI parsing (canvas geometry, DPI, etc.)
#[derive(Debug, Clone)]
pub struct ParsedAiMetadata {
    pub canvas_width: i32,
    pub canvas_height: i32,
    pub render_dpi: f64,
    pub clip_rect: (f64, f64, f64, f64),
    pub screen_frame_height_px: f64,
    pub skipped_bricks: Vec<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ai_path() -> Option<std::path::PathBuf> {
        let p = std::path::PathBuf::from("../../in/_NY1.ai");
        if p.exists() { Some(p) } else { None }
    }

    #[test]
    fn test_decompress_ai_data() {
        let path = match test_ai_path() {
            Some(p) => p,
            None => { eprintln!("Skipping: in/_NY1.ai not found"); return; }
        };
        let data = decompress_ai_data(&path).unwrap();
        assert!(!data.raw.is_empty());
        assert!(data.raw.windows(15).any(|w| w == b"%AI5_BeginLayer"));
        eprintln!("Decompressed {} bytes", data.raw.len());
    }

    #[test]
    fn test_parse_layer_tree() {
        let path = match test_ai_path() {
            Some(p) => p,
            None => { eprintln!("Skipping: in/_NY1.ai not found"); return; }
        };
        let data = decompress_ai_data(&path).unwrap();
        let roots = parse_layer_tree(&data.raw);
        assert!(!roots.is_empty());

        let names: Vec<&str> = roots.iter().map(|r| r.name.as_str()).collect();
        eprintln!("Top-level layers: {:?}", names);
        assert!(names.contains(&"background"), "Missing 'background', found: {:?}", names);
        assert!(names.contains(&"bricks"), "Missing 'bricks', found: {:?}", names);

        let bricks = roots.iter().find(|r| r.name == "bricks").unwrap();
        eprintln!("Bricks layer has {} children", bricks.children.len());
        assert!(bricks.children.len() > 100, "Expected >100 bricks, got {}", bricks.children.len());
    }

    #[test]
    fn test_parse_ai() {
        let path = match test_ai_path() {
            Some(p) => p,
            None => { eprintln!("Skipping: in/_NY1.ai not found"); return; }
        };

        let (bricks, meta, _ai_data) = parse_ai(&path, 900).unwrap();

        eprintln!("Canvas: {}x{}", meta.canvas_width, meta.canvas_height);
        eprintln!("DPI: {:.2}", meta.render_dpi);
        eprintln!("Bricks: {}", bricks.len());
        eprintln!("Skipped: {:?}", meta.skipped_bricks);
        eprintln!("Screen frame height: {:.1}px", meta.screen_frame_height_px);

        // Python produces 183 bricks for NY1 at canvas_height=900
        assert_eq!(bricks.len(), 183, "Expected 183 bricks, got {}", bricks.len());
        assert_eq!(meta.canvas_width, 494);
        assert_eq!(meta.canvas_height, 900);
        assert!(meta.render_dpi > 0.0);
        assert!(meta.screen_frame_height_px > 0.0);

        // All bricks should have polygons (filtered out those without)
        assert!(bricks.iter().all(|b| b.polygon.is_some()));

        // Check a known brick position (first brick in Python output)
        let first = &bricks[0];
        eprintln!("First brick: {} at ({}, {}) {}x{}", first.name, first.x, first.y, first.width, first.height);
    }
}
