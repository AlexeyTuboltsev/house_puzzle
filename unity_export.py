"""
Unity-compatible export for House Puzzle Editor.

Generates house_data.json with:
- blocks (name, position in Unity world coords)
- colliders (polygon paths in local sprite coords)
- steps (wave → block indices)

Coordinate conventions:
- Unity Y-up, canvas Y-down → flip Y
- Unity units = pixels / PPU (auto-calculated as canvas_width / 12)
- Sprite pivot = center → collider paths relative to sprite center
"""

import math

import numpy as np
from PIL import Image


# ---------------------------------------------------------------------------
# Contour tracing — grid-cell boundary following
# ---------------------------------------------------------------------------

def trace_alpha_contours(
    img: Image.Image, alpha_threshold: int = 191
) -> list[list[tuple[int, int]]]:
    """Trace closed contour polygons from the alpha channel of an RGBA image.

    Uses a grid-cell boundary following approach:
    1. Pad the binary mask with zeros.
    2. Find boundary cells (solid cells adjacent to empty).
    3. Follow the boundary using Moore neighborhood tracing.

    Returns a list of contour loops, each a list of (x, y) vertex coordinates
    on the pixel grid. Multiple loops = outer boundary + holes.
    """
    alpha = np.array(img.split()[3])
    h, w = alpha.shape
    solid = alpha > alpha_threshold

    # Pad with zeros so boundary detection works at image edges
    padded = np.zeros((h + 2, w + 2), dtype=bool)
    padded[1:-1, 1:-1] = solid

    # Find all boundary cells: solid cells with at least one non-solid 4-neighbor
    is_boundary = np.zeros_like(padded)
    is_boundary[1:-1, 1:-1] = (
        padded[1:-1, 1:-1] & (
            ~padded[:-2, 1:-1] | ~padded[2:, 1:-1] |
            ~padded[1:-1, :-2] | ~padded[1:-1, 2:]
        )
    )

    # Moore neighborhood: 8 directions (clockwise from right)
    #   0=right, 1=down-right, 2=down, 3=down-left,
    #   4=left, 5=up-left, 6=up, 7=up-right
    moore_dx = [1, 1, 0, -1, -1, -1, 0, 1]
    moore_dy = [0, 1, 1, 1, 0, -1, -1, -1]

    visited_starts = set()
    contours = []

    # Scan for boundary cells top-to-bottom, left-to-right
    for start_y in range(padded.shape[0]):
        for start_x in range(padded.shape[1]):
            if not is_boundary[start_y, start_x]:
                continue
            if (start_y, start_x) in visited_starts:
                continue

            # Moore boundary tracing
            loop = []
            cx, cy = start_x, start_y
            # Start looking from the left (the cell we came from is to the left)
            back_dir = 4  # came from left

            first_visit = True
            while True:
                if not first_visit and cx == start_x and cy == start_y:
                    break
                first_visit = False

                loop.append((cx - 1, cy - 1))  # undo padding offset
                is_boundary[cy, cx] = False  # mark visited

                # Search Moore neighborhood starting from back_dir + 1
                found = False
                for i in range(1, 9):
                    d = (back_dir + i) % 8
                    nx = cx + moore_dx[d]
                    ny = cy + moore_dy[d]
                    if (0 <= nx < padded.shape[1] and 0 <= ny < padded.shape[0]
                            and padded[ny, nx]):
                        back_dir = (d + 4) % 8
                        cx, cy = nx, ny
                        found = True
                        break

                if not found:
                    break

            if len(loop) >= 3:
                visited_starts.add((start_y, start_x))
                contours.append(loop)

    return contours


# ---------------------------------------------------------------------------
# Douglas-Peucker simplification
# ---------------------------------------------------------------------------

def _perpendicular_distance(p, a, b):
    """Distance from point p to line segment a-b."""
    ax, ay = a
    bx, by = b
    dx, dy = bx - ax, by - ay
    len_sq = dx * dx + dy * dy
    if len_sq == 0:
        return math.sqrt((p[0] - ax) ** 2 + (p[1] - ay) ** 2)
    return abs(dx * (ay - p[1]) - dy * (ax - p[0])) / math.sqrt(len_sq)


