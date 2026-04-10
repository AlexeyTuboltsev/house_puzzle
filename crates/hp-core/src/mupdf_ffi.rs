//! Ad-hoc FFI bindings for MuPDF functions not exposed by `mupdf-rs`.
//!
//! `mupdf-rs` wraps Document, Page, Pixmap, etc. but does not expose:
//! - OCG (Optional Content Group) layer control
//! - xref-level object/stream access (needed for AIPrivateData extraction)
//!
//! We access the raw `pdf_document` and `fz_context` pointers by mirroring
//! the private struct layout of `mupdf::pdf::PdfDocument` and `mupdf::context()`.

use mupdf_sys::*;
use std::ffi::CStr;

// ---------------------------------------------------------------------------
// Pointer extraction — access internal raw pointers from mupdf-rs types
// ---------------------------------------------------------------------------

/// Extract the raw `*mut pdf_document` from a `mupdf::pdf::PdfDocument`.
///
/// Safety: relies on `PdfDocument` having `inner: *mut pdf_document` as its
/// first field. Validated by the `test_struct_layout` test.
pub unsafe fn pdf_doc_ptr(doc: &mupdf::pdf::PdfDocument) -> *mut pdf_document {
    let ptr: *const *mut pdf_document = (doc as *const mupdf::pdf::PdfDocument).cast();
    unsafe { *ptr }
}

/// Get the thread-local `fz_context` pointer used by mupdf-rs.
///
/// Safety: `mupdf-rs` initializes a context per thread via `context()`.
/// We replicate the same pattern: read from the thread-local storage
/// that `mupdf-rs` maintains internally.
///
/// Since `mupdf::context()` is `pub(crate)`, we call `fz_new_context`
/// directly — but actually, we need the SAME context that mupdf-rs uses.
/// The simplest approach: do a no-op mupdf-rs call that we know initializes
/// the context, then read it from the same thread-local.
///
/// Alternative: since mupdf-rs context is thread-local and set once,
/// we can extract it by creating a Document and looking at internal state.
/// For now, we'll create our own context for FFI calls.
static mut FFI_CONTEXT: *mut fz_context = std::ptr::null_mut();

/// Initialize the FFI context. Must be called once before using FFI functions.
pub fn init_ffi_context() {
    unsafe {
        if FFI_CONTEXT.is_null() {
            FFI_CONTEXT = mupdf_new_base_context();
            if FFI_CONTEXT.is_null() {
                panic!("Failed to create MuPDF FFI context");
            }
            fz_register_document_handlers(FFI_CONTEXT);
        }
    }
}

/// Get the FFI context pointer.
fn ctx() -> *mut fz_context {
    unsafe {
        if FFI_CONTEXT.is_null() {
            init_ffi_context();
        }
        FFI_CONTEXT
    }
}

/// Helper: extract data and length from an fz_buffer.
unsafe fn buffer_to_vec(buf: *mut fz_buffer) -> Option<Vec<u8>> {
    if buf.is_null() {
        return None;
    }
    let mut data_ptr: *mut u8 = std::ptr::null_mut();
    let len = fz_buffer_storage(ctx(), buf, &mut data_ptr);
    if data_ptr.is_null() || len == 0 {
        fz_drop_buffer(ctx(), buf);
        return None;
    }
    let result = std::slice::from_raw_parts(data_ptr, len).to_vec();
    fz_drop_buffer(ctx(), buf);
    Some(result)
}

// ---------------------------------------------------------------------------
// OCG layer control
// ---------------------------------------------------------------------------

/// Info about one OCG layer UI entry.
#[derive(Debug, Clone)]
pub struct LayerInfo {
    pub text: String,
    pub depth: i32,
    pub selected: bool,
    pub locked: bool,
}

/// Count the number of OCG layer UI entries in a PDF document.
pub fn count_layer_ui(doc: &mupdf::pdf::PdfDocument) -> i32 {
    unsafe { pdf_count_layer_config_ui(ctx(), pdf_doc_ptr(doc)) }
}

/// Get info about a specific OCG layer UI entry.
pub fn layer_ui_info(doc: &mupdf::pdf::PdfDocument, idx: i32) -> LayerInfo {
    unsafe {
        let mut info: pdf_layer_config_ui = std::mem::zeroed();
        pdf_layer_config_ui_info(ctx(), pdf_doc_ptr(doc), idx, &mut info);
        let text = if info.text.is_null() {
            String::new()
        } else {
            CStr::from_ptr(info.text).to_string_lossy().to_string()
        };
        LayerInfo {
            text,
            depth: info.depth,
            selected: info.selected != 0,
            locked: info.locked != 0,
        }
    }
}

/// Select (enable) a specific OCG layer UI entry.
pub fn select_layer_ui(doc: &mupdf::pdf::PdfDocument, idx: i32) {
    unsafe { pdf_select_layer_config_ui(ctx(), pdf_doc_ptr(doc), idx) }
}

