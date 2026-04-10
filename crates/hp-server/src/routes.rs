use axum::{
    Router,
    extract::{Path, State},
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Json,
};
use rust_embed::Embed;
use serde::Deserialize;
use serde_json::json;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use hp_core::{ai_parser, puzzle, render, types::Brick};
use crate::session::{Session, SessionStore};

/// Embedded template (elm.html).
#[derive(Embed)]
#[folder = "../../templates/"]
struct Templates;

/// Embedded static files (elm.js, etc.).
#[derive(Embed)]
#[folder = "../../static/"]
struct StaticFiles;

pub fn build_router(sessions: SessionStore) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/list_pdfs", get(api_list_pdfs))
        .route("/api/load_pdf", post(api_load_pdf))
        .route("/api/s/{key}/load", post(api_load_pdf_keyed))
        .route("/api/merge", post(api_merge))
        .route("/api/s/{key}/merge", post(api_merge_keyed))
        .route("/api/s/{key}/composite.png", get(api_serve_png))
        .route("/api/s/{key}/outlines.png", get(api_serve_png))
        .route("/api/s/{key}/lights.png", get(api_serve_png))
        .route("/api/s/{key}/background.png", get(api_serve_png))
        .route("/api/s/{key}/brick/{*rest}", get(api_serve_brick_png))
        .route("/api/s/{key}/piece/{*rest}", get(api_serve_piece_png))
        .route("/api/s/{key}/piece_outline/{*rest}", get(api_serve_piece_png))
        .route("/static/{*path}", get(static_file))
        .with_state(sessions)
}

// ---------------------------------------------------------------------------
// Static / index
// ---------------------------------------------------------------------------

async fn index() -> Html<String> {
    match Templates::get("elm.html") {
        Some(content) => {
            let html = std::str::from_utf8(content.data.as_ref())
                .unwrap_or("")
                .to_string();
            let elm_js_version = std::fs::metadata("static/elm.js")
                .and_then(|m| m.modified())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                .unwrap_or(0);
            let html = html.replace("{{ elm_version }}", &elm_js_version.to_string());
            Html(html)
        }
        None => Html("<h1>elm.html not found</h1>".to_string()),
    }
}

