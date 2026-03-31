"""
Parse Illustrator-exported PDF files with OCG layers.

Returns HouseData compatible with the existing TIF pipeline — the rest of
the server (puzzle engine, adjacency, export) is unchanged.

Illustrator export setup
------------------------
File → Save As → Adobe PDF
General tab: check "Create Acrobat Layers from Top-Level Layers"

Layer naming conventions
------------------------
- Vector/outline layer: named "vector", "outline", "outlines", "contour", "contours"
  (case-insensitive). Override with the ``outline_layer`` argument.
- Ignored layers: "blueprint", "lights" (case-insensitive). Override with
  ``ignore_layers`` argument.
- All remaining layers are treated as brick layers.

Output size
-----------
Pass ``canvas_width`` (pixels) to fix the output width; DPI is computed
automatically from the artboard dimensions. Or pass ``dpi`` directly.
Default: canvas_width=2000.

Layer toggle note
-----------------
PyMuPDF set_layer_ui_config: action=0 = ON, action=1 = OFF.
"""

from __future__ import annotations

from pathlib import Path

import fitz  # PyMuPDF
import numpy as np
from PIL import Image

from tif_parser import BrickLayer, HouseData

# ---------------------------------------------------------------------------
# Constants
# ---------------------------------------------------------------------------

OUTLINE_NAMES: frozenset[str] = frozenset(
    ["vector", "outline", "outlines", "contour", "contours", "vectors"]
)
IGNORE_NAMES: frozenset[str] = frozenset(["blueprint", "lights"])
_GEOM_SCALE = 0.10      # fraction of full DPI used for fast geometry extraction
_ALPHA_THRESH = 30


# ---------------------------------------------------------------------------
# Internal helpers
# ---------------------------------------------------------------------------

def _compute_dpi(
    page: fitz.Page,
    canvas_width: int | None,
    canvas_height: int | None,
    dpi: int | None,
) -> float:
    if dpi is not None:
        return float(dpi)
    if canvas_height is not None:
        return canvas_height / page.rect.height * 72.0
    if canvas_width is not None:
        return canvas_width / page.rect.width * 72.0
    return 900.0 / page.rect.height * 72.0   # default: 900px tall


def _all_off(doc: fitz.Document) -> None:
    for l in doc.layer_ui_configs():
        doc.set_layer_ui_config(l["number"], action=2)   # 2 = force OFF (0=force ON, 1=toggle)


def _layer_on(doc: fitz.Document, number: int) -> None:
    doc.set_layer_ui_config(number, action=0)            # 0 = force ON


def _render(doc: fitz.Document, page: fitz.Page, mat: fitz.Matrix,
            clip: fitz.Rect | None = None) -> np.ndarray:
    pix = page.get_pixmap(matrix=mat, alpha=True, colorspace=fitz.csRGB, clip=clip)
    return np.frombuffer(pix.samples, dtype=np.uint8).reshape(
        pix.height, pix.width, 4
    ).copy()


def _opaque_bbox(alpha: np.ndarray, scale_ratio: float) -> tuple[int, int, int, int] | None:
    """Bounding box of opaque pixels, scaled by ratio. Returns (x, y, w, h) or None."""
    rows = np.any(alpha > _ALPHA_THRESH, axis=1)
    cols = np.any(alpha > _ALPHA_THRESH, axis=0)
    if not rows.any():
        return None
    y0 = int(np.argmax(rows) * scale_ratio)
    y1 = int((len(rows) - np.argmax(rows[::-1])) * scale_ratio)
    x0 = int(np.argmax(cols) * scale_ratio)
    x1 = int((len(cols) - np.argmax(cols[::-1])) * scale_ratio)
    return x0, y0, max(1, x1 - x0), max(1, y1 - y0)


def _classify(w: int, h: int, cw: int, ch: int) -> str:
    if w >= 0.9 * cw and h >= 0.9 * ch:
        return "full"
    if w < 10 or h < 20:
        return "tiny"
    if w > 350 and h > 400:
        return "window"
    return "brick"