def douglas_peucker(points: list, epsilon: float) -> list:
    """Simplify an open polyline using Douglas-Peucker."""
    if len(points) <= 2:
        return points

    max_dist = 0
    max_idx = 0
    for i in range(1, len(points) - 1):
        d = _perpendicular_distance(points[i], points[0], points[-1])
        if d > max_dist:
            max_dist = d
            max_idx = i

    if max_dist > epsilon:
        left = douglas_peucker(points[: max_idx + 1], epsilon)
        right = douglas_peucker(points[max_idx:], epsilon)
        return left[:-1] + right
    return [points[0], points[-1]]


def douglas_peucker_closed(points: list, epsilon: float) -> list:
    """Simplify a closed polygon using Douglas-Peucker.

    Splits at the two farthest-apart points, simplifies each half, rejoins.
    """
    if len(points) <= 4 or epsilon <= 0:
        return points

    # Find two farthest-apart points
    max_dist = 0
    idx_a, idx_b = 0, 1
    for i in range(len(points)):
        for j in range(i + 1, len(points)):
            d = (points[i][0] - points[j][0]) ** 2 + (points[i][1] - points[j][1]) ** 2
            if d > max_dist:
                max_dist = d
                idx_a, idx_b = i, j

    half1 = points[idx_a: idx_b + 1]
    half2 = points[idx_b:] + points[: idx_a + 1]

    s1 = douglas_peucker(half1, epsilon)
    s2 = douglas_peucker(half2, epsilon)

    return s1[:-1] + s2[:-1]


# ---------------------------------------------------------------------------
# Coordinate conversion
# ---------------------------------------------------------------------------

def pixel_to_unity_position(
    px_x: float, px_y: float, canvas_height: int, ppu: int = 100
) -> dict:
    """Convert pixel center to Unity world position (flip Y)."""
    return {
        "x": round(px_x / ppu, 6),
        "y": round((canvas_height - px_y) / ppu, 6),
        "z": 0.0,
    }


# ---------------------------------------------------------------------------
# Overlap resolution — midline boundary between adjacent pieces
# ---------------------------------------------------------------------------

def _point_in_polygon(px: float, py: float, polygon: list) -> bool:
    """Ray-casting point-in-polygon test."""
    n = len(polygon)
    inside = False
    j = n - 1
    for i in range(n):
        xi, yi = polygon[i]
        xj, yj = polygon[j]
        if ((yi > py) != (yj > py)) and \
           (px < (xj - xi) * (py - yi) / (yj - yi) + xi):
            inside = not inside
        j = i
    return inside


def _nearest_point_on_segment(px, py, x1, y1, x2, y2):
    """Nearest point on line segment (x1,y1)-(x2,y2) to point (px,py)."""
    dx, dy = x2 - x1, y2 - y1
    len_sq = dx * dx + dy * dy
    if len_sq < 1e-12:
        return x1, y1
    t = max(0.0, min(1.0, ((px - x1) * dx + (py - y1) * dy) / len_sq))
    return x1 + t * dx, y1 + t * dy


def _nearest_point_on_polygon_boundary(px, py, polygon):
    """Find nearest point on the closed polygon boundary to (px, py)."""
    best_dist_sq = float('inf')
    best_pt = polygon[0]
    n = len(polygon)
    for i in range(n):
        x1, y1 = polygon[i]
        x2, y2 = polygon[(i + 1) % n]
        nx, ny = _nearest_point_on_segment(px, py, x1, y1, x2, y2)
        d_sq = (px - nx) ** 2 + (py - ny) ** 2
        if d_sq < best_dist_sq:
            best_dist_sq = d_sq
            best_pt = (nx, ny)
    return best_pt


def _bbox(polygon):
    """Bounding box: (min_x, min_y, max_x, max_y)."""
    xs = [p[0] for p in polygon]
    ys = [p[1] for p in polygon]
    return min(xs), min(ys), max(xs), max(ys)


