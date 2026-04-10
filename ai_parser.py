"""
Parse Adobe Illustrator .ai files to extract brick layers.

File format overview
--------------------
An .ai file (AI 24+) is a PDF whose document catalog stores the Illustrator
artwork in a series of AIPrivateData streams compressed with ZStandard.
Concatenated and decompressed, these streams form a single text document
containing the full AI PostScript-dialect program.

Top-level PDF OCG layers (bricks, lights, background) exist in the PDF, but
they are INSUFFICIENT for rendering:
  - Plain/raster bricks: pixel data lives in embedded raster streams inside
    the AI private data; OCG rendering via PyMuPDF works for these.
  - Vector bricks (e.g. gradient-filled windows): the gradient fill exists
    ONLY in the AI private data, NOT in the PDF rendering layer.  PyMuPDF OCG
    rendering of the 'bricks' OCG returns all-transparent pixels for these.
    They must be rendered entirely from the AI private data using PIL.

Expected layer structure
------------------------
    background   — house silhouette (one compound vector path + fill)
    bricks       — group containing N sub-layers, one per brick
        Layer 1 … Layer N   (each is a brick sub-layer)
    lights       — window/door light shapes (for the night-lights game layer)
                   DO NOT USE — lights layer must never be rendered into bricks.

Brick sub-layer types
---------------------
    brick         — plain raster brick; pixel data extracted via _extract_raster.
    mixed_brick   — raster brick with a vector outline; rendered via PyMuPDF OCG
                    and then masked to its vector polygon.
    vector_brick  — fully vector brick rendered from AI private data only via
                    _render_vector_brick_pil (see below).

Vector brick rendering pipeline (_render_vector_brick_pil)
----------------------------------------------------------
A vector brick is a layered object such as a gradient-filled arch window.
It contains exactly two drawable objects in its block text:

  Object 1 — Rectangle with linear gradient fill
    Defined by a Bg operator (gradient reference by name) and an Xm matrix
    (gradient axis transform).  The gradient stops live in a global
    %%AI5_BeginGradient…%%AI5_EndGradient section earlier in the AI text.

  Object 2 — Compound path (arch frame with rectangular cut-outs)
    Defined inside a *u…*U group using the D operator to control add/subtract
    per sub-path.  Fill colour is specified by an Xa operator.

Rendering steps:
  1. Parse the gradient (Bg + Xm + global Bs stops) → render full-brick RGBA
     array with white-to-blue (or other) gradient across all pixels.
  2. Parse the compound path colour (Xa) and shape (D + m/L/C/f ops) →
     render a second RGBA array: opaque pink where the arch frame is, fully
     transparent where the cut-out windows are.
  3. Porter-Duff "over" composite: compound path over gradient.
     Result: pink arch frame with gradient visible through window holes.
  4. Clip to the arch outer polygon (from %_ path lines) using a filled PIL
     polygon mask, zeroing alpha outside the arch boundary.

Key AI operator reference (private data)
-----------------------------------------
Bg      (grad-name) … Bg
        Reference a named gradient as the current fill.  The name matches a
        %%AI5_BeginGradient: (name) section in the global header.

Xm      a b c d tx ty Xm
        Gradient axis transform matrix.  The gradient runs from point
        (tx, ty) in AI space to (tx+a, ty+b).  For vertical gradients,
        a≈0 and b is the height in AI units.

%%AI5_BeginGradient: (name) … %%AI5_EndGradient
        Global gradient definition section.  Contains one Bs line per stop.
        Lines are prefixed with %_ in the outline section; strip the prefix
        before parsing numbers.

Bs      [values…] Bs
        One gradient color stop.  Two forms:
          Long  (≥8 numbers): C M Y K  R G B  [mid params…]  position
            → RGB is at indices 4,5,6 (0–1 range, multiply by 255).
          Short (<8 numbers): gray  [mid params…]  position
            → grayscale stop; replicate gray to R, G, B.
        position is 0–100; divide by 100 to get t ∈ [0, 1].

Xa      C M Y K R G B Xa   (7-param form, preferred)
        Compound path fill colour.  RGB at indices 4,5,6 (0–1 range).
        A 4-param form (CMYK only) also exists as a fallback.

*u / *U
        Begin / end a compound path group.

D       mode D
        Sub-path drawing mode inside *u…*U:
          1 D → next sub-path is ADDED to the shape (solid fill).
          0 D → next sub-path is SUBTRACTED (punches a hole).
        This is NOT even-odd fill.  The D mode persists until changed.

m / L / C / f
        Standard PostScript path operators: moveto / lineto / curveto / fill.
        f (or F) closes and fills the current sub-path, then resets it.

%_ lines
        Path data "outline" stored alongside each brick block.
        Used by _extract_vector_path to recover the outer polygon for
        masking/collider purposes.  Present in all brick types.

Coordinate spaces
-----------------
AI native space   — large absolute coords, e.g. (6000–7100, 3600–5700),
                    y-up.
PDF page space    — 0–1320 × 0–2860 pts, (0,0) at bottom-left, y-up.
PyMuPDF space     — same extent as PDF, but y-down (origin top-left).
Pixel space       — render_dpi/72 × PyMuPDF, (0,0) at top-left of clip rect.
Brick-local px    — pixel space offset by brick's (x, y) top-left corner.

Transforms:
    pymu_x = ai_x + offset_x
    pymu_y = y_base - ai_y          (y_base = PDF page height in pts)
    brick_px_x = (pymu_x - clip_x0) * scale - bl.x
    brick_px_y = (pymu_y - clip_y0) * scale - bl.y

offset_x and y_base are derived once per file by comparing the background
layer's bounding box in AI space with the page ArtBox in PDF space.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from pathlib import Path

import fitz
import numpy as np
from PIL import Image

from tif_parser import BrickLayer, HouseData


# ---------------------------------------------------------------------------
# Internal data structures
# ---------------------------------------------------------------------------

@dataclass
class _LayerBlock:
    name: str
    begin: int          # byte offset into decompressed text
    end: int
    depth: int
    children: list["_LayerBlock"] = field(default_factory=list)


# ---------------------------------------------------------------------------
# Step 1 — decompress AI private data
# ---------------------------------------------------------------------------

def _decompress_ai_data(ai_path: str) -> tuple[bytes, str]:
    """Load and ZStandard-decompress all AIPrivateData streams."""
    import zstandard as zstd

    doc = fitz.open(ai_path)

    # Find the object that contains AIPrivateData1, AIPrivateData2, ...
    priv_xref: int | None = None
    for xref in range(1, doc.xref_length()):
        try:
            obj_str = doc.xref_object(xref, compressed=False)
            if "AIPrivateData1 " in obj_str:
                priv_xref = xref
                break
        except Exception:
            pass

    if priv_xref is None:
        doc.close()
        raise ValueError("No AIPrivateData found in .ai file")

    obj_str = doc.xref_object(priv_xref, compressed=False)
    pairs = re.findall(r"AIPrivateData(\d+) (\d+) 0 R", obj_str)
    pairs_sorted = sorted(pairs, key=lambda p: int(p[0]))

    raw = b"".join(doc.xref_stream(int(xref)) for _, xref in pairs_sorted)
    doc.close()

    # Find ZStandard frame magic: 0x28 0xB5 0x2F 0xFD
    magic = bytes([0x28, 0xB5, 0x2F, 0xFD])
    pos = raw.find(magic)
    if pos < 0:
        raise ValueError("ZStandard magic not found in AIPrivateData")

    dctx = zstd.ZstdDecompressor()
    decompressed = dctx.decompress(raw[pos:], max_output_size=400 * 1024 * 1024)
    text = decompressed.decode("latin-1", errors="replace")
    return decompressed, text


# ---------------------------------------------------------------------------
# Step 2 — parse layer tree from decompressed text
# ---------------------------------------------------------------------------

def _parse_layer_tree(text: str) -> list[_LayerBlock]:
    """
    Parse %AI5_BeginLayer / %AI5_EndLayer pairs into a nested tree.
    Returns the list of top-level LayerBlock nodes.
    """
    begin_re = re.compile(r"%AI5_BeginLayer")
    end_re   = re.compile(r"%AI5_EndLayer")
    name_re  = re.compile(r"Lb\r\(([^)]*)\)")

    events: list[tuple[str, int]] = []
    for m in begin_re.finditer(text):
        events.append(("B", m.start()))
    for m in end_re.finditer(text):
        events.append(("E", m.start()))
    events.sort(key=lambda e: e[1])

    # Assign end positions to begin positions via a stack
    stack: list[_LayerBlock] = []
    roots: list[_LayerBlock] = []

    for typ, pos in events:
        if typ == "B":
            # Extract layer name from nearby text
            snippet = text[pos: pos + 300]
            nm = name_re.search(snippet)
            name = nm.group(1) if nm else ""
            depth = len(stack)
            block = _LayerBlock(name=name, begin=pos, end=pos, depth=depth)
            if stack:
                stack[-1].children.append(block)
            else:
                roots.append(block)
            stack.append(block)
        else:  # "E"
            if stack:
                block = stack.pop()
                block.end = pos + len("%AI5_EndLayer")

    return roots


# ---------------------------------------------------------------------------
# Step 3 — coordinate offset (AI native → PDF page)
# ---------------------------------------------------------------------------

def _compute_ai_transform(background: _LayerBlock, text: str,
                          page: fitz.Page) -> tuple[float, float]:
    """
    Derive transform coefficients from AI-native coords → PyMuPDF page coords (y-down).

    Transform:
        pymu_x = ai_x + offset_x
        pymu_y = y_base  - ai_y

    Returns (offset_x, y_base).
    PyMuPDF uses y-down (origin top-left); AI uses y-up (origin bottom-left).
    """
    block_text = text[background.begin: background.end]

    coords = re.findall(r"(-?\d+\.?\d*)\s+(-?\d+\.?\d*)\s+[mLCl]\b", block_text)
    if not coords:
        art = page.artbox
        return art.x0, art.y1  # fallback: assume AI origin = artbox bottom-left

    xs = [float(c[0]) for c in coords]
    ys = [float(c[1]) for c in coords]
    ai_xmin, ai_ymin = min(xs), min(ys)

    # page.artbox in PyMuPDF y-down: art.y0 = top, art.y1 = bottom
    art = page.artbox
    offset_x = art.x0 - ai_xmin
    y_base = art.y1 + ai_ymin   # pymu_y = y_base - ai_y

    return offset_x, y_base


# ---------------------------------------------------------------------------
# Step 4a — detect vector-only (gradient) bricks by parsing plain path bbox
# ---------------------------------------------------------------------------

def _find_gradient_name(block_text: str) -> str | None:
    """Find the gradient name from a Bg operator line, or None.

    Scans lines instead of using regex on the full block to avoid
    catastrophic backtracking on large blocks (700KB+).
    """
    if "Bg" not in block_text:
        return None
    for line in block_text.split("\r"):
        stripped = line.strip()
        if stripped.endswith("Bg") and "(" in stripped:
            m = re.match(r".*\(([^)]+)\)", stripped)
            if m:
                return m.group(1)
    return None


def _extract_plain_path_bbox(
    block: _LayerBlock, text: str,
) -> tuple[float, float, float, float] | None:
    """
    Extract the AI-space bounding box from plain (non-%_) path operators.

    Used for gradient-fill bricks that have no Xh raster.
    Returns (ai_xmin, ai_ymin, ai_xmax, ai_ymax) in AI y-up coords, or None.
    """
    block_text = text[block.begin: block.end]
    xs: list[float] = []
    ys: list[float] = []
    for line in block_text.split("\r"):
        line = line.strip()
        if line.startswith("%"):
            continue
        parts = line.split()
        if not parts:
            continue
        op = parts[-1]
        if op in ("m", "L", "l") and len(parts) >= 3:
            try:
                xs.append(float(parts[0]))
                ys.append(float(parts[1]))
            except ValueError:
                pass
        elif op in ("C", "c") and len(parts) >= 7:
            # Include all three point pairs (cp1, cp2, endpoint) for correct bbox
            try:
                for i in range(0, 6, 2):
                    xs.append(float(parts[i]))
                    ys.append(float(parts[i + 1]))
            except ValueError:
                pass
    if len(xs) < 2:
        return None
    return (min(xs), min(ys), max(xs), max(ys))


# ---------------------------------------------------------------------------
# Step 4b — extract raster from a brick sub-layer block
# ---------------------------------------------------------------------------

def _extract_raster(block: _LayerBlock, raw_bytes: bytes, text: str
                    ) -> tuple[Image.Image | None, tuple[float, float, float, float] | None]:
    """
    Extract the first raster image from a layer block.

    Returns (RGBA PIL image, (tx, ty, w_pts, h_pts)) in AI coordinate space,
    or (None, None) if no raster found.
    """
    block_text = text[block.begin: block.end]
    block_bytes = raw_bytes[block.begin: block.end]

    # Placement matrix: [ a 0 0 d tx ty ] w h flag Xh
    # Note: shear values may be bare `0` (no decimal point)
    _num = r"-?\d+(?:\.\d+)?"
    mat_m = re.search(
        r"\[\s*(" + _num + r")\s+" + _num + r"\s+" + _num + r"\s+(" + _num + r")"
        r"\s+(" + _num + r")\s+(" + _num + r")\s*\]\s+(\d+)\s+(\d+)\s+\d+\s+Xh",
        block_text,
    )
    if not mat_m:
        return None, None

    a    = float(mat_m.group(1))
    d    = float(mat_m.group(2))
    tx   = float(mat_m.group(3))
    ty   = float(mat_m.group(4))
    img_w = int(mat_m.group(5))
    img_h = int(mat_m.group(6))

    if img_w <= 0 or img_h <= 0:
        return None, None

    # Data follows  %%BeginData: N\rXI\n  or  %%BeginData: NXI\n
    xi_m = re.search(r"%%BeginData:\s*\d+[^\n]*XI\n", block_text)
    if not xi_m:
        return None, None

    data_start_in_block = xi_m.end()
    expected = img_w * img_h * 3
    data = block_bytes[data_start_in_block: data_start_in_block + expected]

    if len(data) < expected:
        return None, None

    try:
        arr = np.frombuffer(bytes(data), dtype=np.uint8).reshape(img_h, img_w, 3)
    except ValueError:
        return None, None

    # White-to-alpha: pixels where R>248 & G>248 & B>248 → transparent
    white_mask = (arr[:, :, 0] > 248) & (arr[:, :, 1] > 248) & (arr[:, :, 2] > 248)
    alpha = np.where(white_mask, 0, 255).astype(np.uint8)
    rgba = np.dstack([arr, alpha])
    img = Image.fromarray(rgba, "RGBA")

    # Size in AI pts
    w_pts = abs(a) * img_w
    h_pts = abs(d) * img_h

    return img, (tx, ty, w_pts, h_pts)


# ---------------------------------------------------------------------------
# Step 5 — extract vector outline from a brick sub-layer block
# ---------------------------------------------------------------------------

def _parse_path_lines(
    lines: list[str],
    offset_x: float,
    y_base: float,
) -> list[list[list[float]]]:
    """
    Parse a sequence of path operator strings into a list of polygons.

    Handles both absolute line/curve operators.
    Returns list of closed polygon point lists in PyMuPDF y-down coords.
    """
    def _to_pymu(ax: float, ay: float) -> tuple[float, float]:
        return ax + offset_x, y_base - ay

    pts: list[tuple[float, float]] = []
    polygons: list[list[list[float]]] = []

    for parts in lines:
        if not parts:
            continue
        op = parts[-1]

        if op == "m" and len(parts) >= 3:
            if len(pts) >= 3:
                polygons.append([[p[0], p[1]] for p in pts])
            pts = [_to_pymu(float(parts[0]), float(parts[1]))]

        elif op in ("L", "l") and len(parts) >= 3:
            pts.append(_to_pymu(float(parts[0]), float(parts[1])))

        elif op in ("C", "c") and len(parts) >= 7:
            if not pts:
                continue
            p1 = pts[-1]
            cp1 = _to_pymu(float(parts[0]), float(parts[1]))
            cp2 = _to_pymu(float(parts[2]), float(parts[3]))
            p4  = _to_pymu(float(parts[4]), float(parts[5]))
            for i in range(1, 9):
                t = i / 9.0
                u = 1.0 - t
                x = u**3*p1[0] + 3*u**2*t*cp1[0] + 3*u*t**2*cp2[0] + t**3*p4[0]
                y = u**3*p1[1] + 3*u**2*t*cp1[1] + 3*u*t**2*cp2[1] + t**3*p4[1]
                pts.append((x, y))
            pts.append(p4)

        elif op in ("n", "N", "f", "F", "s", "S", "b", "B") and len(pts) >= 3:
            polygons.append([[p[0], p[1]] for p in pts])
            pts = []

    if len(pts) >= 3:
        polygons.append([[p[0], p[1]] for p in pts])

    return polygons


def _extract_vector_path(block: _LayerBlock, text: str,
                         offset_x: float, y_base: float,
                         ) -> list[list[float]]:
    """
    Extract the closed vector polygon for a brick layer block.

    %_ lines contain the authoritative AI vector outline for all brick types:
    for raster bricks they appear after %AI5_EndRaster; for vector/mixed bricks
    they appear inline after a small XW placeholder. Reading all %_ path lines
    works for both and correctly returns the outer shape (including arches/curves)
    rather than internal gradient-rectangle paths.

    Points returned in PyMuPDF y-down coords: [[x0,y0], [x1,y1], ...]
    """
    block_text = text[block.begin: block.end]
    _PATH_OPS = ("m", "L", "l", "C", "c", "n", "N", "f", "F", "s", "S", "b", "B")

    # Primary: %_ prefixed lines (present in all brick types)
    parsed_lines = []
    for line in block_text.split("\r"):
        line = line.strip()
        if not line.startswith("%_"):
            continue
        parts = line[2:].split()
        if parts and parts[-1] in _PATH_OPS:
            parsed_lines.append(parts)

    if not parsed_lines:
        # Fallback for bricks that have no %_ path data: read plain operator lines
        for line in block_text.split("\r"):
            line = line.strip()
            if line.startswith("%"):
                continue
            parts = line.split()
            if parts and parts[-1] in _PATH_OPS and len(parts) in (3, 7):
                try:
                    [float(p) for p in parts[:-1]]
                    parsed_lines.append(parts)
                except ValueError:
                    pass

    polygons = _parse_path_lines(parsed_lines, offset_x, y_base)
    if not polygons:
        return []
    return max(polygons, key=len)


# ---------------------------------------------------------------------------
# Step 6 — render composite/lights via PyMuPDF
# ---------------------------------------------------------------------------

def _render_layer_png(ai_path: str, layer_name: str, out_path: str,
                      dpi: float, clip_rect: tuple[float, float, float, float],
                      pdf_offset_px: tuple[int, int] = (0, 0),
                      canvas_size: tuple[int, int] | None = None,
                      ) -> None:
    """Render a single named top-level layer to a PNG.

    pdf_offset_px: (dx, dy) pixel shift to apply to the PyMuPDF render
        to align it with AI-private-data brick coordinates.
        Positive dx means "shift render left" (PDF content is too far right).
    canvas_size: (w, h) if given, output is exactly this size.
    """
    doc = fitz.open(ai_path)
    page = doc[0]
    layers = doc.layer_ui_configs()

    for l in layers:
        doc.set_layer_ui_config(l["number"], action=2)   # all off

    target = next((l for l in layers if l["text"] == layer_name), None)
    if target:
        doc.set_layer_ui_config(target["number"], action=0)

    scale = dpi / 72.0
    mat = fitz.Matrix(scale, scale)
    dx, dy = pdf_offset_px
    cw, ch = canvas_size if canvas_size else (round((clip_rect[2] - clip_rect[0]) * scale),
                                              round((clip_rect[3] - clip_rect[1]) * scale))

    # Expand clip to capture content that shifts past the canvas edge.
    # dx<0: content is right of expected → expand clip right so we render it.
    # dx>0: content is left of expected → expand clip left.
    x0, y0, x1, y1 = clip_rect
    if dx < 0:
        x1 -= dx / scale
    elif dx > 0:
        x0 -= dx / scale
    if dy < 0:
        y1 -= dy / scale
    elif dy > 0:
        y0 -= dy / scale
    clip = fitz.Rect(max(0, x0), max(0, y0),
                     min(page.rect.width, x1), min(page.rect.height, y1))

    pix = page.get_pixmap(matrix=mat, alpha=True, colorspace=fitz.csRGB, clip=clip)
    doc.close()

    src = Image.frombytes("RGBA", (pix.width, pix.height), pix.samples)
    out = Image.new("RGBA", (cw, ch), (0, 0, 0, 0))
    out.paste(src, (dx, dy))
    out.save(out_path, "PNG")


def _render_all_layers_png(ai_path: str, out_path: str,
                            dpi: float, clip_rect: tuple[float, float, float, float],
                            ) -> Image.Image:
    """Render with all layers on and return a PIL Image (used for mixed bricks)."""
    doc = fitz.open(ai_path)
    page = doc[0]
    layers = doc.layer_ui_configs()

    for l in layers:
        doc.set_layer_ui_config(l["number"], action=0)

    mat = fitz.Matrix(dpi / 72.0, dpi / 72.0)
    clip = fitz.Rect(*clip_rect)
    pix = page.get_pixmap(matrix=mat, alpha=True, colorspace=fitz.csRGB, clip=clip)
    doc.close()
    if out_path:
        pix.save(out_path)
    return Image.frombytes("RGBA", (pix.width, pix.height), pix.samples)


def _render_bricks_ocg_png(
    ai_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    pdf_offset_px: tuple[int, int] = (0, 0),
) -> Image.Image:
    """
    Render with ONLY the 'bricks' OCG on — lights layer stays OFF.

    The gradient Rectangle for each window brick lives in the bricks OCG.
    The yellow glow Rectangle lives in the lights OCG.
    Rendering bricks-only gives us gradient fill without any yellow.

    If pdf_offset_px is given, adjusts the clip rect so the PyMuPDF render
    aligns with AI private data coordinates.
    """
    doc = fitz.open(ai_path)
    page = doc[0]
    layers = doc.layer_ui_configs()

    for l in layers:
        doc.set_layer_ui_config(l["number"], action=2)   # all off

    bricks_layer = next((l for l in layers if l["text"] == "bricks"), None)
    if bricks_layer:
        doc.set_layer_ui_config(bricks_layer["number"], action=0)  # bricks on

    scale = dpi / 72.0
    mat = fitz.Matrix(scale, scale)
    dx, dy = pdf_offset_px
    x0, y0, x1, y1 = clip_rect
    clip = fitz.Rect(x0 - dx / scale, y0 - dy / scale,
                     x1 - dx / scale, y1 - dy / scale)
    pix = page.get_pixmap(matrix=mat, alpha=True, colorspace=fitz.csRGB, clip=clip)
    doc.close()
    return Image.frombytes("RGBA", (pix.width, pix.height), pix.samples)


# ---------------------------------------------------------------------------
# Step 7 — render outlines PNG (white stroked vector paths of all bricks)
# ---------------------------------------------------------------------------

def render_ai_lights_png(
    ai_path: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    pdf_offset_px: tuple[int, int] = (0, 0),
    canvas_size: tuple[int, int] | None = None,
) -> None:
    """Render only the 'lights' OCG layer to a PNG."""
    _render_layer_png(ai_path, "lights", out_path, dpi, clip_rect,
                      pdf_offset_px=pdf_offset_px, canvas_size=canvas_size)


def render_ai_background_png(
    ai_path: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    pdf_offset_px: tuple[int, int] = (0, 0),
    canvas_size: tuple[int, int] | None = None,
) -> None:
    """Render only the 'background' OCG layer to a PNG."""
    _render_layer_png(ai_path, "background", out_path, dpi, clip_rect,
                      pdf_offset_px=pdf_offset_px, canvas_size=canvas_size)


def render_ai_outlines_png(
    ai_path: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    stroke_width: float = 3.2,
) -> None:
    """
    Render all brick vector outlines as white strokes on transparent background.

    Reads each brick sub-layer's vector path from AIPrivateData and draws them.
    """
    from PIL import ImageDraw as PilDraw

    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    doc = fitz.open(ai_path)
    page = doc[0]
    offset_x, y_base = _compute_ai_transform(
        next(r for r in roots if r.name == "background"), text, page
    )
    doc.close()

    clip_x0, clip_y0, clip_x1, clip_y1 = clip_rect
    scale = dpi / 72.0
    canvas_w = round((clip_x1 - clip_x0) * scale)
    canvas_h = round((clip_y1 - clip_y0) * scale)
    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0)).save(out_path, "PNG")
        return

    ss = 2
    img_ss = Image.new("RGBA", (canvas_w * ss, canvas_h * ss), (0, 0, 0, 0))
    draw = PilDraw.Draw(img_ss)
    sw_ss = max(1, round(stroke_width * ss))

    for child in bricks_node.children:
        poly = _extract_vector_path(child, text, offset_x, y_base)
        if len(poly) < 2:
            continue
        # poly is in PyMuPDF y-down coords; pixel space is also y-down
        coords = [
            (round((p[0] - clip_x0) * scale * ss),
             round((p[1] - clip_y0) * scale * ss))
            for p in poly
        ]
        for i in range(len(coords) - 1):
            draw.line([coords[i], coords[i + 1]], fill=(255, 255, 255, 255), width=sw_ss)
        if len(coords) >= 3:
            draw.line([coords[-1], coords[0]], fill=(255, 255, 255, 255), width=sw_ss)

    img_ss.resize((canvas_w, canvas_h), Image.LANCZOS).save(out_path, "PNG")


# ---------------------------------------------------------------------------
# Public API — parse_ai
# ---------------------------------------------------------------------------

def parse_ai(
    ai_path: str,
    canvas_height: int = 900,
) -> HouseData:
    """
    Parse a .ai file → HouseData (geometry only; call extract_ai_layers for PNGs).

    The returned HouseData has:
        composite  — BrickLayer for the background layer
        bricks     — one BrickLayer per brick sub-layer
        render_dpi — DPI chosen so the clipped region fills canvas_height
        clip_rect  — (x0, y0, x1, y1) in PDF page pts for rendering
    """
    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    doc = fitz.open(ai_path)
    page = doc[0]

    # Coordinate offset and full-page scale
    bg_node = next((r for r in roots if r.name == "background"), None)
    if bg_node is None:
        raise ValueError("No 'background' layer found in .ai file")

    offset_x, y_base = _compute_ai_transform(bg_node, text, page)

    # Collect brick sub-layers
    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        raise ValueError("No 'bricks' layer found in .ai file")

    # Pass 1: extract all placements to compute canvas extent.
    # Each entry: (child, (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom), layer_type)
    placements: list[tuple[_LayerBlock, tuple[float, float, float, float], str]] = []
    for child in bricks_node.children:
        block_text = text[child.begin: child.end]
        has_gradient = _find_gradient_name(block_text) is not None
        _, mat_ai = _extract_raster(child, raw_bytes, text)
        if mat_ai is not None and not has_gradient:
            # Plain raster brick (and optionally a vector outline for masking).
            tx, ty, w_pts, h_pts = mat_ai
            pymu_x0 = tx + offset_x
            pymu_y_top = y_base - ty
            pymu_x1 = tx + w_pts + offset_x
            pymu_y_bottom = y_base - ty + h_pts
            has_vector = _extract_plain_path_bbox(child, text) is not None
            ltype = "mixed_brick" if has_vector else "brick"
            placements.append((child, (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom), ltype))
        else:
            # Gradient/vector-only brick: use the vector path bbox for the
            # canvas bounding box.  The raster matrix (if present) may refer to
            # a small embedded detail image, not the full brick extent.
            ai_bbox = _extract_plain_path_bbox(child, text)
            if ai_bbox is None:
                continue
            ai_xmin, ai_ymin, ai_xmax, ai_ymax = ai_bbox
            pymu_x0 = ai_xmin + offset_x
            pymu_x1 = ai_xmax + offset_x
            pymu_y_top = y_base - ai_ymax   # ai_ymax = top in AI y-up → smallest pymu y
            pymu_y_bottom = y_base - ai_ymin
            placements.append((child, (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom), "vector_brick"))

    if not placements:
        raise ValueError("No brick rasters found in 'bricks' layer")

    # Compute clip rect in PyMuPDF y-down coords.
    # Bricks bottom = ground level = y=0 in Unity. No padding.
    all_x0 = [p[1][0] for p in placements]
    all_y0 = [p[1][1] for p in placements]
    all_x1 = [p[1][2] for p in placements]
    all_y1 = [p[1][3] for p in placements]

    page_rect = page.rect
    bricks_bottom = max(all_y1)  # pymu y-down: largest y = bottom

    # Extract screen frame bbox (AI coords → PyMuPDF coords)
    screen_node = next((r for r in roots if r.name.lower() == "screen"), None)
    screen_bbox_pymu = None
    if screen_node is not None:
        targets = screen_node.children if screen_node.children else [screen_node]
        for t in targets:
            sb = _extract_plain_path_bbox(t, text)
            if sb is not None:
                # sb is (ai_xmin, ai_ymin, ai_xmax, ai_ymax) in AI y-up
                sb_x0 = sb[0] + offset_x
                sb_x1 = sb[2] + offset_x
                sb_y_top = y_base - sb[3]   # ai_ymax → pymu top
                sb_y_bottom = y_base - sb[1]  # ai_ymin → pymu bottom
                screen_bbox_pymu = (sb_x0, sb_y_top, sb_x1, sb_y_bottom)
                break

    # Clip rect: bricks extent for all edges.
    # Screen frame only provides height for scaling, not clip boundaries.
    # Bottom = bricks bottom (ground level = y=0 in Unity).
    # Top = topmost brick (houses can be taller than screen frame).
    clip_x0 = max(0.0, min(all_x0))
    clip_y0 = max(0.0, min(all_y0))
    clip_x1 = min(page_rect.width, max(all_x1))
    clip_y1 = bricks_bottom

    clip_h_pts = clip_y1 - clip_y0
    clip_w_pts = clip_x1 - clip_x0

    # DPI so clip height == canvas_height px
    dpi = canvas_height / clip_h_pts * 72.0 if clip_h_pts > 0 else 72.0
    scale = dpi / 72.0

    canvas_w_px = round(clip_w_pts * scale)
    canvas_h_px = round(clip_h_pts * scale)

    # Screen frame height in pixels (= 15.5 game units)
    screen_frame_height_px: float = 0.0
    if screen_bbox_pymu is not None:
        screen_h_pts = screen_bbox_pymu[3] - screen_bbox_pymu[1]
        screen_frame_height_px = screen_h_pts * scale

    # Compute pixel offset between AI-private-data coords and PyMuPDF PDF render.
    # The AI data places bricks at positions derived from the Xh raster matrix +
    # artbox transform, but the PDF content stream may render them at a different
    # position.  Measure the offset so overlay layers (lights, background) rendered
    # via PyMuPDF can be shifted to align with the brick canvas.
    pdf_offset_px = (0, 0)
    try:
        import numpy as np
        mat_probe = fitz.Matrix(scale, scale)
        clip_probe = fitz.Rect(clip_x0, clip_y0, clip_x1, clip_y1)
        layers_probe = doc.layer_ui_configs()
        for lp in layers_probe:
            doc.set_layer_ui_config(lp["number"], action=2)
        bl_probe = next((lp for lp in layers_probe if lp["text"] == "bricks"), None)
        if bl_probe:
            doc.set_layer_ui_config(bl_probe["number"], action=0)
        pix_probe = page.get_pixmap(matrix=mat_probe, alpha=True,
                                    colorspace=fitz.csRGB, clip=clip_probe)
        alpha_probe = np.frombuffer(pix_probe.samples, dtype=np.uint8).reshape(
            pix_probe.height, pix_probe.width, 4)[:, :, 3]
        cols_probe = np.any(alpha_probe > 30, axis=0)
        rows_probe = np.any(alpha_probe > 30, axis=1)
        if cols_probe.any() and rows_probe.any():
            pymupdf_x0 = int(np.argmax(cols_probe))
            pymupdf_y0 = int(np.argmax(rows_probe))
            # Expected position from AI brick data
            expected_x0 = round((min(all_x0) - clip_x0) * scale)
            expected_y0 = round((min(all_y0) - clip_y0) * scale)
            dx = expected_x0 - pymupdf_x0
            dy = expected_y0 - pymupdf_y0
            if abs(dx) > 1 or abs(dy) > 1:
                pdf_offset_px = (dx, dy)
    except Exception:
        pass

    house = HouseData(
        source_path=ai_path,
        canvas_width=canvas_w_px,
        canvas_height=canvas_h_px,
        render_dpi=round(dpi, 4),
        clip_rect=(clip_x0, clip_y0, clip_x1, clip_y1),
        screen_frame_height_px=screen_frame_height_px,
        pdf_offset_px=pdf_offset_px,
    )

    # Composite = background layer (full canvas)
    house.composite = BrickLayer(
        index=0,
        name="background",
        x=0, y=0,
        width=canvas_w_px, height=canvas_h_px,
        layer_type="composite",
    )

    # Pass 2: build BrickLayer for each brick (all in PyMuPDF y-down, same as pixel space)
    # Deduplicate by bbox — AI files can contain two layers at the exact same position
    seen_bbox: set[tuple] = set()
    child_by_name: dict[str, _LayerBlock] = {}
    brick_idx = 0
    for child, (pymu_x0, pymu_y_top, pymu_x1, pymu_y_bottom), ltype in placements:
        px = round((pymu_x0 - clip_x0) * scale)
        py = round((pymu_y_top - clip_y0) * scale)
        pw = max(1, round((pymu_x1 - pymu_x0) * scale))
        ph = max(1, round((pymu_y_bottom - pymu_y_top) * scale))

        bbox_key = (px, py, pw, ph)
        if bbox_key in seen_bbox:
            continue
        seen_bbox.add(bbox_key)

        bl = BrickLayer(
            index=brick_idx,
            name=child.name,
            x=px, y=py,
            width=pw, height=ph,
            layer_type=ltype,
        )
        house.bricks.append(bl)
        child_by_name[child.name] = child
        brick_idx += 1

    # Pass 3: extract vector outline polygons from the PostScript data
    for bl in house.bricks:
        child = child_by_name.get(bl.name)
        if child is None:
            continue
        poly = _extract_vector_path(child, text, offset_x, y_base)
        if len(poly) >= 3:
            bl.polygon = [
                [(p[0] - clip_x0) * scale - bl.x,
                 (p[1] - clip_y0) * scale - bl.y]
                for p in poly
            ]

    # Store clip_rect on house for use by extract/render functions
    house.clip_rect = (clip_x0, clip_y0, clip_x1, clip_y1)

    doc.close()
    return house


# ---------------------------------------------------------------------------
# Public API — extract individual brick PNGs
# ---------------------------------------------------------------------------

def extract_ai_brick_png(
    ai_path: str,
    brick_name: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
) -> None:
    """
    Extract a single brick's raster from AIPrivateData and save as PNG.

    The raster is cropped/scaled to the brick's natural pixel size, ready for
    use in puzzle assembly. White background → transparent.
    """
    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        return

    target = next((c for c in bricks_node.children if c.name == brick_name), None)
    if target is None:
        return

    img, _ = _extract_raster(target, raw_bytes, text)
    if img is None:
        return

    img.save(out_path, "PNG")


def extract_ai_layers_batch(
    ai_path: str,
    brick_layers: list[BrickLayer],
    out_dir: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    prefix: str = "brick",
    pdf_offset_px: tuple[int, int] = (0, 0),
) -> None:
    """
    Extract all brick PNGs as full-canvas images.

    Raster bricks: extracted from AIPrivateData.
    Vector/gradient bricks: cropped from a PyMuPDF render of the bricks layer.
    Each output is a canvas-sized RGBA PNG with the brick at its correct position.
    """
    out = Path(out_dir)
    out.mkdir(parents=True, exist_ok=True)

    scale = dpi / 72.0
    canvas_w = round((clip_rect[2] - clip_rect[0]) * scale)
    canvas_h = round((clip_rect[3] - clip_rect[1]) * scale)

    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        return

    child_by_name = {c.name: c for c in bricks_node.children}

    # Compute coordinate transform once (needed for polygon masking)
    offset_x, y_base = _get_ai_transform(ai_path, text, roots)

    for bl in brick_layers:
        out_path = out / f"{prefix}_{bl.id if bl.id else f'{bl.index:03d}'}.png"
        canvas = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))
        child = child_by_name.get(bl.name)

        if bl.layer_type in ("vector_brick", "mixed_brick"):
            if child is not None:
                img = _render_vector_brick_pil(child, text, offset_x, y_base, scale, clip_rect, bl, raw_bytes)
                canvas.paste(img, (bl.x, bl.y), img)
        else:
            if child is None:
                canvas.save(str(out_path), "PNG")
                continue
            img, _ = _extract_raster(child, raw_bytes, text)
            if img is None:
                canvas.save(str(out_path), "PNG")
                continue
            brick_resized = img.resize((max(1, bl.width), max(1, bl.height)), Image.LANCZOS)
            canvas.paste(brick_resized, (bl.x, bl.y), brick_resized)

        canvas.save(str(out_path), "PNG")


def _parse_gradient_stops_from_text(
    full_text: str, grad_name: str
) -> list[tuple[float, int, int, int]]:
    """
    Parse gradient color stops from the global %%AI5_BeginGradient section.

    The section is located by searching for the marker line:
        %%AI5_BeginGradient: (grad_name)
    and ends at %%AI5_EndGradient.

    Each stop is a line ending in "Bs".  Inside the outline section of the
    AI file, these lines are prefixed with "%_" — that prefix is stripped
    before number parsing.  Parenthesised strings (colour names embedded by
    Illustrator) are also stripped.

    Two numeric formats exist:
      Long  (≥8 numbers):  C M Y K  R G B  [midpoint params…]  position Bs
        → take R,G,B at indices 4,5,6 (0–1 range → ×255).
      Short (<8 numbers):  gray  [midpoint params…]  position Bs
        → replicate gray to R,G,B.

    The last number is always the stop position in the range 0–100; it is
    divided by 100 to produce t ∈ [0, 1].

    Returns a list of (t, r, g, b) tuples sorted by t.
    """
    marker = f"%AI5_BeginGradient: ({grad_name})"
    start = full_text.find(marker)
    if start < 0:
        return []
    end = full_text.find("%AI5_EndGradient", start)
    section = full_text[start: end] if end > start else full_text[start: start + 8000]

    stops: list[tuple[float, int, int, int]] = []
    for line in section.split("\r"):
        line = line.strip()
        # Strip AI outline prefix
        if line.startswith("%_"):
            line = line[2:].strip()
        if not line.endswith("Bs"):
            continue
        # Strip parenthesised strings (colour names) before splitting into numbers
        content = re.sub(r"\([^)]*\)", "", line[:-2]).strip()
        try:
            nums = [float(x) for x in content.split()]
        except ValueError:
            continue
        if len(nums) < 2:
            continue

        position = nums[-1] / 100.0  # last value is always the position 0–100

        if len(nums) >= 8:
            # Long format: [C M Y K] [R G B] [params…] [pos]
            r = int(round(nums[4] * 255))
            g = int(round(nums[5] * 255))
            b = int(round(nums[6] * 255))
        else:
            # Short format: [gray] [params…] [pos]  — e.g. white "1 0 1 6 50 0 Bs"
            gray = int(round(nums[0] * 255))
            r = g = b = gray

        stops.append((position, r, g, b))

    return sorted(stops, key=lambda s: s[0])


def _parse_block_gradient(block_text: str, full_text: str) -> dict | None:
    """
    Parse the linear gradient for a vector brick block.

    Extraction is two-stage:
      1. Find the Bg operator in the block text to get the gradient name:
             (gradient-name) … Bg
         The regex stays on a single line (no \\r or \\n) to avoid matching
         a layer-name Ln line that happens to be followed later by Bg.

      2. Find the Xm transform matrix in the block text to get the gradient
         axis in AI native space:
             a  b  c  d  tx  ty  Xm
         For a vertical gradient, a≈0 and b is the length; the axis runs
         from (tx, ty) to (tx+a, ty+b).  Both endpoints are stored so
         _render_gradient_to_array can project each pixel onto the axis.

    Gradient stops are read from the global %%AI5_BeginGradient section in
    full_text via _parse_gradient_stops_from_text.

    Returns a dict:
        {
          "stops": [(t, r, g, b), …],   # sorted by t ∈ [0,1]
          "gx0": float, "gy0": float,   # axis start in AI space
          "gx1": float, "gy1": float,   # axis end   in AI space
        }
    or None if the Bg or Xm operators are absent or stops cannot be found.
    """
    grad_name = _find_gradient_name(block_text)
    if grad_name is None:
        return None

    # Xm transform: a b c d tx ty Xm  (captures a, b, tx, ty)
    _num = r"-?\d+(?:\.\d+)?"
    xm_m = re.search(
        r"(" + _num + r")\s+(" + _num + r")\s+" + _num + r"\s+" + _num
        + r"\s+(" + _num + r")\s+(" + _num + r")\s+Xm",
        block_text,
    )
    if not xm_m:
        return None

    a  = float(xm_m.group(1))
    b  = float(xm_m.group(2))
    tx = float(xm_m.group(3))
    ty = float(xm_m.group(4))

    stops = _parse_gradient_stops_from_text(full_text, grad_name)
    if not stops:
        return None

    return {
        "stops": stops,
        "gx0": tx,        "gy0": ty,
        "gx1": tx + a,    "gy1": ty + b,
    }


def _render_gradient_to_array(
    grad_info: dict,
    ai_to_px,       # callable(ax, ay) -> (px, py) in brick-local pixel coords
    w: int, h: int,
) -> np.ndarray:
    """
    Render a linear gradient into an (h, w, 4) RGBA uint8 numpy array.

    grad_info must have the keys produced by _parse_block_gradient:
        "stops"  — list of (t, r, g, b) tuples sorted by t ∈ [0, 1]
        "gx0/gy0/gx1/gy1" — gradient axis endpoints in AI native space

    The axis endpoints are converted to brick-local pixel coordinates via
    ai_to_px, then for every pixel (x, y) in the output the parameter t is
    computed as the projection of (x, y) onto the axis vector, normalised by
    the axis length and clamped to [0, 1].  Bilinear interpolation between
    adjacent stops gives the per-channel output value.

    Alpha is always 255 (fully opaque).  The result covers the entire
    (h × w) canvas; masking to the brick outline is applied separately.
    """
    px0, py0 = ai_to_px(grad_info["gx0"], grad_info["gy0"])
    px1, py1 = ai_to_px(grad_info["gx1"], grad_info["gy1"])
    stops = grad_info["stops"]

    dx = px1 - px0
    dy = py1 - py0
    length_sq = float(dx * dx + dy * dy)
    if length_sq < 1e-6:
        arr = np.zeros((h, w, 4), dtype=np.uint8)
        if stops:
            arr[:, :, :3] = stops[0][1:4]
            arr[:, :, 3] = 255
        return arr

    ys_idx, xs_idx = np.mgrid[0:h, 0:w]
    t_arr = ((xs_idx.astype(np.float32) - px0) * dx
             + (ys_idx.astype(np.float32) - py0) * dy) / length_sq
    t_arr = np.clip(t_arr, 0.0, 1.0)

    # Initialise with first stop colour
    r_arr = np.full((h, w), float(stops[0][1]), dtype=np.float32)
    g_arr = np.full((h, w), float(stops[0][2]), dtype=np.float32)
    b_arr = np.full((h, w), float(stops[0][3]), dtype=np.float32)

    for i in range(len(stops) - 1):
        t0, r0, g0, b0 = stops[i]
        t1, r1, g1, b1 = stops[i + 1]
        seg = t1 - t0
        if seg <= 0:
            continue
        in_seg = (t_arr >= t0) & (t_arr <= t1)
        frac = np.where(in_seg, (t_arr - t0) / seg, 0.0)
        r_arr = np.where(in_seg, r0 + frac * (r1 - r0), r_arr)
        g_arr = np.where(in_seg, g0 + frac * (g1 - g0), g_arr)
        b_arr = np.where(in_seg, b0 + frac * (b1 - b0), b_arr)

    # Beyond last stop
    last = stops[-1]
    beyond = t_arr >= last[0]
    r_arr = np.where(beyond, float(last[1]), r_arr)
    g_arr = np.where(beyond, float(last[2]), g_arr)
    b_arr = np.where(beyond, float(last[3]), b_arr)

    arr = np.zeros((h, w, 4), dtype=np.uint8)
    arr[:, :, 0] = np.clip(r_arr, 0, 255).astype(np.uint8)
    arr[:, :, 1] = np.clip(g_arr, 0, 255).astype(np.uint8)
    arr[:, :, 2] = np.clip(b_arr, 0, 255).astype(np.uint8)
    arr[:, :, 3] = 255
    return arr


def _parse_compound_path_color(block_text: str) -> tuple[int, int, int] | None:
    """
    Parse the compound path fill colour from the Xa operator.

    Xa specifies a colour in two possible forms:
      7-param (preferred): C M Y K  R G B  Xa
        The RGB triplet at positions 4,5,6 (0–1 range) is used directly.
        Illustrator writes this dual-encoding so both colour models are
        available without conversion.
      4-param (fallback):  C M Y K  Xa
        CMYK-only; converted to RGB via:  R = 255*(1-C)*(1-K), etc.

    Returns (r, g, b) in 0–255 integer range, or None if no Xa is found.
    """
    _num = r"-?\d+(?:\.\d+)?"
    # Try 7-param form first (CMYK + RGB)
    xa7 = re.search(
        r"(" + _num + r")\s+(" + _num + r")\s+(" + _num + r")\s+(" + _num + r")\s+"
        r"(" + _num + r")\s+(" + _num + r")\s+(" + _num + r")\s+Xa",
        block_text,
    )
    if xa7:
        r = int(round(float(xa7.group(5)) * 255))
        g = int(round(float(xa7.group(6)) * 255))
        b = int(round(float(xa7.group(7)) * 255))
        return (r, g, b)
    # Fallback: 4-param CMYK form
    xa4 = re.search(
        r"(" + _num + r")\s+(" + _num + r")\s+(" + _num + r")\s+(" + _num + r")\s+Xa",
        block_text,
    )
    if xa4:
        c  = float(xa4.group(1))
        mv = float(xa4.group(2))
        y  = float(xa4.group(3))
        k  = float(xa4.group(4))
        r = int(round(255 * (1 - c)  * (1 - k)))
        g = int(round(255 * (1 - mv) * (1 - k)))
        b = int(round(255 * (1 - y)  * (1 - k)))
        return (r, g, b)
    return None


def _render_compound_path_addsub(
    block_text: str,
    color_rgb: tuple[int, int, int],
    ai_to_px,
    w: int, h: int,
) -> np.ndarray:
    """
    Render the compound path (arch frame + cut-out holes) using the D operator.

    The compound path lives inside a *u…*U group in the block text.
    Each sub-path is preceded by a D operator that controls whether it adds
    to or subtracts from the filled shape:

        1 D  → add: next sub-path's area is filled (opaque)
        0 D  → subtract: next sub-path's area is punched out (transparent)

    This is NOT even-odd fill.  The D mode is a persistent flag: it applies
    to all subsequent sub-paths until changed.  The typical structure for a
    window brick is:
        1 D  →  arch outer boundary  → f   (adds the pink arch frame)
        0 D  →  upper window rect    → f   (punches hole for upper pane)
        0 D  →  lower window rect    → f   (punches hole for lower pane)

    Sub-path geometry is built from the standard path operators (m, L, l, C,
    c) using ai_to_px to convert from AI native space to brick-local pixels.
    Cubic Bézier curves (C/c) are approximated with 9 intermediate points.
    On each fill (f/F), the current sub-path is rasterised into a temporary
    PIL image, which is then either OR-merged into the mask (d_mode=1) or
    AND-zeroed (d_mode=0).

    Only non-% lines inside the *u…*U section are processed; comment lines
    (including %_ outline data) are skipped.

    Returns an (h, w, 4) RGBA uint8 array:
        opaque (color_rgb, alpha=255) where the frame is,
        transparent (0,0,0,0) where holes were cut out or nothing was drawn.
    """
    from PIL import ImageDraw as PilDraw

    # Isolate the *u…*U section
    lines = block_text.split("\r")
    u_start = next((i for i, l in enumerate(lines) if l.strip() == "*u"), -1)
    U_end   = next((i for i, l in enumerate(lines) if l.strip() == "*U"), -1)
    section_lines = lines[u_start + 1: U_end] if u_start >= 0 and U_end > u_start else lines

    def seg_to_poly(segs: list[tuple]) -> list[tuple[int, int]]:
        pts: list[tuple[float, float]] = []
        for seg in segs:
            if seg[0] == "m":
                pts = [ai_to_px(seg[1], seg[2])]
            elif seg[0] == "L":
                pts.append(ai_to_px(seg[1], seg[2]))
            elif seg[0] == "C":
                if not pts:
                    continue
                p1 = pts[-1]
                cp1 = ai_to_px(seg[1], seg[2])
                cp2 = ai_to_px(seg[3], seg[4])
                p4  = ai_to_px(seg[5], seg[6])
                for i in range(1, 9):
                    t = i / 9.0
                    u = 1.0 - t
                    x = u**3*p1[0] + 3*u**2*t*cp1[0] + 3*u*t**2*cp2[0] + t**3*p4[0]
                    y = u**3*p1[1] + 3*u**2*t*cp1[1] + 3*u*t**2*cp2[1] + t**3*p4[1]
                    pts.append((x, y))
                pts.append(p4)
        return [(int(round(p[0])), int(round(p[1]))) for p in pts]

    _PATH_OPS = {"m", "L", "l", "C", "c", "f", "F", "n", "N"}
    mask = np.zeros((h, w), dtype=np.uint8)
    current: list[tuple] = []
    d_mode = 1  # default: add

    for raw_line in section_lines:
        line = raw_line.strip()
        if line.startswith("%") or not line:
            continue
        parts = line.split()
        if not parts:
            continue
        op = parts[-1]
        try:
            if op == "D" and len(parts) >= 2:
                d_mode = int(float(parts[0]))
            elif op == "m" and len(parts) >= 3:
                current = [("m", float(parts[0]), float(parts[1]))]
            elif op in ("L", "l") and len(parts) >= 3:
                current.append(("L", float(parts[0]), float(parts[1])))
            elif op in ("C", "c") and len(parts) >= 7:
                current.append(("C",
                    float(parts[0]), float(parts[1]),
                    float(parts[2]), float(parts[3]),
                    float(parts[4]), float(parts[5])))
            elif op in ("f", "F"):
                if current:
                    poly = seg_to_poly(current)
                    if len(poly) >= 3:
                        sub_img = Image.new("L", (w, h), 0)
                        PilDraw.Draw(sub_img).polygon(poly, fill=255)
                        sub_arr = np.array(sub_img)
                        if d_mode == 1:
                            mask = np.where(sub_arr > 0, 255, mask)   # add
                        else:
                            mask = np.where(sub_arr > 0, 0, mask)     # subtract / hole
                current = []
            elif op in ("n", "N"):
                current = []
        except (ValueError, IndexError):
            pass

    arr = np.zeros((h, w, 4), dtype=np.uint8)
    arr[:, :, 0] = np.where(mask > 0, color_rgb[0], 0)
    arr[:, :, 1] = np.where(mask > 0, color_rgb[1], 0)
    arr[:, :, 2] = np.where(mask > 0, color_rgb[2], 0)
    arr[:, :, 3] = mask
    return arr


def _render_vector_brick_pil(
    block: "_LayerBlock",
    text: str,
    offset_x: float,
    y_base: float,
    scale: float,
    clip_rect: tuple[float, float, float, float],
    bl,
    raw_bytes: bytes | None = None,
) -> Image.Image:
    """
    Render a vector brick entirely from AI private data using PIL/NumPy.

    This function is used for bricks whose gradient fill exists only in the
    AI private data streams — PyMuPDF OCG rendering returns transparent for
    these because the gradient is not present in the PDF content stream.

    Up to four rendering steps, applied in order:

    Step 1 — Gradient base layer
        _parse_block_gradient reads the Bg (gradient name) and Xm (axis
        transform) operators from the block text, then fetches the stop
        colours from the global %%AI5_BeginGradient section.
        _render_gradient_to_array fills an (h×w) RGBA array by projecting
        each pixel onto the gradient axis and interpolating stops.
        Alpha is 255 everywhere (the outer polygon clip in the last step
        trims it to the brick boundary).

    Step 2a — Compound path overlay (Porter-Duff "over")  [if Xa colour found]
        _parse_compound_path_color reads the Xa operator for the fill colour.
        _render_compound_path_addsub rasterises the *u…*U compound path using
        the D operator (1=add, 0=subtract) to produce an (h×w) RGBA array:
          • opaque (colour) where the frame is
          • transparent where cut-out window panes are
        Porter-Duff "over" compositing then blends this on top of the
        gradient layer using straight (non-premultiplied) alpha:
          out[c] = (cp[c]·α_cp + base[c]·α_base·(1−α_cp)) / α_out
        Result: coloured frame with gradient showing through window holes.
        Used for arch-window bricks (e.g. brick 185).

    Step 2b — Raster overlay  [if raw_bytes supplied and raster found]
        If the block contains an embedded raster image (Xh matrix + pixel
        data), it is extracted via _extract_raster and pasted over the
        gradient using its natural RGBA alpha channel.  The raster itself
        carries the frame colour and is transparent at window holes, so the
        gradient shows through without any additional compositing.
        Used for door/composite bricks (e.g. brick 183).
        Steps 2a and 2b are mutually exclusive: 2a is skipped when no Xa
        colour is found, 2b is skipped when no raster is found.

    Step 3 — Outer polygon clip
        _extract_vector_path reads the %_ outline lines from the block text
        and returns the brick's outer boundary polygon in PyMuPDF coords.
        A filled PIL polygon mask is drawn and np.minimum applied to the
        alpha channel, zeroing out any pixels outside the boundary.

    The lights OCG is never opened; no PyMuPDF rendering is used at any step.

    Returns a PIL RGBA Image sized (bl.width × bl.height).
    """
    from PIL import ImageDraw as PilDraw

    block_text = text[block.begin: block.end]
    clip_x0, clip_y0 = clip_rect[0], clip_rect[1]
    w = max(1, bl.width)
    h = max(1, bl.height)

    def ai_to_px(ax: float, ay: float) -> tuple[float, float]:
        """AI space → brick-local pixel coords (y-down)."""
        pymu_x = ax + offset_x
        pymu_y = y_base - ay
        return (pymu_x - clip_x0) * scale - bl.x, (pymu_y - clip_y0) * scale - bl.y

    # --- Layer 1: gradient ---
    grad_info = _parse_block_gradient(block_text, text)
    if grad_info:
        base_arr = _render_gradient_to_array(grad_info, ai_to_px, w, h)
    else:
        base_arr = np.zeros((h, w, 4), dtype=np.uint8)

    # --- Layer 2: compound path (arch frame with window holes) ---
    cp_color = _parse_compound_path_color(block_text)
    if cp_color:
        cp_arr = _render_compound_path_addsub(block_text, cp_color, ai_to_px, w, h)
        # Alpha-composite cp over base
        cp_a  = cp_arr[:, :, 3:4].astype(np.float32) / 255.0
        ba_a  = base_arr[:, :, 3:4].astype(np.float32) / 255.0
        out_a = cp_a + ba_a * (1.0 - cp_a)
        safe  = np.where(out_a > 0, out_a, 1.0)
        out = np.zeros((h, w, 4), dtype=np.uint8)
        for c in range(3):
            out[:, :, c] = np.clip(
                (cp_arr[:, :, c] * cp_a[:, :, 0]
                 + base_arr[:, :, c] * ba_a[:, :, 0] * (1.0 - cp_a[:, :, 0]))
                / safe[:, :, 0],
                0, 255,
            ).astype(np.uint8)
        out[:, :, 3] = np.clip(out_a[:, :, 0] * 255, 0, 255).astype(np.uint8)
        base_arr = out

    # --- Layer 2b: raster overlay (door/composite bricks) ---
    # When raw_bytes is supplied and the block contains an embedded raster
    # (Xh matrix + pixel data), paste it over the gradient.  The raster
    # carries the frame colour and has alpha=0 at window holes, so the
    # gradient is visible through the holes without extra compositing.
    if raw_bytes is not None and not cp_color:
        raster_img, mat_ai = _extract_raster(block, raw_bytes, text)
        if raster_img is not None and mat_ai is not None:
            tx, ty, w_pts, h_pts = mat_ai
            # Convert raster corners to brick-local pixel coords
            px0, py0 = ai_to_px(tx, ty)
            px1 = px0 + w_pts * scale
            py1 = py0 + h_pts * scale
            rw = max(1, int(round(px1 - px0)))
            rh = max(1, int(round(py1 - py0)))
            raster_resized = raster_img.resize((rw, rh), Image.LANCZOS).convert("RGBA")
            base_img = Image.fromarray(base_arr, "RGBA")
            base_img.paste(raster_resized, (int(round(px0)), int(round(py0))), raster_resized)
            base_arr = np.array(base_img)

    # --- Layer 3: clip to outer polygon ---
    arch_poly = _extract_vector_path(block, text, offset_x, y_base)
    if len(arch_poly) >= 3:
        arch_pts = [
            (int(round((p[0] - clip_x0) * scale - bl.x)),
             int(round((p[1] - clip_y0) * scale - bl.y)))
            for p in arch_poly
        ]
        arch_mask_img = Image.new("L", (w, h), 0)
        PilDraw.Draw(arch_mask_img).polygon(arch_pts, fill=255)
        arch_mask = np.array(arch_mask_img)
        base_arr[:, :, 3] = np.minimum(base_arr[:, :, 3], arch_mask)

    return Image.fromarray(base_arr, "RGBA")


def _mask_crop_to_polygon(
    crop: Image.Image,
    polygon: list[list[float]],
    crop_x: int,
    crop_y: int,
) -> Image.Image:
    """
    Apply a polygon mask to a crop image.

    Pixels outside the polygon become fully transparent.
    The polygon is in full-canvas pixel coords; crop_x/crop_y are the
    top-left of the crop within the canvas.
    """
    from PIL import ImageDraw as PilDraw
    mask = Image.new("L", crop.size, 0)
    draw = PilDraw.Draw(mask)
    pts = [(p[0] - crop_x, p[1] - crop_y) for p in polygon]
    if len(pts) >= 3:
        draw.polygon(pts, fill=255)
    out = crop.convert("RGBA")
    # Combine existing alpha channel with polygon mask using numpy min
    arr = np.array(out)
    mask_arr = np.array(mask)
    arr[:, :, 3] = np.minimum(arr[:, :, 3], mask_arr)
    return Image.fromarray(arr, "RGBA")


def _get_ai_transform(ai_path: str, text: str, roots: list[_LayerBlock]) -> tuple[float, float]:
    """Compute (offset_x, y_base) transform for this AI file."""
    bg_node = next((r for r in roots if r.name == "background"), None)
    if bg_node is None:
        return 0.0, 0.0
    doc = fitz.open(ai_path)
    page = doc[0]
    offset_x, y_base = _compute_ai_transform(bg_node, text, page)
    doc.close()
    return offset_x, y_base


def compose_ai_bricks_png(
    ai_path: str,
    brick_layers: list[BrickLayer],
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
    pdf_offset_px: tuple[int, int] = (0, 0),
) -> None:
    """
    Assemble all brick rasters into a single full-canvas composite PNG.

    Raster bricks: extracted from AIPrivateData and pasted at their positions.
    Vector/gradient bricks: cropped from a PyMuPDF all-on render, then masked
    to their own polygon so adjacent lights/glows don't bleed in.
    """
    scale = dpi / 72.0
    canvas_w = round((clip_rect[2] - clip_rect[0]) * scale)
    canvas_h = round((clip_rect[3] - clip_rect[1]) * scale)

    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        return

    child_by_name = {c.name: c for c in bricks_node.children}

    # Compute coordinate transform once (needed for polygon masking)
    offset_x, y_base = _get_ai_transform(ai_path, text, roots)

    # Render bricks-layer-only once via PyMuPDF for vector/mixed bricks.
    # Render all vector/mixed bricks via _render_vector_brick_pil.
    # This uses AI private data coordinates — correct positioning, no offset issues.
    canvas = Image.new("RGBA", (canvas_w, canvas_h), (0, 0, 0, 0))

    for bl in brick_layers:
        child = child_by_name.get(bl.name)
        if bl.layer_type in ("vector_brick", "mixed_brick"):
            if child is not None:
                img = _render_vector_brick_pil(child, text, offset_x, y_base, scale, clip_rect, bl, raw_bytes)
                canvas.paste(img, (bl.x, bl.y), img)
        else:
            if child is None:
                continue
            img, _ = _extract_raster(child, raw_bytes, text)
            if img is None:
                continue
            brick_resized = img.resize((max(1, bl.width), max(1, bl.height)), Image.LANCZOS)
            canvas.paste(brick_resized, (bl.x, bl.y), brick_resized)

    canvas.save(out_path, "PNG")


def extract_ai_blueprint_bg_png(
    ai_path: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
) -> None:
    """Render the 'background' OCG layer for use as the blueprint background."""
    _render_layer_png(ai_path, "background", out_path, dpi, clip_rect)


# Keep old name for backward compatibility
def extract_ai_composite_png(
    ai_path: str,
    out_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
) -> None:
    """Render the 'background' layer (alias for extract_ai_blueprint_bg_png)."""
    extract_ai_blueprint_bg_png(ai_path, out_path, dpi, clip_rect)


def extract_ai_background_polygon(
    ai_path: str,
    dpi: float,
    clip_rect: tuple[float, float, float, float],
) -> list[list[float]]:
    """Extract the background layer's vector outline in canvas pixel coords."""
    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    doc = fitz.open(ai_path)
    page = doc[0]
    bg_node = next((r for r in roots if r.name == "background"), None)
    if bg_node is None:
        doc.close()
        return []
    offset_x, y_base = _compute_ai_transform(bg_node, text, page)
    doc.close()

    poly = _extract_vector_path(bg_node, text, offset_x, y_base)
    if len(poly) < 3:
        return []

    scale = dpi / 72.0
    clip_x0, clip_y0 = clip_rect[0], clip_rect[1]
    return [[(p[0] - clip_x0) * scale, (p[1] - clip_y0) * scale] for p in poly]


