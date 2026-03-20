"""
Unity-compatible export for House Puzzle Editor.

Generates house_data.json with:
- blocks (name, position in Unity world coords)
- colliders (polygon paths in local sprite coords)
- steps (wave → block indices)

Coordinate conventions:
- Unity Y-up, canvas Y-down → flip Y
- Unity units = pixels / PPU (default 100)
- Sprite pivot = center → collider paths relative to sprite center
"""

import math

import numpy as np
from PIL import Image


# ---------------------------------------------------------------------------
# Contour tracing — grid-cell boundary following
# ---------------------------------------------------------------------------

def trace_alpha_contours(
    img: Image.Image, alpha_threshold: int = 30
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

def build_house_data(
    pieces: list,
    bricks_by_id: dict,
    canvas_width: int,
    canvas_height: int,
    piece_images: dict,  # piece_id -> PIL Image
    waves: list,  # [{"wave": 1, "pieceIds": [0, 3, 7]}, ...]
    ppu: int = 100,
    epsilon: float = 1.5,
    location: str = "Rome",
    position_in_location: int = 0,
    house_name: str = "NewHouse",
) -> dict:
    """Build the complete house_data.json dict for Unity import.

    Args:
        pieces: list of PuzzlePiece objects
        bricks_by_id: dict mapping brick_id -> Brick
        canvas_width, canvas_height: full TIF dimensions in pixels
        piece_images: dict mapping piece_id -> PIL RGBA Image (cropped to piece bbox)
        waves: wave/step data from frontend
        ppu: pixels per Unity unit (default 100)
        epsilon: Douglas-Peucker simplification tolerance in pixels
    """
    blocks = []
    colliders = []

    for piece in pieces:
        name = f"piece_{piece.id:03d}"

        # Position = center of bounding box in Unity world coords
        center_px_x = piece.x + piece.width / 2.0
        center_px_y = piece.y + piece.height / 2.0
        position = pixel_to_unity_position(
            center_px_x, center_px_y, canvas_height, ppu
        )

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
            "isChimney": is_chimney,
        })

        # Generate polygon collider from piece image
        img = piece_images.get(piece.id)
        paths = []
        if img is not None:
            raw_contours = trace_alpha_contours(img)
            for contour in raw_contours:
                simplified = douglas_peucker_closed(contour, epsilon)
                if len(simplified) >= 3:
                    points = contour_to_collider_path(
                        simplified, piece.width, piece.height, ppu
                    )
                    paths.append({"Points": points})

        colliders.append({
            "name": name,
            "offset": {"x": 0.0, "y": 0.0},
            "paths": paths,
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
            steps.append({"wave": 1, "blockIndices": unassigned})
        else:
            # Add unassigned pieces to the last step
            steps[-1]["blockIndices"] = steps[-1]["blockIndices"] + unassigned

    return {
        "ppu": ppu,
        "canvas": {"width": canvas_width, "height": canvas_height},
        "placement": {
            "location": location,
            "position": position_in_location,
            "houseName": house_name,
        },
        "blocks": blocks,
        "colliders": colliders,
        "steps": steps,
        "spriteFolder": "pieces/",
    }
