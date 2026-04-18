//! Tauri commands — ported from hp-server Axum handlers.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hp_core::{ai_parser, puzzle, render, types::Brick};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::session::{Session, SessionStore};

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_version() -> String {
    option_env!("HP_VERSION").unwrap_or("dev").to_string()
}

// ---------------------------------------------------------------------------
// List PDFs
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_pdfs() -> Result<Value, String> {
    let in_dir = PathBuf::from("in");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&in_dir) {
        let mut entries: Vec<_> = entries.filter_map(|e| e.ok()).collect();
        entries.sort_by_key(|e| e.file_name());
        for entry in entries {
            let path = entry.path();
            let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
            if ext.eq_ignore_ascii_case("ai") || ext.eq_ignore_ascii_case("pdf") {
                let name = path
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
                    .to_string();
                let size_mb = std::fs::metadata(&path)
                    .map(|m| (m.len() as f64 / (1024.0 * 1024.0) * 10.0).round() / 10.0)
                    .unwrap_or(0.0);
                files.push(json!({
                    "name": name,
                    "path": path.to_string_lossy(),
                    "size_mb": size_mb,
                }));
            }
        }
    }
    Ok(json!({ "files": files }))
}

// ---------------------------------------------------------------------------
// Native file picker — uses tauri-plugin-dialog
// ---------------------------------------------------------------------------

