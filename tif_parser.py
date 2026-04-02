"""
Parse Photoshop multi-layer TIF files and extract brick layers.

Parses the Photoshop Document Data Block (TIFF tag 37724) to extract
layer geometry and pixel data. Pure Python — no ImageMagick required.
"""

import struct
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
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
    render_dpi: float = 0.0
    warnings: list[str] = field(default_factory=list)
    clip_rect: tuple[float, float, float, float] | None = None  # PDF page clip (x0,y0,x1,y1) in pts
    screen_frame_height_px: float = 0.0  # height of the 'screen' frame in pixels (= 15.5 game units)


# ---------------------------------------------------------------------------
# PackBits (RLE) decoder used by Photoshop layer channel data
# ---------------------------------------------------------------------------

def _decode_packbits(data: bytes, offset: int, expected: int) -> tuple[bytearray, int]:
    """Decode PackBits-compressed data. Returns (decoded_bytes, new_offset)."""
    out = bytearray()
    pos = offset
    end = len(data)
    while len(out) < expected and pos < end:
        n = data[pos]
        if n > 127:
            n = n - 256  # signed
        pos += 1
        if 0 <= n <= 127:
            count = n + 1
            out.extend(data[pos:pos + count])
            pos += count
        elif -127 <= n <= -1:
            count = 1 - n
            out.extend(bytes([data[pos]]) * count)
            pos += 1
        # n == -128: no-op
    return out, pos


# ---------------------------------------------------------------------------
# Parse Photoshop Document Data Block (TIFF tag 37724)
# ---------------------------------------------------------------------------

def _parse_ps_layers(tif_path: str):
    """Parse layer geometry AND channel info from Photoshop tag 37724.

    Returns (layers_info, channel_data_offset, tag_data, endian) or None.
    """
    img = Image.open(tif_path)
    tags = getattr(img, "tag_v2", {})
    data = tags.get(37724, b"")
    if not data:
        return None

    # Find layer info marker
    marker = b"MIB8ryaL"
    mpos = data.find(marker)
    if mpos < 0:
        marker = b"8BIMLayr"
        mpos = data.find(marker)
        if mpos < 0:
            return None

    is_le = data[mpos:mpos + 4] == b"MIB8"
    endian = "<" if is_le else ">"
    pos = mpos + 8
    pos += 4  # skip block length

    count = abs(struct.unpack(f"{endian}h", data[pos:pos + 2])[0])
    pos += 2

    layers = []
    for i in range(count):
        top, left, bottom, right = struct.unpack(f"{endian}iiii", data[pos:pos + 16])
        pos += 16
        num_ch = struct.unpack(f"{endian}H", data[pos:pos + 2])[0]
        pos += 2

        channels = []
        for _ in range(num_ch):
            ch_id = struct.unpack(f"{endian}h", data[pos:pos + 2])[0]
            ch_len = struct.unpack(f"{endian}I", data[pos + 2:pos + 6])[0]
            channels.append({"id": ch_id, "len": ch_len})
            pos += 6

        pos += 8  # blend mode sig + key
        pos += 4  # opacity, clipping, flags, pad
        extra_len = struct.unpack(f"{endian}I", data[pos:pos + 4])[0]
        pos += 4 + extra_len

        w, h = right - left, bottom - top
        layers.append({
            "idx": i, "x": left, "y": top, "w": w, "h": h,
            "channels": channels,
        })

    return layers, pos, data, endian


def _extract_layer_rgba(layer: dict, data: bytes, offset: int,
                        endian: str) -> tuple[Image.Image | None, int]:
    """Extract RGBA image for a single layer from channel data."""
    w, h = layer["w"], layer["h"]
    if w <= 0 or h <= 0:
        for ch in layer["channels"]:
            offset += ch["len"]
        return None, offset

    channel_planes = {}
    for ch_info in layer["channels"]:
        ch_id = ch_info["id"]
        ch_len = ch_info["len"]
        ch_end = offset + ch_len

        comp = struct.unpack(f"{endian}H", data[offset:offset + 2])[0]
        offset += 2
        pixel_count = w * h

        if comp == 0:
            plane_data = data[offset:offset + pixel_count]
            offset += pixel_count
        elif comp == 1:
            # RLE — skip row byte count table, then decode
            offset += h * 2
            plane_data, offset = _decode_packbits(data, offset, pixel_count)
        else:
            offset = ch_end
            plane_data = bytes(pixel_count)

        raw = bytes(plane_data[:pixel_count])
        arr = np.frombuffer(raw, dtype=np.uint8)
        if len(arr) < pixel_count:
            arr = np.pad(arr, (0, pixel_count - len(arr)))
        channel_planes[ch_id] = arr.reshape((h, w))
        offset = max(offset, ch_end)

    r = channel_planes.get(0, np.zeros((h, w), dtype=np.uint8))
    g = channel_planes.get(1, np.zeros((h, w), dtype=np.uint8))
    b = channel_planes.get(2, np.zeros((h, w), dtype=np.uint8))
    a = channel_planes.get(-1, np.full((h, w), 255, dtype=np.uint8))

    rgba = np.stack([r, g, b, a], axis=-1)
    return Image.fromarray(rgba, "RGBA"), offset


# ---------------------------------------------------------------------------
# Cache: parse once, extract all layer images in one pass
# ---------------------------------------------------------------------------

