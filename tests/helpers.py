"""Shared helpers for e2e test suite.

Brick IDs are random UUIDs per session, so comparisons use position-based
matching: each brick is keyed by (x, y, width, height) which is unique.
Piece IDs are deterministic ("p0", "p1", ...) but brick_ids within pieces
are UUIDs, so those are also normalized to position keys.
"""

import hashlib
import json
import urllib.request
from pathlib import Path

BASE_URL = "http://localhost:5050"
BASELINES_DIR = Path(__file__).parent / "baselines"


def api_post(path, data):
    """POST JSON to the server and return parsed response."""
    req = urllib.request.Request(
        f"{BASE_URL}{path}",
        data=json.dumps(data).encode(),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(req, timeout=600) as resp:
        return json.loads(resp.read())


def api_get_png_hash(path):
    """GET a PNG from the server and return its SHA256 hash."""
    with urllib.request.urlopen(f"{BASE_URL}{path}", timeout=60) as resp:
        return hashlib.sha256(resp.read()).hexdigest()


def _brick_pos_key(b):
    """Position-based key for a brick dict. Unique per file."""
    return f"{b['x']},{b['y']},{b['width']},{b['height']}"


def extract_load_snapshot(resp, file_stem=""):
    """Extract stable fields from load_pdf response + PNG hashes.

    Brick IDs are normalized to position keys so snapshots are
    comparable across runs (UUIDs change each time).
    """
    # Build UUID -> position-key mapping for this response
    uuid_to_pos = {}
    for b in resp["bricks"]:
        uuid_to_pos[b["id"]] = _brick_pos_key(b)

    bricks = []
    for b in resp["bricks"]:
        bricks.append({
            "pos_key": _brick_pos_key(b),
            "x": b["x"],
            "y": b["y"],
            "width": b["width"],
            "height": b["height"],
            "type": b.get("type", ""),
            "neighbors": sorted(uuid_to_pos.get(n, str(n)) for n in b.get("neighbors", [])),
        })

    # Capture pixel-level hashes for composite/outlines/lights/background PNGs
    # Use the session key from the response if available, else legacy routes
    key = resp.get("key")
    pfx = f"/api/s/{key}" if key else "/api"
    png_hashes = {}
    for name in ["composite", "outlines", "lights", "background"]:
        try:
            png_hashes[name] = api_get_png_hash(f"{pfx}/{name}.png?f={file_stem}")
        except Exception:
            pass
    # Brick PNGs keyed by position
    for b in resp["bricks"]:
        try:
            png_hashes[f"brick_{_brick_pos_key(b)}"] = api_get_png_hash(f"{pfx}/brick/{b['id']}.png")
        except Exception:
            pass

    return {
        "canvas": resp["canvas"],
        "total_layers": resp.get("total_layers", 0),
        "num_bricks": resp.get("num_bricks", len(resp["bricks"])),
        "render_dpi": resp.get("render_dpi", 0),
        "houseUnitsHigh": resp.get("houseUnitsHigh", 0),
        "has_base": resp.get("has_base", False),
        "bricks": sorted(bricks, key=lambda b: b["pos_key"]),
        "png_hashes": png_hashes,
    }


def extract_merge_snapshot(resp, uuid_to_pos=None):
    """Extract stable fields from merge response.

    If uuid_to_pos is provided, brick_ids are normalized to position keys.
    Piece IDs are deterministic ("p0", "p1", ...) so kept as-is.
    """
    pieces = []
    for p in resp["pieces"]:
        brick_ids = p["brick_ids"]
        if uuid_to_pos:
            brick_ids = sorted(uuid_to_pos.get(bid, bid) for bid in brick_ids)
        else:
            brick_ids = sorted(brick_ids)
        pieces.append({
            "id": p["id"],
            "x": p["x"],
            "y": p["y"],
            "width": p["width"],
            "height": p["height"],
            "brick_ids": brick_ids,
            "num_bricks": p.get("num_bricks", len(p["brick_ids"])),
        })
    return {
        "num_pieces": resp["num_pieces"],
        "pieces": sorted(pieces, key=lambda p: p["id"]),
    }


def compare_load(actual, baseline):
    """Return list of differences between actual and baseline load snapshots."""
    diffs = []
    if actual["canvas"] != baseline["canvas"]:
        diffs.append(f"canvas: {actual['canvas']} != {baseline['canvas']}")
    for key in ["total_layers", "num_bricks", "render_dpi", "houseUnitsHigh", "has_base"]:
        if actual.get(key) != baseline.get(key):
            diffs.append(f"{key}: {actual.get(key)} != {baseline.get(key)}")
    if len(actual["bricks"]) != len(baseline["bricks"]):
        diffs.append(f"brick count: {len(actual['bricks'])} != {len(baseline['bricks'])}")
    else:
        a_map = {b["pos_key"]: b for b in actual["bricks"]}
        b_map = {b["pos_key"]: b for b in baseline["bricks"]}
        for pk in sorted(b_map.keys()):
            if pk not in a_map:
                diffs.append(f"brick at {pk} missing")
            else:
                for field in ["x", "y", "width", "height", "type"]:
                    if a_map[pk][field] != b_map[pk][field]:
                        diffs.append(f"brick {pk}.{field}: {a_map[pk][field]} != {b_map[pk][field]}")
                if a_map[pk].get("neighbors", []) != b_map[pk].get("neighbors", []):
                    diffs.append(f"brick {pk} neighbors differ")
    # PNG pixel-level comparison
    a_hashes = actual.get("png_hashes", {})
    b_hashes = baseline.get("png_hashes", {})
    for name in sorted(set(list(a_hashes.keys()) + list(b_hashes.keys()))):
        ah = a_hashes.get(name)
        bh = b_hashes.get(name)
        if ah is None:
            diffs.append(f"png {name}: missing in actual")
        elif bh is None:
            diffs.append(f"png {name}: missing in baseline")
        elif ah != bh:
            diffs.append(f"png {name}: hash mismatch")
    return diffs


def compare_merge(actual, baseline):
    """Return list of differences between actual and baseline merge snapshots."""
    diffs = []
    if actual["num_pieces"] != baseline["num_pieces"]:
        diffs.append(f"num_pieces: {actual['num_pieces']} != {baseline['num_pieces']}")
    a_map = {p["id"]: p for p in actual["pieces"]}
    b_map = {p["id"]: p for p in baseline["pieces"]}
    for pid in sorted(b_map.keys()):
        if pid not in a_map:
            diffs.append(f"piece {pid} missing")
        else:
            for field in ["x", "y", "width", "height", "num_bricks"]:
                if a_map[pid][field] != b_map[pid][field]:
                    diffs.append(f"piece {pid}.{field}: {a_map[pid][field]} != {b_map[pid][field]}")
            if a_map[pid]["brick_ids"] != b_map[pid]["brick_ids"]:
                diffs.append(f"piece {pid} brick_ids differ")
    return diffs
