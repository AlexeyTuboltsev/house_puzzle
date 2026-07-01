//! Diagnostic: open an AI/PDF file and dump every entry MuPDF
//! exposes via `pdf_count_layer_config_ui` / `pdf_layer_config_ui_info`.
//!
//! Tells us whether individual brick sub-layers are addressable as
//! separate OCGs (the prerequisite for per-piece export rendering).
//!
//! Usage:
//!   cargo run -p hp-core --example dump_ocg -- in/_NY1.ai

use hp_core::mupdf_ffi;

fn main() {
    let path = std::env::args().nth(1).expect("usage: dump_ocg <path>");

    mupdf_ffi::init_ffi_context();

    let doc = mupdf::pdf::PdfDocument::open(&path).expect("open AI");
    let count = mupdf_ffi::count_layer_ui(&doc);
    println!("OCG layer-config-UI entries: {count}");
    println!("{:<6} {:<6} {:<6} {:<6} {}", "idx", "depth", "sel", "lock", "text");
    for i in 0..count {
        let info = mupdf_ffi::layer_ui_info(&doc, i);
        println!(
            "{:<6} {:<6} {:<6} {:<6} {}",
            i,
            info.depth,
            if info.selected { "Y" } else { "." },
            if info.locked { "Y" } else { "." },
            info.text,
        );
    }
}
