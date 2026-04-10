use serde::{Deserialize, Serialize};

/// A point in 2D space.
pub type Point = [f64; 2];

/// A single brick/element layer extracted from an AI file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrickLayer {
    pub index: usize,
    pub name: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default = "default_layer_type")]
    pub layer_type: String,
    /// Assigned post-parse by server (uuid4 or deterministic hash).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    /// Vector outline polygon in brick-local pixel coordinates.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub polygon: Option<Vec<Point>>,
}

fn default_layer_type() -> String {
    "brick".to_string()
}

/// Parsed house data from an AI file.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HouseData {
    pub source_path: String,
    pub canvas_width: i32,
    pub canvas_height: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub composite: Option<BrickLayer>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base: Option<BrickLayer>,
    #[serde(default)]
    pub bricks: Vec<BrickLayer>,
    #[serde(default)]
    pub total_layers: usize,
    #[serde(default)]
    pub render_dpi: f64,
    #[serde(default)]
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub clip_rect: Option<(f64, f64, f64, f64)>,
    #[serde(default)]
    pub screen_frame_height_px: f64,
    #[serde(default = "default_pdf_offset")]
    pub pdf_offset_px: (i32, i32),
    /// Names of bricks dropped during parse (no vector polygon).
    #[serde(default)]
    pub skipped_bricks: Vec<String>,
}

fn default_pdf_offset() -> (i32, i32) {
    (0, 0)
}

/// Engine brick — the puzzle engine's view of a brick.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Brick {
    pub id: String,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    #[serde(default = "default_layer_type")]
    pub brick_type: String,
}

impl Brick {
    pub fn right(&self) -> i32 {
        self.x + self.width
    }

    pub fn bottom(&self) -> i32 {
        self.y + self.height
    }

    pub fn area(&self) -> i64 {
        self.width as i64 * self.height as i64
    }
}

/// A puzzle piece — a group of merged bricks.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PuzzlePiece {
    pub id: String,
    pub brick_ids: Vec<String>,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

/// Result of the merge algorithm.
#[derive(Debug, Clone)]
pub struct MergeResult {
    pub pieces: Vec<PuzzlePiece>,
}
