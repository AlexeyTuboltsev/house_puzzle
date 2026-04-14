use axum::{
    Router,
    extract::{DefaultBodyLimit, Path, State},
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
        .route("/api/upload_file", post(api_upload_file).layer(DefaultBodyLimit::max(200 * 1024 * 1024)))
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
        .route("/api/export", post(api_export))
        .route("/api/s/{key}/export", post(api_export_keyed))
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
            let app_version = option_env!("HP_VERSION").unwrap_or("dev");
            let html = html.replace("{{ elm_version }}", &elm_js_version.to_string());
            let html = html.replace("{{ app_version }}", app_version);
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
// Upload
// ---------------------------------------------------------------------------

async fn api_upload_file(mut multipart: axum::extract::Multipart) -> Response {
    while let Ok(Some(field)) = multipart.next_field().await {
        if field.name() == Some("file") {
            let file_name = field.file_name().unwrap_or("upload.ai").to_string();
            let safe_name = std::path::Path::new(&file_name)
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();

            let data = match field.bytes().await {
                Ok(d) => d,
                Err(e) => return (StatusCode::BAD_REQUEST, Json(json!({"error": e.to_string()}))).into_response(),
            };

            let in_dir = PathBuf::from("in");
            std::fs::create_dir_all(&in_dir).ok();
            let dest = in_dir.join(&safe_name);
            if let Err(e) = std::fs::write(&dest, &data) {
                return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response();
            }

            return Json(json!({"path": dest.to_string_lossy()})).into_response();
        }
    }

    (StatusCode::BAD_REQUEST, Json(json!({"error": "no file"}))).into_response()
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
    let t_total = std::time::Instant::now();
    let file_path = PathBuf::from(&req.path);
    if !file_path.exists() {
        return (StatusCode::NOT_FOUND, Json(json!({"error": format!("File not found: {}", req.path)}))).into_response();
    }

    // Parse AI file (blocking — runs in spawn_blocking)
    let canvas_height = req.canvas_height;
    let deterministic = req.deterministic_ids;
    let path_clone = file_path.clone();

    let t0 = std::time::Instant::now();
    let result = tokio::task::spawn_blocking(move || {
        ai_parser::parse_ai(&path_clone, canvas_height)
    }).await;
    eprintln!("[profile] parse_ai: {:?}", t0.elapsed());

    let (placements, metadata, ai_data) = match result {
        Ok(Ok(v)) => v,
        Ok(Err(e)) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    };

    // Assign brick IDs
    let mut bricks: Vec<Brick> = Vec::new();
    let mut brick_polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let layer_blocks: HashMap<String, ai_parser::LayerBlock> = HashMap::new();
    let mut brick_layer_names: HashMap<String, String> = HashMap::new();

    for p in placements.iter() {
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
        brick_layer_names.insert(id, p.name.clone());
    }

    // Note: adjacency+areas computed after covered-brick filtering below

    // Build extract dir and render PNGs
    let extract_dir = std::env::temp_dir().join("house_puzzle_extract").join(&key);
    std::fs::create_dir_all(&extract_dir).ok();

    // Build (id, placement) pairs for the renderer
    let mut render_bricks: Vec<(String, hp_core::ai_parser::BrickPlacement)> = bricks.iter()
        .zip(placements.iter())
        .map(|(b, p)| (b.id.clone(), p.clone()))
        .collect();

    let cw = metadata.canvas_width as u32;
    let ch = metadata.canvas_height as u32;

    // Render bricks layer via MuPDF OCG — first at (0,0) to probe offset
    let t0 = std::time::Instant::now();
    let bricks_no_offset = match render::render_ocg_layer_image(
        &file_path, "bricks", metadata.render_dpi, metadata.clip_rect,
        cw, ch, (0, 0),
    ) {
        Some(img) => img,
        None => {
            return (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": "Failed to render bricks layer"}))).into_response();
        }
    };
    // Compute pdf_offset from first opaque pixel vs expected position
    let pdf_offset = render::compute_pdf_offset(
        &bricks_no_offset, metadata.expected_brick_min.0, metadata.expected_brick_min.1,
    );
    // Re-render with correct offset so content isn't clipped
    let bricks_layer_img = if pdf_offset != (0, 0) {
        render::render_ocg_layer_image(
            &file_path, "bricks", metadata.render_dpi, metadata.clip_rect,
            cw, ch, pdf_offset,
        ).unwrap_or(bricks_no_offset)
    } else {
        bricks_no_offset
    };
    eprintln!("[profile] OCG bricks (render+offset): {:?} ({}x{}, offset={:?})", t0.elapsed(), bricks_layer_img.width(), bricks_layer_img.height(), pdf_offset);

    // Hybrid brick rendering: raster bricks from embedded data, vector from OCG
    // Hybrid brick rendering: raster bricks from embedded data, vector from OCG
    // Used for piece composition (no polygon mask gaps). MuPDF composite stays for display.
    let t0 = std::time::Instant::now();
    let bp_vec: Vec<(String, hp_core::ai_parser::BrickPlacement)> = render_bricks.clone();
    let brick_images = render::render_brick_images_hybrid(
        &bp_vec, &ai_data.raw, cw, ch, &bricks_layer_img,
    );
    eprintln!("[profile] hybrid render_brick_images: {:?}", t0.elapsed());
    // bricks_layer_img stays as the MuPDF OCG render (seamless composite for display)

    // Filter covered bricks using in-memory images
    // Collect warnings
    let mut all_warnings: Vec<String> = metadata.warnings.clone();
    all_warnings.extend(metadata.skipped_bricks.iter().map(|n| format!("SKIPPED: '{n}' has no vector polygon")));

    // Protect vector bricks from covered-brick removal
    let protected: std::collections::HashSet<String> = render_bricks.iter()
        .filter(|(_, bp)| bp.layer_type == "vector_brick")
        .map(|(id, _)| id.clone())
        .collect();
    let t0 = std::time::Instant::now();
    let covered_ids = render::find_covered_bricks(&bricks, &brick_images, &protected);
    eprintln!("[profile] covered_bricks: {:?}", t0.elapsed());
    if !covered_ids.is_empty() {
        eprintln!("[load] Removing {} covered bricks", covered_ids.len());
        // Add warnings for removed bricks
        for id in &covered_ids {
            let layer_name = brick_layer_names.get(id).cloned().unwrap_or_default();
            all_warnings.push(format!("COVERED: '{}' removed (hidden under another brick)", layer_name));
        }
        bricks.retain(|b| !covered_ids.contains(&b.id));
        render_bricks.retain(|(id, _)| !covered_ids.contains(id));
        for id in &covered_ids {
            brick_polygons.remove(id);
        }
    }

    // Recompute adjacency and areas after filtering
    let adj = puzzle::build_adjacency_vector(&bricks, &brick_polygons, 15.0, 5.0, 2.0);
    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);

    let bricks_layer_arc = std::sync::Arc::new(bricks_layer_img);

    // Store brick RGBA images for piece composition (remove covered ones)
    let mut brick_rgba: HashMap<String, Arc<image::RgbaImage>> = brick_images.into_iter()
        .filter(|(id, _)| !covered_ids.contains(id))
        .map(|(id, img)| (id, Arc::new(img)))
        .collect();

    // Save composite + render outlines + OCG layers — all in parallel
    let t0 = std::time::Instant::now();
    let clip = metadata.clip_rect;
    let dpi = metadata.render_dpi;
    let fp1 = file_path.clone();
    let fp2 = file_path.clone();
    let ed1 = extract_dir.clone();
    let ed2 = extract_dir.clone();
    let ed3 = extract_dir.clone();
    let ed4 = extract_dir.clone();
    let rb = render_bricks.clone();
    let bla = bricks_layer_arc.clone();
    let (_, _, has_lights, has_background) = tokio::join!(
        tokio::task::spawn_blocking(move || {
            render::save_composite(&bla, &ed4.join("composite.png"));
        }),
        tokio::task::spawn_blocking(move || {
            render::render_outlines_png(&rb, cw, ch, &ed3.join("outlines.png"));
        }),
        tokio::task::spawn_blocking(move || {
            render::render_ocg_layer(&fp1, "lights", &ed1.join("lights.png"), dpi, clip, cw, ch, pdf_offset)
        }),
        tokio::task::spawn_blocking(move || {
            render::render_ocg_layer(&fp2, "background", &ed2.join("background.png"), dpi, clip, cw, ch, pdf_offset)
        }),
    );
    let has_lights = has_lights.unwrap_or(false);
    let has_background = has_background.unwrap_or(false);
    eprintln!("[profile] composite+outlines+lights+bg (parallel): {:?}", t0.elapsed());

    // Build brick response data
    let brick_data: Vec<serde_json::Value> = bricks.iter().map(|b| {
        let neighbors: Vec<&str> = adj.get(&b.id)
            .map(|s| s.iter().map(|n| n.as_str()).collect())
            .unwrap_or_default();
        let polygon = brick_polygons.get(&b.id)
            .map(|p| p.iter().map(|pt| json!([pt[0], pt[1]])).collect::<Vec<_>>())
            .unwrap_or_default();
        let layer_name = brick_layer_names.get(&b.id).cloned().unwrap_or_default();
        json!({
            "id": b.id,
            "x": b.x, "y": b.y,
            "width": b.width, "height": b.height,
            "type": b.brick_type,
            "layer_name": layer_name,
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
        let mut store = sessions.lock();
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
            bricks_layer_img: bricks_layer_arc,
            brick_images: HashMap::new(),
            brick_rgba,
        });
    }

    let response = json!({
        "key": key,
        "canvas": { "width": metadata.canvas_width, "height": metadata.canvas_height },
        "total_layers": 0,
        "num_bricks": brick_data.len(),
        "bricks": brick_data,
        "has_composite": true,
        "has_base": false,
        "render_dpi": (metadata.render_dpi * 100.0).round() / 100.0,
        "warnings": all_warnings,
        "houseUnitsHigh": house_units_high,
        "composite_url": format!("{pfx}/composite.png"),
        "outlines_url": format!("{pfx}/outlines.png"),
        "lights_url": if has_lights { Some(format!("{pfx}/lights.png")) } else { None },
        "blueprint_bg_url": if has_background { Some(format!("{pfx}/background.png")) } else { None },
    });

    eprintln!("[profile] TOTAL load: {:?}", t_total.elapsed());
    Json(response).into_response()
}

// ---------------------------------------------------------------------------
// Merge
// ---------------------------------------------------------------------------

async fn api_merge_keyed(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    do_merge(sessions, &key, req).await
}

async fn api_merge(
    State(sessions): State<SessionStore>,
    Json(req): Json<serde_json::Value>,
) -> Response {
    let key = {
        let store = sessions.lock();
        store.keys().last().cloned().unwrap_or_default()
    };
    do_merge(sessions, &key, req).await
}

async fn do_merge(sessions: SessionStore, key: &str, req: serde_json::Value) -> Response {
    let (bricks, placements, polygons, areas, extract_dir, bricks_layer_img, brick_rgba) = {
        let store = sessions.lock();
        let session = match store.get(key) {
            Some(s) => s,
            None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))).into_response(),
        };
        (session.bricks.clone(), session.brick_placements.clone(),
         session.brick_polygons.clone(),
         session.brick_areas.clone(), session.extract_dir.clone(),
         session.bricks_layer_img.clone(), session.brick_rgba.clone())
    };

    let bricks_by_id: HashMap<String, Brick> = bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    // Two modes: recompute (has "pieces" key) or normal merge
    let pieces = if let Some(pieces_arr) = req.get("pieces").and_then(|v| v.as_array()) {
        // Recompute mode: rebuild pieces from supplied definitions
        pieces_arr.iter().filter_map(|p| {
            let id = p.get("id")?.as_str()?.to_string();
            let brick_ids: Vec<String> = p.get("brick_ids")?
                .as_array()?
                .iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect();
            if brick_ids.is_empty() { return None; }
            let mut x = i32::MAX;
            let mut y = i32::MAX;
            let mut x2 = i32::MIN;
            let mut y2 = i32::MIN;
            for bid in &brick_ids {
                if let Some(b) = bricks_by_id.get(bid) {
                    x = x.min(b.x);
                    y = y.min(b.y);
                    x2 = x2.max(b.right());
                    y2 = y2.max(b.bottom());
                }
            }
            Some(hp_core::types::PuzzlePiece {
                id, brick_ids,
                x, y, width: x2 - x, height: y2 - y,
            })
        }).collect()
    } else {
        // Normal merge
        let target = req.get("target_count").and_then(|v| v.as_u64()).unwrap_or(60) as usize;
        let seed = req.get("seed").and_then(|v| v.as_u64()).unwrap_or(42);
        let min_border = req.get("min_border").and_then(|v| v.as_f64()).unwrap_or(5.0);
        let border_gap = req.get("border_gap").and_then(|v| v.as_f64()).unwrap_or(2.0);
        let adj = puzzle::build_adjacency_vector(&bricks, &polygons, 15.0, min_border, border_gap);
        puzzle::merge_bricks(&bricks, target, seed, &adj, &areas)
    };

    // Compute piece polygons first (union of brick vector polygons)
    let bricks_by_id: HashMap<String, Brick> = bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();
    let piece_polys = puzzle::compute_piece_polygons(&pieces, &bricks_by_id, &polygons);

    // Compose piece PNGs by cropping the MuPDF composite with brick polygon masks
    // Composite is seamless internally; mask only clips the outer piece boundary
    render::render_piece_pngs_from_composite(&pieces, &bricks_layer_img, &bricks_by_id, &polygons, &extract_dir);

    let bricks_by_id_ref: HashMap<&str, &Brick> = bricks.iter().map(|b| (b.id.as_str(), b)).collect();

    let pieces_json: Vec<serde_json::Value> = pieces.iter().map(|p| {
        let brick_refs: Vec<serde_json::Value> = p.brick_ids.iter().filter_map(|bid| {
            bricks_by_id_ref.get(bid.as_str()).map(|b| json!({
                "id": b.id, "x": b.x, "y": b.y, "width": b.width, "height": b.height,
            }))
        }).collect();
        let poly = piece_polys.get(&p.id)
            .map(|pts| pts.iter().map(|pt| json!([pt[0], pt[1]])).collect::<Vec<_>>())
            .unwrap_or_default();
        json!({
            "id": p.id,
            "x": p.x, "y": p.y,
            "width": p.width, "height": p.height,
            "brick_ids": p.brick_ids,
            "bricks": brick_refs,
            "polygon": poly,
            "img_url": "",
            "outline_url": "",
        })
    }).collect();

    // Store pieces in session
    {
        let mut store = sessions.lock();
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
// Export
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ExportRequest {
    #[serde(default)]
    waves: Vec<serde_json::Value>,
    #[serde(default)]
    groups: Vec<serde_json::Value>,
    #[serde(default)]
    placement: serde_json::Value,
    #[serde(default = "default_export_height")]
    export_canvas_height: i32,
}

fn default_export_height() -> i32 { 900 }

async fn api_export_keyed(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    Json(req): Json<ExportRequest>,
) -> Response {
    do_export(sessions, &key, req).await
}

async fn api_export(
    State(sessions): State<SessionStore>,
    Json(req): Json<ExportRequest>,
) -> Response {
    let key = {
        let store = sessions.lock();
        store.keys().last().cloned().unwrap_or_default()
    };
    do_export(sessions, &key, req).await
}

async fn do_export(sessions: SessionStore, key: &str, req: ExportRequest) -> Response {
    let (pieces, bricks, metadata, extract_dir) = {
        let store = sessions.lock();
        let session = match store.get(key) {
            Some(s) => s,
            None => return (StatusCode::NOT_FOUND, Json(json!({"error": "Session not found"}))).into_response(),
        };
        (session.pieces.clone(), session.bricks.clone(),
         session.metadata.clone(), session.extract_dir.clone())
    };

    if pieces.is_empty() {
        return (StatusCode::BAD_REQUEST, Json(json!({"error": "No puzzle computed"}))).into_response();
    }

    let bricks_by_id: HashMap<String, Brick> = bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    let placement = &req.placement;
    let location = placement.get("location").and_then(|v| v.as_str()).unwrap_or("Rome");
    let position = placement.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
    let house_name = placement.get("houseName").and_then(|v| v.as_str()).unwrap_or("NewHouse");
    let spacing = placement.get("spacing").and_then(|v| v.as_f64()).unwrap_or(12.0);

    let zip_data = hp_core::export::generate_export_zip(
        &pieces, &bricks_by_id, &extract_dir,
        metadata.canvas_width, metadata.canvas_height,
        metadata.screen_frame_height_px,
        &req.waves, &req.groups,
        location, position, house_name, spacing,
    );

    match zip_data {
        Ok(data) => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "application/zip".to_string()),
             (header::CONTENT_DISPOSITION, format!("attachment; filename=\"{house_name}.zip\""))],
            data,
        ).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, Json(json!({"error": e.to_string()}))).into_response(),
    }
}

