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

/// Floor for the composite raster's render DPI. Native DPI for typical
/// houses sits near 30 (because `CANVAS_HEIGHT_PX` ≈ 900 and the AI
/// pages are large in PDF points), which makes the popup look pixelated
/// the moment you zoom in. We render at `max(native_dpi, this)` so the
/// composite has enough pixels to survive the modal's wheel-zoom.
const MIN_COMPOSITE_DPI: f64 = 200.0;

/// Cached composite raster + the multiplier between native canvas
/// pixels and composite pixels. Pieces clipped from the composite need
/// to scale their canvas-coord polygons by this factor before
/// indexing into the high-res buffer.
struct CompositeImage {
    img: image::RgbaImage,
    upscale: f64,
}

// ──────────────────────────────────────────────────────────────
// Per-file loaded state (snapshot + engine)
// ──────────────────────────────────────────────────────────────

struct LoadedFile {
    ai_path: PathBuf,
    snap: Snapshot,
    bricks_engine: Vec<Brick>,
    adjacency: HashMap<String, HashSet<String>>,
    areas: HashMap<String, f64>,
    /// All-layers-visible composite raster, rendered on first request and
    /// cached for the life of the loaded file. Wrapped in tokio's
    /// OnceCell so concurrent requests share the single render call.
    composite: tokio::sync::OnceCell<Arc<CompositeImage>>,
}

impl LoadedFile {
    fn from_snapshot(ai_path: PathBuf, snap: Snapshot) -> Self {
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
        LoadedFile {
            ai_path,
            snap,
            bricks_engine,
            adjacency,
            areas,
            composite: tokio::sync::OnceCell::new(),
        }
    }

