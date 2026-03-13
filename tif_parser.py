"""
Parse Photoshop multi-layer TIF files and extract brick layers.

Uses Pillow to read layer geometry and pixel data — no external
dependencies (ImageMagick is NOT required).
"""

import struct
from dataclasses import dataclass, field
from pathlib import Path

from PIL import Image


@dataclass
class BrickLayer:
    """A single brick/element layer extracted from a TIF."""
    index: int
    name: str
    x: int
    y: int
    width: int
    height: int
    layer_type: str = "brick"  # brick, window, door, base, composite, tiny


@dataclass
class HouseData:
    """Parsed house data from a TIF file."""
    source_path: str
    canvas_width: int
    canvas_height: int
    composite: BrickLayer | None = None
    base: BrickLayer | None = None
    bricks: list[BrickLayer] = field(default_factory=list)
    total_layers: int = 0


def identify_layers(tif_path: str) -> list[dict]:
    """Read layer geometry from a multi-page TIFF using Pillow.

    For each page/frame, extracts width, height, and pixel offset
    (XPosition/YPosition tags multiplied by resolution).
    """
    img = Image.open(tif_path)
    layers = []
    idx = 0

    while True:
        try:
            img.seek(idx)
        except EOFError:
            break

        w, h = img.size
        tags = getattr(img, "tag_v2", {})

        # XPosition (286) and YPosition (287) are rationals in resolution units.
        # Multiply by XResolution (282) / YResolution (283) to get pixels.
        x_res = _rational_to_float(tags.get(282, 1))
        y_res = _rational_to_float(tags.get(283, 1))
        x_pos = _rational_to_float(tags.get(286, 0))
        y_pos = _rational_to_float(tags.get(287, 0))

        px_x = round(x_pos * x_res) if x_res else 0
        px_y = round(y_pos * y_res) if y_res else 0

        layers.append({
            "idx": idx,
            "w": w,
            "h": h,
            "x": px_x,
            "y": px_y,
        })
        idx += 1

    return layers


def _rational_to_float(val) -> float:
    """Convert a TIFF rational tag value to a float."""
    if val is None:
        return 0.0
    if isinstance(val, (int, float)):
        return float(val)
    if isinstance(val, tuple):
        # IFDRational or (numerator, denominator)
        if len(val) == 2 and val[1] != 0:
            return val[0] / val[1]
        if len(val) >= 1:
            return float(val[0])
    # Pillow IFDRational objects support float()
    try:
        return float(val)
    except (TypeError, ValueError):
        return 0.0


def extract_layer_names(tif_path: str) -> list[str]:
    """Extract Photoshop layer names from TIF ImageSourceData tag."""
    img = Image.open(tif_path)
    tag_data = img.tag_v2.get(37724, b"")
    if not tag_data:
        return []

    names = []
    # Search for UTF-16BE 'Layer ' pattern
    pattern = b"\x00L\x00a\x00y\x00e\x00r\x00 "
    pos = 0
    while True:
        pos = tag_data.find(pattern, pos)
        if pos == -1:
            break
        end = pos
        while end < len(tag_data) - 1 and end < pos + 60:
            ch = struct.unpack(">H", tag_data[end : end + 2])[0]
            if 0x20 <= ch <= 0x7E:
                end += 2
            else:
                break
        name = tag_data[pos:end].decode("utf-16-be", errors="replace").strip()
        if name:
            names.append(name)
        pos += 2
    return names


def classify_layer(layer: dict, canvas_w: int, canvas_h: int) -> str:
    """Classify a layer based on its size relative to the canvas."""
    w, h = layer["w"], layer["h"]
    if w >= canvas_w * 0.9 and h >= canvas_h * 0.9:
        return "full"
    if w < 10 or h < 20:
        return "tiny"
    return "brick"


def parse_tif(tif_path: str) -> HouseData:
    """Parse a Photoshop TIF and extract brick layer metadata."""
    tif_path = str(tif_path)
    raw_layers = identify_layers(tif_path)

    if not raw_layers:
        raise ValueError(f"No layers found in {tif_path}")

    # Canvas size from first (composite) layer
    canvas_w = raw_layers[0]["w"]
    canvas_h = raw_layers[0]["h"]

    house = HouseData(
        source_path=tif_path,
        canvas_width=canvas_w,
        canvas_height=canvas_h,
        total_layers=len(raw_layers),
    )

    full_count = 0
    for layer in raw_layers:
        cls = classify_layer(layer, canvas_w, canvas_h)

        brick = BrickLayer(
            index=layer["idx"],
            name=f"layer_{layer['idx']}",
            x=layer["x"],
            y=layer["y"],
            width=layer["w"],
            height=layer["h"],
        )

        if cls == "full":
            if full_count == 0:
                brick.layer_type = "composite"
                house.composite = brick
            else:
                brick.layer_type = "base"
                house.base = brick
            full_count += 1
        elif cls == "tiny":
            brick.layer_type = "tiny"
        else:
            # Classify windows/doors by size heuristics
            if layer["w"] > 350 and layer["h"] > 400:
                brick.layer_type = "window"
            else:
                brick.layer_type = "brick"
            house.bricks.append(brick)

    return house


def extract_brick_png(tif_path: str, layer_index: int, output_path: str):
    """Extract a single layer as a PNG using Pillow."""
    img = Image.open(tif_path)
    img.seek(layer_index)
    # Convert to RGBA to preserve transparency
    rgba = img.convert("RGBA")
    rgba.save(output_path, format="PNG")


def extract_layers_batch(tif_path: str, layer_indices: list[int],
                         output_dir: str, prefix: str = "brick"):
    """Extract multiple layers as PNGs.

    Opens the TIF once and seeks to each requested frame to avoid
    repeatedly opening a large file.
    """
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    img = Image.open(tif_path)

    for idx in layer_indices:
        output_path = out / f"{prefix}_{idx:03d}.png"
        if output_path.exists():
            continue
        img.seek(idx)
        rgba = img.convert("RGBA")
        rgba.save(str(output_path), format="PNG")


def extract_all_bricks(tif_path: str, house: HouseData, output_dir: str) -> dict:
    """Extract all brick layers as PNGs. Returns manifest dict."""
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    manifest = {
        "source": Path(tif_path).name,
        "canvas": {"width": house.canvas_width, "height": house.canvas_height},
        "bricks": [],
    }

    # Extract composite
    if house.composite:
        comp_path = out / "composite.png"
        extract_brick_png(tif_path, house.composite.index, str(comp_path))
        manifest["composite"] = "composite.png"

    # Extract base
    if house.base:
        base_path = out / "base.png"
        extract_brick_png(tif_path, house.base.index, str(base_path))
        manifest["base"] = "base.png"

    # Extract bricks
    for brick in house.bricks:
        fname = f"brick_{brick.index:03d}.png"
        brick_path = out / fname
        extract_brick_png(tif_path, brick.index, str(brick_path))
        manifest["bricks"].append({
            "id": brick.index,
            "name": brick.name,
            "file": fname,
            "x": brick.x,
            "y": brick.y,
            "width": brick.width,
            "height": brick.height,
            "type": brick.layer_type,
        })

    return manifest
