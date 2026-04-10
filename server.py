#!/usr/bin/env python3
"""
House Puzzle Editor — Flask backend.

Serves the interactive editor UI and exposes API endpoints for
AI file parsing, brick merging, and puzzle generation.
"""

import argparse
import base64
import io
import json
import os
import sys
import tempfile
import threading
import webbrowser
from pathlib import Path

from flask import Flask, jsonify, render_template, request, send_file, send_from_directory
from PIL import Image, ImageDraw

from puzzle_engine import (
    Brick,
    merge_bricks,
    pieces_to_json,
    build_adjacency,
    compute_borders_and_areas,
    compute_piece_bbox,
)

# Support PyInstaller bundled paths
if getattr(sys, 'frozen', False):
    _base_dir = Path(sys._MEIPASS)
else:
    _base_dir = Path(__file__).parent

app = Flask(
    __name__,
    template_folder=str(_base_dir / "templates"),
    static_folder=str(_base_dir / "static"),
)

# In-memory sessions keyed by session ID
_sessions: dict[str, dict] = {}


def _new_session() -> dict:
    """Create a fresh session state dict."""
    return {
        "house": None,          # HouseData
        "bricks": [],           # list[Brick]
        "bricks_by_id": {},     # dict[str, Brick]
        "pieces": [],           # list[PuzzlePiece]
        "tif_path": None,
        "extracted_dir": None,  # temp dir with extracted PNGs
        "border_pixels": {},    # dict[str, set[(x,y)]] per brick
        "brick_areas": {},      # dict[str, int] pixel area per brick
        "brick_polygons": {},   # dict[str, list[[x,y]]] traced outlines per brick
        "canvas_height": None,  # display canvas height (pixels)
        "idx_to_uuid": {},      # dict[int, str] original brick index → UUID
    }


def _get_session(key: str) -> dict:
    """Get session by key or 404."""
    s = _sessions.get(key)
    if s is None:
        from flask import abort
        abort(404, f"Session {key} not found")
    return s


# Compatibility: _state points to the most recently loaded session.
# During migration, old code using _state still works.
# New keyed routes use _get_session(key) directly.
_state = _new_session()
_current_session_key: str = ""

EXTRACT_DIR = Path(tempfile.gettempdir()) / "house_puzzle_extract"
# Presets must be writable → use the directory the exe/script lives in
if getattr(sys, 'frozen', False):
    _app_dir = Path(sys.executable).parent
else:
    _app_dir = Path(__file__).parent
PRESETS_DIR = _app_dir / "presets"
PARAM_KEYS = ["target_count", "min_border", "seed"]

# Version
_version_file = _base_dir / "VERSION"
APP_VERSION = _version_file.read_text().strip() if _version_file.exists() else "dev"


@app.route("/")
def index():
    elm_js = _base_dir / "static" / "elm.js"
    elm_version = int(elm_js.stat().st_mtime) if elm_js.exists() else 0
    return render_template("elm.html", elm_version=elm_version)


# --- Preset API ---

def _safe_name(name: str) -> str:
    """Sanitize preset name for use as filename."""
    import re
    return re.sub(r'[^\w\s\-]', '', name).strip()


@app.route("/api/presets")
def api_list_presets():
    PRESETS_DIR.mkdir(parents=True, exist_ok=True)
    names = sorted(p.stem for p in PRESETS_DIR.glob("*.json"))
    return jsonify({"presets": names})


@app.route("/api/presets/<name>")
def api_get_preset(name):
    path = PRESETS_DIR / f"{_safe_name(name)}.json"
    if not path.exists():
        return jsonify({"error": "Preset not found"}), 404
    with open(path) as f:
        return jsonify(json.load(f))


@app.route("/api/presets", methods=["POST"])
def api_save_preset():
    data = request.get_json(force=True)
    name = _safe_name(data.get("name", ""))
    if not name:
        return jsonify({"error": "Name required"}), 400
    PRESETS_DIR.mkdir(parents=True, exist_ok=True)
    params = {k: data[k] for k in PARAM_KEYS if k in data}
    path = PRESETS_DIR / f"{name}.json"
    with open(path, "w") as f:
        json.dump(params, f, indent=2)
    return jsonify({"ok": True, "name": name})


@app.route("/api/presets/<name>", methods=["PUT"])
def api_rename_preset(name):
    data = request.get_json(force=True)
    new_name = _safe_name(data.get("new_name", ""))
    if not new_name:
        return jsonify({"error": "Name required"}), 400
    old_path = PRESETS_DIR / f"{_safe_name(name)}.json"
    new_path = PRESETS_DIR / f"{new_name}.json"
    if not old_path.exists():
        return jsonify({"error": "Preset not found"}), 404
    if new_path.exists() and new_path != old_path:
        return jsonify({"error": "A preset with that name already exists"}), 409
    old_path.rename(new_path)
    return jsonify({"ok": True, "name": new_name})


@app.route("/api/presets/<name>", methods=["DELETE"])
def api_delete_preset(name):
    path = PRESETS_DIR / f"{_safe_name(name)}.json"
    if path.exists():
        path.unlink()
    return jsonify({"ok": True})


@app.route("/api/list_pdfs", methods=["GET"])
def api_list_pdfs():
    """List available PDF files in the in/ directory."""
    in_dir = _app_dir / "in"
    if not in_dir.exists():
        return jsonify({"files": []})

    files = []
    for f in sorted(in_dir.iterdir()):
        if f.suffix.lower() in (".pdf", ".ai"):
            files.append({
                "name": f.name,
                "path": str(f),
                "size_mb": round(f.stat().st_size / (1024 * 1024), 1),
            })
    return jsonify({"files": files})


@app.route("/api/upload_file", methods=["POST"])
def api_upload_file():
    """Accept a file upload, save to in/ directory, return its path."""
    f = request.files.get("file")
    if not f or not f.filename:
        return jsonify({"error": "no file"}), 400
    in_dir = _app_dir / "in"
    in_dir.mkdir(exist_ok=True)
    dest = in_dir / Path(f.filename).name
    f.save(dest)
    return jsonify({"path": str(dest)})


def _vectorize_bricks(bricks, extract_dir: Path, idx_to_uuid: dict | None = None) -> dict:
    """Read brick polygon cache (pre-populated from PDF vector layer).

    No raster fallback — shapes must come from the PDF outline layer.
    Missing bricks = unmatched vector paths; caller reports them to the user.
    Returns dict[brick_id -> list[[x,y]]] keyed by UUID string IDs.
    """
    cache_path = extract_dir / "brick_polygons.json"
    if not cache_path.exists():
        return {}
    with open(cache_path) as f:
        cached = json.load(f)
    if idx_to_uuid:
        return {idx_to_uuid.get(int(k), k): v for k, v in cached.items()}
    return {int(k): v for k, v in cached.items()}