async fn static_file(Path(path): Path<String>) -> Response {
    let clean_path = path.split('?').next().unwrap_or(&path);
    match StaticFiles::get(clean_path) {
        Some(content) => {
            let mime = mime_guess::from_path(clean_path)
                .first_or_octet_stream()
                .to_string();
            (StatusCode::OK, [(header::CONTENT_TYPE, mime)], content.data.to_vec()).into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn api_list_pdfs() -> Json<serde_json::Value> {
    let in_dir = PathBuf::from("in");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&in_dir) {
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("ai") || ext.eq_ignore_ascii_case("pdf") {
                let name = path.file_name().unwrap_or_default().to_string_lossy().to_string();
                let size_mb = std::fs::metadata(&path)
                    .map(|m| (m.len() as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0)
                    .unwrap_or(0.0);
                files.push(json!({ "name": name, "path": path.to_string_lossy(), "size_mb": size_mb }));
            }
        }
    }
    Json(json!({ "files": files }))
}

// ---------------------------------------------------------------------------
// Load
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct LoadRequest {
    path: String,
    #[serde(default = "default_canvas_height")]
    canvas_height: i32,
    #[serde(default)]
    deterministic_ids: bool,
}

fn default_canvas_height() -> i32 { 900 }

async fn api_load_pdf_keyed(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    Json(req): Json<LoadRequest>,
) -> Response {
    do_load(sessions, key, req).await
}

async fn api_load_pdf(
    State(sessions): State<SessionStore>,
    Json(req): Json<LoadRequest>,
) -> Response {
    let key = uuid::Uuid::new_v4().to_string()[..8].to_string();
    do_load(sessions, key, req).await
}

async fn do_load(sessions: SessionStore, key: String, req: LoadRequest) -> Response {
    let file_path = PathBuf::from(&req.path);
    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": format!("File not found: {}", req.path)}))).into_response();
    }

    // Parse AI file (blocking — runs in spawn_blocking)
    let canvas_height = req.canvas_height;
    let deterministic = req.deterministic_ids;
    let path_clone = file_path.clone();

    let result = tokio::task::spawn_blocking(move || {
        ai_parser::parse_ai(&path_clone, canvas_height)
    }).await;

    let (placements, metadata, ai_data) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    // Assign brick IDs
    let mut bricks: Vec<Brick> = Vec::new();
    let mut brick_polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut layer_blocks: HashMap<String, ai_parser::LayerBlock> = HashMap::new();

    for (i, p) in placements.iter().enumerate() {
        let id = if deterministic {
            use std::hash::{Hash, Hasher};
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            (p.x, p.y, p.width, p.height).hash(&mut hasher);
            format!("{:08x}", hasher.finish() & 0xFFFFFFFF)
        } else {
            uuid::Uuid::new_v4().to_string()[..8].to_string()
        };

        bricks.push(Brick {
            id: id.clone(),
            x: p.x,
            y: p.y,
            width: p.width,
            height: p.height,
            brick_type: p.layer_type.clone(),
        });
        if let Some(poly) = &p.polygon {
            brick_polygons.insert(id.clone(), poly.clone());
        }
    }

    // Compute adjacency and areas
    let adj = puzzle::build_adjacency_vector(&bricks, &brick_polygons, 15.0, 5.0, 2.0);
    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);

    // Build extract dir and render PNGs
    let extract_dir = std::env::temp_dir().join("house_puzzle_extract").join(&key);
    std::fs::create_dir_all(&extract_dir).ok();

    // Build (id, placement) pairs for the renderer
    let render_bricks: Vec<(String, hp_core::ai_parser::BrickPlacement)> = bricks.iter()
        .zip(placements.iter())
        .map(|(b, p)| (b.id.clone(), p.clone()))
        .collect();

    let cw = metadata.canvas_width as u32;
    let ch = metadata.canvas_height as u32;
    let raw = &ai_data.raw;

    // Render brick PNGs (parallelized with rayon)
    render::render_brick_pngs(raw, &render_bricks, cw, ch, &extract_dir);

    // Render composite
    let comp_path = extract_dir.join("composite.png");
    render::render_composite_png(raw, &render_bricks, cw, ch, &comp_path);

    // Build brick response data
    let brick_data: Vec<serde_json::Value> = bricks.iter().map(|b| {
        let neighbors: Vec<&str> = adj.get(&b.id)
            .map(|s| s.iter().map(|n| n.as_str()).collect())
            .unwrap_or_default();
        let polygon = brick_polygons.get(&b.id)
            .map(|p| p.iter().map(|pt| json!([pt[0], pt[1]])).collect::<Vec<_>>())
            .unwrap_or_default();
        json!({
            "id": b.id,
            "x": b.x, "y": b.y,
            "width": b.width, "height": b.height,
            "type": b.brick_type,
            "neighbors": neighbors,
            "polygon": polygon,
        })
    }).collect();

    let pfx = format!("/api/s/{key}");
    let house_units_high = if metadata.screen_frame_height_px > 0.0 {
        (metadata.canvas_height as f64 / metadata.screen_frame_height_px * 15.5 * 10000.0).round() / 10000.0
    } else {
        15.5
    };

    // Store session
    let ai_data = Arc::new(ai_data);
    {
        let mut store = sessions.lock().unwrap();
        store.insert(key.clone(), Session {
            bricks,
            brick_placements: placements,
            brick_polygons,
            brick_areas,
            pieces: Vec::new(),
            metadata: metadata.clone(),
            extract_dir,
            ai_data,
            layer_blocks,
        });
    }

    let response = json!({
        "key": key,
        "canvas": { "width": metadata.canvas_width, "height": metadata.canvas_height },
        "total_layers": 0,
        "num_bricks": brick_data.len(),
        "bricks": brick_data,
        "has_composite": comp_path.exists(),
        "has_base": false,
        "render_dpi": (metadata.render_dpi * 100.0).round() / 100.0,
        "warnings": metadata.skipped_bricks.iter().map(|n| format!("SKIPPED: {n}")).collect::<Vec<_>>(),
        "houseUnitsHigh": house_units_high,
        "composite_url": format!("{pfx}/composite.png"),
        "outlines_url": format!("{pfx}/outlines.png"),
        "lights_url": null,
        "blueprint_bg_url": null,
    });

    Json(response).into_response()
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct MergeRequest {
    #[serde(default = "default_target")]
    target_count: usize,
    #[serde(default = "default_seed")]
    seed: u64,
    #[serde(default = "default_min_border")]
    min_border: f64,
    #[serde(default = "default_border_gap")]
    border_gap: f64,
}

fn default_target() -> usize { 60 }
fn default_seed() -> u64 { 42 }
fn default_min_border() -> f64 { 5.0 }
fn default_border_gap() -> f64 { 2.0 }

async fn api_merge_keyed(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    Json(req): Json<MergeRequest>,
) -> Response {
    do_merge(sessions, &key, req).await
}

async fn api_merge(
    State(sessions): State<SessionStore>,
    Json(req): Json<MergeRequest>,
) -> Response {
    // Use the most recent session (legacy compat)
    let key = {
        let store = sessions.lock().unwrap();
        store.keys().last().cloned().unwrap_or_default()
    };
    do_merge(sessions, &key, req).await
}

async fn do_merge(sessions: SessionStore, key: &str, req: MergeRequest) -> Response {
    let (bricks, polygons, areas, extract_dir) = {
        let store = sessions.lock().unwrap();
        let session = match store.get(key) {
            Some(s) => s,
            None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))).into_response(),
        };
        (session.bricks.clone(), session.brick_polygons.clone(),
         session.brick_areas.clone(), session.extract_dir.clone())
    };

    // Build adjacency with user's params and merge
    let adj = puzzle::build_adjacency_vector(&bricks, &polygons, 15.0, req.min_border, req.border_gap);
    let pieces = puzzle::merge_bricks(&bricks, req.target_count, req.seed, &adj, &areas);

    // Render piece PNGs (composited from brick PNGs on disk)
    let bricks_by_id: HashMap<String, Brick> = bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();
    render::render_piece_pngs(&pieces, &bricks_by_id, &extract_dir);

    let bricks_by_id_ref: HashMap<&str, &Brick> = bricks.iter().map(|b| (b.id.as_str(), b)).collect();

    let pieces_json: Vec<serde_json::Value> = pieces.iter().map(|p| {
        let brick_refs: Vec<serde_json::Value> = p.brick_ids.iter().filter_map(|bid| {
            bricks_by_id_ref.get(bid.as_str()).map(|b| json!({
                "id": b.id, "x": b.x, "y": b.y, "width": b.width, "height": b.height,
            }))
        }).collect();
        json!({
            "id": p.id,
            "x": p.x, "y": p.y,
            "width": p.width, "height": p.height,
            "brick_ids": p.brick_ids,
            "bricks": brick_refs,
            "polygon": [],
            "img_url": "",
            "outline_url": "",
        })
    }).collect();

    // Store pieces in session
    {
        let mut store = sessions.lock().unwrap();
        if let Some(session) = store.get_mut(key) {
            session.pieces = pieces;
        }
    }

    Json(json!({
        "num_pieces": pieces_json.len(),
        "pieces": pieces_json,
    })).into_response()
}

