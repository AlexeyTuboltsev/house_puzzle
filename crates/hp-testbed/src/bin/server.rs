//! Testbed HTTP server. Loads a pre-computed snapshot, runs the current
//! `bezier_merge::merge_piece` for each piece on request, serves SVG path
//! strings. Code edits → `cargo run -p hp-testbed --bin hp-testbed` →
//! refresh the browser.

use anyhow::{Context, Result};
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, Json},
    routing::get,
    Router,
};
use hp_core::bezier::BezierPath;
use hp_core::bezier_merge;
use hp_core::types::PuzzlePiece;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;

#[derive(Debug, Clone, Deserialize)]
struct Transform {
    clip_x0: f64,
    clip_y0: f64,
    scale: f64,
    canvas_width: u32,
    canvas_height: u32,
}

#[derive(Debug, Clone, Deserialize)]
struct BrickIn {
    id: String,
    name: String,
    #[serde(default)]
    layer_type: String,
    beziers: Vec<BezierPath>,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    source_file: String,
    transform: Transform,
    bricks: Vec<BrickIn>,
    pieces: Vec<PuzzlePiece>,
}

#[derive(Clone)]
struct AppState {
    snap: Arc<Snapshot>,
    bricks_by_id: Arc<HashMap<String, BrickIn>>,
}

#[derive(Debug, Serialize)]
struct PieceBrick {
    id: String,
    name: String,
}

#[derive(Debug, Serialize)]
struct PieceOut {
    id: String,
    bricks: Vec<PieceBrick>,
    /// One SVG `d` per closed outline component. Canvas coords, y-down.
    svg: Vec<String>,
    /// Canvas-coord bbox of the piece outline (min_x, min_y, max_x, max_y).
    /// `None` if the merge produced nothing.
    bbox: Option<[f64; 4]>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    source_file: String,
    canvas_width: u32,
    canvas_height: u32,
    pieces: Vec<PieceOut>,
    /// Brick outlines (canvas coords) for overlay/debug. Empty when not requested.
    bricks: Vec<BrickPreview>,
}

#[derive(Debug, Serialize)]
struct BrickPreview {
    id: String,
    name: String,
    svg: Vec<String>,
    /// Endpoint vertices per sub-path, in canvas coords. The i-th Vec<[f64;2]>
    /// corresponds to the i-th entry of `svg`. Vertex ids match the order
    /// returned by `BezierPath::vertices()` (`[start, seg0.to, seg1.to, …]`).
    vertices: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Deserialize)]
struct PiecesQuery {
    piece: Option<String>,
    /// Accept "1" / "true" / "yes" / "on" (case-insensitive). Empty → false.
    with_bricks: Option<String>,
    /// "bezier" (default, curves preserved) or "polyline" (legacy baseline).
    algo: Option<String>,
}