def _drawing_to_points(d: dict, scale: float, bezier_samples: int = 8) -> list[tuple[float, float]]:
    """Convert a PyMuPDF drawing dict to pixel (x, y) points."""
    pts: list[tuple[float, float]] = []
    for item in d["items"]:
        code = item[0]
        if code == "l":
            pts.append((item[1].x * scale, item[1].y * scale))
        elif code == "c":
            p1, cp1, cp2, p4 = item[1], item[2], item[3], item[4]
            pts.append((p1.x * scale, p1.y * scale))
            n = bezier_samples
            for i in range(1, n + 1):
                t = i / (n + 1)
                u = 1.0 - t
                x = u**3*p1.x + 3*u**2*t*cp1.x + 3*u*t**2*cp2.x + t**3*p4.x
                y = u**3*p1.y + 3*u**2*t*cp1.y + 3*u*t**2*cp2.y + t**3*p4.y
                pts.append((x * scale, y * scale))
        elif code == "re":
            r = item[1]
            return [
                (r.x0 * scale, r.y0 * scale), (r.x1 * scale, r.y0 * scale),
                (r.x1 * scale, r.y1 * scale), (r.x0 * scale, r.y1 * scale),
            ]
        elif code == "qu":
            q = item[1]
            return [
                (q.ul.x * scale, q.ul.y * scale), (q.ur.x * scale, q.ur.y * scale),
                (q.lr.x * scale, q.lr.y * scale), (q.ll.x * scale, q.ll.y * scale),
            ]
    return pts


def _iou(a: tuple, b: tuple) -> float:
    ix0, iy0 = max(a[0], b[0]), max(a[1], b[1])
    ix1, iy1 = min(a[2], b[2]), min(a[3], b[3])
    if ix1 <= ix0 or iy1 <= iy0:
        return 0.0
    inter = (ix1 - ix0) * (iy1 - iy0)
    area_a = (a[2] - a[0]) * (a[3] - a[1])
    area_b = (b[2] - b[0]) * (b[3] - b[1])
    return inter / (area_a + area_b - inter) if (area_a + area_b - inter) > 0 else 0.0


def _iob(path_rect: tuple, brick_rect: tuple) -> float:
    """Intersection over Brick area: fraction of the brick's bbox covered by the path bbox.

    Preferred over IoU for outline matching because PDF outline paths often span
    multiple adjacent bricks (shared structural outlines). IoB asks "how much of
    THIS brick does this path cover?" rather than penalising large shared paths.
    """
    ix0, iy0 = max(path_rect[0], brick_rect[0]), max(path_rect[1], brick_rect[1])
    ix1, iy1 = min(path_rect[2], brick_rect[2]), min(path_rect[3], brick_rect[3])
    if ix1 <= ix0 or iy1 <= iy0:
        return 0.0
    inter = (ix1 - ix0) * (iy1 - iy0)
    brick_area = (brick_rect[2] - brick_rect[0]) * (brick_rect[3] - brick_rect[1])
    return inter / brick_area if brick_area > 0 else 0.0