@app.route("/api/s/<key>/load", methods=["POST"])
@app.route("/api/load_pdf", methods=["POST"])
def api_load_pdf(key=None):
    """Load and parse an AI file, extract brick metadata."""
    global _state, _current_session_key
    data = request.get_json()
    file_path = data.get("path", "")

    # Create or reuse session
    if key is None:
        import uuid as _uuid
        key = str(_uuid.uuid4())[:8]
    session = _new_session()
    _sessions[key] = session
    _state = session  # compatibility: _state points to current session
    _current_session_key = key

    if not file_path or not Path(file_path).exists():
        return jsonify({"error": f"File not found: {file_path}"}), 404

    canvas_height: int = int(data.get("canvas_height", 900))

    try:
        from ai_parser import parse_ai
        house = parse_ai(file_path, canvas_height=canvas_height)
    except Exception as e:
        return jsonify({"error": str(e)}), 500

    # Convert to engine Brick objects, deduplicating bricks with identical bbox
    # (the AI file can contain two layers at the exact same position — keep the
    # first occurrence and drop subsequent ones so the puzzle engine never sees
    # phantom "ghost" bricks that inflate piece brick-counts)
    seen_bbox: set[tuple] = set()
    deduped_layers: list = []
    for bl in house.bricks:
        bbox_key = (bl.x, bl.y, bl.width, bl.height)
        if bbox_key not in seen_bbox:
            seen_bbox.add(bbox_key)
            deduped_layers.append(bl)
    house.bricks = deduped_layers

    bricks = [
        Brick(id=f"b{bl.index}", x=bl.x, y=bl.y,
              width=bl.width, height=bl.height, brick_type=bl.layer_type)
        for bl in house.bricks
    ]
    # Map original index → UUID for PNG filename mapping
    _idx_to_uuid = {bl.index: bricks[i].id for i, bl in enumerate(house.bricks)}
    bricks_by_id = {b.id: b for b in bricks}

    # Store state
    _state["house"] = house
    _state["bricks"] = bricks
    _state["bricks_by_id"] = bricks_by_id
    _state["tif_path"] = file_path
    _state["pieces"] = []
    _state["canvas_height"] = canvas_height
    _state["idx_to_uuid"] = _idx_to_uuid

    # Extract all layers as PNGs (always regenerate, no caching)
    import shutil
    extract_dir = EXTRACT_DIR / key
    if extract_dir.exists():
        shutil.rmtree(str(extract_dir))
    extract_dir.mkdir(parents=True, exist_ok=True)
    _state["extracted_dir"] = extract_dir

    from ai_parser import (
        extract_ai_layers_batch,
        compose_ai_bricks_png,
        render_ai_outlines_png,
        render_ai_lights_png,
        render_ai_background_png,
        extract_ai_vector_polygons,
    )
    _ai_kw = dict(dpi=house.render_dpi, clip_rect=house.clip_rect)

    compose_ai_bricks_png(file_path, house.bricks, str(extract_dir / "composite.png"), **_ai_kw, pdf_offset_px=house.pdf_offset_px)
    extract_ai_layers_batch(file_path, house.bricks, str(extract_dir), **_ai_kw, prefix="brick", pdf_offset_px=house.pdf_offset_px)
    render_ai_outlines_png(file_path, str(extract_dir / "outlines.png"), **_ai_kw, stroke_width=3.2)
    _layer_kw = dict(**_ai_kw,
                      pdf_offset_px=house.pdf_offset_px,
                      canvas_size=(house.canvas_width, house.canvas_height))
    render_ai_lights_png(file_path, str(extract_dir / "lights.png"), **_layer_kw)
    render_ai_background_png(file_path, str(extract_dir / "background.png"), **_layer_kw)

    vec_polys = extract_ai_vector_polygons(file_path, house.bricks, **_ai_kw)
    if vec_polys:
        with open(extract_dir / "brick_polygons.json", "w") as f:
            json.dump({str(k): v for k, v in vec_polys.items()}, f)

    # Rename brick PNGs from index-based to UUID-based names
    for bl_idx, brick_uuid in _idx_to_uuid.items():
        old_path = extract_dir / f"brick_{bl_idx:03d}.png"
        new_path = extract_dir / f"brick_{brick_uuid}.png"
        if old_path.exists():
            old_path.rename(new_path)

    # Compute border pixels and areas (single PNG read per brick)
    bp, ba = compute_borders_and_areas(bricks, str(extract_dir))
    _state["border_pixels"] = bp
    _state["brick_areas"] = ba

    # Filter out bricks that are covered by another brick (pixel overlap check).
    # Decoration elements sometimes appear as separate layers on top of wall bricks.
    covered_ids = _find_covered_bricks(bricks, str(extract_dir))
    if covered_ids:
        print(f"[load] Removing {len(covered_ids)} covered bricks: {sorted(covered_ids)}", flush=True)
        bricks = [b for b in bricks if b.id not in covered_ids]
        # Map UUID back to original index for filtering house.bricks
        _uuid_to_idx = {v: k for k, v in _idx_to_uuid.items()}
        covered_indices = {_uuid_to_idx[uid] for uid in covered_ids if uid in _uuid_to_idx}
        house.bricks = [bl for bl in house.bricks if bl.index not in covered_indices]
        bricks_by_id = {b.id: b for b in bricks}
        _state["house"] = house
        _state["bricks"] = bricks
        _state["bricks_by_id"] = bricks_by_id
        # Recompute borders without covered bricks
        bp, ba = compute_borders_and_areas(bricks, str(extract_dir))
        _state["border_pixels"] = bp
        _state["brick_areas"] = ba

    # Vectorize outlines — reads cache pre-populated from PDF vector layer
    polygons = _vectorize_bricks(bricks, extract_dir, _idx_to_uuid)
    _state["brick_polygons"] = polygons

    # Build adjacency for visualization
    adj = build_adjacency(bricks, border_pixels=bp)

    id_to_name = {bl.index: bl.name for bl in house.bricks}
    missing = [b for b in bricks if not polygons.get(b.id)]
    if missing:
        labels = ", ".join(f"#{b.id} '{id_to_name.get(b.id, '?')}'" for b in missing)
        house.warnings.append(f"NO_POLYGON: {labels}")

    brick_data = []
    for b in bricks:
        brick_data.append({
            "id": b.id,
            "x": b.x,
            "y": b.y,
            "width": b.width,
            "height": b.height,
            "type": b.brick_type,
            "neighbors": list(adj.get(b.id, set())),
            "polygon": polygons.get(b.id, []),
        })

    pfx = f"/api/s/{key}"
    return jsonify({
        "key": key,
        "canvas": {"width": house.canvas_width, "height": house.canvas_height},
        "total_layers": house.total_layers,
        "num_bricks": len(bricks),
        "bricks": brick_data,
        "has_composite": (extract_dir / "composite.png").exists(),
        "has_base": house.base is not None,
        "render_dpi": round(house.render_dpi, 2),
        "warnings": house.warnings,
        "houseUnitsHigh": round(house.canvas_height / house.screen_frame_height_px * 15.5, 4) if house.screen_frame_height_px > 0 else 15.5,
        "composite_url": f"{pfx}/composite.png",
        "outlines_url": f"{pfx}/outlines.png",
        "lights_url": f"{pfx}/lights.png" if (extract_dir / "lights.png").exists() else None,
        "blueprint_bg_url": f"{pfx}/background.png" if (extract_dir / "background.png").exists() else None,
    })