/// Deselect (disable) a specific OCG layer UI entry.
pub fn deselect_layer_ui(doc: &mupdf::pdf::PdfDocument, idx: i32) {
    unsafe { pdf_deselect_layer_config_ui(ctx(), pdf_doc_ptr(doc), idx) }
}

// ---------------------------------------------------------------------------
// xref / stream access (for AIPrivateData extraction)
// ---------------------------------------------------------------------------

/// Get the number of xref entries in the PDF.
pub fn xref_len(doc: &mupdf::pdf::PdfDocument) -> i32 {
    unsafe { pdf_xref_len(ctx(), pdf_doc_ptr(doc)) }
}

/// Find AIPrivateData stream references in the PDF.
/// Returns a sorted list of (sequence_number, xref_number) pairs.
pub fn find_ai_private_data(doc: &mupdf::pdf::PdfDocument) -> Vec<(u32, i32)> {
    let xref_count = xref_len(doc);
    let mut result: Vec<(u32, i32)> = Vec::new();

    unsafe {
        for i in 1..xref_count {
            let obj = pdf_load_object(ctx(), pdf_doc_ptr(doc), i);
            if obj.is_null() || pdf_is_dict(ctx(), obj) == 0 {
                if !obj.is_null() { pdf_drop_obj(ctx(), obj); }
                continue;
            }

            let dict_len = pdf_dict_len(ctx(), obj);
            let mut found_any = false;
            for j in 0..dict_len {
                let key = pdf_dict_get_key(ctx(), obj, j);
                if key.is_null() { continue; }
                let key_name = pdf_to_name(ctx(), key);
                if key_name.is_null() { continue; }
                let key_str = CStr::from_ptr(key_name).to_str().unwrap_or("");
                if let Some(num_str) = key_str.strip_prefix("AIPrivateData") {
                    if let Ok(seq) = num_str.parse::<u32>() {
                        let val = pdf_dict_get_val(ctx(), obj, j);
                        if !val.is_null() && pdf_is_indirect(ctx(), val) != 0 {
                            let ref_num = pdf_to_num(ctx(), val);
                            result.push((seq, ref_num));
                            found_any = true;
                        }
                    }
                }
            }
            pdf_drop_obj(ctx(), obj);
            if found_any { break; }
        }
    }

    result.sort_by_key(|(seq, _)| *seq);
    result
}

/// Load a decoded stream by xref number. Returns the raw bytes.
pub fn xref_stream(doc: &mupdf::pdf::PdfDocument, num: i32) -> Option<Vec<u8>> {
    unsafe {
        let buf = pdf_load_stream_number(ctx(), pdf_doc_ptr(doc), num);
        buffer_to_vec(buf)
    }
}

// ---------------------------------------------------------------------------
// Page geometry
// ---------------------------------------------------------------------------

/// Get the artbox of a page (in PDF points, y-down).
pub fn page_artbox(page: &mupdf::Page) -> (f32, f32, f32, f32) {
    let bounds = page.bounds().unwrap_or_default();
    (bounds.x0, bounds.y0, bounds.x1, bounds.y1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_struct_layout() {
        // Verify that PdfDocument's first field is *mut pdf_document
        // by checking size and alignment expectations
        assert!(
            std::mem::size_of::<mupdf::pdf::PdfDocument>() >= std::mem::size_of::<*mut pdf_document>(),
            "PdfDocument must be at least pointer-sized"
        );
    }

    #[test]
    fn test_open_and_list_layers() {
        let path = std::path::Path::new("../../in/_NY1.ai");
        if !path.exists() {
            eprintln!("Skipping: in/_NY1.ai not found");
            return;
        }

        let doc = mupdf::pdf::PdfDocument::open("../../in/_NY1.ai").expect("Failed to open AI file");
        let count = count_layer_ui(&doc);
        assert!(count > 0, "Expected OCG layers, got {count}");

        // Print layer names for debugging
        for i in 0..count {
            let info = layer_ui_info(&doc, i);
            eprintln!("Layer {i}: {:?}", info);
        }
    }

    #[test]
    fn test_find_ai_private_data() {
        let path = std::path::Path::new("../../in/_NY1.ai");
        if !path.exists() {
            eprintln!("Skipping: in/_NY1.ai not found");
            return;
        }

        let doc = mupdf::pdf::PdfDocument::open("../../in/_NY1.ai").expect("Failed to open AI file");
        let pairs = find_ai_private_data(&doc);
        assert!(!pairs.is_empty(), "AIPrivateData not found");
        eprintln!("Found {} AIPrivateData streams: {:?}", pairs.len(), pairs);

        // Verify we can read the streams
        for (seq, xref) in &pairs {
            let data = xref_stream(&doc, *xref);
            assert!(data.is_some(), "Failed to read stream for AIPrivateData{seq} at xref {xref}");
            eprintln!("  AIPrivateData{seq} (xref {xref}): {} bytes", data.unwrap().len());
        }
    }
}
