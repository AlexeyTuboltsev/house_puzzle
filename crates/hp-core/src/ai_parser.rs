//! AI file parser — extracts brick layers, geometry, and vector polygons.
//!
//! AI files are PDF-based with embedded PostScript data (AIPrivateData streams).
//! This module:
//! 1. Extracts and decompresses AIPrivateData via MuPDF FFI
//! 2. Parses the PostScript layer tree
//! 3. (TODO) Extracts brick placement and vector polygon data

use anyhow::{Context, Result, bail};
use regex::bytes::Regex;
use std::path::Path;

use crate::mupdf_ffi;

/// Helper: convert a byte slice to &str (ASCII portion).
/// Split bytes into lines, handling \r, \n, and \r\n.
/// Returns (line_bytes, line_start_offset) pairs.
fn split_lines(data: &[u8]) -> Vec<(&[u8], usize)> {
    let mut result = Vec::new();
    let mut start = 0;
    let mut i = 0;
    while i < data.len() {
        if data[i] == b'\r' {
            result.push((&data[start..i], start));
            if i + 1 < data.len() && data[i + 1] == b'\n' {
                i += 1; // skip \n in \r\n
            }
            start = i + 1;
        } else if data[i] == b'\n' {
            result.push((&data[start..i], start));
            start = i + 1;
        }
        i += 1;
    }
    if start < data.len() {
        result.push((&data[start..], start));
    }
    result
}

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
pub fn decompress_ai_data(ai_path: &Path) -> Result<AiPrivateData> {
    let doc = mupdf::pdf::PdfDocument::open(ai_path.to_str().unwrap_or(""))
        .context("opening AI file as PDF")?;

    // Find AIPrivateData references using dictionary-level access
    let pairs = mupdf_ffi::find_ai_private_data(&doc);
    if pairs.is_empty() {
        bail!("No AIPrivateData found in .ai file");
    }

    // Concatenate all stream data
    let mut raw = Vec::new();
    for (_, xref) in &pairs {
        if let Some(data) = mupdf_ffi::xref_stream(&doc, *xref) {
            raw.extend_from_slice(&data);
        }
    }

    if raw.is_empty() {
        bail!("AIPrivateData streams are empty");
    }

    // Find ZStandard frame magic: 0x28 0xB5 0x2F 0xFD
    let magic = [0x28u8, 0xB5, 0x2F, 0xFD];
    let pos = raw.windows(4)
        .position(|w| w == magic)
        .context("ZStandard magic not found in AIPrivateData")?;

    // Decompress
    let compressed = &raw[pos..];
    let decompressed = zstd::decode_all(std::io::Cursor::new(compressed))
        .context("ZStd decompression failed")?;

    Ok(AiPrivateData {
        raw: decompressed,
    })
}

