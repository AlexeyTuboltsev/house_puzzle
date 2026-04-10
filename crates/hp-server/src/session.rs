//! In-memory session management for loaded AI files.

use hp_core::ai_parser::{AiPrivateData, BrickPlacement, LayerBlock, ParsedAiMetadata};
use hp_core::types::{Brick, PuzzlePiece};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

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
}

/// Thread-safe session store.
pub type SessionStore = Arc<Mutex<HashMap<String, Session>>>;

pub fn new_session_store() -> SessionStore {
    Arc::new(Mutex::new(HashMap::new()))
}
