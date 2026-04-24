//! Testbed HTTP server. Scans a directory of AI files, lazy-loads a
//! snapshot per file on first access (cached on disk + in memory), then
//! serves bezier piece outlines for any seed on demand.

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
use hp_core::puzzle;
use hp_core::types::{Brick, PuzzlePiece};
use hp_testbed::{load_or_build, scan_ai_dir, Snapshot};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::Mutex;

const DEFAULT_SEED: u64 = 42;
const DEFAULT_TARGET: usize = 120;

// ──────────────────────────────────────────────────────────────
// Per-file loaded state (snapshot + engine)
// ──────────────────────────────────────────────────────────────

struct LoadedFile {
    snap: Snapshot,
    bricks_engine: Vec<Brick>,
    adjacency: HashMap<String, HashSet<String>>,
    areas: HashMap<String, f64>,
}

impl LoadedFile {
    fn from_snapshot(snap: Snapshot) -> Self {
        let t = snap.transform.clone();
        let bricks_engine: Vec<Brick> = snap
            .bricks
            .iter()
            .map(|b| {
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                let mut max_x = f64::NEG_INFINITY;
                let mut max_y = f64::NEG_INFINITY;
                let mut probe = |p: [f64; 2]| {
                    if p[0] < min_x { min_x = p[0]; }
                    if p[1] < min_y { min_y = p[1]; }
                    if p[0] > max_x { max_x = p[0]; }
                    if p[1] > max_y { max_y = p[1]; }
                };
                for bp in &b.beziers {
                    probe(bp.start);
                    for seg in &bp.segments { probe(seg.end()); }
                }
                let x = ((min_x - t.clip_x0) * t.scale).round() as i32;
                let y = ((min_y - t.clip_y0) * t.scale).round() as i32;
                let w = (((max_x - min_x) * t.scale).round() as i32).max(1);
                let h = (((max_y - min_y) * t.scale).round() as i32).max(1);
                Brick {
                    id: b.id.clone(),
                    x, y, width: w, height: h,
                    brick_type: b.layer_type.clone(),
                }
            })
            .collect();

        let brick_beziers: HashMap<String, Vec<BezierPath>> = snap
            .bricks
            .iter()
            .map(|b| (b.id.clone(), b.beziers.clone()))
            .collect();

        let min_border_pymu = 5.0 / snap.transform.scale;
        let adjacency =
            puzzle::build_adjacency_bezier(&bricks_engine, &brick_beziers, min_border_pymu);
        let areas = puzzle::compute_bezier_areas(&bricks_engine, &brick_beziers);
        LoadedFile { snap, bricks_engine, adjacency, areas }
    }
}

// ──────────────────────────────────────────────────────────────
// App state
// ──────────────────────────────────────────────────────────────

#[derive(Clone)]
struct AppState {
    in_dir: Arc<PathBuf>,
    cache_dir: Arc<PathBuf>,
    files: Arc<Vec<String>>, // available file stems (no extension)
    loaded: Arc<Mutex<HashMap<String, Arc<LoadedFile>>>>,
    load_lock: Arc<Mutex<()>>, // serialise expensive first-loads
}

impl AppState {
    async fn get_or_load(&self, name: &str) -> Result<Arc<LoadedFile>, String> {
        // Fast path: already in memory.
        {
            let loaded = self.loaded.lock().await;
            if let Some(f) = loaded.get(name) {
                return Ok(f.clone());
            }
        }
        // Slow path: serialise to avoid duplicate parse work.
        let _guard = self.load_lock.lock().await;
        {
            // Recheck under the load lock.
            let loaded = self.loaded.lock().await;
            if let Some(f) = loaded.get(name) {
                return Ok(f.clone());
            }
        }
        let ai_path = self.in_dir.join(format!("{name}.ai"));
        if !ai_path.exists() {
            return Err(format!("AI file not found: {}", ai_path.display()));
        }
        let cache_dir = (*self.cache_dir).clone();
        let ai_clone = ai_path.clone();
        let snap = tokio::task::spawn_blocking(move || {
            load_or_build(&ai_clone, &cache_dir, DEFAULT_TARGET, DEFAULT_SEED)
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?;
        let loaded_file = Arc::new(LoadedFile::from_snapshot(snap));
        self.loaded.lock().await.insert(name.to_string(), loaded_file.clone());
        Ok(loaded_file)
    }
}

// ──────────────────────────────────────────────────────────────
// API types
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Serialize)]
struct FileEntry {
    name: String,
    ready: bool,
}