def _match_paths_to_bricks(
    paths: list[dict], brick_layers: list[BrickLayer]
) -> dict[int, list[list[float]]]:
    """Match PDF outline paths to bricks using IoB (intersection over brick area).

    Key design decisions:
    - IoB not IoU: PDF outline paths span multiple adjacent bricks (shared structural
      outlines). IoB scores how much of THIS brick the path covers, without penalising
      the path for also covering neighbouring bricks.
    - Paths are SHAREABLE: adjacent bricks may legitimately share the same outline
      path. used_paths exclusion is intentionally absent. used_bricks ensures each
      brick is matched to at most one path (its best-covering match).
    - Threshold 0.15: path must cover at least 15% of the brick's bounding box area.
    """
    candidates: list[tuple[float, int, int]] = []
    for bi, bl in enumerate(brick_layers):
        brick_box = (bl.x, bl.y, bl.x + bl.width, bl.y + bl.height)
        for pi, path in enumerate(paths):
            score = _iob(path["rect"], brick_box)
            if score >= 0.15:
                candidates.append((score, bi, pi))
    candidates.sort(reverse=True)  # best IoB first

    used_bricks: set[int] = set()
    result: dict[int, list[list[float]]] = {}
    for score, bi, pi in candidates:
        if bi in used_bricks:
            continue
        bl = brick_layers[bi]
        result[bl.index] = [[p[0] - bl.x, p[1] - bl.y] for p in paths[pi]["pts"]]
        used_bricks.add(bi)
    return result


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def parse_pdf(
    pdf_path: str,
    canvas_width: int | None = None,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    outline_layer: str | None = None,
    ignore_layers: list[str] | None = None,
) -> HouseData:
    """
    Parse an Illustrator-exported PDF with OCG layers.

    Args:
        pdf_path:      Path to the PDF file.
        canvas_width:  Target canvas width in pixels. DPI computed automatically.
                       Ignored if ``dpi`` is given explicitly.
        dpi:           Override render DPI directly.
        outline_layer: Exact layer name for vector outlines. Auto-detected if None.
        ignore_layers: Layer names to skip (default: blueprint, lights).

    Returns:
        HouseData with geometry populated. Call extract_pdf_layers_batch() to write PNGs.
        BrickLayer.name stores the layer name; BrickLayer.index is sequential (0, 1, 2…).
    """
    _ignore = frozenset(n.lower() for n in (ignore_layers or [])) | IGNORE_NAMES
    _outline = outline_layer

    doc = fitz.open(pdf_path)
    page = doc[0]

    full_dpi = _compute_dpi(page, canvas_width, canvas_height, dpi)
    geom_dpi = full_dpi * _GEOM_SCALE
    full_scale = full_dpi / 72.0
    geom_scale = geom_dpi / 72.0
    geom_mat = fitz.Matrix(geom_scale, geom_scale)

    canvas_w = round(page.rect.width * full_scale)
    canvas_h = round(page.rect.height * full_scale)

    layers = doc.layer_ui_configs()
    if not layers:
        doc.close()
        raise ValueError(
            "PDF has no layers. In Illustrator: Save As PDF → "
            "General tab → check 'Create Acrobat Layers from Top-Level Layers'."
        )

    # Identify outline layer number
    outline_num: int | None = None
    if _outline:
        outline_num = next(
            (l["number"] for l in layers if l["text"] == _outline), None
        )
    else:
        outline_num = next(
            (l["number"] for l in layers if l["text"].lower() in OUTLINE_NAMES), None
        )

    house = HouseData(
        source_path=pdf_path,
        canvas_width=canvas_w,
        canvas_height=canvas_h,
        render_dpi=round(full_dpi, 4),
    )

    # Compute composite opaque pixel count at geom scale for background detection
    _all_off(doc)
    for l in layers:
        if l["text"].lower() not in _ignore and l["number"] != outline_num:
            _layer_on(doc, l["number"])
    arr_composite_low = _render(doc, page, geom_mat)
    composite_opaque_geom = int(np.count_nonzero(arr_composite_low[:, :, 3] > 128))
    # Threshold: layers with > 70% of composite opaque pixels are background/cumulative renders
    bg_threshold = composite_opaque_geom * 0.70

    full_count = 0
    brick_idx = 0

    for l in layers:
        num = l["number"]
        name = l["text"]
        house.total_layers += 1

        if num == outline_num:
            continue  # vector layer — geometry only, no PNG

        if name.lower() in _ignore:
            continue  # blueprint, lights, etc.

        # Render at low DPI for fast bounding-box extraction
        _all_off(doc)
        _layer_on(doc, num)
        arr_low = _render(doc, page, geom_mat)
        bbox = _opaque_bbox(arr_low[:, :, 3], full_scale / geom_scale)
        if bbox is None:
            continue  # empty layer

        # Skip background/cumulative render layers (>70% of composite opaque pixels)
        layer_opaque_geom = int(np.count_nonzero(arr_low[:, :, 3] > 128))
        if layer_opaque_geom > bg_threshold:
            continue  # background layer — not a puzzle piece

        x, y, w, h = bbox
        cls = _classify(w, h, canvas_w, canvas_h)

        if cls == "tiny":
            continue

        if cls == "full":
            layer = BrickLayer(
                index=num,   # use layer number as id for composite/base
                name=name, x=0, y=0,
                width=canvas_w, height=canvas_h,
                layer_type="composite" if full_count == 0 else "base",
            )
            if full_count == 0:
                house.composite = layer
            elif full_count == 1:
                house.base = layer
            full_count += 1
            continue

        house.bricks.append(BrickLayer(
            index=brick_idx,
            name=name,
            x=x, y=y,
            width=w, height=h,
            layer_type=cls,
        ))
        # Store the layer number on the object for extraction (not in dataclass — use a dict)
        house.bricks[-1]._layer_num = num   # type: ignore[attr-defined]
        brick_idx += 1

    # Layer warnings
    layer_names_lower = {l["text"].lower() for l in layers}
    if outline_num is None:
        house.warnings.append(
            "No outline/vector layer found — polygon colliders will be traced from raster."
        )
    if not house.bricks:
        house.warnings.append("No brick layers found in this PDF.")
    duplicate_names = [
        name for name in (bl.name for bl in house.bricks)
        if sum(1 for b in house.bricks if b.name == name) > 1
    ]
    if duplicate_names:
        seen: set[str] = set()
        unique_dups = [n for n in duplicate_names if not (n in seen or seen.add(n))]  # type: ignore[func-returns-value]
        house.warnings.append(
            f"Duplicate layer names (resolved by layer number): {', '.join(unique_dups)}"
        )

    # Crop canvas to house content bounding box so the house fills the target height
    if house.bricks:
        xs  = [b.x for b in house.bricks]
        ys  = [b.y for b in house.bricks]
        x1s = [b.x + b.width  for b in house.bricks]
        y1s = [b.y + b.height for b in house.bricks]

        # Convert full-scale pixel bbox → PDF page points
        hx0 = min(xs)  / full_scale
        hy0 = min(ys)  / full_scale
        hx1 = max(x1s) / full_scale
        hy1 = max(y1s) / full_scale

        # Add 5 % padding, clamp to page
        pad_x = (hx1 - hx0) * 0.05
        pad_y = (hy1 - hy0) * 0.05
        clip_x0 = max(0.0, hx0 - pad_x)
        clip_y0 = max(0.0, hy0 - pad_y)
        clip_x1 = min(page.rect.width,  hx1 + pad_x)
        clip_y1 = min(page.rect.height, hy1 + pad_y)
        house.clip_rect = (clip_x0, clip_y0, clip_x1, clip_y1)

        # Choose DPI so the clipped region fills canvas_height
        clip_h_pts = clip_y1 - clip_y0
        clip_w_pts = clip_x1 - clip_x0
        if canvas_height and clip_h_pts > 0:
            clip_dpi = canvas_height / clip_h_pts * 72.0
        else:
            clip_dpi = full_dpi
        new_scale = clip_dpi / 72.0

        # Crop origin in new-scale pixels
        ox = clip_x0 * new_scale
        oy = clip_y0 * new_scale
        sf = new_scale / full_scale   # rescale factor for existing brick px coords

        for b in house.bricks:
            b.x      = round(b.x      * sf - ox)
            b.y      = round(b.y      * sf - oy)
            b.width  = max(1, round(b.width  * sf))
            b.height = max(1, round(b.height * sf))

        house.canvas_width  = round(clip_w_pts * new_scale)
        house.canvas_height = round(clip_h_pts * new_scale)
        house.render_dpi    = round(clip_dpi, 4)

    doc.close()
    return house