// ---------------------------------------------------------------------------
// PNG serving (stubs — will serve actual rendered PNGs later)
// ---------------------------------------------------------------------------

async fn api_serve_png(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let uri = req.uri().path().to_string();
    let filename = uri.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");

    let extract_dir = {
        let store = sessions.lock().unwrap();
        match store.get(&key) {
            Some(s) => s.extract_dir.clone(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };

    let file_path = extract_dir.join(filename);
    if !file_path.exists() {
        // Return transparent 1x1 PNG as placeholder
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png".to_string()),
             (header::CACHE_CONTROL, "no-store".to_string())],
            include_bytes!("../assets/transparent_1x1.png").to_vec(),
        ).into_response();
    }

    let data = std::fs::read(&file_path).unwrap_or_default();
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "image/png".to_string()),
         (header::CACHE_CONTROL, "no-store".to_string())],
        data,
    ).into_response()
}

async fn api_serve_brick_png(
    State(sessions): State<SessionStore>,
    Path((key, rest)): Path<(String, String)>,
) -> Response {
    let brick_id = rest.trim_end_matches(".png");
    let extract_dir = {
        let store = sessions.lock().unwrap();
        match store.get(&key) {
            Some(s) => s.extract_dir.clone(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };

    let file_path = extract_dir.join(format!("brick_{brick_id}.png"));
    if !file_path.exists() {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png".to_string())],
            include_bytes!("../assets/transparent_1x1.png").to_vec(),
        ).into_response();
    }

    let data = std::fs::read(&file_path).unwrap_or_default();
    (StatusCode::OK, [(header::CONTENT_TYPE, "image/png".to_string())], data).into_response()
}

async fn api_serve_piece_png(
    State(sessions): State<SessionStore>,
    Path((key, rest)): Path<(String, String)>,
    req: axum::extract::Request,
) -> Response {
    let piece_id = rest.trim_end_matches(".png");
    let uri = req.uri().path().to_string();
    let is_outline = uri.contains("piece_outline");
    let prefix = if is_outline { "piece_outline" } else { "piece" };

    let extract_dir = {
        let store = sessions.lock().unwrap();
        match store.get(&key) {
            Some(s) => s.extract_dir.clone(),
            None => return StatusCode::NOT_FOUND.into_response(),
        }
    };

    let file_path = extract_dir.join(format!("{prefix}_{piece_id}.png"));
    if !file_path.exists() {
        return (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png".to_string())],
            include_bytes!("../assets/transparent_1x1.png").to_vec(),
        ).into_response();
    }

    let data = std::fs::read(&file_path).unwrap_or_default();
    (StatusCode::OK, [(header::CONTENT_TYPE, "image/png".to_string())], data).into_response()
}