_ps_cache: dict[str, dict | None] = {}


def _get_ps_data(tif_path: str) -> dict | None:
    """Get or cache extracted layer images from PS tag 37724."""
    if tif_path not in _ps_cache:
        result = _parse_ps_layers(tif_path)
        if not result:
            _ps_cache[tif_path] = None
            return None

        layers, ch_offset, data, endian = result
        images = {}
        offset = ch_offset
        for layer in layers:
            img, offset = _extract_layer_rgba(layer, data, offset, endian)
            if img:
                images[layer["idx"] + 1] = img

        _ps_cache[tif_path] = images
    return _ps_cache[tif_path]


# ---------------------------------------------------------------------------
# Public API
# ---------------------------------------------------------------------------

def identify_layers(tif_path: str) -> list[dict]:
    """Read layer geometry from a Photoshop TIF.

    Index 0 is the TIFF composite page; PS layers start at index 1.
    """
    result = _parse_ps_layers(tif_path)
    if result:
        layers, _, _, _ = result
        img = Image.open(tif_path)
        cw, ch = img.size
        composite = {"idx": 0, "w": cw, "h": ch, "x": 0, "y": 0}
        out = []
        for l in layers:
            if l["w"] > 0 and l["h"] > 0:
                out.append({"idx": l["idx"] + 1, "w": l["w"], "h": l["h"],
                            "x": l["x"], "y": l["y"]})
        return [composite] + out

    # Fallback: standard multi-page TIFF
    return _identify_layers_pillow(tif_path)


def _identify_layers_pillow(tif_path: str) -> list[dict]:
    img = Image.open(tif_path)
    layers = []
    idx = 0
    while True:
        try:
            img.seek(idx)
        except EOFError:
            break
        w, h = img.size
        layers.append({"idx": idx, "w": w, "h": h, "x": 0, "y": 0})
        idx += 1
    return layers


def classify_layer(layer: dict, canvas_w: int, canvas_h: int) -> str:
    w, h = layer["w"], layer["h"]
    if w >= canvas_w * 0.9 and h >= canvas_h * 0.9:
        return "full"
    if w < 10 or h < 20:
        return "tiny"
    return "brick"


def parse_tif(tif_path: str) -> HouseData:
    tif_path = str(tif_path)
    raw_layers = identify_layers(tif_path)

    if not raw_layers:
        raise ValueError(f"No layers found in {tif_path}")

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
            x=layer["x"], y=layer["y"],
            width=layer["w"], height=layer["h"],
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
            if layer["w"] > 350 and layer["h"] > 400:
                brick.layer_type = "window"
            else:
                brick.layer_type = "brick"
            house.bricks.append(brick)

    return house


def extract_brick_png(tif_path: str, layer_index: int, output_path: str):
    """Extract a single layer as a PNG."""
    if layer_index == 0:
        img = Image.open(tif_path).convert("RGBA")
        img.save(output_path, format="PNG")
        return

    images = _get_ps_data(tif_path)
    if images and layer_index in images:
        images[layer_index].save(output_path, format="PNG")
    else:
        # Fallback: Pillow multi-page
        img = Image.open(tif_path)
        img.seek(layer_index)
        img.convert("RGBA").save(output_path, format="PNG")


def extract_layers_batch(tif_path: str, layer_indices: list[int],
                         output_dir: str, prefix: str = "brick"):
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    # Pre-parse all layers in one pass
    _get_ps_data(tif_path)

    for idx in layer_indices:
        output_path = out / f"{prefix}_{idx:03d}.png"
        if output_path.exists():
            continue
        extract_brick_png(tif_path, idx, str(output_path))


def extract_layer_names(tif_path: str) -> list[str]:
    img = Image.open(tif_path)
    tag_data = img.tag_v2.get(37724, b"")
    if not tag_data:
        return []

    names = []
    pattern = b"\x00L\x00a\x00y\x00e\x00r\x00 "
    pos = 0
    while True:
        pos = tag_data.find(pattern, pos)
        if pos == -1:
            break
        end = pos
        while end < len(tag_data) - 1 and end < pos + 60:
            ch = struct.unpack(">H", tag_data[end:end + 2])[0]
            if 0x20 <= ch <= 0x7E:
                end += 2
            else:
                break
        name = tag_data[pos:end].decode("utf-16-be", errors="replace").strip()
        if name:
            names.append(name)
        pos += 2
    return names


def extract_all_bricks(tif_path: str, house: HouseData, output_dir: str) -> dict:
    out = Path(output_dir)
    out.mkdir(parents=True, exist_ok=True)

    manifest = {
        "source": Path(tif_path).name,
        "canvas": {"width": house.canvas_width, "height": house.canvas_height},
        "bricks": [],
    }

    if house.composite:
        extract_brick_png(tif_path, house.composite.index, str(out / "composite.png"))
        manifest["composite"] = "composite.png"

    if house.base:
        extract_brick_png(tif_path, house.base.index, str(out / "base.png"))
        manifest["base"] = "base.png"

    for brick in house.bricks:
        fname = f"brick_{brick.index:03d}.png"
        extract_brick_png(tif_path, brick.index, str(out / fname))
        manifest["bricks"].append({
            "id": brick.index, "name": brick.name, "file": fname,
            "x": brick.x, "y": brick.y,
            "width": brick.width, "height": brick.height,
            "type": brick.layer_type,
        })

    return manifest