/// Opens a native file-open dialog filtered to `.pdf` and `.ai` files.
/// Returns the selected file path as a string, or `null` if the user cancelled.
#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    // blocking_pick_file must not run on the async executor — use spawn_blocking.
    let file_path = tokio::task::spawn_blocking(move || {
        app.dialog()
            .file()
            .add_filter("PDF / AI Files", &["pdf", "ai"])
            .blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    // `FilePath` may be a `file://` URL on Linux (xdg-desktop-portal).
    // `into_path()` converts both variants to a plain `PathBuf`.
    file_path
        .map(|fp| {
            fp.into_path()
                .map_err(|e| e.to_string())
                .map(|p| p.to_string_lossy().into_owned())
        })
        .transpose()
}

// ---------------------------------------------------------------------------
// Load PDF — mirrors do_load in routes.rs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn load_pdf(
    sessions: tauri::State<'_, SessionStore>,
    path: String,
    canvas_height: Option<i32>,
    deterministic_ids: Option<bool>,
) -> Result<Value, String> {
    let canvas_height = canvas_height.unwrap_or(900);
    let deterministic = deterministic_ids.unwrap_or(false);

    let t_total = std::time::Instant::now();
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {path}"));
    }

    // Generate a short session key
    let key = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // Parse AI file in blocking thread
    let path_clone = file_path.clone();
    let t0 = std::time::Instant::now();
    let parse_result = tokio::task::spawn_blocking(move || {
        ai_parser::parse_ai(&path_clone, canvas_height)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    eprintln!("[profile] parse_ai: {:?}", t0.elapsed());

    let (placements, metadata, ai_data) = parse_result;

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

    let extract_dir = std::env::temp_dir()
        .join("house_puzzle_extract")
        .join(&key);
    std::fs::create_dir_all(&extract_dir).map_err(|e| e.to_string())?;

    let mut render_bricks: Vec<(String, hp_core::ai_parser::BrickPlacement)> = bricks
        .iter()
        .zip(placements.iter())
        .map(|(b, p)| (b.id.clone(), p.clone()))
        .collect();

    let cw = metadata.canvas_width as u32;
    let ch = metadata.canvas_height as u32;

    // Render bricks OCG layer, compute offset, re-render if needed
    let t0 = std::time::Instant::now();
    let fp_bricks = file_path.clone();
    let clip = metadata.clip_rect;
    let dpi = metadata.render_dpi;
    let expected_min = metadata.expected_brick_min;
    let bricks_no_offset = tokio::task::spawn_blocking(move || {
        render::render_ocg_layer_image(
            &fp_bricks, "bricks", dpi, clip, cw, ch, (0, 0),
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Failed to render bricks layer".to_string())?;

    let pdf_offset = render::compute_pdf_offset(
        &bricks_no_offset,
        expected_min.0,
        expected_min.1,
    );

    let bricks_layer_img = if pdf_offset != (0, 0) {
        let fp2 = file_path.clone();
        let bricks_no_offset_clone = bricks_no_offset;
        tokio::task::spawn_blocking(move || {
            render::render_ocg_layer_image(
                &fp2, "bricks", dpi, clip, cw, ch, pdf_offset,
            )
            .unwrap_or(bricks_no_offset_clone)
        })
        .await
        .map_err(|e| e.to_string())?
    } else {
        bricks_no_offset
    };
    eprintln!(
        "[profile] OCG bricks (render+offset): {:?} ({}x{}, offset={:?})",
        t0.elapsed(),
        bricks_layer_img.width(),
        bricks_layer_img.height(),
        pdf_offset,
    );

    // Hybrid brick rendering
    let t0 = std::time::Instant::now();
    let bp_vec: Vec<(String, hp_core::ai_parser::BrickPlacement)> = render_bricks.clone();
    let bla_for_hybrid = bricks_layer_img.clone();
    let ai_raw = ai_data.raw.clone();
    let brick_images_map = tokio::task::spawn_blocking(move || {
        render::render_brick_images_hybrid(&bp_vec, &ai_raw, cw, ch, &bla_for_hybrid)
    })
    .await
    .map_err(|e| e.to_string())?;
    eprintln!("[profile] hybrid render_brick_images: {:?}", t0.elapsed());

    // Filter covered bricks
    let mut all_warnings: Vec<String> = metadata.warnings.clone();
    all_warnings.extend(
        metadata
            .skipped_bricks
            .iter()
            .map(|n| format!("SKIPPED: '{n}' has no vector polygon")),
    );

    let protected: std::collections::HashSet<String> = render_bricks
        .iter()
        .filter(|(_, bp)| bp.layer_type == "vector_brick")
        .map(|(id, _)| id.clone())
        .collect();

    let t0 = std::time::Instant::now();
    let covered_ids = render::find_covered_bricks(&bricks, &brick_images_map, &protected);
    eprintln!("[profile] covered_bricks: {:?}", t0.elapsed());

    if !covered_ids.is_empty() {
        eprintln!("[load] Removing {} covered bricks", covered_ids.len());
        for id in &covered_ids {
            let layer_name = brick_layer_names.get(id).cloned().unwrap_or_default();
            all_warnings.push(format!(
                "COVERED: '{}' removed (hidden under another brick)",
                layer_name
            ));
        }
        bricks.retain(|b| !covered_ids.contains(&b.id));
        render_bricks.retain(|(id, _)| !covered_ids.contains(id));
        for id in &covered_ids {
            brick_polygons.remove(id);
        }
    }

    // Recompute adjacency and areas
    let adj = puzzle::build_adjacency_vector(&bricks, &brick_polygons, 15.0, 5.0, 2.0);
    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);

    let bricks_layer_arc = Arc::new(bricks_layer_img);

    let brick_rgba: HashMap<String, Arc<image::RgbaImage>> = brick_images_map
        .into_iter()
        .filter(|(id, _)| !covered_ids.contains(id))
        .map(|(id, img)| (id, Arc::new(img)))
        .collect();

    // Render composite + outlines + OCG layers in parallel
    let t0 = std::time::Instant::now();
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
            render::render_ocg_layer(
                &fp2,
                "background",
                &ed2.join("background.png"),
                dpi,
                clip,
                cw,
                ch,
                pdf_offset,
            )
        }),
    );
    let has_lights = has_lights.unwrap_or(false);
    let has_background = has_background.unwrap_or(false);
    eprintln!(
        "[profile] composite+outlines+lights+bg (parallel): {:?}",
        t0.elapsed()
    );

    // Build brick response data
    let brick_data: Vec<Value> = bricks
        .iter()
        .map(|b| {
            let neighbors: Vec<&str> = adj
                .get(&b.id)
                .map(|s| s.iter().map(|n| n.as_str()).collect())
                .unwrap_or_default();
            let polygon = brick_polygons
                .get(&b.id)
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
        })
        .collect();

    let house_units_high = if metadata.screen_frame_height_px > 0.0 {
        (metadata.canvas_height as f64 / metadata.screen_frame_height_px * 15.5 * 10000.0).round()
            / 10000.0
    } else {
        15.5
    };

    // Store session
    let ai_data = Arc::new(ai_data);
    {
        let mut store = sessions.lock();
        store.insert(
            key.clone(),
            Session {
                bricks,
                brick_placements: placements,
                brick_polygons,
                brick_areas,
                pieces: Vec::new(),
                metadata: metadata.clone(),
                extract_dir: extract_dir.clone(),
                ai_data,
                layer_blocks,
                bricks_layer_img: bricks_layer_arc,
                brick_images: HashMap::new(),
                brick_rgba,
            },
        );
    }

    eprintln!("[profile] TOTAL load: {:?}", t_total.elapsed());

    Ok(json!({
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
        "has_lights": has_lights,
        "has_background": has_background,
    }))
}