def _bboxes_overlap(bb1, bb2):
    """Check if two bounding boxes overlap."""
    return (bb1[0] <= bb2[2] and bb1[2] >= bb2[0] and
            bb1[1] <= bb2[3] and bb1[3] >= bb2[1])


def resolve_collider_overlaps(
    pieces: list,
    piece_contours_local: dict,
    scale: float = 1.0,
) -> dict:
    """Adjust contour vertices so adjacent pieces share a midline boundary.

    For each vertex of piece A that lies inside piece B, find the nearest
    point on B's boundary and move the vertex to the midpoint.  All
    adjustments are computed from the *original* polygons and applied in
    one pass, so processing order doesn't matter.

    Works in canvas pixel space; returns adjusted contours in local piece
    image coords (same coordinate system as the input).

    See UNITY_INTEGRATION.md §5 for why this is needed.
    """
    # Build canvas-space contours with bboxes
    canvas_data = {}
    for piece in pieces:
        contours = piece_contours_local.get(piece.id)
        if not contours:
            continue
        ox = piece.x * scale
        oy = piece.y * scale
        canvas_contours = []
        bboxes = []
        for contour in contours:
            cpts = [(ox + x, oy + y) for x, y in contour]
            canvas_contours.append(cpts)
            bboxes.append(_bbox(cpts))
        canvas_data[piece.id] = {
            "contours": canvas_contours,
            "bboxes": bboxes,
            "ox": ox,
            "oy": oy,
        }

    piece_ids = list(canvas_data.keys())

    # Collect all adjustments: (pid, contour_idx, vertex_idx) → (new_x, new_y)
    adjustments = {}

    for pid in piece_ids:
        data = canvas_data[pid]
        for ci, contour in enumerate(data["contours"]):
            bb_a = data["bboxes"][ci]
            for vi, (vx, vy) in enumerate(contour):
                best_dist_sq = float("inf")
                best_mid = None

                for other_pid in piece_ids:
                    if other_pid == pid:
                        continue
                    other = canvas_data[other_pid]
                    for oci, other_contour in enumerate(other["contours"]):
                        if not _bboxes_overlap(bb_a, other["bboxes"][oci]):
                            continue
                        if not _point_in_polygon(vx, vy, other_contour):
                            continue
                        nx, ny = _nearest_point_on_polygon_boundary(
                            vx, vy, other_contour
                        )
                        d_sq = (vx - nx) ** 2 + (vy - ny) ** 2
                        if d_sq < best_dist_sq:
                            best_dist_sq = d_sq
                            best_mid = ((vx + nx) / 2.0, (vy + ny) / 2.0)

                if best_mid is not None:
                    adjustments[(pid, ci, vi)] = best_mid

    # Apply adjustments, convert back to local piece coords
    result = {}
    for pid in piece_ids:
        data = canvas_data[pid]
        ox, oy = data["ox"], data["oy"]
        adjusted = []
        for ci, contour in enumerate(data["contours"]):
            new_pts = []
            for vi, (vx, vy) in enumerate(contour):
                if (pid, ci, vi) in adjustments:
                    ax, ay = adjustments[(pid, ci, vi)]
                    new_pts.append((ax - ox, ay - oy))
                else:
                    new_pts.append((vx - ox, vy - oy))
            adjusted.append(new_pts)
        result[pid] = adjusted

    # Pieces without contours keep their originals
    for piece in pieces:
        if piece.id not in result:
            result[piece.id] = piece_contours_local.get(piece.id, [])

    return result


def contour_to_collider_path(
    contour: list[tuple[float, float]],
    piece_width: int,
    piece_height: int,
    ppu: int = 100,
) -> list[dict]:
    """Convert contour from local pixel coords to Unity PolygonCollider2D local coords.

    Sprite pivot = center, so:
      local_x = (px - width/2) / ppu
      local_y = (height/2 - py) / ppu   (flip Y)
    """
    cx = piece_width / 2.0
    cy = piece_height / 2.0
    return [
        {
            "x": round((p[0] - cx) / ppu, 6),
            "y": round((cy - p[1]) / ppu, 6),
        }
        for p in contour
    ]


