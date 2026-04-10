//! AI file parser — extracts brick layers, geometry, and vector polygons.
//!
//! AI files are PDF-based with embedded PostScript data (AIPrivateData streams).
//! This module:
//! 1. Extracts and decompresses AIPrivateData via MuPDF FFI
//! 2. Parses the PostScript layer tree
//! 3. (TODO) Extracts brick placement and vector polygon data

use regex::Regex;
use std::path::Path;

use crate::mupdf_ffi;

/// A parsed layer block from the AI PostScript data.
#[derive(Debug, Clone)]
pub struct LayerBlock {
    pub name: String,
    pub begin: usize,
    pub end: usize,
    pub depth: usize,
    pub children: Vec<LayerBlock>,
}

/// Raw AI data: the decompressed bytes and decoded text.
pub struct AiPrivateData {
    pub raw: Vec<u8>,
    pub text: String,
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

    // Decode as latin-1 (each byte maps directly to its unicode codepoint)
    let text: String = decompressed.iter().map(|&b| b as char).collect();

    Ok(AiPrivateData {
        raw: decompressed,
        text,
    })
}

/// Parse `%AI5_BeginLayer` / `%AI5_EndLayer` pairs into a nested tree.
pub fn parse_layer_tree(text: &str) -> Vec<LayerBlock> {
    let begin_re = Regex::new(r"%AI5_BeginLayer").unwrap();
    let end_re = Regex::new(r"%AI5_EndLayer").unwrap();
    let name_re = Regex::new(r"Lb\r\(([^)]*)\)").unwrap();

    // Collect all begin/end events with positions
    let mut events: Vec<(char, usize)> = Vec::new();
    for m in begin_re.find_iter(text) {
        events.push(('B', m.start()));
    }
    for m in end_re.find_iter(text) {
        events.push(('E', m.start()));
    }
    events.sort_by_key(|e| e.1);

    let mut stack: Vec<LayerBlock> = Vec::new();
    let mut roots: Vec<LayerBlock> = Vec::new();

    for (typ, pos) in events {
        if typ == 'B' {
            // Extract layer name from nearby text
            let snippet_end = (pos + 300).min(text.len());
            let snippet = &text[pos..snippet_end];
            let name = name_re.captures(snippet)
                .map(|c| c[1].to_string())
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
            // End layer
            if let Some(mut block) = stack.pop() {
                block.end = pos + "%AI5_EndLayer".len();
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
        assert!(!data.text.is_empty());
        assert!(data.text.contains("%AI5_BeginLayer"));
        eprintln!("Decompressed {} bytes, {} chars", data.raw.len(), data.text.len());
    }

    #[test]
    fn test_parse_layer_tree() {
        let path = match test_ai_path() {
            Some(p) => p,
            None => { eprintln!("Skipping: in/_NY1.ai not found"); return; }
        };
        let data = decompress_ai_data(&path).unwrap();
        let roots = parse_layer_tree(&data.text);
        assert!(!roots.is_empty());

        let names: Vec<&str> = roots.iter().map(|r| r.name.as_str()).collect();
        eprintln!("Top-level layers: {:?}", names);
        assert!(names.contains(&"background"), "Missing 'background', found: {:?}", names);
        assert!(names.contains(&"bricks"), "Missing 'bricks', found: {:?}", names);

        let bricks = roots.iter().find(|r| r.name == "bricks").unwrap();
        eprintln!("Bricks layer has {} children", bricks.children.len());
        assert!(bricks.children.len() > 100, "Expected >100 bricks, got {}", bricks.children.len());
    }
}