// ---------------------------------------------------------------------------
// PNG serving
// ---------------------------------------------------------------------------

async fn api_serve_png(
    State(sessions): State<SessionStore>,
    Path(key): Path<String>,
    req: axum::extract::Request,
) -> Response {
    let uri = req.uri().path().to_string();
    let filename = uri.rsplit('/').next().unwrap_or("").split('?').next().unwrap_or("");

    let extract_dir = {
        let store = sessions.lock();
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

    // Try serving from session cache first, then generate on demand
    let png_bytes = {
        let store = sessions.lock();
        let session = match store.get(&key) {
            Some(s) => s,
            None => return StatusCode::NOT_FOUND.into_response(),
        };

        if let Some(cached) = session.brick_images.get(brick_id) {
            Some(cached.clone())
        } else {
            None
        }
    };

    if let Some(bytes) = png_bytes {
        return (StatusCode::OK, [(header::CONTENT_TYPE, "image/png".to_string())], bytes.as_ref().clone()).into_response();
    }

    // Generate on demand from bricks_layer_img
    let generated = {
        let mut store = sessions.lock();
        let session = match store.get_mut(&key) {
            Some(s) => s,
            None => return StatusCode::NOT_FOUND.into_response(),
        };

        // Find the brick placement
        let bp_idx = session.bricks.iter().position(|b| b.id == brick_id);
        if let Some(idx) = bp_idx {
            let bp = &session.brick_placements[idx];
            let cw = session.metadata.canvas_width as u32;
            let ch = session.metadata.canvas_height as u32;
            let mut canvas = image::RgbaImage::new(cw, ch);

            let poly = bp.polygon.as_ref();
            for dy in 0..bp.height.max(0) {
                for dx in 0..bp.width.max(0) {
                    let sx = (bp.x + dx) as u32;
                    let sy = (bp.y + dy) as u32;
                    if sx < session.bricks_layer_img.width() && sy < session.bricks_layer_img.height() {
                        let px = session.bricks_layer_img.get_pixel(sx, sy);
                        if px[3] > 0 {
                            let in_poly = match poly {
                                Some(pts) if pts.len() >= 3 => {
                                    render::point_in_polygon(dx as f64 + 0.5, dy as f64 + 0.5, pts)
                                }
                                _ => true,
                            };
                            if in_poly {
                                canvas.put_pixel(sx, sy, *px);
                            }
                        }
                    }
                }
            }

            // Encode to PNG bytes
            let mut buf = std::io::Cursor::new(Vec::new());
            canvas.write_to(&mut buf, image::ImageOutputFormat::Png).ok();
            let bytes = Arc::new(buf.into_inner());
            session.brick_images.insert(brick_id.to_string(), bytes.clone());
            Some(bytes)
        } else {
            None
        }
    };

    match generated {
        Some(bytes) => (StatusCode::OK, [(header::CONTENT_TYPE, "image/png".to_string())], bytes.as_ref().clone()).into_response(),
        None => (
            StatusCode::OK,
            [(header::CONTENT_TYPE, "image/png".to_string())],
            include_bytes!("../assets/transparent_1x1.png").to_vec(),
        ).into_response(),
    }
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
        let store = sessions.lock();
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
