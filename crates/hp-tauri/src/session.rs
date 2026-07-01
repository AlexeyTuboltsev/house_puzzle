//! In-memory session management.

use hp_core::ai_parser::{BrickPlacement, ParsedAiMetadata};
use hp_core::bezier::BezierPath;
use hp_core::types::{Brick, PuzzlePiece};
use image::RgbaImage;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

/// Per-session state for a loaded AI file.
pub struct Session {
    pub bricks: Vec<Brick>,
    pub brick_placements: Vec<BrickPlacement>,
    pub brick_polygons: HashMap<String, Vec<[f64; 2]>>,
    /// Bezier paths per brick in PyMuPDF coords (y-down). Source of
    /// truth for brick / piece / blueprint outlines — preserves cubic
    /// curves through the merge instead of tessellating to polygons.
    pub brick_beziers: HashMap<String, Vec<BezierPath>>,
    pub brick_areas: HashMap<String, f64>,
    pub pieces: Vec<PuzzlePiece>,
    pub metadata: ParsedAiMetadata,
    pub extract_dir: PathBuf,
    /// OCG bricks layer render — shared for brick/piece image serving.
    pub bricks_layer_img: Arc<RgbaImage>,
    /// Per-brick images (canvas-sized, polygon-masked). Lazy-populated as PNG bytes.
    pub brick_images: HashMap<String, Arc<Vec<u8>>>,
    /// Per-brick RGBA images for piece composition.
    pub brick_rgba: HashMap<String, Arc<RgbaImage>>,
    /// Original AI file path — kept so the lazy lights / background
    /// renderers can re-open the document without parsing it again.
    pub ai_path: PathBuf,
    /// Legacy integer-pixel pymu→PDF offset. Always (0, 0) now —
    /// the new pipeline uses `shifted_clip` below for both the
    /// bricks render and the lazy lights/background renders. Field
    /// kept for read-side backward-compatibility.
    #[allow(dead_code)]
    pub pdf_offset: (i32, i32),
    /// Sub-pixel-precise pymu→PDF bleed translation, in PDF points.
    /// Computed once at load time via `ocg_inject::analyse_brick_blocks`.
    /// `shifted_clip = metadata.clip_rect + bleed_pts`.
    #[allow(dead_code)]
    pub bleed_pts: (f64, f64),
    /// `metadata.clip_rect` translated by `bleed_pts` so MuPDF
    /// renders the page region aligned with the parser's pymu frame.
    /// Lights / background lazy renders use this.
    pub shifted_clip: (f64, f64, f64, f64),
    /// Maps hashed brick ID → AI layer name (e.g. "Layer 45").
    /// Needed at export time to translate hashed IDs back to OCG names.
    pub brick_layer_names: HashMap<String, String>,
}

/// Thread-safe session store.
pub type SessionStore = Arc<Mutex<HashMap<String, Session>>>;

pub fn new_session_store() -> SessionStore {
    Arc::new(Mutex::new(HashMap::new()))
}