@app.route("/api/s/<key>/composite.png")
@app.route("/api/composite.png")
def api_composite(key=None):
    """Serve the composite image."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    comp_path = s["extracted_dir"] / "composite.png"
    if not comp_path.exists():
        return "Composite not extracted", 404
    resp = send_file(str(comp_path), mimetype="image/png")
    resp.headers["Cache-Control"] = "no-store"
    return resp



@app.route("/api/s/<key>/base.png")
@app.route("/api/base.png")
def api_base(key=None):
    """Serve the base layer image."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    base_path = s["extracted_dir"] / "base.png"
    if not base_path.exists():
        return "Base not extracted", 404
    resp = send_file(str(base_path), mimetype="image/png")
    resp.headers["Cache-Control"] = "no-store"
    return resp


@app.route("/api/s/<key>/brick/<brick_id>.png")
@app.route("/api/brick/<brick_id>.png")
def api_brick_png(brick_id, key=None):
    """Serve an individual brick layer as PNG."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    brick_path = s["extracted_dir"] / f"brick_{brick_id}.png"
    if not brick_path.exists():
        return f"Brick {brick_id} not found", 404
    return send_file(str(brick_path), mimetype="image/png")


@app.route("/api/s/<key>/piece/<piece_id>.png")
@app.route("/api/piece/<piece_id>.png")
def api_piece_png(piece_id, key=None):
    """Serve a composited piece PNG."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    p = s["extracted_dir"] / f"piece_{piece_id}.png"
    if not p.exists():
        return "Not found", 404
    return send_file(str(p), mimetype="image/png")