#[derive(Debug, Serialize)]
struct FileList {
    files: Vec<FileEntry>,
    current: Option<String>,
}

#[derive(Debug, Serialize)]
struct PieceBrick { id: String, name: String }

#[derive(Debug, Serialize)]
struct PieceOut {
    id: String,
    bricks: Vec<PieceBrick>,
    svg: Vec<String>,
    bbox: Option<[f64; 4]>,
}

#[derive(Debug, Serialize)]
struct BrickPreview {
    id: String,
    name: String,
    svg: Vec<String>,
    vertices: Vec<Vec<[f64; 2]>>,
}

#[derive(Debug, Serialize)]
struct ApiResponse {
    file: String,
    source_file: String,
    canvas_width: u32,
    canvas_height: u32,
    seed: u64,
    target: usize,
    pieces: Vec<PieceOut>,
    bricks: Vec<BrickPreview>,
}

#[derive(Debug, Deserialize)]
struct PiecesQuery {
    file: Option<String>,
    piece: Option<String>,
    with_bricks: Option<String>,
    algo: Option<String>,
    seed: Option<u64>,
    target: Option<usize>,
}

fn truthy(s: &Option<String>) -> bool {
    match s.as_deref().map(|v| v.to_ascii_lowercase()) {
        Some(v) => matches!(v.as_str(), "1" | "true" | "yes" | "on"),
        None => false,
    }
}

// ──────────────────────────────────────────────────────────────
// Server
// ──────────────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let in_dir = PathBuf::from(std::env::args().nth(1).unwrap_or_else(|| {
        std::env::var("HP_IN_DIR").unwrap_or_else(|_|
            "../../in".to_string())
    }));
    let cache_dir = PathBuf::from(
        std::env::args().nth(2).unwrap_or_else(|| "crates/hp-testbed/testbed".to_string())
    );
    let port: u16 = std::env::var("HP_TESTBED_PORT")
        .ok()
        .and_then(|v| v.parse().ok())
        .unwrap_or(5051);

    let ai_files = scan_ai_dir(&in_dir).context("scanning AI dir")?;
    let files: Vec<String> = ai_files
        .iter()
        .filter_map(|p| p.file_stem().and_then(|s| s.to_str()).map(String::from))
        .collect();
    eprintln!(
        "[testbed] watching {} for AI files (found {}). cache: {}",
        in_dir.display(),
        files.len(),
        cache_dir.display()
    );

    let state = AppState {
        in_dir: Arc::new(in_dir),
        cache_dir: Arc::new(cache_dir),
        files: Arc::new(files),
        loaded: Arc::new(Mutex::new(HashMap::new())),
        load_lock: Arc::new(Mutex::new(())),
    };

    let app = Router::new()
        .route("/", get(index))
        .route("/api/files", get(files_handler))
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

async fn files_handler(State(state): State<AppState>) -> Json<FileList> {
    let loaded = state.loaded.lock().await;
    let files: Vec<FileEntry> = state
        .files
        .iter()
        .map(|name| {
            // "ready" if either loaded in memory or cached on disk.
            let ready = loaded.contains_key(name)
                || state.cache_dir.join(format!("{name}.json")).exists();
            FileEntry { name: name.clone(), ready }
        })
        .collect();
    let current = loaded.keys().next().cloned();
    Json(FileList { files, current })
}

