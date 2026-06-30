//! Tauri commands for the House Puzzle editor.

use base64::{Engine as _, engine::general_purpose::STANDARD as BASE64};
use hp_core::{ai_parser, bezier::BezierPath, bezier_merge, puzzle, render, types::Brick};
use rayon::prelude::*;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::session::{Session, SessionStore};

// ---------------------------------------------------------------------------
// Version
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn get_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
pub fn log_to_stderr(msg: String) {
    eprintln!("[webview] {msg}");
}

/// Whether the binary was launched with `--test-mode`. The frontend
/// reads this at startup and shows the test-only `in/` file list
/// (used by the E2E driver to click a fixture button). Production
/// builds never pass `--test-mode`, so end users never see it.
#[tauri::command]
pub fn get_test_mode() -> bool {
    std::env::args().any(|a| a == "--test-mode")
}

// ---------------------------------------------------------------------------
// List PDFs
// ---------------------------------------------------------------------------

#[tauri::command]
pub fn list_pdfs() -> Result<Value, String> {
    // HP_IN_DIR env var overrides default; then try CWD, then next to binary
    let in_dir = if let Ok(env_dir) = std::env::var("HP_IN_DIR") {
        PathBuf::from(env_dir)
    } else if PathBuf::from("in").is_dir() {
        PathBuf::from("in")
    } else if let Ok(exe) = std::env::current_exe() {
        let beside_exe = exe.parent().unwrap_or(std::path::Path::new(".")).join("in");
        if beside_exe.is_dir() { beside_exe } else { PathBuf::from("in") }
    } else {
        PathBuf::from("in")
    };
    eprintln!("[list_pdfs] looking in: {:?} (exists={})", in_dir, in_dir.is_dir());
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
                let abs_path = path.canonicalize().unwrap_or(path.clone());
                files.push(json!({
                    "name": name,
                    "path": abs_path.to_string_lossy(),
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
///
/// The last successfully picked directory is persisted to the app data
/// directory (see `last_dir_path`) and re-used as the dialog's start
/// location on subsequent invocations. Falls back to the OS default if
/// nothing has been saved yet (or the saved path no longer exists).
#[tauri::command]
pub async fn pick_file(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let stored_path = last_dir_path(&app);
    let default_dir: Option<PathBuf> = stored_path
        .as_ref()
        .and_then(|p| std::fs::read_to_string(p).ok())
        .map(|s| PathBuf::from(s.trim().to_string()))
        .filter(|p| p.is_dir());

    // blocking_pick_file must not run on the async executor — use spawn_blocking.
    let app_for_dialog = app.clone();
    let default_dir_for_dialog = default_dir.clone();
    let file_path = tokio::task::spawn_blocking(move || {
        let mut builder = app_for_dialog
            .dialog()
            .file()
            .add_filter("PDF / AI Files", &["pdf", "ai"]);
        if let Some(dir) = default_dir_for_dialog {
            builder = builder.set_directory(dir);
        }
        builder.blocking_pick_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    // `FilePath` may be a `file://` URL on Linux (xdg-desktop-portal).
    // `into_path()` converts both variants to a plain `PathBuf`.
    let result_path = file_path
        .map(|fp| {
            fp.into_path()
                .map_err(|e| e.to_string())
                .map(|p| p.to_string_lossy().into_owned())
        })
        .transpose()?;

    // Persist the parent directory so the next dialog opens in the same place.
    if let (Some(picked), Some(stored)) = (result_path.as_ref(), stored_path.as_ref()) {
        if let Some(parent) = PathBuf::from(picked).parent() {
            if let Some(parent_dir) = stored.parent() {
                let _ = std::fs::create_dir_all(parent_dir);
            }
            let _ = std::fs::write(stored, parent.to_string_lossy().as_bytes());
        }
    }

    Ok(result_path)
}

/// Path to the file we use to remember the last directory the user
/// picked from. `None` when the OS won't give us an app-data dir.
fn last_dir_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().app_data_dir().ok().map(|d| d.join("last_open_dir.txt"))
}

/// Path to the file we use to remember the last directory the user
/// saved an export ZIP to. Same sidecar pattern as `last_dir_path`.
fn last_export_dir_path(app: &tauri::AppHandle) -> Option<PathBuf> {
    use tauri::Manager;
    app.path().app_data_dir().ok().map(|d| d.join("last_export_dir.txt"))
}

fn read_last_dir(p: &Path) -> Option<PathBuf> {
    std::fs::read_to_string(p)
        .ok()
        .map(|s| PathBuf::from(s.trim().to_string()))
        .filter(|p| p.is_dir())
}

fn write_last_dir(stored: &Path, picked_file: &Path) {
    if let Some(parent) = picked_file.parent() {
        if let Some(stored_parent) = stored.parent() {
            let _ = std::fs::create_dir_all(stored_parent);
        }
        let _ = std::fs::write(stored, parent.to_string_lossy().as_bytes());
    }
}

// ---------------------------------------------------------------------------
// Load PDF — mirrors do_load in routes.rs
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn load_pdf(
    window: tauri::WebviewWindow,
    sessions: tauri::State<'_, SessionStore>,
    path: String,
    canvas_height: Option<i32>,
    deterministic_ids: Option<bool>,
) -> Result<Value, String> {
    let canvas_height = canvas_height.unwrap_or(900);
    // Default to deterministic brick IDs (hashed from x/y/w/h via the
    // process-stable SipHash with key (0,0)). UUIDs gave a fresh ID
    // every run, which leaked through the merge algorithm: two runs
    // with the same seed and the same brick set produced different
    // piece shapes, because tie-breaks landed in random alphabetical
    // order. The IDs are session-only and have no persistence story
    // that would care which form they take.
    let deterministic = deterministic_ids.unwrap_or(true);

    let t_total = std::time::Instant::now();
    let file_path = PathBuf::from(&path);
    if !file_path.exists() {
        return Err(format!("File not found: {path}"));
    }
    eprintln!(
        "[profile] load_pdf START: {} (canvas_height={canvas_height})",
        file_path.display()
    );

    // Generate a short session key
    let key = uuid::Uuid::new_v4().to_string()[..8].to_string();

    // ── Parse + bezier extraction (always fresh) ──────────────────────
    //
    // No on-disk cache: the user workflow is "open file, find an error,
    // fix it in Illustrator, reopen" — caching even by mtime had bitten
    // us with stale results. Always re-parse, accept the per-load cost.
    //
    // Run `parse_ai` first (its phases depend on each other), then
    // overlap bezier extraction (per-brick PostScript reparse, rayon
    // parallel) with the bricks OCG MuPDF render — neither needs the
    // other's output. Wall time of the join ≈ max(bezier, bricks_OCG).
    let path_for_parse = file_path.clone();
    let t0 = std::time::Instant::now();
    let parse_result = tokio::task::spawn_blocking(move || {
        ai_parser::parse_ai(&path_for_parse, canvas_height)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;
    eprintln!("[profile] parse_ai: {:?}", t0.elapsed());

    let (placements, metadata, ai_data) = parse_result;

    let cw = metadata.canvas_width as u32;
    let ch = metadata.canvas_height as u32;
    let clip = metadata.clip_rect;
    let dpi = metadata.render_dpi;
    let expected_min = metadata.expected_brick_min;

    let t_parallel = std::time::Instant::now();
    let ai_raw_for_beziers = ai_data.raw.clone();
    let placements_for_beziers: Vec<ai_parser::BrickPlacement> = placements.clone();
    let metadata_for_beziers = metadata.clone();
    let bezier_fut = tokio::task::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let res: Vec<Vec<BezierPath>> = placements_for_beziers
            .par_iter()
            .map(|p| {
                let block = ai_parser::LayerBlock {
                    name: p.name.clone(),
                    begin: p.block_begin,
                    end: p.block_end,
                    depth: 0,
                    children: Vec::new(),
                };
                ai_parser::extract_vector_path_bezier(
                    &block,
                    &ai_raw_for_beziers,
                    metadata_for_beziers.offset_x,
                    metadata_for_beziers.y_base,
                )
            })
            .collect();
        eprintln!(
            "[profile] extract_vector_path_bezier (parallel): {:?}",
            t0.elapsed()
        );
        res
    });

    // Analyse the AI's brick content stream — walk q...Q blocks, match
    // them to parser placements, derive the sub-pixel-precise pymu→PDF
    // bleed. The export pipeline already does this; the editor now uses
    // the same `bleed_pts` for both:
    //   - shifted MuPDF clip → bricks layer pixmap lands canvas-aligned
    //   - direct-extract overlay → raster bricks paint exactly where
    //     the parser polygons say they should
    //
    // This replaces the legacy `compute_pdf_offset` + shifted-re-render
    // path that broke on AI files with malformed OCG metadata (Sand9
    // returned blank; Sand10 ended up with the composite shifted vs
    // outlines).
    let fp_analyse = file_path.clone();
    let placements_for_analyse = placements.clone();
    let metadata_for_analyse = metadata.clone();
    let analyse_fut = tokio::task::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let doc = hp_core::lopdf::Document::load(&fp_analyse)
            .map_err(|e| format!("hp_core::lopdf::Document::load: {e}"))?;
        let page_id = doc.page_iter().next()
            .ok_or_else(|| "PDF has no pages".to_string())?;
        let analysis = hp_core::ocg_inject::analyse_brick_blocks(
            &doc, page_id, &fp_analyse, &placements_for_analyse, &metadata_for_analyse,
        ).map_err(|e| format!("analyse_brick_blocks: {e}"))?;
        eprintln!("[profile] analyse_brick_blocks (walk + probe + bleed): {:?}", t0.elapsed());
        Ok::<(hp_core::lopdf::Document, hp_core::ocg_inject::BrickBlockAnalysis), String>((doc, analysis))
    });

    let (bezier_per_brick, analyse_res) = tokio::join!(bezier_fut, analyse_fut);
    let bezier_per_brick = bezier_per_brick.map_err(|e| e.to_string())?;
    let (doc, analysis) = analyse_res.map_err(|e| e.to_string())??;
    eprintln!(
        "[profile] bezier + analyse (overlapped): {:?}",
        t_parallel.elapsed()
    );
    let bleed_pts = analysis.bleed_pts;
    let page_height_pt = analysis.page_height_pt;
    let blocks = analysis.blocks;

    // Shifted clip — meta.clip_rect translated by bleed_pts so MuPDF
    // renders the bricks layer aligned with the parser's pymu frame.
    let shifted_clip = (
        clip.0 + bleed_pts.0, clip.1 + bleed_pts.1,
        clip.2 + bleed_pts.0, clip.3 + bleed_pts.1,
    );

    let t_render = std::time::Instant::now();
    let fp_bricks = file_path.clone();
    let (bricks_pixmap, _, _) = tokio::task::spawn_blocking(move || {
        render::render_ocg_layer_pixmap_clipped(&fp_bricks, "bricks", dpi, shifted_clip)
    })
    .await
    .map_err(|e| e.to_string())?
    .ok_or_else(|| "Failed to render bricks layer".to_string())?;
    eprintln!("[profile] OCG bricks render (shifted clip): {:?}", t_render.elapsed());

    let mut bricks_layer_img = render::compose_clipped_canvas(
        &bricks_pixmap, "bricks", cw, ch, (0, 0),
    );
    // Count opaque pixels in the MuPDF-only layer to detect the
    // "bricks OCG is empty" case (Sand9). Surface this as a warning
    // below if it's far below what the parser expects.
    let mupdf_alpha_pixels: u64 = bricks_pixmap.pixels().filter(|p| p[3] > 30).count() as u64;
    let bricks_pixmap = Some(bricks_pixmap);

    // Direct-extract overlay: every Image XObject in the page's content
    // stream is decoded via lopdf and composited on top of MuPDF's
    // pixels — gives canvas-aligned raster bricks even when MuPDF's OCG
    // render returns an empty or shifted result (Sand9, Sand10).
    let t_overlay = std::time::Instant::now();
    let image_block_count = blocks.iter()
        .filter(|b| matches!(b.content, hp_core::ocg_inject::BrickContent::Image { .. }))
        .count();
    hp_core::raster_extract::compose_image_blocks_onto_canvas(
        &doc, &blocks, 0..blocks.len(),
        &mut bricks_layer_img, clip, page_height_pt, bleed_pts,
        dpi, false,
    );
    eprintln!(
        "[profile] direct-extract overlay ({} blocks): {:?}",
        blocks.len(), t_overlay.elapsed()
    );
    drop(doc);

    // ── Phantom polygon drop (E2) ─────────────────────────────────────
    //
    // Identify parser placements whose polygon-bbox region has ZERO
    // opaque pixels in the rendered bricks_layer_img. These are
    // "phantom bricks": the AI's private layer data listed them but
    // no actual content renders there. They create floating outlines
    // in the live preview, then empty holes after puzzle generation.
    // Real culprits we've seen: Sand10 had ~50 % phantoms caused by
    // wrong-coordinate layers in the AI private data.
    //
    // We sample the polygon-bbox interior (clipped to the canvas) and
    // count alpha > 30. Threshold: zero opaque pixels → drop. Anything
    // > 0 → keep (we err on the side of preserving real bricks; even
    // a few pixels of soft alpha bleed is enough to vote "real").
    let t_phantom = std::time::Instant::now();
    let canvas_w_px = bricks_layer_img.width() as i32;
    let canvas_h_px = bricks_layer_img.height() as i32;
    let phantom_mask: Vec<bool> = placements.iter().map(|p| {
        let poly = match p.polygon.as_ref() {
            Some(poly) if poly.len() >= 3 => poly,
            _ => return false, // no polygon → not phantom by this metric
        };
        let bx = p.pymu_x.max(0) as f64;
        let by = p.pymu_y.max(0) as f64;
        let mut x0 = f64::MAX; let mut y0 = f64::MAX;
        let mut x1 = f64::MIN; let mut y1 = f64::MIN;
        for v in poly {
            let cx = v[0] + bx;
            let cy = v[1] + by;
            if cx < x0 { x0 = cx; } if cy < y0 { y0 = cy; }
            if cx > x1 { x1 = cx; } if cy > y1 { y1 = cy; }
        }
        let ix0 = (x0.floor() as i32).clamp(0, canvas_w_px);
        let iy0 = (y0.floor() as i32).clamp(0, canvas_h_px);
        let ix1 = (x1.ceil()  as i32).clamp(0, canvas_w_px);
        let iy1 = (y1.ceil()  as i32).clamp(0, canvas_h_px);
        if ix1 <= ix0 || iy1 <= iy0 { return true; } // bbox outside canvas
        for y in iy0..iy1 {
            for x in ix0..ix1 {
                if bricks_layer_img.get_pixel(x as u32, y as u32)[3] > 30 {
                    return false; // found opaque content → real brick
                }
            }
        }
        true // no opaque content anywhere in the bbox → phantom
    }).collect();
    let phantom_count = phantom_mask.iter().filter(|&&p| p).count();
    let placements_before = placements.len();
    eprintln!(
        "[profile] phantom polygon scan ({} placements, {} phantom): {:?}",
        placements_before, phantom_count, t_phantom.elapsed(),
    );

    // Drop phantoms from placements + bezier_per_brick (same indices)
    // so every downstream consumer (brick IDs, render_bricks, session,
    // export) sees a consistent set with no ghosts.
    let placements: Vec<hp_core::ai_parser::BrickPlacement> = placements
        .into_iter()
        .zip(phantom_mask.iter())
        .filter_map(|(p, ph)| if *ph { None } else { Some(p) })
        .collect();
    let bezier_per_brick: Vec<Vec<BezierPath>> = bezier_per_brick
        .into_iter()
        .zip(phantom_mask.iter())
        .filter_map(|(b, ph)| if *ph { None } else { Some(b) })
        .collect();

    // Assign brick IDs.
    let mut bricks: Vec<Brick> = Vec::new();
    let mut brick_polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
    let mut brick_beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
    let mut brick_layer_names: HashMap<String, String> = HashMap::new();
    let mut brick_pymu_rects: HashMap<String, (i32, i32, i32, i32)> = HashMap::new();

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
        brick_beziers.insert(id.clone(), bezier_per_brick[i].clone());
        brick_layer_names.insert(id.clone(), p.name.clone());
        brick_pymu_rects.insert(id, (p.pymu_x, p.pymu_y, p.pymu_w, p.pymu_h));
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

    // Legacy `pdf_offset` field — kept on the session for any other
    // callers that still read it, but always zero now: shifted_clip
    // above handles the pymu↔PDF alignment for the bricks render,
    // so no further compose-time offset is needed.
    let pdf_offset = (0i32, 0i32);
    let _ = expected_min; // probe value, no longer consumed here

    drop(bricks_pixmap);
    eprintln!(
        "[profile] bricks_layer ready: {}x{}, bleed_pts=({:.2}, {:.2})",
        bricks_layer_img.width(),
        bricks_layer_img.height(),
        bleed_pts.0, bleed_pts.1,
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

    // E3 — surface E1 / E2 findings so the artist can see when the
    // AI file's OCG metadata is incomplete.
    //
    // OCG mismatch heuristic: MuPDF's bricks-OCG render came back
    // nearly empty (< 1000 alpha pixels — essentially blank) but the
    // content stream contains ≥ 10 raster bricks that the direct-
    // extract path had to paint anyway. Sand9 is the canonical case
    // (0 alpha pixels, 163 image blocks). NY-class well-authored
    // files end up well above this floor.
    if mupdf_alpha_pixels < 1000 && image_block_count >= 10 {
        all_warnings.push(format!(
            "OCG mismatch: bricks OCG render is nearly empty ({} alpha pixels) — \
             {} raster bricks loaded via direct extract instead. \
             In Illustrator, verify every brick path is inside the `bricks` layer.",
            mupdf_alpha_pixels, image_block_count,
        ));
    }
    if phantom_count > 0 {
        all_warnings.push(format!(
            "Phantom polygons dropped: {} of {} parser bricks had no rendered content. \
             Likely caused by stale layer entries in the AI's private data — \
             open the file in Illustrator and check for empty bricks/Layer NNN/ sub-layers.",
            phantom_count, placements_before,
        ));
    }

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
            brick_beziers.remove(id);
        }
    }

    // Polygon-based adjacency (brick_polygons are correctly filtered by extract_vector_path;
    // brick_beziers may include spurious clip-mask paths that cause false adjacency).
    let scale = if metadata.render_dpi > 0.0 { metadata.render_dpi / 72.0 } else { 1.0 };
    let min_border_px = 5.0;
    let _ = &brick_pymu_rects; // kept for diagnostics / future raster adjacency
    let adj = puzzle::build_adjacency_vector(&bricks, &brick_polygons, 15.0, min_border_px, 2.0);
    let brick_areas = puzzle::compute_polygon_areas(&bricks, &brick_polygons);

    let bricks_layer_arc = Arc::new(bricks_layer_img);

    let brick_rgba: HashMap<String, Arc<image::RgbaImage>> = brick_images_map
        .into_iter()
        .filter(|(id, _)| !covered_ids.contains(id))
        .map(|(id, img)| (id, Arc::new(img)))
        .collect();

    // Save the composite raster. The old `outlines.png` raster is gone:
    // since the bezier port the editor draws pre-gen brick outlines
    // straight from `brick.outline_paths` SVGs, so the standalone PNG
    // was redundant — and rendering it was the slow side of this
    // parallel block. Composite save alone is fast (~0.2 s); no need
    // to spawn-and-join for a single task. Lights and background stay
    // lazy via `ensure_lights_image` / `ensure_background_image`.
    let t0 = std::time::Instant::now();
    let ed4 = extract_dir.clone();
    let bla = bricks_layer_arc.clone();
    tokio::task::spawn_blocking(move || {
        render::save_composite(&bla, &ed4.join("composite.png"));
    })
    .await
    .map_err(|e| e.to_string())?;
    let has_lights = metadata.has_lights_layer;
    eprintln!("[profile] save_composite: {:?}", t0.elapsed());

    // Build brick response data. `polygon` (legacy, brick-local px) stays
    // for any consumers that haven't switched yet; `outline_paths` is the
    // bezier-derived set of SVG path `d=` strings in canvas pixels.
    let clip_x0 = metadata.clip_rect.0;
    let clip_y0 = metadata.clip_rect.1;
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
            let outline_paths: Vec<String> = brick_beziers
                .get(&b.id)
                .map(|paths| {
                    paths
                        .iter()
                        .map(|bp| bp.transform([-clip_x0, -clip_y0], scale).to_svg_d())
                        .collect()
                })
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
                "outline_paths": outline_paths,
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
    {
        let mut store = sessions.lock();
        store.insert(
            key.clone(),
            Session {
                bricks,
                brick_placements: placements,
                brick_polygons,
                brick_beziers,
                brick_areas,
                pieces: Vec::new(),
                metadata: metadata.clone(),
                extract_dir: extract_dir.clone(),
                bricks_layer_img: bricks_layer_arc,
                brick_images: HashMap::new(),
                brick_rgba,
                ai_path: file_path.clone(),
                pdf_offset,
                bleed_pts,
                shifted_clip,
                brick_layer_names,
            },
        );
    }

    eprintln!("[profile] TOTAL load: {:?}", t_total.elapsed());

    // Return file paths — the JS side converts to asset:// URLs via
    // convertFileSrc(). Lights and background are NOT in this response;
    // the frontend invokes `ensure_lights_image` / `ensure_background_image`
    // when it actually needs them and gets a path back then.
    let composite_path = extract_dir.join("composite.png").to_string_lossy().to_string();

    // Reflect the opened file in the window title so the user can see at
    // a glance which AI they're editing — important when they're flipping
    // between several files looking for source-side bugs.
    if let Some(stem) = file_path.file_stem().and_then(|s| s.to_str()) {
        let _ = window.set_title(&format!("House Puzzle Editor — {stem}"));
    }

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
        "composite_url": composite_path,
        "has_lights": has_lights,
    }))
}

