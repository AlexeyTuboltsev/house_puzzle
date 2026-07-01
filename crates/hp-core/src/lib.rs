/// Default canvas height (px) used when no caller supplies one.
///
/// This is the height of the **exported** house raster — production live
/// preview overrides it with the actual height of the frontend's render
/// div. The testbed and the export pipeline use this default. The
/// parser feeds it into the unit→pixel conversion that derives the
/// `render_dpi` so the rendered `screen` frame is exactly this many
/// pixels tall.
pub const CANVAS_HEIGHT_PX: u32 = 900;

/// Game-side height of one screen frame, expressed in Unity units. The
/// AI's `screen` layer rectangle represents this many units, and the
/// rendered raster is calibrated so it ends up `canvas_height` pixels tall.
pub const HOUSE_UNITS_HIGH: f64 = 15.5;

pub mod types;
pub mod mupdf_ffi;
pub mod bezier;
pub mod bezier_merge;
pub mod ai_parser;
pub mod render;
pub mod puzzle;
pub mod export;
pub mod ocg_inject;
pub mod raster_extract;

// Re-export lopdf::Document so callers (notably the Tauri shell)
// can hold the document hp_core::ocg_inject::analyse_brick_blocks +
// hp_core::raster_extract::compose_image_blocks_onto_canvas need
// without taking a direct lopdf dep.
pub use lopdf;