@app.route("/api/s/<key>/piece_outline/<piece_id>.png")
@app.route("/api/piece_outline/<piece_id>.png")
def api_piece_outline_png(piece_id, key=None):
    """Serve a piece outline PNG."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    p = s["extracted_dir"] / f"piece_outline_{piece_id}.png"
    if not p.exists():
        return "Not found", 404
    return send_file(str(p), mimetype="image/png")


@app.route("/api/s/<key>/outlines.png")
@app.route("/api/outlines.png")
def api_outlines_png(key=None):
    """Serve the full-house vector outline layer PNG."""
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No PDF loaded", 404
    p = s["extracted_dir"] / "outlines.png"
    if not p.exists():
        return "Not found", 404
    resp = send_file(str(p), mimetype="image/png")
    resp.headers["Cache-Control"] = "no-store"
    return resp


@app.route("/api/s/<key>/lights.png")
@app.route("/api/lights.png")
def api_lights_png(key=None):
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No file loaded", 404
    p = s["extracted_dir"] / "lights.png"
    if not p.exists():
        return "Not found", 404
    resp = send_file(str(p), mimetype="image/png")
    resp.headers["Cache-Control"] = "no-store"
    return resp


@app.route("/api/s/<key>/background.png")
@app.route("/api/background.png")
def api_background_png(key=None):
    s = _get_session(key) if key else _state
    if not s["extracted_dir"]:
        return "No file loaded", 404
    p = s["extracted_dir"] / "background.png"
    if not p.exists():
        return "Not found", 404
    resp = send_file(str(p), mimetype="image/png")
    resp.headers["Cache-Control"] = "no-store"
    return resp


def _compute_piece_polygons(pieces, bricks_by_id, brick_polygons):
    """Compute merged polygon outline for each piece using Shapely vector union.

    CANONICAL OUTLINE RULE: piece outlines are ALWAYS derived from brick vector
    polygons. Uses Shapely unary_union on the tessellated brick polygon points so
    arch/curve detail from the original AI paths is fully preserved.

    Adjacent bricks share edges (zero-area intersection), so each polygon is
    expanded by BRIDGE px before union and then contracted back. This bridges
    shared edges without meaningfully distorting the outline shape.

    Bricks missing a vector polygon fall back to their bounding box rectangle.

    Returns dict[piece_id -> list of [x, y] in canvas coords].
    """
    from shapely.geometry import Polygon as ShapelyPolygon
    from shapely.ops import unary_union

    BRIDGE = 3  # px expand/contract to bridge shared edges between adjacent bricks

    result = {}
    for piece in pieces:
        bricks = [bricks_by_id[bid] for bid in piece.brick_ids if bid in bricks_by_id]
        if not bricks:
            result[piece.id] = []
            continue

        polys = []
        for b in bricks:
            poly = brick_polygons.get(b.id, [])
            if poly and len(poly) >= 3:
                # brick-local → canvas coords, then expand to bridge shared edges
                canvas_pts = [(x + b.x, y + b.y) for x, y in poly]
                try:
                    p = ShapelyPolygon(canvas_pts)
                    if not p.is_valid:
                        p = p.buffer(0)
                    if not p.is_empty:
                        polys.append(p.buffer(BRIDGE))
                        continue
                except Exception:
                    pass
            # Fallback: bounding box
            polys.append(ShapelyPolygon([
                (b.x, b.y), (b.x + b.width, b.y),
                (b.x + b.width, b.y + b.height), (b.x, b.y + b.height),
            ]).buffer(BRIDGE))

        if not polys:
            result[piece.id] = []
            continue

        try:
            union = unary_union(polys).buffer(-BRIDGE)
        except Exception:
            result[piece.id] = []
            continue

        # Take the largest polygon if the result is a MultiPolygon
        if union.geom_type == "MultiPolygon":
            union = max(union.geoms, key=lambda g: g.area)

        if union.is_empty or union.geom_type != "Polygon":
            result[piece.id] = []
            continue

        coords = list(union.exterior.coords)
        result[piece.id] = [[x, y] for x, y in coords]

    return result


def _pieces_json_with_polygons(pieces, bricks_by_id, brick_polygons):
    """Build JSON response for a list of pieces, computing polygon outlines."""
    pieces_json = pieces_to_json(pieces, bricks_by_id)
    piece_polys = _compute_piece_polygons(pieces, bricks_by_id, brick_polygons)
    for p in pieces_json:
        p["polygon"] = piece_polys.get(p["id"], [])
        p["img_url"] = f"/api/piece/{p['id']}.png"
        p["outline_url"] = f"/api/piece_outline/{p['id']}.png"
    return pieces_json


@app.route("/api/s/<key>/merge", methods=["POST"])
@app.route("/api/merge", methods=["POST"])
def api_merge(key=None):
    """Merge bricks into puzzle pieces.

    Two modes:
    - Normal (no 'pieces' key): run merge algorithm with target_count/seed/min_border.
    - Recompute (with 'pieces' key): skip merge, recompute bboxes + polygon outlines
      for the supplied piece definitions [{id, brick_ids}, ...].  Used after manual
      edits to restore polygon outlines via the canonical brick-vector pipeline.
    """
    s = _get_session(key) if key else _state
    data = request.get_json()

    if not s["bricks_by_id"]:
        return jsonify({"error": "No PDF loaded"}), 400

    # ── Recompute mode: pre-defined pieces supplied by caller ─────────────────
    if "pieces" in data:
        class _P:
            def __init__(self, pid, brick_ids, bricks_by_id):
                self.id = pid
                self.brick_ids = brick_ids
                bricks = [bricks_by_id[bid] for bid in brick_ids if bid in bricks_by_id]
                if bricks:
                    self.x = min(b.x for b in bricks)
                    self.y = min(b.y for b in bricks)
                    self.width = max(b.x + b.width for b in bricks) - self.x
                    self.height = max(b.y + b.height for b in bricks) - self.y
                else:
                    self.x = self.y = self.width = self.height = 0

        pieces = [_P(p["id"], p.get("brick_ids", []), s["bricks_by_id"])
                  for p in data["pieces"]]
        extract_dir = s.get("extracted_dir")
        if extract_dir and s.get("tif_path") and Path(s["tif_path"]).suffix.lower() in (".pdf", ".ai"):
            for piece in pieces:
                _assemble_piece_png(piece, s["bricks_by_id"], extract_dir)
                _render_piece_outline_png(piece, s["bricks_by_id"], extract_dir)
        pieces_json = _pieces_json_with_polygons(
            pieces, s["bricks_by_id"], s.get("brick_polygons", {})
        )
        return jsonify({"num_pieces": len(pieces), "pieces": pieces_json})

    # ── Normal mode: run merge algorithm ─────────────────────────────────────
    if not s["bricks"]:
        return jsonify({"error": "No PDF loaded"}), 400

    target_count = data.get("target_count")
    seed = data.get("seed", 42)
    min_border = data.get("min_border", 5)
    border_gap = data.get("border_gap", 2)

    result = merge_bricks(
        s["bricks"],
        target_piece_count=target_count,
        seed=seed,
        min_border=min_border,
        border_gap=border_gap,
        border_pixels=s.get("border_pixels"),
        brick_areas=s.get("brick_areas"),
    )

    s["pieces"] = result.pieces

    extract_dir = s.get("extracted_dir")
    if extract_dir and s.get("tif_path") and Path(s["tif_path"]).suffix.lower() in (".pdf", ".ai"):
        for piece in result.pieces:
            _assemble_piece_png(piece, s["bricks_by_id"], extract_dir)
            _render_piece_outline_png(piece, s["bricks_by_id"], extract_dir)

    pieces_json = _pieces_json_with_polygons(
        result.pieces, s["bricks_by_id"], s.get("brick_polygons", {})
    )

    return jsonify({
        "num_pieces": len(result.pieces),
        "pieces": pieces_json,
    })


@app.route("/api/s/<key>/update_piece", methods=["POST"])
@app.route("/api/update_piece", methods=["POST"])
def api_update_piece(key=None):
    """Move a brick between pieces (manual correction)."""
    s = _get_session(key) if key else _state
    data = request.get_json()
    brick_id = data.get("brick_id")
    from_piece_id = data.get("from_piece")
    to_piece_id = data.get("to_piece")

    if s["pieces"] is None:
        return jsonify({"error": "No pieces computed"}), 400

    pieces = s["pieces"]

    # Find source and target pieces
    src = next((p for p in pieces if p.id == from_piece_id), None)
    dst = next((p for p in pieces if p.id == to_piece_id), None)

    if not src or brick_id not in src.brick_ids:
        return jsonify({"error": "Brick not found in source piece"}), 400

    if not dst:
        return jsonify({"error": "Target piece not found"}), 400

    # Move brick
    src.brick_ids.remove(brick_id)
    dst.brick_ids.append(brick_id)

    # Remove empty pieces
    s["pieces"] = [p for p in pieces if p.brick_ids]

    # Recompute bboxes
    for p in s["pieces"]:
        p.x, p.y, p.width, p.height = compute_piece_bbox(
            p.brick_ids, s["bricks_by_id"]
        )

    pieces_json = pieces_to_json(s["pieces"], s["bricks_by_id"])
    return jsonify({"num_pieces": len(s["pieces"]), "pieces": pieces_json})


@app.route("/api/s/<key>/blueprint", methods=["POST"])
@app.route("/api/blueprint", methods=["POST"])
def api_blueprint(key=None):
    """Generate blueprint overlay with 4px white lines on piece boundaries."""
    s = _get_session(key) if key else _state
    if not s["pieces"]:
        return jsonify({"error": "No pieces computed"}), 400

    house = s["house"]
    w, h = house.canvas_width, house.canvas_height

    # Create transparent image
    blueprint = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(blueprint)

    line_width = 4
    line_color = (255, 255, 255, 255)

    for piece in s["pieces"]:
        bricks = [s["bricks_by_id"][bid] for bid in piece.brick_ids]
        if len(bricks) <= 1:
            # Single brick = the whole piece is the boundary
            b = bricks[0]
            draw.rectangle(
                [b.x, b.y, b.x + b.width, b.y + b.height],
                outline=line_color, width=line_width,
            )
        else:
            # Draw outline of the piece bounding box
            # For accurate outlines, draw the outer boundary of the merged shape
            # Simple approach: draw each brick's edges that are on the piece boundary
            _draw_piece_outline(draw, bricks, line_color, line_width)

    # Save to buffer
    buf = io.BytesIO()
    blueprint.save(buf, format="PNG")
    buf.seek(0)

    return send_file(buf, mimetype="image/png", download_name="blueprint.png")


def _draw_piece_outline(draw, bricks, color, width):
    """Draw the outer boundary of a group of bricks.

    For each brick edge, only draw it if it's not shared with another brick
    in the same piece (i.e., it's an external edge).
    """
    # Collect all edges with their brick ownership
    # An edge is defined by two endpoints
    h_edges = {}  # (y, x_start, x_end) -> count
    v_edges = {}  # (x, y_start, y_end) -> count

    for b in bricks:
        # Top edge
        key = (b.y, b.x, b.right)
        h_edges[key] = h_edges.get(key, 0) + 1
        # Bottom edge
        key = (b.bottom, b.x, b.right)
        h_edges[key] = h_edges.get(key, 0) + 1
        # Left edge
        key = (b.x, b.y, b.bottom)
        v_edges[key] = v_edges.get(key, 0) + 1
        # Right edge
        key = (b.right, b.y, b.bottom)
        v_edges[key] = v_edges.get(key, 0) + 1

    half = width // 2

    # Draw edges that appear only once (external edges)
    for (y, x1, x2), count in h_edges.items():
        if count == 1:
            draw.line([(x1, y), (x2, y)], fill=color, width=width)

    for (x, y1, y2), count in v_edges.items():
        if count == 1:
            draw.line([(x, y1), (x, y2)], fill=color, width=width)


@app.route("/api/s/<key>/export", methods=["POST"])
@app.route("/api/export", methods=["POST"])
def api_export(key=None):
    """Export puzzle pieces as PNG sprites + JSON manifest."""
    s = _get_session(key) if key else _state
    if not s["pieces"] or not s["tif_path"]:
        return jsonify({"error": "No puzzle computed"}), 400

    import zipfile

    data = request.get_json(force=True) or {}
    house = s["house"]
    tif_path = s["tif_path"]
    pieces = s["pieces"]

    waves_data = data.get("waves", [])

    # Re-render bricks at export resolution into a temp dir.
    # The user can specify a canvas height different from the display resolution.
    export_canvas_height = int(data.get("export_canvas_height", house.canvas_height))
    export_scale = export_canvas_height / house.canvas_height
    export_canvas_w = round(house.canvas_width * export_scale)

    # Resize sprites to target PPU=50 to match existing houses.
    # Use screen-frame height to pin house height in Unity units (15.5 units = screen frame).
    TARGET_PPU = 50
    if house.screen_frame_height_px > 0:
        scale = TARGET_PPU * 15.5 / (house.screen_frame_height_px * export_scale)
    else:
        TARGET_WORLD_WIDTH = 11.4
        target_canvas_w = TARGET_PPU * TARGET_WORLD_WIDTH  # 570
        scale = target_canvas_w / export_canvas_w

    from ai_parser import extract_ai_layers_batch, compose_ai_bricks_png
    export_dir = Path(tempfile.mkdtemp())
    export_dpi = house.render_dpi * export_canvas_height / house.canvas_height
    extract_ai_layers_batch(
        tif_path, house.bricks, str(export_dir),
        dpi=export_dpi, clip_rect=house.clip_rect, prefix="brick"
    )
    # Rename export brick PNGs from index-based to UUID-based names
    for bl_idx, brick_uuid in s.get("idx_to_uuid", {}).items():
        old_p = export_dir / f"brick_{bl_idx:03d}.png"
        new_p = export_dir / f"brick_{brick_uuid}.png"
        if old_p.exists():
            old_p.rename(new_p)
    export_comp_path = export_dir / "composite.png"
    compose_ai_bricks_png(tif_path, house.bricks, str(export_comp_path),
                          dpi=export_dpi, clip_rect=house.clip_rect,
                          pdf_offset_px=house.pdf_offset_px)

    zip_buffer = io.BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        # piece_images_orig: display-resolution images (for flat/blue/mask — uses display coords)
        # piece_images: export-resolution images (written to ZIP pieces/)
        piece_images_orig = {}
        piece_images = {}
        for piece in pieces:
            # Display-resolution piece image (for flat/blue compositing)
            disp_img = Image.new("RGBA", (piece.width, piece.height), (0, 0, 0, 0))
            # Export-resolution piece image (for the ZIP piece PNG)
            exp_w = max(1, round(piece.width * export_scale))
            exp_h = max(1, round(piece.height * export_scale))
            exp_img = Image.new("RGBA", (exp_w, exp_h), (0, 0, 0, 0))

            for bid in piece.brick_ids:
                # Cached display-resolution brick (full canvas PNG)
                disp_p = s["extracted_dir"] / f"brick_{bid}.png"
                if disp_p.exists():
                    disp_br = Image.open(str(disp_p)).convert("RGBA")
                    disp_img.paste(disp_br, (-round(piece.x), -round(piece.y)), disp_br)
                # Export-resolution brick (full canvas PNG)
                exp_p = export_dir / f"brick_{bid}.png"
                if exp_p.exists():
                    exp_br = Image.open(str(exp_p)).convert("RGBA")
                    exp_img.paste(exp_br,
                                  (-round(piece.x * export_scale),
                                   -round(piece.y * export_scale)),
                                  exp_br)

            piece_images_orig[piece.id] = disp_img

            # Write export-resolution piece PNG to ZIP (then scale to Unity PPU)
            new_w = max(1, round(exp_img.width * scale))
            new_h = max(1, round(exp_img.height * scale))
            scaled_img = exp_img.resize((new_w, new_h), Image.LANCZOS)
            piece_images[piece.id] = scaled_img

            fname = f"piece_{piece.id}.png"
            buf = io.BytesIO()
            scaled_img.save(buf, format="PNG")
            zf.writestr(f"pieces/{fname}", buf.getvalue())

        # Load composite source image
        comp_path = export_dir / "composite.png"
        comp_src = None
        if comp_path.exists():
            comp_src = Image.open(str(comp_path)).convert("RGBA")

        def _write_scaled(zf, name, img):
            """Resize to target scale and write to ZIP."""
            sw = max(1, round(img.width * scale))
            sh = max(1, round(img.height * scale))
            img = img.resize((sw, sh), Image.LANCZOS)
            buf = io.BytesIO()
            img.save(buf, format="PNG")
            zf.writestr(name, buf.getvalue())

        # Build house_data.json first — we need groundOffset for overlay alignment
        placement = data.get("placement", {})
        groups_data = data.get("groups", [])
        from unity_export import build_house_data
        house_data = build_house_data(
            pieces=pieces,
            bricks_by_id=s["bricks_by_id"],
            canvas_width=house.canvas_width,
            canvas_height=house.canvas_height,
            waves=waves_data,
            ppu=TARGET_PPU,
            scale=scale,
            location=placement.get("location", "Rome"),
            position_in_location=placement.get("position", 0),
            house_name=placement.get("houseName", "NewHouse"),
            spacing=float(placement.get("spacing", 12.0)),
            piece_images=piece_images,
            groups=groups_data,
        )

        # blue.png — AI background layer exported as "blue"
        bg_src_path = s["extracted_dir"] / "background.png"
        if bg_src_path.exists():
            bg_src = Image.open(str(bg_src_path)).convert("RGBA")
            _write_scaled(zf, "blue.png", bg_src)

        # lights.png — AI lights layer overlay
        lights_src_path = s["extracted_dir"] / "lights.png"
        if lights_src_path.exists():
            lights_src = Image.open(str(lights_src_path)).convert("RGBA")
            _write_scaled(zf, "lights.png", lights_src)

        # scheme.png — rasterize the vetted SVG outline paths from the frontend.
        frontend_outlines = data.get("outlines", [])
        scheme = _rasterize_outlines(
            frontend_outlines, house.canvas_width, house.canvas_height
        )
        _write_scaled(zf, "scheme.png", scheme)

        # light.png — outer house outline from background vector polygon
        from ai_parser import extract_ai_background_polygon
        bg_polygon = extract_ai_background_polygon(
            s["tif_path"], dpi=house.render_dpi, clip_rect=house.clip_rect
        )
        light = _rasterize_outline_boundary(
            bg_polygon, house.canvas_width, house.canvas_height
        )
        _write_scaled(zf, "light.png", light)

        # flat.png — composite clipped to house shape (FullHouse sprite)
        if comp_src:
            flat = _generate_flat(pieces, piece_images_orig, comp_src)
            _write_scaled(zf, "flat.png", flat)

        zf.writestr("house_data.json", json.dumps(house_data, indent=2))

    # Clean up temp export dir for PDFs
    if export_dir is not None:
        import shutil
        shutil.rmtree(str(export_dir), ignore_errors=True)

    # Save ZIP to in/ directory
    placement = data.get("placement", {})
    house_name = placement.get("houseName", "NewHouse")
    out_path = _app_dir / "in" / f"{house_name}_export.zip"
    with open(out_path, "wb") as f:
        f.write(zip_buffer.getvalue())

    # Also save to /tmp for Unity import shortcut
    import shutil
    shutil.copy2(str(out_path), "/tmp/house_puzzle_export.zip")

    return jsonify({"ok": True, "path": str(out_path)})


def _rasterize_outline_boundary(bg_polygon, canvas_w, canvas_h):
    """Generate house outline from the background vector polygon.

    Draws the background polygon as a 4px white stroke on transparent canvas.
    """
    img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    if not bg_polygon or len(bg_polygon) < 3:
        return img
    draw = ImageDraw.Draw(img)
    coords = [(p[0], p[1]) for p in bg_polygon]
    # Close the polygon
    if coords[0] != coords[-1]:
        coords.append(coords[0])
    for i in range(len(coords) - 1):
        draw.line([coords[i], coords[i + 1]], fill=(255, 255, 255, 255), width=4)
    return img


def _rasterize_outlines(outlines, canvas_w, canvas_h):
    """Rasterize frontend SVG outline paths into a scheme image.

    Args:
        outlines: list of {pieceId, points: [[x,y], ...]} from the frontend
        canvas_w, canvas_h: original canvas dimensions
    Returns:
        PIL Image with white outline strokes on transparent background.
    """
    img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    stroke_width = 4

    for outline in outlines:
        pts = outline.get("points", [])
        if len(pts) < 3:
            continue
        coords = [(p[0], p[1]) for p in pts]
        # Draw polygon outline (closed path) with round joins
        draw.polygon(coords, outline=(255, 255, 255, 255))
        # Draw thicker lines to match SVG stroke-width=4
        for i in range(len(coords)):
            draw.line(
                [coords[i], coords[(i + 1) % len(coords)]],
                fill=(255, 255, 255, 255),
                width=stroke_width,
                joint="curve",
            )

    return img


def _generate_blueprint(house, pieces):
    """Generate blueprint image."""
    w, h = house.canvas_width, house.canvas_height
    blueprint = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(blueprint)

    for piece in pieces:
        bricks = [_state["bricks_by_id"][bid] for bid in piece.brick_ids]
        if len(bricks) <= 1:
            b = bricks[0]
            draw.rectangle(
                [b.x, b.y, b.x + b.width, b.y + b.height],
                outline=(255, 255, 255, 255), width=4,
            )
        else:
            _draw_piece_outline(
                draw, bricks, (255, 255, 255, 255), 4
            )

    return blueprint


def _trace_piece_outlines(piece_images):
    """Trace vectorized contour polygons for each piece using alpha contour tracing.

    Returns list of (piece_id, contours) where contours is a list of simplified
    polygon loops in piece-local pixel coords.
    """
    from unity_export import trace_alpha_contours, douglas_peucker_closed

    result = []
    for pid, img in piece_images.items():
        contours = trace_alpha_contours(img)
        simplified = []
        for contour in contours:
            s = douglas_peucker_closed(contour, 2.0)
            if len(s) >= 3:
                simplified.append(s)
        result.append((pid, simplified))
    return result


def _find_covered_bricks(bricks, extract_dir):
    """Find bricks whose opaque pixels are mostly covered by another brick.

    For each pair of bricks with overlapping bboxes, load their PNGs and check
    pixel-level alpha overlap.  If >=80% of the smaller brick's opaque pixels
    are also opaque in the larger brick, the smaller one is "covered" and should
    be removed.
    """
    import numpy as np
    covered = set()
    # Precompute bbox tuples for quick overlap test
    brick_bboxes = {}
    for b in bricks:
        brick_bboxes[b.id] = (b.x, b.y, b.x + b.width, b.y + b.height)

    # Cache alpha arrays (only the brick region, not full canvas)
    alpha_cache = {}

    def get_alpha(bid):
        if bid not in alpha_cache:
            p = Path(extract_dir) / f"brick_{bid}.png"
            if p.exists():
                img = Image.open(str(p)).convert("RGBA")
                alpha_cache[bid] = np.array(img.split()[3])
            else:
                alpha_cache[bid] = None
        return alpha_cache[bid]

    brick_list = list(bricks)
    for i, a in enumerate(brick_list):
        if a.id in covered:
            continue
        ax0, ay0, ax1, ay1 = brick_bboxes[a.id]
        for j, b in enumerate(brick_list):
            if j <= i or b.id in covered:
                continue
            bx0, by0, bx1, by1 = brick_bboxes[b.id]
            # Quick bbox overlap check
            if ax0 >= bx1 or bx0 >= ax1 or ay0 >= by1 or by0 >= ay1:
                continue
            # Determine which is smaller
            area_a = a.width * a.height
            area_b = b.width * b.height
            if area_a <= area_b:
                small, big = a, b
            else:
                small, big = b, a
            # Load alphas (full canvas PNGs)
            alpha_s = get_alpha(small.id)
            alpha_b = get_alpha(big.id)
            if alpha_s is None or alpha_b is None:
                continue
            # Both are canvas-sized, compare directly
            opaque_s = alpha_s > 30
            opaque_b = alpha_b > 30
            overlap_pixels = int(np.sum(opaque_s & opaque_b))
            total_s = int(np.sum(opaque_s))
            if total_s > 0 and overlap_pixels / total_s >= 0.8:
                covered.add(small.id)
    return covered


def _build_house_mask(piece_images, pieces, canvas_w, canvas_h):
    """Build a house silhouette mask by compositing all piece alpha channels."""
    import numpy as np

    mask = np.zeros((canvas_h, canvas_w), dtype=np.uint8)
    for piece in pieces:
        img = piece_images.get(piece.id)
        if img is None:
            continue
        alpha = np.array(img.split()[3])
        x, y = round(piece.x), round(piece.y)
        h, w = alpha.shape
        # Clip to canvas bounds
        sx = max(0, -x)
        sy = max(0, -y)
        ex = min(w, canvas_w - x)
        ey = min(h, canvas_h - y)
        if sx >= ex or sy >= ey:
            continue
        region = alpha[sy:ey, sx:ex]
        mask[y + sy:y + ey, x + sx:x + ex] = np.maximum(
            mask[y + sy:y + ey, x + sx:x + ex], region
        )
    return mask


def _generate_scheme(pieces, piece_images):
    """Generate scheme.png — piece boundary lines for wave fill shader.

    Existing houses use very low-alpha anti-aliased lines (alpha ~1) because
    the SchemeMaterial shader reads alpha as a mask pattern.
    We draw white lines at full alpha matching existing houses.
    """
    canvas_w, canvas_h = _state["house"].canvas_width, _state["house"].canvas_height
    img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)
    outlines = _trace_piece_outlines(piece_images)

    for pid, contours in outlines:
        piece = next(p for p in pieces if p.id == pid)
        ox, oy = piece.x, piece.y
        for contour in contours:
            # Convert piece-local coords to canvas coords
            pts = [(p[0] + ox, p[1] + oy) for p in contour]
            if len(pts) >= 3:
                draw.polygon(pts, outline=(255, 255, 255, 255))
                for i in range(len(pts)):
                    draw.line(
                        [pts[i], pts[(i + 1) % len(pts)]],
                        fill=(255, 255, 255, 255), width=6
                    )

    return img


def _generate_light(pieces, piece_images):
    """Generate light.png — white outline around house perimeter only.

    Traces the combined house silhouette (all pieces composited) to get
    only the outer boundary. No internal piece lines.
    Matches existing houses' Outline sprite (e.g. Rome/10/light.png).
    """
    from unity_export import trace_alpha_contours, douglas_peucker_closed

    canvas_w, canvas_h = _state["house"].canvas_width, _state["house"].canvas_height

    # Build full house composite, close inter-piece gaps, then trace outer boundary
    import numpy as np
    from PIL import ImageFilter
    house_composite = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    for piece in pieces:
        pimg = piece_images.get(piece.id)
        if pimg is None:
            continue
        house_composite.paste(pimg, (round(piece.x), round(piece.y)), pimg)

    # Morphological close on alpha to eliminate inter-piece gaps
    alpha = np.array(house_composite.split()[3])
    mask_binary = Image.fromarray((alpha > 30).astype(np.uint8) * 255)
    mask_closed = mask_binary.filter(ImageFilter.MaxFilter(7))
    mask_closed = mask_closed.filter(ImageFilter.MinFilter(7))
    # Rebuild composite with closed alpha for contour tracing
    solid = Image.new("RGBA", (canvas_w, canvas_h), (255, 255, 255, 255))
    solid.putalpha(mask_closed)

    contours = trace_alpha_contours(solid)
    img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(img)

    # Only keep contours with significant perimeter (skip tiny specks)
    for contour in contours:
        simplified = douglas_peucker_closed(contour, 2.0)
        if len(simplified) < 10:
            continue
        for i in range(len(simplified)):
            draw.line(
                [simplified[i], simplified[(i + 1) % len(simplified)]],
                fill=(255, 255, 255, 255), width=8
            )

    return img


def _generate_blue(pieces, piece_images):
    """Generate blue.png — solid blue fill in the shape of the house silhouette.

    Uses traced SVG contours to create a filled blue shape matching the
    actual house outline (not rectangles).
    Matches existing houses' Background sprite (e.g. Rome/10/blue.png).
    """
    from unity_export import trace_alpha_contours, douglas_peucker_closed

    canvas_w, canvas_h = _state["house"].canvas_width, _state["house"].canvas_height

    # Build full house composite to trace outer boundary
    house_composite = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    for piece in pieces:
        pimg = piece_images.get(piece.id)
        if pimg is None:
            continue
        house_composite.paste(pimg, (round(piece.x), round(piece.y)), pimg)

    # Use alpha mask from composite, dilate to close inter-piece gaps
    import numpy as np
    from PIL import ImageFilter
    mask = np.array(house_composite.split()[3])
    mask_binary = Image.fromarray((mask > 30).astype(np.uint8) * 255)
    # Dilate (MaxFilter) then erode (MinFilter) = morphological close
    mask_closed = mask_binary.filter(ImageFilter.MaxFilter(5))
    mask_closed = mask_closed.filter(ImageFilter.MinFilter(5))

    blue = Image.new("RGBA", (canvas_w, canvas_h), (51, 85, 204, 255))
    blue.putalpha(mask_closed)
    return blue


def _generate_flat(pieces, piece_images, comp_img):
    """Generate flat.png — composite image clipped to house silhouette.

    Matches existing houses' FullHouse sprite (e.g. Rome/10/flat.png).
    """
    import numpy as np

    canvas_w, canvas_h = _state["house"].canvas_width, _state["house"].canvas_height

    # Build house mask from piece alphas
    house_composite = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
    for piece in pieces:
        pimg = piece_images.get(piece.id)
        if pimg is None:
            continue
        house_composite.paste(pimg, (round(piece.x), round(piece.y)), pimg)

    mask = np.array(house_composite.split()[3])
    mask_binary = Image.fromarray((mask > 30).astype(np.uint8) * 255)
    # Morphological close to fill inter-piece gaps
    from PIL import ImageFilter
    mask_closed = mask_binary.filter(ImageFilter.MaxFilter(5))
    mask_closed = mask_closed.filter(ImageFilter.MinFilter(5))
    mask = np.array(mask_closed)

    if comp_img.size != (canvas_w, canvas_h):
        comp_img = comp_img.resize((canvas_w, canvas_h), Image.LANCZOS)
    result = comp_img.copy().convert("RGBA")
    a = np.array(result.split()[3])
    combined = np.minimum(a, mask)
    result.putalpha(Image.fromarray(combined))
    return result


def _assemble_piece_png(piece, bricks_by_id, extract_dir: Path) -> Path:
    """Composite brick PNGs into a piece PNG at display resolution.

    Brick PNGs are full-canvas; paste each at (-piece.x, -piece.y).
    Returns path to saved PNG.
    """
    piece_w = max(1, int(piece.width))
    piece_h = max(1, int(piece.height))
    out = Image.new("RGBA", (piece_w, piece_h), (0, 0, 0, 0))
    for bid in piece.brick_ids:
        brick_path = extract_dir / f"brick_{bid}.png"
        if brick_path.exists():
            br = Image.open(str(brick_path)).convert("RGBA")
            out.paste(br, (-round(piece.x), -round(piece.y)), br)
    out_path = extract_dir / f"piece_{piece.id}.png"
    out.save(str(out_path), "PNG")
    return out_path


def _render_piece_outline_png(piece, bricks_by_id, extract_dir: Path, stroke_width: int = 4) -> Path:
    """Build outer-boundary outline PNG for a piece.

    1. Load each brick's PNG alpha channel (full canvas), paste at (-piece.x, -piece.y) into piece-sized mask.
    2. MinFilter to erode → subtract → boundary ring.
    3. Return RGBA with white at boundary pixels, transparent elsewhere.
    Returns path to saved PNG.
    """
    import numpy as np
    from PIL import ImageFilter

    piece_w = max(1, int(piece.width))
    piece_h = max(1, int(piece.height))
    mask = Image.new("L", (piece_w, piece_h), 0)
    for bid in piece.brick_ids:
        brick_path = extract_dir / f"brick_{bid}.png"
        if brick_path.exists():
            br = Image.open(str(brick_path)).convert("RGBA")
            alpha = br.split()[3]
            mask.paste(alpha, (-round(piece.x), -round(piece.y)))
    eroded = mask.filter(ImageFilter.MinFilter(stroke_width * 2 + 1))
    outline_arr = np.clip(
        np.array(mask).astype(int) - np.array(eroded).astype(int), 0, 255
    ).astype(np.uint8)
    out = Image.new("RGBA", (piece_w, piece_h), (255, 255, 255, 255))
    out.putalpha(Image.fromarray(outline_arr))
    out_path = extract_dir / f"piece_outline_{piece.id}.png"
    out.save(str(out_path), "PNG")
    return out_path


@app.route("/debug/trace")
def debug_trace_page():
    """Debug page for comparing JS vs Python brick outline tracing."""
    return render_template("debug_trace.html")


@app.route("/api/debug/trace/<int:brick_id>")
def api_debug_trace(brick_id):
    """Return Python-traced polygon for a brick PNG."""
    if not _state["extracted_dir"]:
        return jsonify({"error": "No PDF loaded"}), 400
    brick_path = _state["extracted_dir"] / f"brick_{brick_id:03d}.png"
    if not brick_path.exists():
        return jsonify({"error": f"Brick {brick_id} not found"}), 404
    from tracer import trace_brick_png
    pts = trace_brick_png(str(brick_path))
    return jsonify({"brick_id": brick_id, "polygon": pts})


@app.route("/api/debug/bricks")
def api_debug_bricks():
    """Return list of brick IDs and metadata for the debug trace page."""
    if not _state["bricks"]:
        return jsonify({"error": "No PDF loaded"}), 400
    bricks = [
        {"id": b.id, "x": b.x, "y": b.y, "w": b.width, "h": b.height}
        for b in _state["bricks"]
    ]
    return jsonify({"bricks": bricks})


@app.route("/debug/brick/<int:brick_idx>")
def debug_brick_page(brick_idx):
    """Simple HTML page showing a single brick rendered in isolation."""
    house = _state.get("house")
    total = len(house.bricks) if house else 0
    prev_idx = brick_idx - 1 if brick_idx > 0 else 0
    next_idx = brick_idx + 1 if brick_idx < total - 1 else total - 1
    return f"""<!DOCTYPE html><html><head><meta charset=utf-8>
<title>Brick {brick_idx}</title>
<style>body{{background:#333;color:#eee;font:14px monospace;text-align:center}}
img{{border:1px solid #888;background:repeating-conic-gradient(#555 0% 25%,#444 0% 50%) 0 0/20px 20px;max-width:600px}}</style>
</head><body>
<h2>Brick #{brick_idx} (0-based index)</h2>
<p><a href="/debug/brick/{prev_idx}" style="color:#adf">← prev</a> &nbsp;
<a href="/debug/brick/{next_idx}" style="color:#adf">next →</a></p>
<img src="/api/debug/brick_render/{brick_idx}" alt="brick {brick_idx}"><br>
<p>Total bricks: {total}</p>
</body></html>"""


@app.route("/api/debug/brick_render/<int:brick_idx>")
def api_debug_brick_render(brick_idx):
    """Render a single brick tightly cropped, returned as PNG.

    Uses loaded state if available, otherwise loads NY1.ai directly.
    """
    from ai_parser import (
        _decompress_ai_data, _render_bricks_ocg_png,
        _mask_crop_to_polygon,
        _extract_raster, _extract_vector_path, _get_ai_transform,
        _render_vector_brick_pil,
        _parse_layer_tree, parse_ai,
    )
    import io as _io
    from PIL import Image as PilImage

    house = _state.get("house")
    if house is None or not house.source_path.endswith(".ai"):
        ai_path = "/app/in/NY1.ai"
        try:
            house = parse_ai(ai_path)
        except Exception as e:
            return jsonify({"error": f"Could not load AI file: {e}"}), 500

    if brick_idx < 0 or brick_idx >= len(house.bricks):
        return jsonify({"error": f"brick_idx {brick_idx} out of range (0–{len(house.bricks)-1})"}), 404

    bl = house.bricks[brick_idx]
    scale = house.render_dpi / 72.0
    clip = house.clip_rect

    raw_bytes, text = _decompress_ai_data(house.source_path)
    roots = _parse_layer_tree(text)
    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    child_by_name = {c.name: c for c in bricks_node.children} if bricks_node else {}
    child = child_by_name.get(bl.name)

    if bl.layer_type == "vector_brick":
        if child is not None:
            offset_x, y_base = _get_ai_transform(house.source_path, text, roots)
            crop = _render_vector_brick_pil(child, text, offset_x, y_base, scale, clip, bl, raw_bytes)
        else:
            crop = PilImage.new("RGBA", (max(1, bl.width), max(1, bl.height)), (0, 0, 0, 0))
    elif bl.layer_type == "mixed_brick":
        bricks_only = _render_bricks_ocg_png(house.source_path, house.render_dpi, clip)
        crop = bricks_only.crop((bl.x, bl.y, bl.x + bl.width, bl.y + bl.height))
        if child is not None:
            offset_x, y_base = _get_ai_transform(house.source_path, text, roots)
            poly = _extract_vector_path(child, text, offset_x, y_base)
            scale_poly = [
                [(p[0] - clip[0]) * scale, (p[1] - clip[1]) * scale]
                for p in poly
            ]
            if len(scale_poly) >= 3:
                crop = _mask_crop_to_polygon(crop, scale_poly, bl.x, bl.y)
    else:
        crop = PilImage.new("RGBA", (max(1, bl.width), max(1, bl.height)), (0, 0, 0, 0))
        if child:
            img, _ = _extract_raster(child, raw_bytes, text)
            if img:
                crop = img.resize((max(1, bl.width), max(1, bl.height)), PilImage.LANCZOS)

    buf = _io.BytesIO()
    crop.save(buf, "PNG")
    buf.seek(0)
    return send_file(buf, mimetype="image/png")


if __name__ == "__main__":
    try:
        parser = argparse.ArgumentParser(description="House Puzzle Editor")
        parser.add_argument("--port", type=int, default=5050)
        parser.add_argument("--host", default="0.0.0.0")
        parser.add_argument("--no-browser", action="store_true", help="Don't auto-open browser")
        args = parser.parse_args()

        url = f"http://localhost:{args.port}"
        print(f"House Puzzle Editor v{APP_VERSION}")
        print(f"Starting at {url}")

        is_frozen = getattr(sys, 'frozen', False)

        if not args.no_browser:
            threading.Timer(1.0, lambda: webbrowser.open(url)).start()

        app.run(host=args.host, port=args.port, debug=False,
                threaded=True)
    except Exception as e:
        print(f"\nERROR: {e}")
        import traceback
        traceback.print_exc()
        if getattr(sys, 'frozen', False):
            input("\nPress Enter to close...")