/// Parse `%AI5_BeginLayer` / `%AI5_EndLayer` pairs into a nested tree.
/// Operates on raw bytes — all offsets are byte positions.
pub fn parse_layer_tree(data: &[u8]) -> Vec<LayerBlock> {
    // memmem byte-search is ~50× faster than the regex DFA for these
    // literal needles, and that's the dominant cost on a typical AI
    // (~50 MB of decompressed PostScript).
    let begin_finder = memchr::memmem::Finder::new(b"%AI5_BeginLayer");
    let end_finder = memchr::memmem::Finder::new(b"%AI5_EndLayer");
    static NAME_RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let name_re = NAME_RE.get_or_init(|| {
        Regex::new(r"Lb[\r\n]+\(([^)]*)\)").expect("static regex pattern is valid")
    });

    let mut events: Vec<(char, usize)> = Vec::new();
    for pos in begin_finder.find_iter(data) {
        events.push(('B', pos));
    }
    for pos in end_finder.find_iter(data) {
        events.push(('E', pos));
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
    let coord_re = Regex::new(r"(-?\d+\.?\d*)\s+(-?\d+\.?\d*)\s+[mLCl]\b")
        .expect("static regex pattern is valid");

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
        let b = mupdf_ffi::page_mediabox(page);
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
    for (line_bytes, _) in split_lines(block_data) {
        let s = bstr(line_bytes).trim();
        if s.ends_with("Bg") && s.contains('(') {
            return true;
        }
    }
    false
}

/// Extract the raster placement matrix from an Xh operator.
/// Returns (tx, ty, w_pts, h_pts) in AI coordinate space.
fn extract_raster_matrix(block_data: &[u8]) -> Option<(f64, f64, f64, f64)> {
    // Compile this regex once across all calls — we hit it on every brick
    // (~600× per AI), and `regex::Regex::new` is multi-millisecond.
    static RE: std::sync::OnceLock<Regex> = std::sync::OnceLock::new();
    let re = RE.get_or_init(|| {
        let num = r"-?\d+(?:\.\d+)?";
        let pattern = format!(
            r"\[\s*({n})\s+{n}\s+{n}\s+({n})\s+({n})\s+({n})\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh",
            n = num
        );
        Regex::new(&pattern).expect("static regex pattern is valid")
    });
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

    // Find %%BeginData / %%EndData byte ranges to skip binary raster data.
    // Use byte-level search (not line splitting) for precise matching.
    let begin_marker = b"%%BeginData:";
    let end_marker = b"%%EndData";
    let mut skip_ranges: Vec<(usize, usize)> = Vec::new();
    {
        let mut pos = 0;
        while let Some(start) = block_data[pos..].windows(begin_marker.len())
            .position(|w| w == begin_marker)
        {
            let abs_start = pos + start;
            if let Some(end) = block_data[abs_start..].windows(end_marker.len())
                .position(|w| w == end_marker)
            {
                let abs_end = abs_start + end + end_marker.len();
                skip_ranges.push((abs_start, abs_end));
                pos = abs_end;
            } else {
                // No matching EndData — skip rest of block
                skip_ranges.push((abs_start, block_data.len()));
                break;
            }
        }
    }

    let in_skip_range = |byte_offset: usize| -> bool {
        skip_ranges.iter().any(|&(s, e)| byte_offset >= s && byte_offset < e)
    };

    for (line_bytes, line_start) in split_lines(block_data) {
        if in_skip_range(line_start) {
            continue;
        }
        let line = bstr(line_bytes).trim().to_string();
        if line.starts_with('%') {
            continue;
        }
        let parts: Vec<&str> = line.split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        let op = *parts.last().expect("guarded by parts.is_empty() check");
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

/// Shoelace formula for polygon area (absolute value).
fn polygon_area(pts: &[[f64; 2]]) -> f64 {
    if pts.len() < 3 { return 0.0; }
    let mut area = 0.0;
    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        area += pts[i][0] * pts[j][1];
        area -= pts[j][0] * pts[i][1];
    }
    area.abs() / 2.0
}

/// Parse path operator lines into bezier paths (curves preserved).
/// Returns (paths, open_path_count). Coords are in PyMuPDF y-down space.
pub fn parse_path_lines_bezier(
    lines: &[Vec<&str>],
    offset_x: f64,
    y_base: f64,
) -> (Vec<crate::bezier::BezierPath>, usize) {
    use crate::bezier::{BezierPath, Segment};

    let to_pymu = |ax: f64, ay: f64| -> [f64; 2] { [ax + offset_x, y_base - ay] };

    let mut start: Option<[f64; 2]> = None;
    let mut segs: Vec<Segment> = Vec::new();
    let mut prev: Option<[f64; 2]> = None;
    let mut paths: Vec<BezierPath> = Vec::new();
    let mut open_paths = 0usize;

    // Auto-close: if a brick sub-path was drawn open (start ≠ last seg
    // endpoint), append an implicit Line back to start so the merge sees
    // a closed outline. NY9n had 4 such bricks (b012, b020, b022, b026)
    // because the artist forgot the closing edge — every other NY file
    // has zero. The validator should still flag these so the artist
    // fixes them at source.
    let flush_open = |paths: &mut Vec<BezierPath>,
                      open_paths: &mut usize,
                      start: Option<[f64; 2]>,
                      segs: &mut Vec<Segment>| {
        if let (Some(s), true) = (start, !segs.is_empty()) {
            let last = segs.last().map(|g| g.end()).unwrap_or(s);
            let d = ((s[0] - last[0]).powi(2) + (s[1] - last[1]).powi(2)).sqrt();
            if d >= 1.0 {
                segs.push(Segment::Line { to: s });
                *open_paths += 1;
            }
            paths.push(BezierPath { start: s, segments: std::mem::take(segs) });
        }
    };

    for parts in lines {
        if parts.is_empty() { continue; }
        let op = *parts.last().expect("guarded by parts.is_empty() check");
        match op {
            "m" if parts.len() >= 3 => {
                flush_open(&mut paths, &mut open_paths, start, &mut segs);
                let x: f64 = parts[0].parse().unwrap_or(0.0);
                let y: f64 = parts[1].parse().unwrap_or(0.0);
                start = Some(to_pymu(x, y));
                prev = start;
                segs.clear();
            }
            "L" | "l" if parts.len() >= 3 => {
                let x: f64 = parts[0].parse().unwrap_or(0.0);
                let y: f64 = parts[1].parse().unwrap_or(0.0);
                let to = to_pymu(x, y);
                segs.push(Segment::Line { to });
                prev = Some(to);
            }
            "C" | "c" if parts.len() >= 7 => {
                if prev.is_none() { continue; }
                let cp1 = to_pymu(parts[0].parse().unwrap_or(0.0), parts[1].parse().unwrap_or(0.0));
                let cp2 = to_pymu(parts[2].parse().unwrap_or(0.0), parts[3].parse().unwrap_or(0.0));
                let to  = to_pymu(parts[4].parse().unwrap_or(0.0), parts[5].parse().unwrap_or(0.0));
                segs.push(Segment::Cubic { cp1, cp2, to });
                prev = Some(to);
            }
            "n" | "N" | "f" | "F" | "s" | "S" | "b" | "B" => {
                // close/fill/stroke — emit whatever we've collected as closed,
                // auto-closing if start ≠ last segment endpoint.
                if let (Some(s), false) = (start, segs.is_empty()) {
                    let last = segs.last().map(|g| g.end()).unwrap_or(s);
                    let d = ((s[0] - last[0]).powi(2) + (s[1] - last[1]).powi(2)).sqrt();
                    if d >= 1.0 {
                        segs.push(Segment::Line { to: s });
                        open_paths += 1;
                    }
                    paths.push(BezierPath { start: s, segments: std::mem::take(&mut segs) });
                }
                start = None;
                prev = None;
                segs.clear();
            }
            _ => {}
        }
    }
    flush_open(&mut paths, &mut open_paths, start, &mut segs);
    (paths, open_paths)
}

/// Parse path operator lines into polygons (PyMuPDF y-down coords).
/// Returns (polygons, open_path_count).
fn parse_path_lines(
    lines: &[Vec<&str>],
    offset_x: f64,
    y_base: f64,
) -> (Vec<Vec<[f64; 2]>>, usize) {
    let to_pymu = |ax: f64, ay: f64| -> [f64; 2] {
        [ax + offset_x, y_base - ay]
    };

    let mut pts: Vec<[f64; 2]> = Vec::new();
    let mut polygons: Vec<Vec<[f64; 2]>> = Vec::new();
    let mut open_paths = 0usize;

    // Auto-close: if the artist forgot the closing edge (last point ≠
    // start), append the start point so the polygon is geometrically
    // closed. The same fix already lives in `parse_path_lines_bezier`
    // for the same reason — without it, bricks with open sub-paths
    // (e.g. NY8 'Layer 320', NY9n b012/b020/b022/b026) get silently
    // dropped by the downstream "no polygon" validation. `open_paths`
    // is still bumped so the future Illustrator validator can flag the
    // source error.
    let flush_open = |polygons: &mut Vec<Vec<[f64; 2]>>,
                      open_paths: &mut usize,
                      pts: &mut Vec<[f64; 2]>| {
        if pts.len() >= 3 {
            let first = *pts.first().expect("guarded by pts.len() >= 3");
            let last = *pts.last().expect("guarded by pts.len() >= 3");
            let d = ((first[0] - last[0]).powi(2) + (first[1] - last[1]).powi(2)).sqrt();
            if d >= 1.0 {
                pts.push(first);
                *open_paths += 1;
            }
            polygons.push(std::mem::take(pts));
        } else if !pts.is_empty() {
            *open_paths += 1;
            pts.clear();
        }
    };

    for parts in lines {
        if parts.is_empty() {
            continue;
        }
        let op = *parts.last().expect("guarded by parts.is_empty() check");

        match op {
            "m" if parts.len() >= 3 => {
                // New sub-path — flush previous (auto-closing if open).
                flush_open(&mut polygons, &mut open_paths, &mut pts);
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
                let p1 = *pts.last().expect("guarded by pts.is_empty() check");
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
                // Close / fill / stroke operator — flush whatever we've
                // collected, auto-closing if the artist drew it open.
                flush_open(&mut polygons, &mut open_paths, &mut pts);
            }
            _ => {}
        }
    }

    // Trailing path at end of input.
    flush_open(&mut polygons, &mut open_paths, &mut pts);
    (polygons, open_paths)
}

/// Extract the vector polygon for a brick from %_ prefixed PostScript path lines.
///
/// When a layer contains multiple sub-paths, they are classified into 4 cases:
/// 1. **Containment**: larger object fully contains smaller ones (e.g. window frame
///    around glass) → keep only the outermost polygon.
/// 2. **Overlap**: objects overlap → union them into one polygon.
/// 3. **Adjacent**: objects are separate but within `ADJACENCY_DIST` px → union
///    original vectors and bridge gaps with thin rectangles. Outer shapes preserved.
/// 4. **Independent**: objects are far apart → keep only the largest, log a warning.
/// Extract all bezier sub-paths for a brick layer, curves preserved,
/// coordinates in AI PyMuPDF space (y-down). Used by the bezier-merge
/// testbed; avoids tessellation.
pub fn extract_vector_path_bezier(
    block: &LayerBlock,
    data: &[u8],
    offset_x: f64,
    y_base: f64,
) -> Vec<crate::bezier::BezierPath> {
    let block_data = &data[block.begin..block.end];
    let lines: Vec<String> = split_lines(block_data)
        .iter()
        .map(|(l, _)| bstr(l).trim().to_string())
        .collect();

    let mut parsed_lines: Vec<Vec<String>> = Vec::new();
    for line in &lines {
        if !line.starts_with("%_") { continue; }
        let parts: Vec<String> = line[2..].split_whitespace().map(|s| s.to_string()).collect();
        if !parts.is_empty()
            && PATH_OPS.contains(&parts.last().expect("guarded by is_empty").as_str())
        {
            parsed_lines.push(parts);
        }
    }
    if parsed_lines.is_empty() {
        for line in &lines {
            if line.starts_with('%') { continue; }
            let parts: Vec<String> = line.split_whitespace().map(|s| s.to_string()).collect();
            if !parts.is_empty()
                && PATH_OPS.contains(&parts.last().expect("guarded by is_empty").as_str())
                && (parts.len() == 3 || parts.len() == 7)
            {
                let all_numeric = parts[..parts.len() - 1]
                    .iter()
                    .all(|p| p.parse::<f64>().is_ok());
                if all_numeric { parsed_lines.push(parts); }
            }
        }
    }
    let refs: Vec<Vec<&str>> = parsed_lines
        .iter()
        .map(|parts| parts.iter().map(|s| s.as_str()).collect())
        .collect();
    let (paths, _open) = parse_path_lines_bezier(&refs, offset_x, y_base);
    // Drop trivial/degenerate paths (fewer than 2 segments → can't be a closed shape)
    paths.into_iter().filter(|p| p.segments.len() >= 2).collect()
}

fn extract_vector_path(
    block: &LayerBlock,
    data: &[u8],
    offset_x: f64,
    y_base: f64,
    warnings: &mut Vec<String>,
) -> Vec<[f64; 2]> {
    let block_data = &data[block.begin..block.end];

    // Parse all lines into owned strings (handles \r, \n, \r\n)
    let lines: Vec<String> = split_lines(block_data)
        .iter()
        .map(|(l, _)| bstr(l).trim().to_string())
        .collect();

    // Primary: %_ prefixed lines
    let mut parsed_lines: Vec<Vec<String>> = Vec::new();
    for line in &lines {
        if !line.starts_with("%_") {
            continue;
        }
        let parts: Vec<String> = line[2..].split_whitespace().map(|s| s.to_string()).collect();
        if !parts.is_empty() && PATH_OPS.contains(&parts.last().expect("guarded by parts.is_empty() check").as_str()) {
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
                && PATH_OPS.contains(&parts.last().expect("guarded by parts.is_empty() check").as_str())
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

    let trace = block.name == "Layer 81" || block.name == "Layer 82" || block.name == "Layer 83" || block.name == "Layer 84";
    if trace {
        eprintln!("[TRACE] layer '{}': {} parsed path lines", block.name, parsed_lines.len());
        for (i, parts) in parsed_lines.iter().enumerate() {
            eprintln!("[TRACE]   line {}: {}", i, parts.join(" "));
        }
    }

    // Convert to &str slices for parse_path_lines
    let refs: Vec<Vec<&str>> = parsed_lines.iter()
        .map(|parts| parts.iter().map(|s| s.as_str()).collect())
        .collect();
    let (polygons, open_paths) = parse_path_lines(&refs, offset_x, y_base);

    if open_paths > 0 {
        warnings.push(format!(
            "Layer '{}': {} unclosed path(s) — discarded (open paths are not valid brick outlines)",
            block.name, open_paths
        ));
    }

    if trace {
        eprintln!("[TRACE] layer '{}': parse_path_lines produced {} polygons, {} open paths discarded",
            block.name, polygons.len(), open_paths);
        for (i, poly) in polygons.iter().enumerate() {
            let area = polygon_area(poly);
            eprintln!("[TRACE]   polygon {}: {} pts, area={:.1}", i, poly.len(), area);
            for (j, pt) in poly.iter().enumerate() {
                eprintln!("[TRACE]     v{}: ({:.1}, {:.1})", j, pt[0], pt[1]);
            }
        }
    }

    // Filter to significant polygons (≥3 points, area > 10px²)
    let significant: Vec<Vec<[f64; 2]>> = polygons.into_iter()
        .filter(|p| p.len() >= 3 && polygon_area(p).abs() > 10.0)
        .collect();

    if trace {
        eprintln!("[TRACE] layer '{}': {} significant polygons after filter", block.name, significant.len());
    }

    if significant.is_empty() {
        return vec![];
    }
    if significant.len() == 1 {
        if trace {
            eprintln!("[TRACE] layer '{}': single polygon, returning as-is", block.name);
        }
        return significant.into_iter().next().unwrap();
    }

    // --- Multiple polygons on this layer: classify each pair ---
    use geo::{Coord, LineString, Polygon as GeoPoly};
    use geo::algorithm::area::Area;
    use geo_clipper::Clipper;

    const ADJACENCY_DIST: f64 = 15.0;
    const GAP_BRIDGE_WIDTH: f64 = 2.0;
    let factor = 1000.0;

    // Compute bboxes
    let bboxes: Vec<(f64, f64, f64, f64)> = significant.iter().map(|poly| {
        let (mut x0, mut y0) = (f64::INFINITY, f64::INFINITY);
        let (mut x1, mut y1) = (f64::NEG_INFINITY, f64::NEG_INFINITY);
        for p in poly { x0 = x0.min(p[0]); y0 = y0.min(p[1]); x1 = x1.max(p[0]); y1 = y1.max(p[1]); }
        (x0, y0, x1, y1)
    }).collect();

    let bbox_contains = |outer: (f64, f64, f64, f64), inner: (f64, f64, f64, f64)| -> bool {
        let m = 2.0;
        inner.0 >= outer.0 - m && inner.1 >= outer.1 - m
            && inner.2 <= outer.2 + m && inner.3 <= outer.3 + m
    };

    let bbox_overlaps = |a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)| -> bool {
        a.0 < b.2 && a.2 > b.0 && a.1 < b.3 && a.3 > b.1
    };

    let bbox_distance = |a: (f64, f64, f64, f64), b: (f64, f64, f64, f64)| -> f64 {
        let dx = if a.2 < b.0 { b.0 - a.2 } else if b.2 < a.0 { a.0 - b.2 } else { 0.0 };
        let dy = if a.3 < b.1 { b.1 - a.3 } else if b.3 < a.1 { a.1 - b.3 } else { 0.0 };
        (dx * dx + dy * dy).sqrt()
    };

    let n = significant.len();

    // Sort by area descending — index 0 is the largest
    let mut order: Vec<usize> = (0..n).collect();
    order.sort_by(|&a, &b| {
        polygon_area(&significant[b]).abs()
            .partial_cmp(&polygon_area(&significant[a]).abs())
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    // Classify each smaller polygon relative to the largest and its group
    // Start with the largest polygon; try to absorb others.
    let mut absorbed: Vec<bool> = vec![false; n];
    let mut to_merge: Vec<usize> = vec![order[0]]; // always include the largest

    for &i in &order[1..] {
        let mut relationship = "independent";

        let area_i = polygon_area(&significant[i]).abs();

        // Check against all already-absorbed polygons for any connection
        for &j in &to_merge {
            let area_j = polygon_area(&significant[j]).abs();

            if bbox_contains(bboxes[j], bboxes[i]) {
                // Bbox is contained — but is it truly containment (glass inside
                // frame) or two halves of a diagonal split?
                // True containment: inner is small relative to outer (< 30%).
                // Diagonal split: both halves are large relative to each other.
                let ratio = if area_j > 0.0 { area_i / area_j } else { 1.0 };
                if ratio < 0.3 {
                    // Case 1: genuinely contained (e.g. glass inside frame)
                    relationship = "contained";
                } else {
                    // Case 2: overlapping halves (e.g. diagonal window split)
                    relationship = "overlap";
                }
                break;
            }
            if bbox_overlaps(bboxes[j], bboxes[i]) {
                // Case 2: overlap — include for union
                relationship = "overlap";
                break;
            }
            if bbox_distance(bboxes[j], bboxes[i]) < ADJACENCY_DIST {
                // Case 3: adjacent within threshold — include for union+bridge
                relationship = "adjacent";
                break;
            }
        }

        if trace {
            eprintln!("[TRACE] layer '{}': poly {} → {}", block.name, i, relationship);
        }

        match relationship {
            "contained" => {
                // Case 1: fully contained — just drop it
                absorbed[i] = true;
            }
            "overlap" | "adjacent" => {
                // Cases 2 & 3: include in merge group
                to_merge.push(i);
                absorbed[i] = true;
            }
            _ => {
                // Case 4: independent — will be discarded
                absorbed[i] = true;
            }
        }
    }

    // Log case 4 discards
    let discarded: Vec<usize> = (0..n).filter(|i| {
        absorbed[*i] && !to_merge.contains(i)
    }).collect();
    if !discarded.is_empty() {
        warnings.push(format!(
            "MULTI_OBJECT: layer '{}' has {} polygons, discarded {} independent objects",
            block.name, n, discarded.len()
        ));
    }

    // If only the largest survived, return it directly
    if to_merge.len() == 1 {
        return significant.into_iter().nth(to_merge[0]).unwrap();
    }

    // Convert merge group to geo Polygons
    let mut geo_polys: Vec<GeoPoly<f64>> = Vec::new();
    for &idx in &to_merge {
        let pts = &significant[idx];
        let mut coords: Vec<Coord<f64>> = pts.iter()
            .map(|p| Coord { x: p[0], y: p[1] })
            .collect();
        if coords.len() < 3 { continue; }
        if coords.first() != coords.last() {
            coords.push(coords[0]);
        }
        let poly = GeoPoly::new(LineString::new(coords), vec![]);
        if poly.unsigned_area() > 1.0 {
            geo_polys.push(poly);
        }
    }

    if geo_polys.is_empty() {
        return significant.into_iter().next().unwrap();
    }
    if geo_polys.len() == 1 {
        return geo_polys[0].exterior().0.iter().map(|c| [c.x, c.y]).collect();
    }

    // Union all polygons in the merge group (handles cases 2 & 3)
    let mut union = geo::MultiPolygon(vec![geo_polys[0].clone()]);
    for poly in &geo_polys[1..] {
        union = Clipper::union(&union, poly, factor);
    }
    union.0.retain(|p| p.unsigned_area() > 1.0);

    // Case 3: bridge remaining gaps between disconnected components
    if union.0.len() > 1 {
        let mut bridges: Vec<GeoPoly<f64>> = Vec::new();
        for i in 0..union.0.len() {
            for j in (i + 1)..union.0.len() {
                let (dist, pt_a, pt_b) =
                    crate::puzzle::nearest_edge_points(union.0[i].exterior(), union.0[j].exterior());
                if dist < ADJACENCY_DIST {
                    bridges.push(crate::puzzle::make_bridge_rect(pt_a, pt_b, GAP_BRIDGE_WIDTH));
                }
            }
        }
        for bridge in &bridges {
            union = Clipper::union(&union, bridge, factor);
        }
        union.0.retain(|p| p.unsigned_area() > 1.0);
    }

    // Take the largest resulting polygon
    let final_poly = union.0.into_iter()
        .max_by(|a, b| a.unsigned_area().partial_cmp(&b.unsigned_area())
            .unwrap_or(std::cmp::Ordering::Equal))
        .unwrap();

    let result: Vec<[f64; 2]> = final_poly.exterior().0.iter().map(|c| [c.x, c.y]).collect();
    if trace {
        eprintln!("[TRACE] layer '{}': final merged polygon {} pts, area={:.1}",
            block.name, result.len(), final_poly.unsigned_area());
        for (i, pt) in result.iter().enumerate() {
            eprintln!("[TRACE]   v{}: ({:.1}, {:.1})", i, pt[0], pt[1]);
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Main parse function: collect brick placements + polygons
// ---------------------------------------------------------------------------

/// A brick placement extracted from the AI file.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
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
) -> Result<(Vec<BrickPlacement>, ParsedAiMetadata, AiPrivateData)> {
    // Step 1: decompress AI private data
    let t0 = std::time::Instant::now();
    let ai_data = decompress_ai_data(ai_path)?;
    eprintln!("[parse_ai] decompress: {:?}", t0.elapsed());
    let data = &ai_data.raw;

    // Step 2: parse layer tree
    let t0 = std::time::Instant::now();
    let roots = parse_layer_tree(data);
    eprintln!("[parse_ai] layer_tree: {:?} — roots: {:?}", t0.elapsed(), roots.iter().map(|r| &r.name).collect::<Vec<_>>());
    // Hard blocks: required layers must exist
    let root_names: Vec<&str> = roots.iter().map(|r| r.name.as_str()).collect();
    let bg = roots.iter().find(|r| r.name == "background")
        .context("Missing required layer 'background'")?;
    let bricks_node = roots.iter().find(|r| r.name == "bricks")
        .context("Missing required layer 'bricks'")?;
    if bricks_node.children.is_empty() {
        anyhow::bail!("Layer 'bricks' is empty — no brick sub-layers found");
    }
    if !root_names.contains(&"screen") {
        eprintln!("[parse_ai] WARNING: layer 'screen' is missing — DPI will be estimated");
    }

    // Step 3: open as PDF for page geometry
    let doc = mupdf::pdf::PdfDocument::open(ai_path.to_str().unwrap_or(""))
        .context("opening AI file as PDF for page geometry")?;
    let page = doc.load_page(0)
        .context("loading page 0 of AI file")?;

    // Get artbox via FFI (more accurate than page.bounds() which returns mediabox)
    let artbox = mupdf_ffi::pdf_page_artbox(ai_path.to_str().unwrap_or(""));
    eprintln!("[parse] artbox via FFI: {:?}", artbox);
    eprintln!("[parse] page.bounds(): {:?}", page.bounds());

    let t0 = std::time::Instant::now();
    let (offset_x, y_base) = compute_ai_transform(bg, data, &page, artbox);
    eprintln!("[parse_ai] transform: {:?}", t0.elapsed());

    // Step 4: collect brick placements (PyMuPDF y-down coords).
    // Each child's classification is independent — fan out via rayon.
    let t0 = std::time::Instant::now();
    struct RawPlacement<'a> {
        child: &'a LayerBlock,
        pymu_bbox: (f64, f64, f64, f64), // x0, y_top, x1, y_bottom
        layer_type: String,
    }

    use rayon::prelude::*;
    let placements: Vec<RawPlacement> = bricks_node
        .children
        .par_iter()
        .filter_map(|child| {
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
                    return Some(RawPlacement {
                        child,
                        pymu_bbox: (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom),
                        layer_type: ltype.to_string(),
                    });
                }
            }
            // Gradient/vector-only brick
            if let Some((ai_xmin, ai_ymin, ai_xmax, ai_ymax)) =
                extract_plain_path_bbox(child, data)
            {
                let pymu_x0 = ai_xmin + offset_x;
                let pymu_x1 = ai_xmax + offset_x;
                let pymu_y_top = y_base - ai_ymax;
                let pymu_y_bottom = y_base - ai_ymin;
                return Some(RawPlacement {
                    child,
                    pymu_bbox: (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom),
                    layer_type: "vector_brick".to_string(),
                });
            }
            None
        })
        .collect();

    if placements.is_empty() {
        bail!("No brick rasters found in 'bricks' layer");
    }

    eprintln!("[parse_ai] placements: {:?} ({} bricks)", t0.elapsed(), placements.len());

    // Compute clip rect and scale
    let all_x0: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.0).collect();
    let all_y0: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.1).collect();
    let all_x1: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.2).collect();
    let all_y1: Vec<f64> = placements.iter().map(|p| p.pymu_bbox.3).collect();

    let page_rect = mupdf_ffi::page_mediabox(&page);
    let clip_x0 = all_x0.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let clip_y0 = all_y0.iter().cloned().fold(f64::INFINITY, f64::min).max(0.0);
    let clip_x1 = all_x1.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(page_rect.2 as f64);
    let clip_y1 = all_y1.iter().cloned().fold(f64::NEG_INFINITY, f64::max).min(page_rect.3 as f64);

    let clip_h_pts = clip_y1 - clip_y0;
    let clip_w_pts = clip_x1 - clip_x0;

    // Screen frame height in PDF points — determines the rendering scale.
    // The artist draws a `screen` layer rectangle representing
    // `HOUSE_UNITS_HIGH` Unity units; we render at the DPI that makes
    // that frame `CANVAS_HEIGHT_PX` pixels tall.
    let pixels_per_unit: f64 = crate::CANVAS_HEIGHT_PX as f64 / crate::HOUSE_UNITS_HIGH;
    let screen_node = roots.iter().find(|r| r.name.eq_ignore_ascii_case("screen"));
    let mut screen_frame_h_pts: f64 = 0.0;
    if let Some(sn) = screen_node {
        let targets = if sn.children.is_empty() { vec![sn] } else { sn.children.iter().collect() };
        for t in targets {
            if let Some((_, ai_ymin, _, ai_ymax)) = extract_plain_path_bbox(t, data) {
                screen_frame_h_pts = (y_base - ai_ymin) - (y_base - ai_ymax);
                break;
            }
        }
    }

    // DPI from screen frame: HOUSE_UNITS_HIGH units = screen_frame_h_pts
    // PDF points = HOUSE_UNITS_HIGH * pixels_per_unit pixels.
    let dpi = if screen_frame_h_pts > 0.0 {
        pixels_per_unit * crate::HOUSE_UNITS_HIGH / screen_frame_h_pts * 72.0
    } else if clip_h_pts > 0.0 {
        // Fallback: fit canvas_height
        canvas_height as f64 / clip_h_pts * 72.0
    } else {
        72.0
    };
    let scale = dpi / 72.0;

    let canvas_w_px = (clip_w_pts * scale).round() as i32;
    let canvas_h_px = (clip_h_pts * scale).round() as i32;
    let screen_frame_height_px = screen_frame_h_pts * scale;

    eprintln!("[parse] clip: {:.0}x{:.0} pts, screen_frame: {:.0} pts", clip_w_pts, clip_h_pts, screen_frame_h_pts);
    eprintln!("[parse] DPI={:.2}, scale={:.4}, canvas={}x{}", dpi, scale, canvas_w_px, canvas_h_px);

    // Expected brick min position — used by caller to compute pdf_offset
    // from the OCG bricks layer render (avoids duplicate render).
    let expected_brick_min_x = ((all_x0.iter().cloned().fold(f64::INFINITY, f64::min) - clip_x0) * scale).round() as i32;
    let expected_brick_min_y = ((all_y0.iter().cloned().fold(f64::INFINITY, f64::min) - clip_y0) * scale).round() as i32;

    // Build BrickPlacements — deduplicate by bbox, extract polygons.
    //
    // The expensive part is `extract_vector_path` (PostScript parsing per
    // brick).  Run that in parallel for every placement that survives the
    // bbox dedup, then assemble the final list sequentially so we keep
    // deterministic ordering.
    let t0 = std::time::Instant::now();
    let mut seen_bbox = std::collections::HashSet::new();
    let mut keep_idx: Vec<usize> = Vec::with_capacity(placements.len());
    let mut bbox_px: Vec<(i32, i32, i32, i32)> = Vec::with_capacity(placements.len());
    for (i, p) in placements.iter().enumerate() {
        let px = ((p.pymu_bbox.0 - clip_x0) * scale).round() as i32;
        let py = ((p.pymu_bbox.1 - clip_y0) * scale).round() as i32;
        let pw = ((p.pymu_bbox.2 - p.pymu_bbox.0) * scale).round().max(1.0) as i32;
        let ph = ((p.pymu_bbox.3 - p.pymu_bbox.1) * scale).round().max(1.0) as i32;
        let key = (px, py, pw, ph);
        if seen_bbox.insert(key) {
            keep_idx.push(i);
            bbox_px.push(key);
        }
    }

    // Parallel polygon extraction.
    let polys_with_warnings: Vec<(Vec<[f64; 2]>, Vec<String>)> = keep_idx
        .par_iter()
        .map(|&i| {
            let mut local_warnings: Vec<String> = Vec::new();
            let poly = extract_vector_path(
                placements[i].child, data, offset_x, y_base, &mut local_warnings,
            );
            (poly, local_warnings)
        })
        .collect();

    let mut results: Vec<BrickPlacement> = Vec::with_capacity(keep_idx.len());
    let mut skipped_bricks: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    for (slot, &orig_i) in keep_idx.iter().enumerate() {
        let p = &placements[orig_i];
        let (px, py, pw, ph) = bbox_px[slot];
        let (poly_pymu, local_warnings) = &polys_with_warnings[slot];
        warnings.extend(local_warnings.iter().cloned());
        let _ = px; let _ = py; let _ = pw; let _ = ph;
        // (Re-bind below to keep the rest of the original block unchanged.)
        let polygon: Option<Vec<[f64; 2]>> = if poly_pymu.len() >= 3 {
            Some(
                poly_pymu.iter().map(|pt| {
                    [(pt[0] - clip_x0) * scale - px as f64,
                     (pt[1] - clip_y0) * scale - py as f64]
                }).collect()
            )
        } else {
            // No vector polygon — brick will be discarded in validation
            None
        };

        // Note: multi-object layers are now handled in extract_vector_path(),
        // which groups spatially adjacent polygons (within 15px) and merges
        // them via convex hull. No separate warning needed here.

        // Use polygon bounding box as the brick's true extent.
        // The initial px/py/pw/ph came from raster matrix or plain path bbox,
        // but the polygon is the authoritative shape for ALL bricks.
        let (fx, fy, fw, fh) = if let Some(ref poly) = polygon {
            if poly.len() >= 3 {
                let min_x = poly.iter().map(|p| p[0]).fold(f64::INFINITY, f64::min);
                let min_y = poly.iter().map(|p| p[1]).fold(f64::INFINITY, f64::min);
                let max_x = poly.iter().map(|p| p[0]).fold(f64::NEG_INFINITY, f64::max);
                let max_y = poly.iter().map(|p| p[1]).fold(f64::NEG_INFINITY, f64::max);
                let new_x = px + min_x.floor() as i32;
                let new_y = py + min_y.floor() as i32;
                let new_w = (max_x - min_x).ceil() as i32;
                let new_h = (max_y - min_y).ceil() as i32;
                (new_x, new_y, new_w.max(1), new_h.max(1))
            } else {
                (px, py, pw, ph)
            }
        } else {
            (px, py, pw, ph)
        };

        // Shift polygon to new origin
        let polygon = if fx != px || fy != py {
            polygon.map(|poly| {
                let sx = (px - fx) as f64;
                let sy = (py - fy) as f64;
                poly.iter().map(|p| [p[0] + sx, p[1] + sy]).collect()
            })
        } else {
            polygon
        };

        results.push(BrickPlacement {
            name: p.child.name.clone(),
            layer_type: p.layer_type.clone(),
            x: fx,
            y: fy,
            width: fw,
            height: fh,
            polygon,
            block_begin: p.child.begin,
            block_end: p.child.end,
        });
    }

    eprintln!("[parse_ai] build_placements+polygons: {:?} ({} results)", t0.elapsed(), results.len());

    // ── Validation pass ─────────────────────────────────────────────────

    // 5. Degenerate polygons: < 3 points or near-zero area
    {
        let before = results.len();
        results.retain(|b| {
            match &b.polygon {
                Some(poly) if poly.len() >= 3 => {
                    let area = polygon_area(poly).abs();
                    if area < 1.0 {
                        warnings.push(format!(
                            "Layer '{}': degenerate polygon (area={:.1}) — discarded", b.name, area
                        ));
                        false
                    } else {
                        true
                    }
                }
                Some(poly) => {
                    warnings.push(format!(
                        "Layer '{}': polygon has only {} points — discarded", b.name, poly.len()
                    ));
                    false
                }
                None => {
                    // No polygon at all — already logged by extract_vector_path
                    false
                }
            }
        });
        let removed = before - results.len();
        if removed > 0 {
            eprintln!("[validation] removed {} degenerate/missing polygon bricks", removed);
        }
    }

    // 2 & 3. Overlap / containment detection between bricks from different layers.
    // Convert brick polygons to canvas coords for comparison.
    {
        use geo::{Coord, LineString, Polygon as GeoPoly};
        use geo::algorithm::area::Area;
        use geo::algorithm::bounding_rect::BoundingRect;
        use geo_clipper::Clipper;

        let factor = 1000.0;
        const OVERLAP_THRESHOLD: f64 = 0.1; // 10% of smaller brick's area = significant overlap

        // Build geo polygons in canvas coords for each brick
        let geo_polys: Vec<Option<GeoPoly<f64>>> = results.iter().map(|b| {
            let pts = b.polygon.as_ref()?;
            if pts.len() < 3 { return None; }
            let mut coords: Vec<Coord<f64>> = pts.iter()
                .map(|p| Coord { x: p[0] + b.x as f64, y: p[1] + b.y as f64 })
                .collect();
            if coords.first() != coords.last() {
                coords.push(coords[0]);
            }
            let poly = GeoPoly::new(LineString::new(coords), vec![]);
            if poly.unsigned_area() > 1.0 { Some(poly) } else { None }
        }).collect();

        let mut to_remove: std::collections::HashSet<usize> = std::collections::HashSet::new();

        for i in 0..results.len() {
            if to_remove.contains(&i) { continue; }
            let pa = match &geo_polys[i] { Some(p) => p, None => continue };
            let area_a = pa.unsigned_area();

            for j in (i + 1)..results.len() {
                if to_remove.contains(&j) { continue; }
                let pb = match &geo_polys[j] { Some(p) => p, None => continue };
                let area_b = pb.unsigned_area();

                // Quick bbox check
                let a_bb = pa.bounding_rect();
                let b_bb = pb.bounding_rect();
                if let (Some(a_r), Some(b_r)) = (a_bb, b_bb) {
                    if a_r.max().x < b_r.min().x || b_r.max().x < a_r.min().x
                        || a_r.max().y < b_r.min().y || b_r.max().y < a_r.min().y
                    {
                        continue; // no overlap possible
                    }
                }

                // Compute intersection
                let inter = Clipper::intersection(&geo::MultiPolygon(vec![pa.clone()]),
                                                   pb, factor);
                let inter_area: f64 = inter.0.iter().map(|p| p.unsigned_area()).sum();
                if inter_area < 1.0 { continue; }

                let smaller_area = area_a.min(area_b);
                let overlap_ratio = inter_area / smaller_area;

                if overlap_ratio > 0.9 {
                    // Case 3: near-full containment — discard the smaller brick
                    let (discard, keep) = if area_a < area_b { (i, j) } else { (j, i) };
                    warnings.push(format!(
                        "Layer '{}' is fully contained within Layer '{}' ({:.0}% overlap) — discarded",
                        results[discard].name, results[keep].name, overlap_ratio * 100.0
                    ));
                    to_remove.insert(discard);
                } else if overlap_ratio > OVERLAP_THRESHOLD {
                    // Case 2: significant overlap — discard the smaller, flag it
                    let (discard, keep) = if area_a < area_b { (i, j) } else { (j, i) };
                    warnings.push(format!(
                        "Layer '{}' overlaps Layer '{}' ({:.0}% of smaller area) — Layer '{}' discarded",
                        results[discard].name, results[keep].name, overlap_ratio * 100.0,
                        results[discard].name
                    ));
                    to_remove.insert(discard);
                }
            }
        }

        if !to_remove.is_empty() {
            eprintln!("[validation] removing {} overlapping/contained bricks", to_remove.len());
            let mut idx = 0;
            results.retain(|_| {
                let keep = !to_remove.contains(&idx);
                idx += 1;
                keep
            });
        }
    }

    let has_lights_layer = root_names.contains(&"lights");
    let metadata = ParsedAiMetadata {
        canvas_width: canvas_w_px,
        canvas_height: canvas_h_px,
        render_dpi: dpi,
        clip_rect: (clip_x0, clip_y0, clip_x1, clip_y1),
        screen_frame_height_px,
        skipped_bricks,
        expected_brick_min: (expected_brick_min_x, expected_brick_min_y),
        warnings,
        offset_x,
        y_base,
        has_lights_layer,
    };

    Ok((results, metadata, ai_data))
}

/// Metadata from AI parsing (canvas geometry, DPI, etc.)
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ParsedAiMetadata {
    pub canvas_width: i32,
    pub canvas_height: i32,
    pub render_dpi: f64,
    pub clip_rect: (f64, f64, f64, f64),
    pub screen_frame_height_px: f64,
    pub skipped_bricks: Vec<String>,
    pub expected_brick_min: (i32, i32),
    pub warnings: Vec<String>,
    /// AI → PyMuPDF transform. `pymu_x = ai_x + offset_x`, `pymu_y = y_base - ai_y`.
    /// Exposed so downstream code (testbed, bezier merge) can re-parse path
    /// bytes from the raw AI without redoing the artbox probe.
    pub offset_x: f64,
    pub y_base: f64,
    /// Whether the AI file declares a `lights` OCG layer. Used by the
    /// lazy lights renderer in hp-tauri so the frontend knows whether
    /// to expose the "Show lights" control without a probe round-trip.
    #[serde(default)]
    pub has_lights_layer: bool,
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

        let (bricks, meta, _ai_data) = parse_ai(&path, crate::CANVAS_HEIGHT_PX as i32).unwrap();

        eprintln!("Canvas: {}x{}", meta.canvas_width, meta.canvas_height);
        eprintln!("DPI: {:.2}", meta.render_dpi);
        eprintln!("Bricks: {}", bricks.len());
        eprintln!("Skipped: {:?}", meta.skipped_bricks);
        eprintln!("Screen frame height: {:.1}px", meta.screen_frame_height_px);

        // Python produces 183 bricks for NY1 at canvas_height=900
        assert_eq!(bricks.len(), 183, "Expected 183 bricks, got {}", bricks.len());
        assert_eq!(meta.canvas_width, 494);
        assert_eq!(meta.canvas_height as u32, crate::CANVAS_HEIGHT_PX);
        assert!(meta.render_dpi > 0.0);
        assert!(meta.screen_frame_height_px > 0.0);

        // All bricks should have polygons (filtered out those without)
        assert!(bricks.iter().all(|b| b.polygon.is_some()));

        // Check a known brick position (first brick in Python output)
        let first = &bricks[0];
        eprintln!("First brick: {} at ({}, {}) {}x{}", first.name, first.x, first.y, first.width, first.height);
    }

    // ── Auto-close unit tests (PR #75) ──────────────────────────────────
    //
    // `parse_path_lines` used to drop any sub-path whose start vertex
    // didn't match the last segment endpoint — bricks where every
    // sub-path was open ended up with `polygon: None` and got silently
    // killed by the validation step. The fix mirrors the auto-close
    // already present in `parse_path_lines_bezier`. These tests pin the
    // behaviour so it can't regress again.

    #[test]
    fn parse_path_lines_auto_closes_open_subpath() {
        // Three points, no closing edge — the artist forgot to draw it.
        // Pre-fix this returned (vec![], 1). Post-fix the polygon is
        // returned with an explicit closing vertex appended, and
        // open_paths is still bumped so the upcoming Illustrator
        // validator can flag the source.
        let lines: Vec<Vec<&str>> = vec![
            vec!["100", "100", "m"],
            vec!["200", "100", "L"],
            vec!["200", "200", "L"],
        ];
        let (polygons, open_paths) = parse_path_lines(&lines, 0.0, 0.0);

        assert_eq!(polygons.len(), 1, "expected one auto-closed polygon, got {}", polygons.len());
        assert_eq!(open_paths, 1, "open_paths should still flag the source error");

        let poly = &polygons[0];
        assert!(
            poly.len() >= 4,
            "auto-closed polygon needs the closing vertex appended (got {} pts)",
            poly.len()
        );
        let first = poly.first().unwrap();
        let last = poly.last().unwrap();
        let d = ((first[0] - last[0]).powi(2) + (first[1] - last[1]).powi(2)).sqrt();
        assert!(
            d < 1.0,
            "polygon should be geometrically closed after auto-close (gap={d})"
        );
    }

    #[test]
    fn parse_path_lines_keeps_explicitly_closed_subpath_clean() {
        // Same shape, drawn properly: explicit closing edge then a fill
        // operator. Auto-close should be a no-op here — `open_paths`
        // stays 0 and we don't append a duplicate vertex.
        let lines: Vec<Vec<&str>> = vec![
            vec!["100", "100", "m"],
            vec!["200", "100", "L"],
            vec!["200", "200", "L"],
            vec!["100", "200", "L"],
            vec!["100", "100", "L"],
            vec!["f"],
        ];
        let (polygons, open_paths) = parse_path_lines(&lines, 0.0, 0.0);

        assert_eq!(polygons.len(), 1);
        assert_eq!(open_paths, 0, "explicitly closed path should not bump open_paths");
        // 5 input vertices; auto-close shouldn't add a sixth.
        assert_eq!(polygons[0].len(), 5);
    }

    // ── Regression: NY8 'Layer 320' must survive the parse ──────────────
    //
    // 'Layer 320' is a brick in `_NY8.ai` whose polygon sub-paths are
    // all drawn open (artist forgot the closing edge). Between
    // v0.4.1 (validation tightened) and v0.4.6 (auto-close ported),
    // the brick was silently dropped — visible in the editor as a
    // missing element on the canvas with no warning. This test pins
    // that the brick is parsed successfully going forward.

    fn test_ai_path_ny8() -> Option<std::path::PathBuf> {
        for candidate in [
            "../../in/fixed/_NY8.ai",
            "../../in/_NY8.ai",
        ] {
            let p = std::path::PathBuf::from(candidate);
            if p.exists() { return Some(p); }
        }
        None
    }

    #[test]
    fn regression_ny8_layer_320_present() {
        let path = match test_ai_path_ny8() {
            Some(p) => p,
            None => { eprintln!("Skipping: NY8.ai not found in in/ or in/fixed/"); return; }
        };

        let (bricks, _meta, _ai_data) = parse_ai(&path, crate::CANVAS_HEIGHT_PX as i32).unwrap();

        let names: Vec<&str> = bricks.iter().map(|b| b.name.as_str()).collect();
        assert!(
            names.contains(&"Layer 320"),
            "regression: 'Layer 320' missing from NY8 brick list (had {} bricks). \
             This is the open-sub-path brick that auto-close should rescue.",
            bricks.len()
        );

        // Sanity: the brick should have a polygon — that's the whole
        // point of the auto-close fix. If it ever comes back without
        // one we're back to the pre-fix bug.
        let layer_320 = bricks.iter().find(|b| b.name == "Layer 320").unwrap();
        assert!(
            layer_320.polygon.is_some(),
            "'Layer 320' parsed but has no polygon — auto-close didn't fire"
        );
        let poly = layer_320.polygon.as_ref().unwrap();
        assert!(poly.len() >= 3, "'Layer 320' polygon has too few vertices ({})", poly.len());
    }

    // ── parse_path_lines: vector cleanup edge cases ────────────────────

    /// AI files store coordinates in their own y-up space; the parser
    /// converts to PyMuPDF y-down via `pymu_y = y_base - ai_y`. Pin the
    /// transform so a refactor doesn't silently flip handedness.
    #[test]
    fn parse_path_lines_translates_ai_to_pymu_coords() {
        // y_base = 100, offset_x = 5 → AI (10, 30) becomes pymu (15, 70).
        let lines: Vec<Vec<&str>> = vec![
            vec!["10", "30", "m"],
            vec!["20", "30", "L"],
            vec!["20", "40", "L"],
            vec!["10", "40", "L"],
            vec!["f"],
        ];
        let (polys, _) = parse_path_lines(&lines, 5.0, 100.0);
        assert_eq!(polys.len(), 1);
        assert_eq!(polys[0][0], [15.0, 70.0], "first vertex should be transformed");
        assert_eq!(polys[0][1], [25.0, 70.0]);
        assert_eq!(polys[0][2], [25.0, 60.0]);
    }

    /// Two `m` operators in a row → two separate sub-paths returned.
    /// Both are auto-closed if the artist forgot the closing edge.
    #[test]
    fn parse_path_lines_handles_multiple_subpaths() {
        let lines: Vec<Vec<&str>> = vec![
            vec!["0", "0", "m"],
            vec!["10", "0", "L"],
            vec!["10", "10", "L"],
            vec!["0", "10", "L"],
            vec!["0", "0", "L"],
            vec!["100", "100", "m"],
            vec!["110", "100", "L"],
            vec!["110", "110", "L"],
            vec!["100", "110", "L"],
            vec!["100", "100", "L"],
        ];
        let (polys, open_paths) = parse_path_lines(&lines, 0.0, 0.0);
        assert_eq!(polys.len(), 2, "expected two sub-paths, got {}", polys.len());
        assert_eq!(open_paths, 0, "both sub-paths are explicitly closed");
    }

    /// A `C` operator should tessellate the cubic into many vertices
    /// (1 moveto + 8 interpolated + 1 endpoint = 10), preserving the
    /// curve's shape when downstream consumers treat it as a polygon.
    /// We assert "many" rather than an exact count because the
    /// auto-close pass may append one closing vertex when the cubic's
    /// endpoint differs from the moveto — both behaviours are
    /// correct, the count is incidental.
    #[test]
    fn parse_path_lines_tessellates_cubic_bezier() {
        // Single moveto, then a cubic from (0,0) to (100,0) bulging.
        let lines: Vec<Vec<&str>> = vec![
            vec!["0", "0", "m"],
            vec!["0", "100", "100", "100", "100", "0", "C"],
            vec!["f"],
        ];
        let (polys, _) = parse_path_lines(&lines, 0.0, 0.0);
        assert_eq!(polys.len(), 1);
        // Without tessellation we'd have at most 2 vertices (start + endpoint).
        // With tessellation we get ~10 — pin a generous floor that catches
        // any regression to "no tessellation at all".
        assert!(
            polys[0].len() >= 9,
            "expected cubic to tessellate into many vertices, got {}", polys[0].len()
        );
        // Some vertex other than start/end should sit off the
        // straight chord — proves the curve was actually sampled
        // rather than collapsed to two endpoints.
        let mid = polys[0][polys[0].len() / 2];
        assert!(mid[1].abs() > 10.0, "cubic should bulge off the x-axis, mid={:?}", mid);
    }

    // ── parse_path_lines_bezier: same edges, bezier flavour ────────────

    /// The bezier extractor must auto-close just like the polygon one
    /// (regression: NY8 'Layer 320', NY9n b012/b020/b022/b026).
    #[test]
    fn parse_path_lines_bezier_auto_closes_open_subpath() {
        let lines: Vec<Vec<&str>> = vec![
            vec!["0", "0", "m"],
            vec!["10", "0", "L"],
            vec!["10", "10", "L"],
        ];
        let (paths, open_paths) = parse_path_lines_bezier(&lines, 0.0, 0.0);
        assert_eq!(paths.len(), 1);
        assert_eq!(open_paths, 1, "open sub-path should still bump open_paths");
        // The auto-close appends a final Line back to start. We drew 2
        // segments, plus the implicit closing segment → 3 segments.
        assert_eq!(paths[0].segments.len(), 3, "auto-close should append a closing Line");
        assert_eq!(
            paths[0].segments.last().unwrap().end(),
            paths[0].start,
            "last segment must end at start vertex"
        );
    }

    /// The bezier extractor stores a `C` operator as a `Segment::Cubic`
    /// — no tessellation. This is the whole reason for keeping a
    /// separate bezier path: the merge can preserve the curve.
    #[test]
    fn parse_path_lines_bezier_preserves_cubic_segments() {
        use crate::bezier::Segment;
        let lines: Vec<Vec<&str>> = vec![
            vec!["0", "0", "m"],
            vec!["0", "100", "100", "100", "100", "0", "C"],
            vec!["f"],
        ];
        let (paths, _) = parse_path_lines_bezier(&lines, 0.0, 0.0);
        assert_eq!(paths.len(), 1);
        let cubic = paths[0].segments.iter().find(|s| matches!(s, Segment::Cubic { .. }));
        assert!(cubic.is_some(), "cubic operator must produce a Segment::Cubic");
    }

    /// Multiple `m`-separated sub-paths come back as multiple
    /// BezierPaths in order. Coordinates are flipped to PyMuPDF
    /// y-down via `pymu_y = y_base - ai_y`; `y_base = 110` here so
    /// the two startpoints land at intuitive values.
    #[test]
    fn parse_path_lines_bezier_handles_multiple_subpaths() {
        let lines: Vec<Vec<&str>> = vec![
            vec!["0", "0", "m"],
            vec!["10", "0", "L"],
            vec!["10", "10", "L"],
            vec!["0", "10", "L"],
            vec!["0", "0", "L"],
            vec!["100", "100", "m"],
            vec!["110", "100", "L"],
            vec!["110", "110", "L"],
            vec!["100", "110", "L"],
            vec!["100", "100", "L"],
        ];
        let (paths, _) = parse_path_lines_bezier(&lines, 0.0, 110.0);
        assert_eq!(paths.len(), 2);
        // y_base=110 → AI y=0 maps to pymu y=110, AI y=100 maps to pymu y=10.
        assert_eq!(paths[0].start, [0.0, 110.0]);
        assert_eq!(paths[1].start, [100.0, 10.0]);
    }
}