def extract_ai_vector_polygons(
    ai_path: str,
    brick_layers: list[BrickLayer],
    dpi: float,
    clip_rect: tuple[float, float, float, float],
) -> dict[int, list[list[float]]]:
    """
    Extract vector outline polygons for all bricks.

    Returns {brick.index: [[x, y], ...]} in brick-local pixel coordinates.
    """
    raw_bytes, text = _decompress_ai_data(ai_path)
    roots = _parse_layer_tree(text)

    doc = fitz.open(ai_path)
    page = doc[0]
    offset_x, y_base = _compute_ai_transform(
        next(r for r in roots if r.name == "background"), text, page
    )
    doc.close()

    bricks_node = next((r for r in roots if r.name == "bricks"), None)
    if bricks_node is None:
        return {}

    child_by_name = {c.name: c for c in bricks_node.children}
    scale = dpi / 72.0
    clip_x0, clip_y0 = clip_rect[0], clip_rect[1]

    result: dict[int, list[list[float]]] = {}
    for bl in brick_layers:
        child = child_by_name.get(bl.name)
        if child is None:
            continue
        # poly is in PyMuPDF y-down coords
        poly = _extract_vector_path(child, text, offset_x, y_base)
        if len(poly) < 3:
            continue
        # Convert to pixel coords relative to brick's top-left (y-down in both)
        poly_local = [
            [(p[0] - clip_x0) * scale - bl.x,
             (p[1] - clip_y0) * scale - bl.y]
            for p in poly
        ]
        result[bl.index] = poly_local

    return result
