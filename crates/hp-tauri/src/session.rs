//! In-memory session management — mirrors hp-server/src/session.rs.

use hp_core::ai_parser::{AiPrivateData, BrickPlacement, LayerBlock, ParsedAiMetadata};
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
    pub brick_areas: HashMap<String, f64>,
    pub pieces: Vec<PuzzlePiece>,
    pub metadata: ParsedAiMetadata,
    pub extract_dir: PathBuf,
    pub ai_data: Arc<AiPrivateData>,
    pub layer_blocks: HashMap<String, LayerBlock>,
    /// OCG bricks layer render — shared for brick/piece image serving.
    pub bricks_layer_img: Arc<RgbaImage>,
    /// Per-brick images (canvas-sized, polygon-masked). Lazy-populated as PNG bytes.
    pub brick_images: HashMap<String, Arc<Vec<u8>>>,
    /// Per-brick RGBA images for piece composition.
    pub brick_rgba: HashMap<String, Arc<RgbaImage>>,
}

/// Thread-safe session store.
pub type SessionStore = Arc<Mutex<HashMap<String, Session>>>;

pub fn new_session_store() -> SessionStore {
    Arc::new(Mutex::new(HashMap::new()))
}