# ---------------------------------------------------------------------------
# Main orchestrator
# ---------------------------------------------------------------------------

def _auto_waves_by_y(
    blocks: list[dict], piece_ids: list[int], num_waves: int = 3
) -> list[dict]:
    """Split piece IDs into waves by ascending block Y position.

    Bottom blocks (lowest Y) go into wave 1 so they form a physical
    foundation before upper blocks become available.
    """
    # Sort by Y position (ascending = bottom first)
    sorted_ids = sorted(piece_ids, key=lambda i: blocks[i]["position"]["y"])
    wave_size = max(1, len(sorted_ids) // num_waves)
    steps = []
    for w in range(num_waves):
        start = w * wave_size
        end = start + wave_size if w < num_waves - 1 else len(sorted_ids)
        if start >= len(sorted_ids):
            break
        steps.append({
            "wave": w + 1,
            "blockIndices": sorted_ids[start:end],
        })
    return steps


def build_house_data(
    pieces: list,
    bricks_by_id: dict,
    canvas_width: int,
    canvas_height: int,
    waves: list,  # [{"wave": 1, "pieceIds": [0, 3, 7]}, ...]
    ppu: int = 50,
    scale: float = 1.0,
    location: str = "Rome",
    position_in_location: int = 0,
    house_name: str = "NewHouse",
    spacing: float = 12.0,  # gap before this house in scrollable list (Unity units)
    piece_images: dict | None = None,  # {piece_id: PIL Image} for collider gen
    dp_epsilon: float = 0.5,  # Douglas-Peucker simplification tolerance (pixels)
    groups: list | None = None,  # [{"pieceIds": [0, 3]}, ...] — pieces that move together
) -> dict:
    """Build the complete house_data.json dict for Unity import.

    Args:
        pieces: list of PuzzlePiece objects
        bricks_by_id: dict mapping brick_id -> Brick
        canvas_width, canvas_height: full TIF dimensions in pixels (original)
        waves: wave/step data from frontend
        ppu: pixels per Unity unit (applied after scaling)
        scale: factor applied to all pixel coords (sprites are resized by this)
    """
    # Apply scale to canvas dimensions (sprites are resized in the export)
    scaled_w = round(canvas_width * scale)
    scaled_h = round(canvas_height * scale)
    blocks = []

    # Center X on canvas midpoint; Y bottom-aligned (Y=0 at canvas bottom)
    canvas_center_x = scaled_w / 2.0 / ppu

    for piece in pieces:
        name = f"piece_{piece.id:03d}"

        # Position = center of bounding box in scaled pixel coords
        center_px_x = (piece.x + piece.width / 2.0) * scale
        center_px_y = (piece.y + piece.height / 2.0) * scale
        abs_pos = pixel_to_unity_position(
            center_px_x, center_px_y, scaled_h, ppu
        )
        position = {
            "x": round(abs_pos["x"] - canvas_center_x, 6),
            "y": round(abs_pos["y"], 6),  # bottom-aligned, no Y offset
            "z": 0.0,
        }

        # Determine isChimney from brick types
        brick_types = set()
        for bid in piece.brick_ids:
            b = bricks_by_id.get(bid)
            if b and hasattr(b, "layer_type"):
                brick_types.add(b.layer_type)
        is_chimney = "chimney" in brick_types

        blocks.append({
            "name": name,
            "position": position,
            "orderInLayer": 0,
            "isChimney": is_chimney,
        })

    # Build steps from waves
    # piece.id == index in blocks array (pieces are ordered 0..N-1)
    # All pieces must be in a step; unassigned pieces go into a final step
    all_piece_ids = set(range(len(pieces)))
    assigned_ids = set()
    steps = []
    for w in waves:
        piece_ids = w.get("pieceIds", [])
        assigned_ids.update(piece_ids)
        steps.append({
            "wave": w.get("wave", len(steps) + 1),
            "blockIndices": piece_ids,
        })
    unassigned = sorted(all_piece_ids - assigned_ids)
    if unassigned:
        if not steps:
            # Auto-assign waves by Y position (bottom → top).
            # The game requires blocks to be physically supported during
            # placement: bottom blocks must form a foundation before upper
            # blocks become available.  Split into 3 waves by Y terciles.
            steps = _auto_waves_by_y(blocks, unassigned, num_waves=3)
        else:
            # Add unassigned pieces to the last step
            steps[-1]["blockIndices"] = steps[-1]["blockIndices"] + unassigned

    # Generate colliders from piece images (trace + simplify)
    # Collider generation: trace pixel-accurate contours, convert to Unity coords.
    # Raw contours (~400-500 vertices) follow piece boundaries closely, so adjacent
    # pieces overlap by at most ~1-2px. The physics push from this is negligible.
    # This matches the PSD pipeline which also uses pixel-level contours.
    colliders = []
    if piece_images:
        for piece in pieces:
            img = piece_images.get(piece.id)
            if img is None:
                colliders.append({"paths": []})
                continue
            contours = trace_alpha_contours(img)
            paths = []
            for contour in contours:
                if len(contour) < 3:
                    continue
                # Simplify slightly to reduce vertex count without losing accuracy
                simplified = douglas_peucker_closed(contour, dp_epsilon)
                if len(simplified) < 3:
                    continue
                points = contour_to_collider_path(simplified, img.width, img.height, ppu)
                paths.append({"points": points})
            colliders.append({"paths": paths})

    # Compute ground offset: how far the lowest collider point is above Y=0.
    # The importer shifts the house down by this amount so bottom blocks touch ground.
    ground_offset = 0.0
    if blocks and colliders:
        min_world_bottom = float("inf")
        for b, c in zip(blocks, colliders):
            if not c.get("paths"):
                continue
            col_min_y = min(
                p["y"] for path in c["paths"] for p in path["points"]
            )
            world_bottom = b["position"]["y"] + col_min_y
            if world_bottom < min_world_bottom:
                min_world_bottom = world_bottom
        if min_world_bottom < float("inf"):
            ground_offset = min_world_bottom

    # ScalingFactor controls UI tray piece sizing (multiplied by sprite pixel size).
    # Compute from average sprite width to target ~220px in the tray.
    # Existing houses: SF=1 with ~200px sprites, SF=2 with ~110px sprites.
    if piece_images:
        avg_w = sum(img.width for img in piece_images.values()) / len(piece_images)
        scaling_factor = max(1.0, round(220.0 / avg_w))
    else:
        scaling_factor = 2.0

    # SameBlocksSettings: each entry is the list of block indices in the same group.
    # Pieces in the same editor group get identical lists; ungrouped pieces get [i].
    # piece.id == index in blocks array.
    piece_to_group: dict[int, list[int]] = {}
    if groups:
        for g in groups:
            ids = g.get("pieceIds", [])
            for pid in ids:
                piece_to_group[pid] = ids
    same_blocks_settings = []
    for piece in pieces:
        group_ids = piece_to_group.get(piece.id)
        same_blocks_settings.append(group_ids if group_ids is not None else [piece.id])

    # Spacing is passed as a parameter (default 12.0 Unity units).
    # Caller can override for first-in-location (0) or custom gaps.

    result = {
        "ppu": ppu,
        "scalingFactor": scaling_factor,
        "spacing": spacing,
        "reward": 100,
        "canvas": {"width": scaled_w, "height": scaled_h},
        "groundOffset": round(ground_offset, 6),
        "placement": {
            "location": location,
            "position": position_in_location,
            "houseName": house_name,
        },
        "blocks": blocks,
        "steps": steps,
        "spriteFolder": "pieces/",
        "sameBlocksSettings": same_blocks_settings,
    }
    if colliders:
        result["colliders"] = colliders
    return result