async fn pieces_handler(
    State(state): State<AppState>,
    Query(q): Query<PiecesQuery>,
) -> Result<Json<ApiResponse>, (StatusCode, String)> {
    // Pick a file: explicit ?file=, else any loaded one, else first cached
    // on disk, else the first AI on disk.
    let name = pick_file(&state, q.file.as_deref()).await;
    let name = match name {
        Some(n) => n,
        None => return Err((StatusCode::NOT_FOUND, "no AI files available".into())),
    };

    let file = state.get_or_load(&name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;

    let t = &file.snap.transform;
    let to_canvas = |bp: &BezierPath| bp.transform([-t.clip_x0, -t.clip_y0], t.scale);

    let default_target = file.snap.pieces.len();
    let recompute = q.seed.map_or(false, |s| s != DEFAULT_SEED)
        || q.target.map_or(false, |t| t != default_target);

    let fresh: Option<Vec<PuzzlePiece>> = if recompute {
        let seed = q.seed.unwrap_or(DEFAULT_SEED);
        let target = q.target.unwrap_or(default_target);
        Some(puzzle::merge_bricks(
            &file.bricks_engine,
            target,
            seed,
            &file.adjacency,
            &file.areas,
        ))
    } else { None };

    let source: &[PuzzlePiece] = match fresh.as_deref() {
        Some(v) => v,
        None => file.snap.pieces.as_slice(),
    };

    let selected: Vec<&PuzzlePiece> = match &q.piece {
        Some(id) => source.iter().filter(|p| &p.id == id).collect(),
        None => source.iter().collect(),
    };

    let use_polyline = matches!(q.algo.as_deref(), Some("polyline"));

    // Bricks-by-id lookup for this file.
    let bricks_by_id: HashMap<&str, &hp_testbed::BrickOut> =
        file.snap.bricks.iter().map(|b| (b.id.as_str(), b)).collect();

    let mut pieces_out: Vec<PieceOut> = Vec::with_capacity(selected.len());
    for piece in &selected {
        let mut brick_paths: Vec<BezierPath> = Vec::new();
        for bid in &piece.brick_ids {
            if let Some(b) = bricks_by_id.get(bid.as_str()) {
                brick_paths.extend(b.beziers.iter().cloned());
            }
        }
        let merged = if use_polyline {
            bezier_merge::merge_piece(&brick_paths)
        } else {
            bezier_merge::merge_piece_bezier(&brick_paths)
        };

        let mut min_x = f64::INFINITY;
        let mut min_y = f64::INFINITY;
        let mut max_x = f64::NEG_INFINITY;
        let mut max_y = f64::NEG_INFINITY;
        for bp in &merged {
            let scaled = to_canvas(bp);
            for v in scaled.vertices() {
                min_x = min_x.min(v[0]); min_y = min_y.min(v[1]);
                max_x = max_x.max(v[0]); max_y = max_y.max(v[1]);
            }
        }
        let bbox = if merged.is_empty() || !min_x.is_finite() {
            None
        } else { Some([min_x, min_y, max_x, max_y]) };

        let svg: Vec<String> = merged.iter().map(|bp| to_canvas(bp).to_svg_d()).collect();
        let bricks: Vec<PieceBrick> = piece
            .brick_ids
            .iter()
            .map(|bid| {
                let name = bricks_by_id.get(bid.as_str()).map(|b| b.name.clone()).unwrap_or_default();
                PieceBrick { id: bid.clone(), name }
            })
            .collect();
        pieces_out.push(PieceOut { id: piece.id.clone(), bricks, svg, bbox });
    }

    let bricks_out = if truthy(&q.with_bricks) {
        let wanted: HashSet<&str> = match &q.piece {
            Some(_) => pieces_out.iter().flat_map(|p| p.bricks.iter().map(|b| b.id.as_str())).collect(),
            None => bricks_by_id.keys().copied().collect(),
        };
        let mut v: Vec<BrickPreview> = Vec::new();
        for id in wanted {
            if let Some(b) = bricks_by_id.get(id) {
                let scaled: Vec<BezierPath> = b.beziers.iter().map(&to_canvas).collect();
                let svg: Vec<String> = scaled.iter().map(|bp| bp.to_svg_d()).collect();
                let vertices: Vec<Vec<[f64; 2]>> = scaled.iter().map(|bp| bp.vertices()).collect();
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
        file: name,
        source_file: file.snap.source_file.clone(),
        canvas_width: t.canvas_width,
        canvas_height: t.canvas_height,
        seed: q.seed.unwrap_or(DEFAULT_SEED),
        target: q.target.unwrap_or(default_target),
        pieces: pieces_out,
        bricks: bricks_out,
    }))
}

async fn pick_file(state: &AppState, requested: Option<&str>) -> Option<String> {
    if let Some(name) = requested {
        if state.files.iter().any(|f| f == name) {
            return Some(name.to_string());
        }
    }
    // Prefer an already-loaded file; else a cached one; else the first listed.
    {
        let loaded = state.loaded.lock().await;
        if let Some(k) = loaded.keys().next() {
            return Some(k.clone());
        }
    }
    for name in state.files.iter() {
        if state.cache_dir.join(format!("{name}.json")).exists() {
            return Some(name.clone());
        }
    }
    state.files.first().cloned()
}
