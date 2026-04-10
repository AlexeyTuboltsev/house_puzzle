use axum::{
    Router,
    extract::Path,
    http::{StatusCode, header},
    response::{Html, IntoResponse, Response},
    routing::get,
    Json,
};
use rust_embed::Embed;
use serde_json::json;
use std::path::PathBuf;

/// Embedded template (elm.html).
#[derive(Embed)]
#[folder = "../../templates/"]
struct Templates;

/// Embedded static files (elm.js, etc.).
#[derive(Embed)]
#[folder = "../../static/"]
struct StaticFiles;

pub fn build_router() -> Router {
    Router::new()
        .route("/", get(index))
        .route("/api/list_pdfs", get(api_list_pdfs))
        .route("/static/{*path}", get(static_file))
}

/// Serve the main page — elm.html with cache-busting version injected.
async fn index() -> Html<String> {
    match Templates::get("elm.html") {
        Some(content) => {
            let html = std::str::from_utf8(content.data.as_ref())
                .unwrap_or("")
                .to_string();
            // Inject elm.js mtime for cache busting
            let elm_js_version = std::fs::metadata("static/elm.js")
                .and_then(|m| m.modified())
                .map(|t| t.duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs())
                .unwrap_or(0);
            let html = html.replace("{{ elm_version }}", &elm_js_version.to_string());
            Html(html)
        }
        None => Html("<h1>elm.html not found in embedded assets</h1>".to_string()),
    }
}

/// Serve static files (elm.js, editor.js, etc.).
async fn static_file(Path(path): Path<String>) -> Response {
    // Strip query params (cache busting ?v=...)
    let clean_path = path.split('?').next().unwrap_or(&path);

    match StaticFiles::get(clean_path) {
        Some(content) => {
            let mime = mime_guess::from_path(clean_path)
                .first_or_octet_stream()
                .to_string();
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, mime)],
                content.data.to_vec(),
            )
                .into_response()
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// List AI files in the in/ directory.
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
                files.push(json!({
                    "name": name,
                    "path": path.to_string_lossy(),
                    "size_mb": size_mb,
                }));
            }
        }
    }

    Json(json!({ "files": files }))
}