// ---------------------------------------------------------------------------
// Lazy OCG layer renders (lights + blueprint background)
// ---------------------------------------------------------------------------
//
// Both layers used to be rendered eagerly inside `load_pdf` even though
// most loads don't enter waves / blueprint mode and don't toggle
// "Show lights". The frontend now invokes one of these commands the
// first time it needs the image; we render-once-and-cache to disk so
// subsequent toggles are free.

async fn ensure_ocg_layer_image(
    sessions: tauri::State<'_, SessionStore>,
    key: &str,
    layer_name: &'static str,
    file_name: &str,
) -> Result<Option<String>, String> {
    let (extract_dir, file_path, dpi, shifted_clip, cw, ch) = {
        let store = sessions.lock();
        let session = store
            .get(key)
            .ok_or_else(|| format!("Session not found: {key}"))?;
        (
            session.extract_dir.clone(),
            session.ai_path.clone(),
            session.metadata.render_dpi,
            session.shifted_clip,
            session.metadata.canvas_width as u32,
            session.metadata.canvas_height as u32,
        )
    };

    let out_path = extract_dir.join(file_name);
    if out_path.exists() {
        return Ok(Some(out_path.to_string_lossy().to_string()));
    }

    let out = out_path.clone();
    let fp = file_path.clone();
    // The bleed-shifted clip is built into `shifted_clip` (the bricks
    // render uses it too) — no separate compose-time pdf_offset needed.
    let ok = tokio::task::spawn_blocking(move || {
        let t0 = std::time::Instant::now();
        let r = render::render_ocg_layer(&fp, layer_name, &out, dpi, shifted_clip, cw, ch, (0, 0));
        eprintln!(
            "[profile] lazy render_ocg_layer({}): {:?} -> {}",
            layer_name,
            t0.elapsed(),
            r
        );
        r
    })
    .await
    .map_err(|e| e.to_string())?;

    if ok {
        Ok(Some(out_path.to_string_lossy().to_string()))
    } else {
        Ok(None)
    }
}