// ---------------------------------------------------------------------------
// Merge pieces — mirrors do_merge in routes.rs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn merge_pieces(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    target_count: Option<u64>,
    seed: Option<u64>,
    min_border: Option<f64>,
    border_gap: Option<f64>,
    // Optional: recompute mode — array of { id, brick_ids } objects.
    pieces: Option<Vec<Value>>,
) -> Result<Value, String> {
    let (bricks, polygons, areas, extract_dir, bricks_layer_img, brick_rgba) = {
        let store = sessions.lock();
        let session = store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?;
        (
            session.bricks.clone(),
            session.brick_polygons.clone(),
            session.brick_areas.clone(),
            session.extract_dir.clone(),
            session.bricks_layer_img.clone(),
            session.brick_rgba.clone(),
        )
    };

    let bricks_by_id: HashMap<String, Brick> = bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    let computed_pieces: Vec<hp_core::types::PuzzlePiece> = if let Some(pieces_arr) = pieces {
        // Recompute mode: rebuild pieces from supplied definitions
        pieces_arr
            .iter()
            .filter_map(|p| {
                let id = p.get("id")?.as_str()?.to_string();
                let brick_ids: Vec<String> = p
                    .get("brick_ids")?
                    .as_array()?
                    .iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect();
                if brick_ids.is_empty() {
                    return None;
                }
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
                    id,
                    brick_ids,
                    x,
                    y,
                    width: x2 - x,
                    height: y2 - y,
                })
            })
            .collect()
    } else {
        // Normal merge
        let target = target_count.unwrap_or(60) as usize;
        let seed_val = seed.unwrap_or(42);
        let min_b = min_border.unwrap_or(5.0);
        let b_gap = border_gap.unwrap_or(2.0);
        let adj = puzzle::build_adjacency_vector(&bricks, &polygons, 15.0, min_b, b_gap);
        puzzle::merge_bricks(&bricks, target, seed_val, &adj, &areas)
    };

    // Compute piece polygons
    let piece_polys =
        puzzle::compute_piece_polygons(&computed_pieces, &bricks_by_id, &polygons);

    // Render piece PNGs
    let pieces_clone = computed_pieces.clone();
    let piece_polys_clone = piece_polys.clone();
    let ed = extract_dir.clone();
    let bla = bricks_layer_img.clone();
    tokio::task::spawn_blocking(move || {
        render::render_piece_pngs_from_composite(
            &pieces_clone,
            &bla,
            &piece_polys_clone,
            &ed,
        );
    })
    .await
    .map_err(|e| e.to_string())?;

    let bricks_by_id_ref: HashMap<&str, &Brick> =
        bricks.iter().map(|b| (b.id.as_str(), b)).collect();

    let pieces_json: Vec<Value> = computed_pieces
        .iter()
        .map(|p| {
            let brick_refs: Vec<Value> = p
                .brick_ids
                .iter()
                .filter_map(|bid| {
                    bricks_by_id_ref.get(bid.as_str()).map(|b| {
                        json!({
                            "id": b.id, "x": b.x, "y": b.y,
                            "width": b.width, "height": b.height,
                        })
                    })
                })
                .collect();
            let poly = piece_polys
                .get(&p.id)
                .map(|pts| pts.iter().map(|pt| json!([pt[0], pt[1]])).collect::<Vec<_>>())
                .unwrap_or_default();
            json!({
                "id": p.id,
                "x": p.x, "y": p.y,
                "width": p.width, "height": p.height,
                "brick_ids": p.brick_ids,
                "bricks": brick_refs,
                "polygon": poly,
            })
        })
        .collect();

    // Store pieces in session
    {
        let mut store = sessions.lock();
        if let Some(session) = store.get_mut(&key) {
            session.pieces = computed_pieces;
        }
    }

    Ok(json!({
        "num_pieces": pieces_json.len(),
        "pieces": pieces_json,
    }))
}

// ---------------------------------------------------------------------------
// Image helpers — return PNG as base64 string
// ---------------------------------------------------------------------------

const TRANSPARENT_1X1_PNG: &[u8] = include_bytes!("../assets/transparent_1x1.png");

fn read_png_base64(path: &PathBuf) -> String {
    let data = std::fs::read(path).unwrap_or_else(|_| TRANSPARENT_1X1_PNG.to_vec());
    BASE64.encode(&data)
}

/// Returns composite / outlines / lights / background PNG as base64.
/// `image_type` must be one of: "composite", "outlines", "lights", "background"
#[tauri::command]
pub fn get_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    image_type: String,
) -> Result<String, String> {
    let extract_dir = {
        let store = sessions.lock();
        store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?
            .extract_dir
            .clone()
    };

    let filename = match image_type.as_str() {
        "composite" => "composite.png",
        "outlines" => "outlines.png",
        "lights" => "lights.png",
        "background" => "background.png",
        other => return Err(format!("Unknown image type: {other}")),
    };

    Ok(read_png_base64(&extract_dir.join(filename)))
}