    /// Render (once) the all-layers-visible composite raster at a DPI of
    /// at least `MIN_COMPOSITE_DPI`, cropped to the canvas. Returns the
    /// rendered image plus the upscale factor (composite DPI / native
    /// DPI ≥ 1.0) so callers that need to project canvas-coord polygons
    /// into the composite's pixel grid can do so.
    ///
    /// MuPDF renders the page using the PDF's MediaBox/CropBox origin,
    /// which can sit a few pixels off the AI's content origin. To align
    /// with our canvas (which uses the AI's clip rect as its origin) we
    /// render the bricks OCG layer once at native DPI / offset=(0,0) and
    /// call `compute_pdf_offset` to derive the per-file shift; that
    /// offset (in pixels) is then scaled by the upscale factor for the
    /// final composite render.
    async fn composite(&self) -> Result<Arc<CompositeImage>, String> {
        self.composite
            .get_or_try_init(|| async {
                let ai_path = self.ai_path.clone();
                let t = self.snap.transform.clone();

                // Expected min brick position in canvas pixels at native DPI.
                let mut min_x = f64::INFINITY;
                let mut min_y = f64::INFINITY;
                for b in &self.snap.bricks {
                    for bp in &b.beziers {
                        let probe = |p: [f64; 2]| (p[0], p[1]);
                        let (sx, sy) = probe(bp.start);
                        if sx < min_x { min_x = sx; }
                        if sy < min_y { min_y = sy; }
                        for seg in &bp.segments {
                            let (ex, ey) = probe(seg.end());
                            if ex < min_x { min_x = ex; }
                            if ey < min_y { min_y = ey; }
                        }
                    }
                }
                tokio::task::spawn_blocking(move || {
                    let native_dpi = t.scale * 72.0;
                    let render_dpi = native_dpi.max(MIN_COMPOSITE_DPI);
                    let render_scale = render_dpi / 72.0;
                    let upscale = render_dpi / native_dpi;
                    let out_w = (t.canvas_width as f64 * upscale).round() as u32;
                    let out_h = (t.canvas_height as f64 * upscale).round() as u32;
                    let clip = (t.clip_x0, t.clip_y0, 0.0, 0.0);

                    // Probe at the FINAL render DPI so the offset is
                    // measured at full precision — measuring at native
                    // DPI and scaling up amplifies the ±0.5 pixel
                    // quantisation error by `upscale` (e.g. 6.8× at 200
                    // DPI for a 30-DPI house, → ±3.4 px misalignment).
                    let expected_min_x = ((min_x - t.clip_x0) * render_scale).round() as i32;
                    let expected_min_y = ((min_y - t.clip_y0) * render_scale).round() as i32;
                    let probe = hp_core::render::render_ocg_layer_image(
                        &ai_path, "bricks", render_dpi, clip,
                        out_w, out_h, (0, 0),
                    );
                    let pdf_offset_render = match probe.as_ref() {
                        Some(img) => hp_core::render::compute_pdf_offset(
                            img, expected_min_x, expected_min_y,
                        ),
                        None => (0, 0),
                    };
                    eprintln!(
                        "[composite] native_dpi={native_dpi:.2} render_dpi={render_dpi:.2} \
                         upscale={upscale:.2} expected_min_render=({},{}) \
                         offset_render=({},{}) out={out_w}x{out_h}",
                        expected_min_x, expected_min_y,
                        pdf_offset_render.0, pdf_offset_render.1,
                    );

                    let img = hp_core::render::render_composite_image(
                        &ai_path, render_dpi, clip,
                        out_w, out_h, pdf_offset_render,
                    )
                    .ok_or_else(|| "render_composite_image returned None".to_string())?;
                    Ok::<_, String>(Arc::new(CompositeImage { img, upscale }))
                })
                .await
                .map_err(|e| e.to_string())?
            })
            .await
            .cloned()
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
        let loaded_file = Arc::new(LoadedFile::from_snapshot(ai_path.clone(), snap));
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
        .route("/api/composite", get(composite_handler))
        .route("/api/piece-png", get(piece_png_handler))
        .route("/api/brick-png", get(brick_png_handler))
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

// ──────────────────────────────────────────────────────────────
// Composite & piece-PNG handlers
// ──────────────────────────────────────────────────────────────

#[derive(Debug, Deserialize)]
struct PieceQuery {
    file: Option<String>,
    piece: String,
    seed: Option<u64>,
    target: Option<usize>,
}

#[derive(Debug, Deserialize)]
struct CompositeQuery {
    file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct BrickQuery {
    file: Option<String>,
    brick: String,
}

fn png_response(img: &image::RgbaImage) -> Result<axum::response::Response, (StatusCode, String)> {
    use axum::response::IntoResponse;
    let mut buf: Vec<u8> = Vec::new();
    image::DynamicImage::ImageRgba8(img.clone())
        .write_to(&mut std::io::Cursor::new(&mut buf), image::ImageOutputFormat::Png)
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    Ok((
        [
            ("content-type", "image/png"),
            ("cache-control", "no-store"),
        ],
        buf,
    )
        .into_response())
}

async fn composite_handler(
    State(state): State<AppState>,
    Query(q): Query<CompositeQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let name = pick_file(&state, q.file.as_deref())
        .await
        .ok_or((StatusCode::NOT_FOUND, "no AI files available".into()))?;
    let file = state.get_or_load(&name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let composite = file
        .composite()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    png_response(&composite.img)
}

async fn piece_png_handler(
    State(state): State<AppState>,
    Query(q): Query<PieceQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let name = pick_file(&state, q.file.as_deref())
        .await
        .ok_or((StatusCode::NOT_FOUND, "no AI files available".into()))?;
    let file = state.get_or_load(&name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let composite = file
        .composite()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let upscale = composite.upscale;

    // Resolve the piece (recompute if seed/target differ from snapshot).
    let default_target = file.snap.pieces.len();
    let recompute = q.seed.map_or(false, |s| s != DEFAULT_SEED)
        || q.target.map_or(false, |t| t != default_target);
    let fresh: Option<Vec<PuzzlePiece>> = if recompute {
        let seed = q.seed.unwrap_or(DEFAULT_SEED);
        let target = q.target.unwrap_or(default_target);
        Some(puzzle::merge_bricks(
            &file.bricks_engine, target, seed, &file.adjacency, &file.areas,
        ))
    } else { None };
    let source: &[PuzzlePiece] = match fresh.as_deref() {
        Some(v) => v,
        None => file.snap.pieces.as_slice(),
    };
    let piece = source
        .iter()
        .find(|p| p.id == q.piece)
        .ok_or((StatusCode::NOT_FOUND, format!("piece {} not found", q.piece)))?;

    // Merge the piece's bricks to the bezier outline, then tessellate
    // each closed loop to a polygon in canvas coordinates and clip the
    // composite by alpha-masking everything outside the union of polygons.
    let bricks_by_id: HashMap<&str, &hp_testbed::BrickOut> =
        file.snap.bricks.iter().map(|b| (b.id.as_str(), b)).collect();
    let mut input: Vec<BezierPath> = Vec::new();
    for bid in &piece.brick_ids {
        if let Some(b) = bricks_by_id.get(bid.as_str()) {
            input.extend(b.beziers.iter().cloned());
        }
    }
    let merged = hp_core::bezier_merge::merge_piece_bezier(&input);

    // Polygons in the composite's pixel grid: bezier coords are in PDF
    // points, the canvas is `t.scale` px/pt, and the composite is
    // `upscale` × that.
    let t = &file.snap.transform;
    let composite_scale = t.scale * upscale;
    let polys: Vec<Vec<[f64; 2]>> = merged
        .iter()
        .map(|bp| {
            bp.transform([-t.clip_x0, -t.clip_y0], composite_scale)
                .tessellate(16)
        })
        .collect();
    if polys.is_empty() || polys.iter().all(|p| p.len() < 3) {
        return Err((StatusCode::NOT_FOUND, "piece outline empty".into()));
    }

    // Piece bbox in composite-pixel coords, padded by upscale × 2 native px.
    let (mut x0, mut y0, mut x1, mut y1) =
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for poly in &polys {
        for v in poly {
            x0 = x0.min(v[0]); y0 = y0.min(v[1]);
            x1 = x1.max(v[0]); y1 = y1.max(v[1]);
        }
    }
    let pad = 2.0 * upscale;
    let cw = composite.img.width() as f64;
    let ch = composite.img.height() as f64;
    let bx0 = (x0 - pad).max(0.0).floor() as u32;
    let by0 = (y0 - pad).max(0.0).floor() as u32;
    let bx1 = (x1 + pad).min(cw - 1.0).ceil() as u32;
    let by1 = (y1 + pad).min(ch - 1.0).ceil() as u32;
    let bw = bx1.saturating_sub(bx0).max(1);
    let bh = by1.saturating_sub(by0).max(1);

    let composite_arc = composite.clone();
    let masked = tokio::task::spawn_blocking(move || {
        let mut out = image::RgbaImage::new(bw, bh);
        for y in 0..bh {
            for x in 0..bw {
                let cx = bx0 + x;
                let cy = by0 + y;
                let inside = polys
                    .iter()
                    .any(|p| point_in_ring([cx as f64 + 0.5, cy as f64 + 0.5], p));
                if inside {
                    out.put_pixel(x, y, *composite_arc.img.get_pixel(cx, cy));
                }
            }
        }
        out
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    png_response(&masked)
}

/// Composite clipped by a single brick's outline (not the piece's). Used
/// by the testbed when the user clicks a brick — they see exactly that
/// brick at high resolution rather than the whole piece it belongs to.
async fn brick_png_handler(
    State(state): State<AppState>,
    Query(q): Query<BrickQuery>,
) -> Result<axum::response::Response, (StatusCode, String)> {
    let name = pick_file(&state, q.file.as_deref())
        .await
        .ok_or((StatusCode::NOT_FOUND, "no AI files available".into()))?;
    let file = state.get_or_load(&name).await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let composite = file
        .composite()
        .await
        .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e))?;
    let upscale = composite.upscale;

    let brick = file.snap.bricks.iter()
        .find(|b| b.id == q.brick)
        .ok_or((StatusCode::NOT_FOUND, format!("brick {} not found", q.brick)))?;

    // Run the same dedup/merge that piece outlines use, so a compound
    // brick (outer + inner cutout) clips the composite exactly the way
    // the bezier merge would draw it.
    let merged = hp_core::bezier_merge::merge_piece_bezier(&brick.beziers);

    let t = &file.snap.transform;
    let composite_scale = t.scale * upscale;
    let polys: Vec<Vec<[f64; 2]>> = merged
        .iter()
        .map(|bp| {
            bp.transform([-t.clip_x0, -t.clip_y0], composite_scale)
                .tessellate(16)
        })
        .collect();
    if polys.is_empty() || polys.iter().all(|p| p.len() < 3) {
        return Err((StatusCode::NOT_FOUND, "brick outline empty".into()));
    }

    let (mut x0, mut y0, mut x1, mut y1) =
        (f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);
    for poly in &polys {
        for v in poly {
            x0 = x0.min(v[0]); y0 = y0.min(v[1]);
            x1 = x1.max(v[0]); y1 = y1.max(v[1]);
        }
    }
    let pad = 2.0 * upscale;
    let cw = composite.img.width() as f64;
    let ch = composite.img.height() as f64;
    let bx0 = (x0 - pad).max(0.0).floor() as u32;
    let by0 = (y0 - pad).max(0.0).floor() as u32;
    let bx1 = (x1 + pad).min(cw - 1.0).ceil() as u32;
    let by1 = (y1 + pad).min(ch - 1.0).ceil() as u32;
    let bw = bx1.saturating_sub(bx0).max(1);
    let bh = by1.saturating_sub(by0).max(1);

    let composite_arc = composite.clone();
    let masked = tokio::task::spawn_blocking(move || {
        let mut out = image::RgbaImage::new(bw, bh);
        for y in 0..bh {
            for x in 0..bw {
                let cx = bx0 + x;
                let cy = by0 + y;
                let inside = polys
                    .iter()
                    .any(|p| point_in_ring([cx as f64 + 0.5, cy as f64 + 0.5], p));
                if inside {
                    out.put_pixel(x, y, *composite_arc.img.get_pixel(cx, cy));
                }
            }
        }
        out
    })
    .await
    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()))?;
    png_response(&masked)
}

/// Even-odd ray cast for point-in-polygon.
fn point_in_ring(p: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let n = ring.len();
    if n < 3 { return false; }
    let mut inside = false;
    let (px, py) = (p[0], p[1]);
    let mut j = n - 1;
    for i in 0..n {
        let (xi, yi) = (ring[i][0], ring[i][1]);
        let (xj, yj) = (ring[j][0], ring[j][1]);
        if ((yi > py) != (yj > py))
            && (px < (xj - xi) * (py - yi) / (yj - yi + 1e-12) + xi)
        {
            inside = !inside;
        }
        j = i;
    }
    inside
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