#[tauri::command]
pub async fn ensure_lights_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
) -> Result<Option<String>, String> {
    ensure_ocg_layer_image(sessions, &key, "lights", "lights.png").await
}

#[tauri::command]
pub async fn ensure_background_image(
    sessions: tauri::State<'_, SessionStore>,
    key: String,
) -> Result<Option<String>, String> {
    ensure_ocg_layer_image(sessions, &key, "background", "background.png").await
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
    let (bricks, polygons, beziers, areas, extract_dir, bricks_layer_img, _brick_rgba, scale, clip_x0, clip_y0, brick_placements) = {
        let store = sessions.lock();
        let session = store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?;
        let scale = if session.metadata.render_dpi > 0.0 {
            session.metadata.render_dpi / 72.0
        } else {
            1.0
        };
        (
            session.bricks.clone(),
            session.brick_polygons.clone(),
            session.brick_beziers.clone(),
            session.brick_areas.clone(),
            session.extract_dir.clone(),
            session.bricks_layer_img.clone(),
            session.brick_rgba.clone(),
            scale,
            session.metadata.clip_rect.0,
            session.metadata.clip_rect.1,
            session.brick_placements.clone(),
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
        // Polygon-based adjacency (correctly filtered by extract_vector_path;
        // beziers may include spurious clip-mask paths causing false adjacency).
        // `border_gap` accepted for backward compat with older Elm builds.
        let _ = border_gap;
        let _ = &brick_placements;
        let min_border_px = min_border.unwrap_or(5.0);
        eprintln!(
            "[merge] target_count={target} seed={seed_val} min_border_px={min_border_px} \
             bricks={}",
            bricks.len(),
        );
        let adj = puzzle::build_adjacency_vector(&bricks, &polygons, 15.0, min_border_px, 2.0);
        let pieces = puzzle::merge_bricks(&bricks, target, seed_val, &adj, &areas);
        eprintln!("[merge] result: {} pieces", pieces.len());
        pieces
    };

    // Per-piece bezier outlines. `merge_piece_bezier` walks each piece's
    // brick beziers to a clean closed outline (preserves cubics; drops
    // shared edges between bricks). Convert PyMu → canvas px and emit
    // SVG path `d=` strings for the frontend.
    let piece_outline_paths: HashMap<String, Vec<String>> = computed_pieces
        .par_iter()
        .map(|p| {
            let mut input: Vec<BezierPath> = Vec::new();
            for bid in &p.brick_ids {
                if let Some(paths) = beziers.get(bid) {
                    input.extend(paths.iter().cloned());
                }
            }
            let merged = bezier_merge::merge_piece_bezier(&input);
            let svg: Vec<String> = merged
                .iter()
                .map(|bp| bp.transform([-clip_x0, -clip_y0], scale).to_svg_d())
                .collect();
            (p.id.clone(), svg)
        })
        .collect();

    // Legacy polygon piece outlines kept for the per-piece PNG renderer
    // below (it expects flat polygons). The frontend gets `outline_paths`
    // (bezier-derived) for SVG drawing and `polygon` for legacy click /
    // hit-test consumers.
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
            // piece_polys is now a list of rings per piece (multi-
            // component). Frontend `polygon` field stays single-ring
            // for back-compat — emit the largest ring. `outline_paths`
            // (already multi-component beziers below) is the source of
            // truth for any consumer that needs the full shape.
            let poly = piece_polys
                .get(&p.id)
                .and_then(|rings| rings.iter()
                    .max_by_key(|r| r.len())
                    .map(|pts| pts.iter().map(|pt| json!([pt[0], pt[1]])).collect::<Vec<_>>()))
                .unwrap_or_default();
            let outline_paths: Vec<&String> = piece_outline_paths
                .get(&p.id)
                .map(|v| v.iter().collect())
                .unwrap_or_default();
            let img_path = extract_dir.join(format!("piece_{}.png", p.id)).to_string_lossy().to_string();
            let outline_path = extract_dir.join(format!("piece_outline_{}.png", p.id)).to_string_lossy().to_string();
            json!({
                "id": p.id,
                "x": p.x, "y": p.y,
                "width": p.width, "height": p.height,
                "brick_ids": p.brick_ids,
                "bricks": brick_refs,
                "polygon": poly,
                "outline_paths": outline_paths,
                "img_url": img_path,
                "outline_url": outline_path,
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

/// Build the export ZIP at `export_dpi` and write it straight to a
/// user-picked path via the native save dialog. Returns:
/// - `Some(path)` on a successful save,
/// - `None` if the user cancelled the dialog,
/// - `Err(msg)` for I/O / render failures.
///
/// The dialog opens at the last directory the user saved to (sidecar
/// `last_export_dir.txt` under `app_data_dir`) and pre-fills the
/// filename with the unique `export-<unix_secs>` id stamped onto
/// this export's logs — so the user can match the saved file to
/// the `[export <id>]` lines in stderr.
#[tauri::command]
pub async fn export_data(
    app: tauri::AppHandle,
    sessions: tauri::State<'_, SessionStore>,
    key: String,
    waves: Option<Vec<Value>>,
    groups: Option<Vec<Value>>,
    assets_dpi: Option<f64>,
    pieces_dpi: Option<f64>,
    outline_stroke_px: Option<i32>,
) -> Result<Option<String>, String> {
    let (pieces, bricks, brick_polygons, brick_beziers, metadata, placements, extract_dir, ai_path, brick_layer_names) = {
        let store = sessions.lock();
        let session = store
            .get(&key)
            .ok_or_else(|| format!("Session not found: {key}"))?;
        (
            session.pieces.clone(),
            session.bricks.clone(),
            session.brick_polygons.clone(),
            session.brick_beziers.clone(),
            session.metadata.clone(),
            session.brick_placements.clone(),
            session.extract_dir.clone(),
            session.ai_path.clone(),
            session.brick_layer_names.clone(),
        )
    };

    if pieces.is_empty() {
        return Err("No puzzle computed".to_string());
    }

    let bricks_by_id: HashMap<String, Brick> =
        bricks.iter().map(|b| (b.id.clone(), b.clone())).collect();

    let waves_val = waves.unwrap_or_default();
    let groups_val = groups.unwrap_or_default();

    // Default 300 DPI on both inputs when the frontend doesn't pick
    // one. `assets_dpi` drives the non-piece assets (composite,
    // background, highlight, lights, outlines); `pieces_dpi` drives
    // the per-piece sprites — both can be set independently from
    // the export panel. `outline_stroke_px` drives outlines.png
    // stroke width (in pixels at assets_dpi); default 3.
    let assets_dpi = assets_dpi.unwrap_or(300.0);
    let pieces_dpi = pieces_dpi.unwrap_or(300.0);
    // Outline stroke is clamped to [1, 50] px on the Rust side too —
    // mirrors the on-blur cap the export panel enforces, so a
    // misbehaving / out-of-date frontend (or anyone hitting the
    // command directly) can't request a 1000-px stroke that would
    // saturate outlines.png and take forever to render.
    let outline_stroke_px = outline_stroke_px.unwrap_or(3).clamp(1, 50);
    let loaded_dpi = metadata.render_dpi;

    // A unique stamp for this export, used both as the default
    // save-dialog filename AND prefixed onto the export log lines
    // so the user can correlate a saved file back to its [export
    // <id>] log block. Format: `export-<unix_secs>` — sortable,
    // grep-able, and effectively collision-free at human cadence.
    let export_id = {
        use std::time::{SystemTime, UNIX_EPOCH};
        let secs = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        format!("export-{}", secs)
    };
    eprintln!(
        "[{} ] starting (assets_dpi={}, pieces_dpi={}, outline_stroke={}px)",
        export_id, assets_dpi, pieces_dpi, outline_stroke_px,
    );

    // ALWAYS re-render export assets into a dedicated sub-dir under
    // `extract_dir`. We can't reuse the live-preview cache even when
    // DPIs happen to match because the export bundle includes assets
    // the preview doesn't produce (composite + background) and uses
    // vector-traced piece outlines (preview outlines are pixel-edge
    // traced from the rasterised mask, which stair-steps at high DPI).
    let export_pieces_dir = extract_dir.join(format!(
        "export_a{}_p{}",
        assets_dpi.round() as i64,
        pieces_dpi.round() as i64,
    ));

    // render_export_pieces returns the per-piece rects re-trimmed to
    // the alpha bbox of each piece's actual rendered content (the
    // input bbox is the union of brick bboxes and can overshoot the
    // visible pixels by 2–3×, leaving large transparent overhangs in
    // the sprite). These trimmed rects live in EXPORT-DPI canvas
    // coords — same coord system as composite.png and the per-piece
    // PNGs we just wrote. The ZIP/Unity path divides them back to
    // loaded-DPI (the contract `generate_export_zip` was built around).
    let trimmed_pieces_export_dpi: Vec<hp_core::types::PuzzlePiece> = {
        let pieces_for_render = pieces.clone();
        let bricks_for_render = bricks_by_id.clone();
        let brick_polys_for_render = brick_polygons.clone();
        let brick_beziers_for_render = brick_beziers.clone();
        let meta_for_render = metadata.clone();
        let placements_for_render = placements.clone();
        let ai_path_for_render = ai_path.clone();
        let out_dir_for_render = export_pieces_dir.clone();
        let brick_layer_names_for_render = brick_layer_names.clone();
        tokio::task::spawn_blocking(move || {
            hp_core::render::render_export_pieces(
                &ai_path_for_render,
                &placements_for_render,
                &meta_for_render,
                &pieces_for_render,
                &bricks_for_render,
                &brick_polys_for_render,
                &brick_beziers_for_render,
                &brick_layer_names_for_render,
                assets_dpi,
                pieces_dpi,
                outline_stroke_px,
                &out_dir_for_render,
            )
        })
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())?
    };

    // Down-scale the trimmed pieces (in pieces_dpi coords) to
    // loaded-DPI for the ZIP/Unity path. `generate_export_zip`
    // re-applies `scale = pieces_dpi / loaded_dpi` internally when
    // building house_data.json (so the sprite centre lands at the
    // right Unity world position), so this path is sensitive to the
    // input being loaded-DPI.
    let trimmed_pieces_loaded_dpi: Vec<hp_core::types::PuzzlePiece> = {
        let inv_scale = if pieces_dpi > 0.0 {
            loaded_dpi / pieces_dpi
        } else {
            1.0
        };
        trimmed_pieces_export_dpi
            .iter()
            .map(|p| hp_core::types::PuzzlePiece {
                id: p.id.clone(),
                brick_ids: p.brick_ids.clone(),
                x: ((p.x as f64) * inv_scale).round() as i32,
                y: ((p.y as f64) * inv_scale).round() as i32,
                width: ((p.width as f64) * inv_scale).round().max(1.0) as i32,
                height: ((p.height as f64) * inv_scale).round().max(1.0) as i32,
            })
            .collect()
    };

    let bricks_for_encode = bricks_by_id.clone();
    let metadata_for_encode = metadata.clone();
    let extract_dir_for_encode = export_pieces_dir.clone();
    let waves_for_encode = waves_val.clone();
    let groups_for_encode = groups_val.clone();
    let pieces_for_zip = trimmed_pieces_loaded_dpi;
    let file_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, String> {
        hp_core::export::generate_export_zip(
            &pieces_for_zip,
            &bricks_for_encode,
            &extract_dir_for_encode,
            metadata_for_encode.canvas_width,
            metadata_for_encode.canvas_height,
            metadata_for_encode.screen_frame_height_px,
            loaded_dpi,
            pieces_dpi,
            &waves_for_encode,
            &groups_for_encode,
        )
        .map_err(|e| e.to_string())
    })
    .await
    .map_err(|e| e.to_string())??;

    // Show the native save dialog. Default directory comes from the
    // last-export sidecar; filename is `<picture-basename>.zip`
    // (the AI file's stem, verbatim — the picture can sit
    // anywhere, not just in `in/`, so we don't strip any leading
    // characters). Falls back to `export-<id>.zip` if the basename
    // can't be derived. User cancellation returns `Ok(None)` so
    // the frontend can flip the export button back without showing
    // an error.
    let stored_path = last_export_dir_path(&app);
    let default_dir = stored_path.as_deref().and_then(read_last_dir);
    let default_name = ai_path
        .file_stem()
        .and_then(|s| s.to_str())
        .filter(|s| !s.is_empty())
        .map(|stem| format!("{}.zip", stem))
        .unwrap_or_else(|| format!("{}.zip", export_id));

    let app_for_dialog = app.clone();
    let dialog_path = tokio::task::spawn_blocking(move || {
        use tauri_plugin_dialog::DialogExt;
        let mut builder = app_for_dialog
            .dialog()
            .file()
            .add_filter("ZIP archive", &["zip"])
            .set_file_name(&default_name);
        if let Some(dir) = default_dir {
            builder = builder.set_directory(dir);
        }
        builder.blocking_save_file()
    })
    .await
    .map_err(|e| e.to_string())?;

    let save_path = match dialog_path {
        None => return Ok(None),
        Some(fp) => fp.into_path().map_err(|e| e.to_string())?,
    };

    std::fs::write(&save_path, &file_bytes)
        .map_err(|e| format!("writing {}: {e}", save_path.display()))?;

    if let Some(stored) = stored_path.as_deref() {
        write_last_dir(stored, &save_path);
    }

    Ok(Some(save_path.to_string_lossy().into_owned()))
}

// ---------------------------------------------------------------------------
// Screenshot — uses native webview snapshot (no OS permission needed on macOS)
// ---------------------------------------------------------------------------

#[tauri::command]
pub async fn save_screenshot(
    window: tauri::WebviewWindow,
    path: String,
) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos_screenshot::take_snapshot(&window, &path).await?;
    }

    #[cfg(not(target_os = "macos"))]
    {
        // On other platforms, save_screenshot is called with base64 data from JS
        let _ = window;
        let _ = path;
    }

    Ok(())
}