fn truthy(s: &Option<String>) -> bool {
    match s.as_deref().map(|v| v.to_ascii_lowercase()) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => false,
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    let snap_path = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("testbed/snapshot.json"));
    let port: u16 = std::env::var("HP_TESTBED_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5051);

    eprintln!("[testbed] loading {}", snap_path.display());
    let raw = std::fs::read(&snap_path).with_context(|| format!("reading {}", snap_path.display()))?;
    let snap: Snapshot = serde_json::from_slice(&raw).context("parsing snapshot json")?;
    eprintln!(
        "[testbed] loaded {} bricks, {} pieces ({}x{})",
        snap.bricks.len(),
        snap.pieces.len(),
        snap.transform.canvas_width,
        snap.transform.canvas_height
    );

    let bricks_by_id: HashMap<String, BrickIn> =
        snap.bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();
    let state = AppState {
        snap: Arc::new(snap),
        bricks_by_id: Arc::new(bricks_by_id),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/pieces", get(pieces_handler))
        .with_state(state);

    let addr = SocketAddr::from(([127, 0, 0, 1], port));
    eprintln!("[testbed] listening on http://{addr}");
    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

async fn index() -> impl axum::response::IntoResponse {
    (
        [("cache-control", "no-store, no-cache, must-revalidate")],
        Html(include_str!("../../static/index.html")),
    )
}

async fn pieces_handler(
    State(state): State<AppState>,
    Query(q): Query<PiecesQuery>,
) -> Result<Json<ApiResponse>, (StatusCode, String)> {
    let t = &state.snap.transform;
    let to_canvas = |bp: &BezierPath| bp.transform([-t.clip_x0, -t.clip_y0], t.scale);

    let selected: Vec<&PuzzlePiece> = match &q.piece {
        Some(id) => state
            .snap
            .pieces
            .iter()
            .filter(|p| &p.id == id)
            .collect(),
        None => state.snap.pieces.iter().collect(),
    };

    let use_polyline = matches!(q.algo.as_deref(), Some("polyline"));

    let mut pieces_out: Vec<PieceOut> = Vec::with_capacity(selected.len());
    for piece in selected {
        let mut brick_paths: Vec<BezierPath> = Vec::new();
        for bid in &piece.brick_ids {
            if let Some(b) = state.bricks_by_id.get(bid) {
                brick_paths.extend(b.beziers.iter().cloned());
            }
        }
        let merged = if use_polyline {
            bezier_merge::merge_piece(&brick_paths)
        } else {
            bezier_merge::merge_piece_bezier(&brick_paths)
        };

        // Compute piece canvas-coord bbox from merged bezier paths.
        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for bp in &merged {
            let scaled = to_canvas(bp);
            for v in scaled.vertices() {
                min_x = min_x.min(v[0]);
                min_y = min_y.min(v[1]);
                max_x = max_x.max(v[0]);
                max_y = max_y.max(v[1]);
            }
        }
        let bbox = if merged.is_empty() || !min_x.is_finite() {
            None
        } else {
            Some([min_x, min_y, max_x, max_y])
        };

        let svg: Vec<String> = merged.iter().map(|bp| to_canvas(bp).to_svg_d()).collect();
        let bricks: Vec<PieceBrick> = piece
            .brick_ids
            .iter()
            .map(|bid| {
                let name = state
                    .bricks_by_id
                    .get(bid)
                    .map(|b| b.name.clone())
                    .unwrap_or_default();
                PieceBrick { id: bid.clone(), name }
            })
            .collect();
        pieces_out.push(PieceOut {
            id: piece.id.clone(),
            bricks,
            svg,
            bbox,
        });
    }

    let bricks_out = if truthy(&q.with_bricks) {
        let brick_ids: std::collections::HashSet<&str> = match &q.piece {
            Some(_) => pieces_out
                .iter()
                .flat_map(|p| p.bricks.iter().map(|b| b.id.as_str()))
                .collect(),
            None => state.bricks_by_id.keys().map(|s| s.as_str()).collect(),
        };
        let mut v: Vec<BrickPreview> = Vec::new();
        for id in brick_ids {
            if let Some(b) = state.bricks_by_id.get(id) {
                let scaled: Vec<BezierPath> = b.beziers.iter().map(to_canvas).collect();
                let svg: Vec<String> = scaled.iter().map(|bp| bp.to_svg_d()).collect();
                let vertices: Vec<Vec<[f64; 2]>> =
                    scaled.iter().map(|bp| bp.vertices()).collect();
                v.push(BrickPreview {
                    id: b.id.clone(),
                    name: b.name.clone(),
                    svg,
                    vertices,
                });
            }
        }
        v.sort_by(|a, b| a.id.cmp(&b.id));
        v
    } else {
        Vec::new()
    };

    Ok(Json(ApiResponse {
        source_file: state.snap.source_file.clone(),
        canvas_width: t.canvas_width,
        canvas_height: t.canvas_height,
        pieces: pieces_out,
        bricks: bricks_out,
    }))
}