def _get_layer_num(bl: BrickLayer, layers: list[dict]) -> int | None:
    """Retrieve layer number from BrickLayer — from _layer_num if set, else by name match."""
    if hasattr(bl, "_layer_num"):
        return bl._layer_num  # type: ignore[attr-defined]
    # Fallback: first matching name
    return next((l["number"] for l in layers if l["text"] == bl.name), None)


def extract_pdf_brick_png(
    pdf_path: str,
    layer_name: str,
    out_path: str,
    canvas_width: int | None = None,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    clip_rect: tuple[float, float, float, float] | None = None,
) -> None:
    """Render a single named layer to a PNG file."""
    doc = fitz.open(pdf_path)
    page = doc[0]
    full_dpi = _compute_dpi(page, canvas_width, canvas_height, dpi)
    mat = fitz.Matrix(full_dpi / 72.0, full_dpi / 72.0)
    clip = fitz.Rect(*clip_rect) if clip_rect else None
    layers = doc.layer_ui_configs()
    num = next((l["number"] for l in layers if l["text"] == layer_name), None)
    if num is None:
        doc.close()
        raise ValueError(f"Layer '{layer_name}' not found in {pdf_path}")
    _all_off(doc)
    _layer_on(doc, num)
    arr = _render(doc, page, mat, clip=clip)
    doc.close()
    Image.fromarray(arr, "RGBA").save(out_path, "PNG")