#[cfg(target_os = "macos")]
mod macos_screenshot {
    use std::sync::{Arc, Mutex};

    pub async fn take_snapshot(
        window: &tauri::WebviewWindow,
        path: &str,
    ) -> Result<(), String> {
        let path = path.to_string();
        let result: Arc<Mutex<Option<Result<(), String>>>> = Arc::new(Mutex::new(None));
        let result_clone = result.clone();

        window.with_webview(move |webview| {
            use objc2::rc::Retained;
            use objc2::runtime::{AnyClass, AnyObject};
            use objc2::{msg_send, msg_send_id};

            unsafe {
                let wk_webview = webview.inner() as *const AnyObject as *mut AnyObject;
                if wk_webview.is_null() {
                    *result_clone.lock().unwrap() = Some(Err("null webview".into()));
                    return;
                }

                // Create a nil snapshot configuration (captures full webview)
                let config: *const AnyObject = std::ptr::null();

                // Build the completion handler block
                let path_for_block = path.clone();
                let result_for_block = result_clone.clone();

                let block = block2::RcBlock::new(move |ns_image: *mut AnyObject, error: *mut AnyObject| {
                    if ns_image.is_null() {
                        let err_msg = if !error.is_null() {
                            let desc: Retained<AnyObject> = msg_send_id![error, localizedDescription];
                            let utf8: *const u8 = msg_send![&desc, UTF8String];
                            if !utf8.is_null() {
                                std::ffi::CStr::from_ptr(utf8 as *const _).to_string_lossy().to_string()
                            } else {
                                "unknown error".to_string()
                            }
                        } else {
                            "null image, no error".to_string()
                        };
                        *result_for_block.lock().unwrap() = Some(Err(err_msg));
                        return;
                    }

                    // NSImage → TIFF → NSBitmapImageRep → PNG
                    let tiff_data: Retained<AnyObject> = msg_send_id![ns_image, TIFFRepresentation];
                    let bitmap_class = AnyClass::get(c"NSBitmapImageRep").unwrap();
                    let bitmap_rep: Retained<AnyObject> = msg_send_id![
                        msg_send_id![bitmap_class, alloc], initWithData: &*tiff_data
                    ];

                    let empty_dict_class = AnyClass::get(c"NSDictionary").unwrap();
                    let empty_dict: Retained<AnyObject> = msg_send_id![empty_dict_class, dictionary];
                    let png_data: Retained<AnyObject> = msg_send_id![
                        &bitmap_rep, representationUsingType: 4usize, properties: &*empty_dict
                    ];

                    let png_bytes: *const u8 = msg_send![&png_data, bytes];
                    let png_len: usize = msg_send![&png_data, length];
                    let png_slice = std::slice::from_raw_parts(png_bytes, png_len);

                    match std::fs::write(&path_for_block, png_slice) {
                        Ok(_) => {
                            eprintln!("[screenshot] saved: {} ({} bytes)", path_for_block, png_len);
                            *result_for_block.lock().unwrap() = Some(Ok(()));
                        }
                        Err(e) => {
                            *result_for_block.lock().unwrap() = Some(Err(e.to_string()));
                        }
                    }
                });

                // Call [wkWebView takeSnapshotWithConfiguration:nil completionHandler:block]
                let _: () = msg_send![
                    wk_webview,
                    takeSnapshotWithConfiguration: config,
                    completionHandler: &*block
                ];
            }
        }).map_err(|e| format!("with_webview failed: {e}"))?;

        // Wait for the async completion handler
        for _ in 0..100 {
            if result.lock().unwrap().is_some() {
                return result.lock().unwrap().take().unwrap();
            }
            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
        }

        Err("screenshot timeout".into())
    }
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