/// Returns a brick PNG (polygon-masked) as base64.
#[tauri::command]
pub fn get_brick_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    brick_id: String,
) -> Result<String, String> {
    // Check cache first
    let cached = {
        let store = sessions.lock();
        store
            .get(&key)
            .and_then(|s| s.brick_images.get(&brick_id).cloned())
    };

    if let Some(bytes) = cached {
        return Ok(BASE64.encode(bytes.as_ref()));
    }

    // Generate on demand
    let mut store = sessions.lock();
    let session = store
        .get_mut(&key)
        .ok_or_else(|| format!("Session not found: {key}"))?;

    let bp_idx = session.bricks.iter().position(|b| b.id == brick_id);
    match bp_idx {
        None => Ok(BASE64.encode(TRANSPARENT_1X1_PNG)),
        Some(idx) => {
            let bp = &session.brick_placements[idx];
            let bw = bp.width.max(0) as u32;
            let bh = bp.height.max(0) as u32;
            let mut brick_img = image::RgbaImage::new(bw, bh);
            let poly = bp.polygon.as_ref();

            for dy in 0..bp.height.max(0) {
                for dx in 0..bp.width.max(0) {
                    let sx = (bp.x + dx) as u32;
                    let sy = (bp.y + dy) as u32;
                    if sx < session.bricks_layer_img.width()
                        && sy < session.bricks_layer_img.height()
                    {
                        let px = session.bricks_layer_img.get_pixel(sx, sy);
                        if px[3] > 0 {
                            let in_poly = match poly {
                                Some(pts) if pts.len() >= 3 => render::point_in_polygon(
                                    dx as f64 + 0.5,
                                    dy as f64 + 0.5,
                                    pts,
                                ),
                                _ => true,
                            };
                            if in_poly {
                                brick_img.put_pixel(dx as u32, dy as u32, *px);
                            }
                        }
                    }
                }
            }

            let mut buf = std::io::Cursor::new(Vec::new());
            brick_img
                .write_to(&mut buf, image::ImageOutputFormat::Png)
                .map_err(|e| e.to_string())?;
            let bytes = Arc::new(buf.into_inner());
            session
                .brick_images
                .insert(brick_id.clone(), bytes.clone());
            Ok(BASE64.encode(bytes.as_ref()))
        }
    }
}

/// Returns a piece PNG as base64.
#[tauri::command]
pub fn get_piece_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    piece_id: String,
) -> Result<String, String> {
    let extract_dir = {
        let store = sessions.lock();
        store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?
            .extract_dir
            .clone()
    };

    let file_path = extract_dir.join(format!("piece_{piece_id}.png"));
    Ok(read_png_base64(&file_path))
}

/// Returns a piece outline PNG as base64.
#[tauri::command]
pub fn get_piece_outline_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    piece_id: String,
) -> Result<String, String> {
    let extract_dir = {
        let store = sessions.lock();
        store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?
            .extract_dir
            .clone()
    };

    let file_path = extract_dir.join(format!("piece_outline_{piece_id}.png"));
    Ok(read_png_base64(&file_path))
}

// ---------------------------------------------------------------------------
// Export — mirrors do_export in routes.rs; returns base64-encoded ZIP
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn export_data(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    waves: Option<Vec<Value>>,
    groups: Option<Vec<Value>>,
    placement: Option<Value>,
    export_canvas_height: Option<i32>,
) -> Result<String, String> {
    let (pieces, bricks, metadata, extract_dir) = {
        let store = sessions.lock();
        let session = store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?;
        (
            session.pieces.clone(),
            session.bricks.clone(),
            session.metadata.clone(),
            session.extract_dir.clone(),
        )
    };

    if pieces.is_empty() {
        return Err("No puzzle computed".to_string());
    }

    let bricks_by_id: HashMap<String, Brick> =
        bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    let placement = placement.unwrap_or_else(|| json!({}));
    let location = placement
        .get("location")
        .and_then(|v| v.as_str())
        .unwrap_or("Rome")
        .to_string();
    let position = placement
        .get("position")
        .and_then(|v| v.as_i64())
        .unwrap_or(0) as i32;
    let house_name = placement
        .get("houseName")
        .and_then(|v| v.as_str())
        .unwrap_or("NewHouse")
        .to_string();
    let spacing = placement
        .get("spacing")
        .and_then(|v| v.as_f64())
        .unwrap_or(12.0);

    let waves_val = waves.unwrap_or_default();
    let groups_val = groups.unwrap_or_default();

    let zip_data = tokio::task::spawn_blocking(move || {
        hp_core::export::generate_export_zip(
            &pieces,
            &bricks_by_id,
            &extract_dir,
            metadata.canvas_width,
            metadata.canvas_height,
            metadata.screen_frame_height_px,
            &waves_val,
            &groups_val,
            &location,
            position,
            &house_name,
            spacing,
        )
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    Ok(BASE64.encode(&zip_data))
}

// ---------------------------------------------------------------------------
// Updater
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn check_for_updates(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_updater::UpdaterExt;

    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;

    Ok(update.map(|u| u.version.to_string()))
}