def extract_pdf_layers_batch(
    pdf_path: str,
    brick_layers: list[BrickLayer],
    out_dir: str,
    canvas_width: int | None = None,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    clip_rect: tuple[float, float, float, float] | None = None,
    prefix: str = "brick",
) -> None:
    """
    Render each brick layer to {out_dir}/{prefix}_{index:03d}.png.
    Skips files that already exist.
    """
    out_dir_path = Path(out_dir)
    doc = fitz.open(pdf_path)
    page = doc[0]
    full_dpi = _compute_dpi(page, canvas_width, canvas_height, dpi)
    mat = fitz.Matrix(full_dpi / 72.0, full_dpi / 72.0)
    clip = fitz.Rect(*clip_rect) if clip_rect else None
    layers = doc.layer_ui_configs()

    for bl in brick_layers:
        out_path = out_dir_path / f"{prefix}_{bl.index:03d}.png"
        if out_path.exists():
            continue
        num = _get_layer_num(bl, layers)
        if num is None:
            continue
        _all_off(doc)
        _layer_on(doc, num)
        arr = _render(doc, page, mat, clip=clip)
        Image.fromarray(arr, "RGBA").save(str(out_path), "PNG")

    doc.close()


def extract_pdf_composite_png(
    pdf_path: str,
    out_path: str,
    canvas_width: int | None = None,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    clip_rect: tuple[float, float, float, float] | None = None,
    ignore_layers: list[str] | None = None,
) -> None:
    """Render all non-ignored layers together as the composite."""
    _ignore = frozenset(n.lower() for n in (ignore_layers or [])) | IGNORE_NAMES
    doc = fitz.open(pdf_path)
    page = doc[0]
    full_dpi = _compute_dpi(page, canvas_width, canvas_height, dpi)
    mat = fitz.Matrix(full_dpi / 72.0, full_dpi / 72.0)
    clip = fitz.Rect(*clip_rect) if clip_rect else None
    # Turn on all except ignored layers
    for l in doc.layer_ui_configs():
        if l["text"].lower() in _ignore:
            doc.set_layer_ui_config(l["number"], action=1)
        else:
            doc.set_layer_ui_config(l["number"], action=0)
    arr = _render(doc, page, mat, clip=clip)
    doc.close()
    Image.fromarray(arr, "RGBA").save(out_path, "PNG")


