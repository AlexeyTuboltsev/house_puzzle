#!/usr/bin/env python3
"""
House Puzzle Editor — Flask backend.

Serves the interactive editor UI and exposes API endpoints for
TIF parsing, brick merging, and blueprint generation.
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

from tif_parser import parse_tif, extract_brick_png, extract_layers_batch
from unity_export import build_house_data
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

# In-memory state
_state = {
    "house": None,          # HouseData
    "bricks": [],           # list[Brick]
    "bricks_by_id": {},     # dict[int, Brick]
    "pieces": [],           # list[PuzzlePiece]
    "tif_path": None,
    "extracted_dir": None,  # temp dir with extracted PNGs
    "border_pixels": {},    # dict[int, set[(x,y)]] per brick
    "brick_areas": {},      # dict[int, int] pixel area per brick
}

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
    return render_template("index.html", version=APP_VERSION)


@app.route("/manage")
def manage():
    return render_template("manage.html")


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


@app.route("/api/list_tifs", methods=["GET"])
def api_list_tifs():
    """List available TIF files in the in/ directory."""
    in_dir = Path("in")
    if not in_dir.exists():
        return jsonify({"tifs": []})

    tifs = []
    for f in sorted(in_dir.iterdir()):
        if f.suffix.lower() in (".tif", ".tiff"):
            tifs.append({
                "name": f.name,
                "path": str(f),
                "size_mb": round(f.stat().st_size / (1024 * 1024), 1),
            })
    return jsonify({"tifs": tifs})


@app.route("/api/upload_tif", methods=["POST"])
def api_upload_tif():
    """Upload a TIF file and save it to the in/ directory."""
    if "file" not in request.files:
        return jsonify({"error": "No file provided"}), 400

    f = request.files["file"]
    if not f.filename:
        return jsonify({"error": "No file selected"}), 400

    ext = Path(f.filename).suffix.lower()
    if ext not in (".tif", ".tiff"):
        return jsonify({"error": "Only .tif/.tiff files are supported"}), 400

    in_dir = Path("in")
    in_dir.mkdir(parents=True, exist_ok=True)
    dest = in_dir / f.filename
    f.save(str(dest))

    return jsonify({
        "path": str(dest),
        "name": f.filename,
        "size_mb": round(dest.stat().st_size / (1024 * 1024), 1),
    })


@app.route("/api/load_tif", methods=["POST"])
def api_load_tif():
    """Load and parse a TIF file, extract brick metadata."""
    data = request.get_json()
    tif_path = data.get("path", "")

    if not tif_path or not Path(tif_path).exists():
        return jsonify({"error": f"File not found: {tif_path}"}), 404

    try:
        house = parse_tif(tif_path)
    except Exception as e:
        return jsonify({"error": str(e)}), 500

    # Convert to engine Brick objects
    bricks = []
    for bl in house.bricks:
        bricks.append(Brick(
            id=bl.index,
            x=bl.x,
            y=bl.y,
            width=bl.width,
            height=bl.height,
            brick_type=bl.layer_type,
        ))

    bricks_by_id = {b.id: b for b in bricks}

    # Store state
    _state["house"] = house
    _state["bricks"] = bricks
    _state["bricks_by_id"] = bricks_by_id
    _state["tif_path"] = tif_path
    _state["pieces"] = []

    # Extract all layers as PNGs
    extract_dir = EXTRACT_DIR / Path(tif_path).stem.replace(" ", "_")
    extract_dir.mkdir(parents=True, exist_ok=True)
    _state["extracted_dir"] = extract_dir

    comp_path = extract_dir / "composite.png"
    if house.composite and not comp_path.exists():
        extract_brick_png(tif_path, house.composite.index, str(comp_path))

    # Extract all brick PNGs (parallel)
    brick_indices = [bl.index for bl in house.bricks]
    extract_layers_batch(tif_path, brick_indices, str(extract_dir), prefix="brick")

    # Also extract base layer
    if house.base:
        base_path = extract_dir / "base.png"
        if not base_path.exists():
            extract_brick_png(tif_path, house.base.index, str(base_path))

    # Compute border pixels and areas in one pass (single PNG read per brick)
    bp, ba = compute_borders_and_areas(bricks, str(extract_dir))
    _state["border_pixels"] = bp
    _state["brick_areas"] = ba

    # Build adjacency for visualization (using pixel shapes)
    adj = build_adjacency(bricks, border_pixels=bp)

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
        })

    return jsonify({
        "canvas": {"width": house.canvas_width, "height": house.canvas_height},
        "total_layers": house.total_layers,
        "num_bricks": len(bricks),
        "bricks": brick_data,
        "has_composite": house.composite is not None,
        "has_base": house.base is not None,
    })


@app.route("/api/composite.png")
def api_composite():
    """Serve the composite image."""
    if not _state["extracted_dir"]:
        return "No TIF loaded", 404
    comp_path = _state["extracted_dir"] / "composite.png"
    if not comp_path.exists():
        return "Composite not extracted", 404
    return send_file(str(comp_path), mimetype="image/png")


@app.route("/api/base.png")
def api_base():
    """Serve the base layer image."""
    if not _state["extracted_dir"]:
        return "No TIF loaded", 404
    base_path = _state["extracted_dir"] / "base.png"
    if not base_path.exists():
        return "Base not extracted", 404
    return send_file(str(base_path), mimetype="image/png")


@app.route("/api/brick/<int:brick_id>.png")
def api_brick_png(brick_id):
    """Serve an individual brick layer as PNG."""
    if not _state["extracted_dir"]:
        return "No TIF loaded", 404
    brick_path = _state["extracted_dir"] / f"brick_{brick_id:03d}.png"
    if not brick_path.exists():
        return f"Brick {brick_id} not found", 404
    return send_file(str(brick_path), mimetype="image/png")


@app.route("/api/merge", methods=["POST"])
def api_merge():
    """Merge bricks into puzzle pieces."""
    data = request.get_json()

    if not _state["bricks"]:
        return jsonify({"error": "No TIF loaded"}), 400

    target_count = data.get("target_count")
    seed = data.get("seed", 42)
    min_border = data.get("min_border", 5)
    border_gap = data.get("border_gap", 2)

    result = merge_bricks(
        _state["bricks"],
        target_piece_count=target_count,
        seed=seed,
        min_border=min_border,
        border_gap=border_gap,
        border_pixels=_state.get("border_pixels"),
        brick_areas=_state.get("brick_areas"),
    )

    _state["pieces"] = result.pieces

    pieces_json = pieces_to_json(result.pieces, _state["bricks_by_id"])

    return jsonify({
        "num_pieces": len(result.pieces),
        "pieces": pieces_json,
    })


@app.route("/api/update_piece", methods=["POST"])
def api_update_piece():
    """Move a brick between pieces (manual correction)."""
    data = request.get_json()
    brick_id = data.get("brick_id")
    from_piece_id = data.get("from_piece")
    to_piece_id = data.get("to_piece")

    if _state["pieces"] is None:
        return jsonify({"error": "No pieces computed"}), 400

    pieces = _state["pieces"]

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
    _state["pieces"] = [p for p in pieces if p.brick_ids]

    # Recompute bboxes
    for p in _state["pieces"]:
        p.x, p.y, p.width, p.height = compute_piece_bbox(
            p.brick_ids, _state["bricks_by_id"]
        )

    pieces_json = pieces_to_json(_state["pieces"], _state["bricks_by_id"])
    return jsonify({"num_pieces": len(_state["pieces"]), "pieces": pieces_json})


@app.route("/api/blueprint", methods=["POST"])
def api_blueprint():
    """Generate blueprint overlay with 4px white lines on piece boundaries."""
    if not _state["pieces"]:
        return jsonify({"error": "No pieces computed"}), 400

    house = _state["house"]
    w, h = house.canvas_width, house.canvas_height

    # Create transparent image
    blueprint = Image.new("RGBA", (w, h), (0, 0, 0, 0))
    draw = ImageDraw.Draw(blueprint)

    line_width = 4
    line_color = (255, 255, 255, 255)

    for piece in _state["pieces"]:
        bricks = [_state["bricks_by_id"][bid] for bid in piece.brick_ids]
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


@app.route("/api/export", methods=["POST"])
def api_export():
    """Export puzzle pieces as PNG sprites + JSON manifest."""
    if not _state["pieces"] or not _state["tif_path"]:
        return jsonify({"error": "No puzzle computed"}), 400

    import zipfile

    data = request.get_json(force=True) or {}
    house = _state["house"]
    tif_path = _state["tif_path"]
    pieces = _state["pieces"]

    waves_data = data.get("waves", [])

    # Resize sprites to target PPU=50 to match existing houses.
    # Existing houses: ~600px wide canvas at PPU=50 → ~12 Unity units wide.
    # Slightly smaller (570px) so tray piece sizes match existing houses.
    TARGET_PPU = 50
    TARGET_WORLD_WIDTH = 11.4
    target_canvas_w = TARGET_PPU * TARGET_WORLD_WIDTH  # 570
    scale = target_canvas_w / house.canvas_width

    zip_buffer = io.BytesIO()
    with zipfile.ZipFile(zip_buffer, "w", zipfile.ZIP_DEFLATED) as zf:
        # Build piece images at ORIGINAL resolution (for asset generation),
        # then scale for piece PNGs in the ZIP
        piece_images_orig = {}
        piece_images = {}
        for piece in pieces:
            piece_img = Image.new("RGBA", (piece.width, piece.height), (0, 0, 0, 0))

            for bid in piece.brick_ids:
                b = _state["bricks_by_id"][bid]
                tmp_brick = str(Path(tempfile.gettempdir()) / f"_brick_{b.id}.png")
                extract_brick_png(tif_path, b.id, tmp_brick)
                brick_img = Image.open(tmp_brick).convert("RGBA")
                rel_x = b.x - piece.x
                rel_y = b.y - piece.y
                piece_img.paste(brick_img, (rel_x, rel_y), brick_img)

            piece_images_orig[piece.id] = piece_img

            # Resize to target scale for piece PNGs
            new_w = max(1, round(piece_img.width * scale))
            new_h = max(1, round(piece_img.height * scale))
            scaled_img = piece_img.resize((new_w, new_h), Image.LANCZOS)
            piece_images[piece.id] = scaled_img

            fname = f"piece_{piece.id:03d}.png"
            buf = io.BytesIO()
            scaled_img.save(buf, format="PNG")
            zf.writestr(f"pieces/{fname}", buf.getvalue())

        # Load composite source image
        comp_path = _state["extracted_dir"] / "composite.png"
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

        # scheme.png — rasterize the vetted SVG outline paths from the frontend.
        # These are the exact paths the user sees and approves in the blueprint view.
        frontend_outlines = data.get("outlines", [])
        scheme = _rasterize_outlines(
            frontend_outlines, house.canvas_width, house.canvas_height
        )
        _write_scaled(zf, "scheme.png", scheme)

        # light.png — outer house outline from SVG paths (same source as scheme)
        light = _rasterize_outline_boundary(
            frontend_outlines, house.canvas_width, house.canvas_height
        )
        _write_scaled(zf, "light.png", light)

        # flat.png — composite clipped to house shape (FullHouse sprite)
        # blue.png — same silhouette filled with blue (Background sprite)
        if comp_src:
            flat = _generate_flat(pieces, piece_images_orig, comp_src)
            _write_scaled(zf, "flat.png", flat)

        # blue.png — house silhouette filled with blue (same mask as flat)
        import numpy as np
        from PIL import ImageFilter
        mask_raw = _build_house_mask(
            piece_images_orig, pieces,
            house.canvas_width, house.canvas_height,
        )
        mask_bin = Image.fromarray((mask_raw > 30).astype(np.uint8) * 255)
        mask_closed = mask_bin.filter(ImageFilter.MaxFilter(5))
        mask_closed = mask_closed.filter(ImageFilter.MinFilter(5))
        blue = Image.new("RGBA", (house.canvas_width, house.canvas_height),
                         (51, 85, 204, 255))
        blue.putalpha(mask_closed)
        _write_scaled(zf, "blue.png", blue)

        # Unity house_data.json (blocks, steps, colliders)
        placement = data.get("placement", {})
        house_data = build_house_data(
            pieces=pieces,
            bricks_by_id=_state["bricks_by_id"],
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
        )
        zf.writestr("house_data.json", json.dumps(house_data, indent=2))

    zip_buffer.seek(0)
    return send_file(
        zip_buffer,
        mimetype="application/zip",
        as_attachment=True,
        download_name="house_puzzle_export.zip",
    )


def _rasterize_outline_boundary(outlines, canvas_w, canvas_h):
    """Rasterize only the outer boundary of the house from SVG outline paths.

    Draws all piece outlines as filled polygons to get the house silhouette,
    then extracts the outer edge as a white stroke.
    """
    import numpy as np
    from PIL import ImageFilter

    # Fill all piece outlines to get house silhouette
    silhouette = Image.new("L", (canvas_w, canvas_h), 0)
    draw = ImageDraw.Draw(silhouette)
    for outline in outlines:
        pts = outline.get("points", [])
        if len(pts) < 3:
            continue
        coords = [(p[0], p[1]) for p in pts]
        draw.polygon(coords, fill=255)

    # Morphological close to fill gaps
    silhouette = silhouette.filter(ImageFilter.MaxFilter(5))
    silhouette = silhouette.filter(ImageFilter.MinFilter(5))

    # Extract outer edge: dilate - original
    dilated = silhouette.filter(ImageFilter.MaxFilter(9))
    edge = np.array(dilated).astype(np.int16) - np.array(silhouette).astype(np.int16)
    edge = np.clip(edge, 0, 255).astype(np.uint8)

    img = Image.new("RGBA", (canvas_w, canvas_h), (255, 255, 255, 255))
    img.putalpha(Image.fromarray(edge))
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

        app.run(host=args.host, port=args.port, debug=not is_frozen)
    except Exception as e:
        print(f"\nERROR: {e}")
        import traceback
        traceback.print_exc()
        if getattr(sys, 'frozen', False):
            input("\nPress Enter to close...")