def extract_pdf_vector_polygons(
    pdf_path: str,
    brick_layers: list[BrickLayer],
    canvas_width: int | None = None,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    clip_rect: tuple[float, float, float, float] | None = None,
    outline_layer: str | None = None,
) -> dict[int, list[list[float]]] | None:
    """
    Extract vector outline polygons from the outline layer and match to bricks by IoU.
    Returns {brick.index: [[x, y], ...]} in brick-local pixel coords.
    Returns None if no outline layer exists.

    IMPORTANT: Every shape in this pipeline is a complex polygon from the PDF vector
    layer. There are NO simple rectangles. If a brick ends up with no polygon match
    it will be absent from the returned dict — the caller must treat that as an error,
    not fall back to a bounding-box rectangle.
    """
    doc = fitz.open(pdf_path)
    page = doc[0]
    full_dpi = _compute_dpi(page, canvas_width, canvas_height, dpi)
    scale = full_dpi / 72.0
    # Clip origin in pixel space (for coordinate offsetting)
    clip_ox = clip_rect[0] * scale if clip_rect else 0.0
    clip_oy = clip_rect[1] * scale if clip_rect else 0.0
    layers = doc.layer_ui_configs()

    _outline = outline_layer
    if _outline:
        outline_num = next(
            (l["number"] for l in layers if l["text"] == _outline), None
        )
    else:
        outline_num = next(
            (l["number"] for l in layers if l["text"].lower() in OUTLINE_NAMES), None
        )

    if outline_num is None:
        doc.close()
        return None

    _all_off(doc)
    _layer_on(doc, outline_num)
    drawings = page.get_drawings()
    doc.close()

    if not drawings:
        return None

    paths = []
    for d in drawings:
        pts = _drawing_to_points(d, scale)
        if len(pts) < 3:
            continue
        pts = [(x - clip_ox, y - clip_oy) for x, y in pts]
        xs = [p[0] for p in pts]
        ys = [p[1] for p in pts]
        paths.append({
            "pts": pts,
            "rect": (min(xs), min(ys), max(xs), max(ys)),
        })

    return _match_paths_to_bricks(paths, brick_layers) if paths else None


def render_outlines_png(
    pdf_path: str,
    out_path: str,
    canvas_height: int | None = 900,
    dpi: int | None = None,
    clip_rect: tuple[float, float, float, float] | None = None,
    outline_layer: str | None = None,
    stroke_width: float = 4.0,
) -> None:
    """Render the vector/outline layer as white strokes on a transparent background.

    Uses PIL ImageDraw at 2x supersampling then downscales for antialiasing.
    stroke_width is in output pixel space (before supersampling).
    """
    from PIL import ImageDraw as PilDraw

    doc = fitz.open(pdf_path)
    page = doc[0]
    full_dpi = _compute_dpi(page, None, canvas_height, dpi)
    scale = full_dpi / 72.0
    # Canvas size is clipped region size if clip_rect is given, else full page
    if clip_rect:
        canvas_w = round((clip_rect[2] - clip_rect[0]) * scale)
        canvas_h = round((clip_rect[3] - clip_rect[1]) * scale)
        clip_ox  = clip_rect[0] * scale
        clip_oy  = clip_rect[1] * scale
    else:
        canvas_w = round(page.rect.width  * scale)
        canvas_h = round(page.rect.height * scale)
        clip_ox  = 0.0
        clip_oy  = 0.0
    layers = doc.layer_ui_configs()

    _outline = outline_layer
    if _outline:
        outline_num = next(
            (l["number"] for l in layers if l["text"] == _outline), None
        )
    else:
        outline_num = next(
            (l["number"] for l in layers if l["text"].lower() in OUTLINE_NAMES), None
        )

    if outline_num is None:
        doc.close()
        # No outline layer: save empty transparent image
        img = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
        img.save(out_path, "PNG")
        return

    _all_off(doc)
    _layer_on(doc, outline_num)
    drawings = page.get_drawings()
    doc.close()

    # Render at 2x resolution for antialiasing, then downscale
    ss = 2
    ss_w, ss_h = canvas_w * ss, canvas_h * ss
    img_ss = Image.new("RGBA", (ss_w, ss_h), (0, 0, 0, 0))
    draw = PilDraw.Draw(img_ss)
    sw_ss = max(1, round(stroke_width * ss))

    for d in drawings:
        pts = _drawing_to_points(d, scale)
        if len(pts) < 2:
            continue
        coords = [(round((x - clip_ox) * ss), round((y - clip_oy) * ss)) for x, y in pts]
        for i in range(len(coords) - 1):
            draw.line([coords[i], coords[i + 1]], fill=(255, 255, 255, 255), width=sw_ss)
        if len(coords) >= 3:
            fx, fy = coords[0]
            lx, ly = coords[-1]
            if abs(fx - lx) <= 2 * ss and abs(fy - ly) <= 2 * ss:
                draw.line([coords[-1], coords[0]], fill=(255, 255, 255, 255), width=sw_ss)

    img = img_ss.resize((canvas_w, canvas_h), Image.LANCZOS)
    img.save(out_path, "PNG")
