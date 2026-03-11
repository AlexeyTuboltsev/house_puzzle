"""
Puzzle piece merging engine.

Takes a list of bricks with positions/sizes and merges them into
puzzle pieces based on adjacency and size constraints.
"""

import random
from collections import defaultdict
from dataclasses import dataclass, field
from pathlib import Path

import numpy as np
from PIL import Image


# Adjacency threshold: bricks within this many pixels are considered neighbors
ADJACENCY_THRESHOLD = 15


@dataclass
class Brick:
    id: int
    x: int
    y: int
    width: int
    height: int
    brick_type: str = "brick"

    @property
    def right(self):
        return self.x + self.width

    @property
    def bottom(self):
        return self.y + self.height

    @property
    def area(self):
        return self.width * self.height

    @property
    def cx(self):
        return self.x + self.width / 2

    @property
    def cy(self):
        return self.y + self.height / 2


@dataclass
class PuzzlePiece:
    id: int
    brick_ids: list[int] = field(default_factory=list)
    # Bounding box (computed from constituent bricks)
    x: int = 0
    y: int = 0
    width: int = 0
    height: int = 0


def compute_border_pixels(img_path: str, brick_x: int, brick_y: int) -> set[tuple[int, int]]:
    """Compute absolute coordinates of border pixels for a brick.

    A border pixel is an opaque pixel (alpha > 30) that has at least one
    transparent neighbor (or is at the image edge).  Returns a set of
    (abs_x, abs_y) tuples in canvas coordinates.
    """
    img = Image.open(img_path).convert("RGBA")
    alpha = np.array(img.split()[3])
    opaque = alpha > 30
    h, w = opaque.shape

    if not opaque.any():
        return set()

    # Pad with False so edge pixels are automatically borders
    padded = np.zeros((h + 2, w + 2), dtype=bool)
    padded[1:-1, 1:-1] = opaque

    border_mask = opaque & (
        ~padded[:-2, 1:-1] |   # above
        ~padded[2:, 1:-1] |    # below
        ~padded[1:-1, :-2] |   # left
        ~padded[1:-1, 2:]      # right
    )

    ys, xs = np.where(border_mask)
    return set(zip((xs + brick_x).tolist(), (ys + brick_y).tolist()))


def compute_all_border_pixels(bricks: list[Brick],
                              extract_dir: str,
                              prefix: str = "brick") -> dict[int, set[tuple[int, int]]]:
    """Compute border pixels for all bricks from their extracted PNGs."""
    result = {}
    extract_path = Path(extract_dir)
    for b in bricks:
        png_path = extract_path / f"{prefix}_{b.id:03d}.png"
        if png_path.exists():
            result[b.id] = compute_border_pixels(str(png_path), b.x, b.y)
        else:
            result[b.id] = set()
    return result


def compute_brick_areas(bricks: list[Brick],
                        extract_dir: str,
                        prefix: str = "brick") -> dict[int, int]:
    """Count opaque pixels (alpha > 30) for each brick — actual pixel area."""
    result = {}
    extract_path = Path(extract_dir)
    for b in bricks:
        png_path = extract_path / f"{prefix}_{b.id:03d}.png"
        if png_path.exists():
            img = Image.open(str(png_path)).convert("RGBA")
            alpha = np.array(img.split()[3])
            result[b.id] = int((alpha > 30).sum())
        else:
            result[b.id] = b.width * b.height  # fallback to bbox
    return result


def _pixel_adjacency(border_a: set, border_b: set,
                     gap: int, min_border: int) -> bool:
    """Check if two border pixel sets share enough common border.

    Uses spatial bucketing for efficiency: bucket border_b pixels into
    a grid of cell size `gap`, then for each border_a pixel check only
    the 3x3 neighbourhood of grid cells.
    """
    if not border_a or not border_b:
        return False

    cell = max(gap, 1)
    buckets: dict[tuple[int, int], list[tuple[int, int]]] = defaultdict(list)
    for (bx, by) in border_b:
        buckets[(bx // cell, by // cell)].append((bx, by))

    count = 0
    for (ax, ay) in border_a:
        gx, gy = ax // cell, ay // cell
        found = False
        for dx in range(-1, 2):
            if found:
                break
            for dy in range(-1, 2):
                bucket = buckets.get((gx + dx, gy + dy))
                if not bucket:
                    continue
                for (bx, by) in bucket:
                    if abs(ax - bx) <= gap and abs(ay - by) <= gap:
                        found = True
                        break
                if found:
                    break
        if found:
            count += 1
            if count >= min_border:
                return True
    return False


def build_adjacency(bricks: list[Brick], gap: int = ADJACENCY_THRESHOLD,
                    min_border: int = 5,
                    border_pixels: dict[int, set] | None = None) -> dict[int, set[int]]:
    """Build adjacency graph.

    When *border_pixels* is provided (dict mapping brick id to set of
    (x, y) border pixel coordinates), adjacency is determined by actual
    pixel shapes: two bricks are adjacent if at least *min_border* border
    pixels of one are within *gap* pixels of a border pixel of the other.

    Without border_pixels, falls back to bounding-box heuristic.
    """
    adj = defaultdict(set)
    n = len(bricks)

    for i in range(n):
        a = bricks[i]
        for j in range(i + 1, n):
            b = bricks[j]

            # Fast bounding-box pre-filter (always applied)
            if not (a.x - gap < b.right and a.right + gap > b.x and
                    a.y - gap < b.bottom and a.bottom + gap > b.y):
                continue

            if border_pixels is not None:
                # Pixel-level adjacency
                ba = border_pixels.get(a.id, set())
                bb = border_pixels.get(b.id, set())
                if _pixel_adjacency(ba, bb, gap, min_border):
                    adj[a.id].add(b.id)
                    adj[b.id].add(a.id)
            else:
                # Bounding-box fallback
                h_gap = max(0, max(a.x, b.x) - min(a.right, b.right))
                v_overlap = min(a.bottom, b.bottom) - max(a.y, b.y)
                v_gap = max(0, max(a.y, b.y) - min(a.bottom, b.bottom))
                h_overlap = min(a.right, b.right) - max(a.x, b.x)

                if (h_gap <= gap and v_overlap >= min_border) or \
                   (v_gap <= gap and h_overlap >= min_border):
                    adj[a.id].add(b.id)
                    adj[b.id].add(a.id)

    return adj


def compute_piece_bbox(piece_brick_ids: list[int], bricks_by_id: dict[int, Brick]) -> tuple[int, int, int, int]:
    """Compute bounding box for a set of bricks."""
    xs = []
    ys = []
    rs = []
    bs = []
    for bid in piece_brick_ids:
        b = bricks_by_id[bid]
        xs.append(b.x)
        ys.append(b.y)
        rs.append(b.right)
        bs.append(b.bottom)
    x = min(xs)
    y = min(ys)
    return x, y, max(rs) - x, max(bs) - y


@dataclass
class MergeResult:
    pieces: list["PuzzlePiece"]


def merge_bricks(bricks: list[Brick], target_piece_count: int | None = None,
                 seed: int = 42,
                 min_border: int = 5,
                 border_pixels: dict[int, set] | None = None,
                 brick_areas: dict[int, int] | None = None) -> MergeResult:
    """
    Merge bricks into puzzle pieces using area-balanced adjacency grouping.

    Produces exactly *target_piece_count* pieces (or as close as possible
    given connectivity constraints).  All merges require real pixel adjacency
    — no bounding-box fallback.

    Algorithm:
    Phase 0 — Exclude oversized bricks whose area already exceeds the
              target area per piece; they become fixed solo pieces.
    Phase 1 — Among remaining bricks, repeatedly merge the smallest piece
              with the adjacent neighbor that brings the combined area
              closest to the (recomputed) target.
    Phase 2 — Stop when the target count is reached.
    """
    bricks_by_id = {b.id: b for b in bricks}
    adj = build_adjacency(bricks, min_border=min_border, border_pixels=border_pixels)

    # Brick areas: use pixel areas if provided, else bounding-box area
    areas = brick_areas if brick_areas else {b.id: b.area for b in bricks}

    all_ids = set(b.id for b in bricks)

    if target_piece_count is not None:
        target_count = max(1, target_piece_count)
    else:
        target_count = max(1, len(all_ids) // 3)

    # --- Phase 0: exclude oversized bricks ---
    # Any brick whose area alone >= target_area becomes a fixed solo piece.
    # Iterate until stable (excluding bricks changes target_area).
    total_area = sum(areas.get(bid, 0) for bid in all_ids)
    fixed_ids: set[int] = set()
    for _ in range(10):  # converge quickly
        target_area = total_area / max(1, target_count)
        new_fixed = set(bid for bid in all_ids if areas.get(bid, 0) >= target_area)
        if new_fixed == fixed_ids:
            break
        fixed_ids = new_fixed
        mergeable_area = sum(areas.get(bid, 0) for bid in all_ids if bid not in fixed_ids)
        target_mergeable = max(1, target_count - len(fixed_ids))
        target_area = mergeable_area / target_mergeable if target_mergeable > 0 else target_area

    mergeable_ids = all_ids - fixed_ids
    target_mergeable = max(1, target_count - len(fixed_ids))
    mergeable_area = sum(areas.get(bid, 0) for bid in mergeable_ids)
    target_area = mergeable_area / max(1, target_mergeable)

    rng = random.Random(seed)

    # Initialize: each mergeable brick is its own piece
    piece_of = {}
    pieces_dict: dict[int, list[int]] = {}
    piece_area: dict[int, int] = {}
    for bid in mergeable_ids:
        piece_of[bid] = bid
        pieces_dict[bid] = [bid]
        piece_area[bid] = areas.get(bid, 0)

    # Build piece-level adjacency (only among mergeable bricks)
    piece_adj: dict[int, set[int]] = defaultdict(set)
    for bid in mergeable_ids:
        pid = piece_of[bid]
        for nbr in adj.get(bid, set()):
            if nbr not in mergeable_ids:
                continue
            npid = piece_of[nbr]
            if npid != pid:
                piece_adj[pid].add(npid)
                piece_adj[npid].add(pid)

    def _merge(keep_pid: int, absorb_pid: int):
        """Merge absorb_pid into keep_pid, updating all bookkeeping."""
        pieces_dict[keep_pid] = pieces_dict[keep_pid] + pieces_dict[absorb_pid]
        piece_area[keep_pid] = piece_area[keep_pid] + piece_area[absorb_pid]
        for bid in pieces_dict[absorb_pid]:
            piece_of[bid] = keep_pid

        # Update piece adjacency
        for neighbor_pid in piece_adj[absorb_pid]:
            if neighbor_pid == keep_pid:
                continue
            piece_adj[neighbor_pid].discard(absorb_pid)
            piece_adj[neighbor_pid].add(keep_pid)
            piece_adj[keep_pid].add(neighbor_pid)
        piece_adj[keep_pid].discard(absorb_pid)

        del pieces_dict[absorb_pid]
        del piece_area[absorb_pid]
        del piece_adj[absorb_pid]

    # --- Phase 1: merge down to target_mergeable ---
    # Repeatedly merge the smallest piece with its best neighbor
    # (the one producing combined area closest to target_area).
    while len(pieces_dict) > target_mergeable:
        # Find the smallest piece that has at least one neighbor
        candidates = sorted(pieces_dict.keys(), key=lambda p: piece_area[p])

        merged = False
        for smallest_pid in candidates:
            neighbors = piece_adj.get(smallest_pid, set())
            if not neighbors:
                continue

            # Score each neighbor
            cur_area = piece_area[smallest_pid]
            best_nbr = None
            best_score = float('inf')
            nbr_list = list(neighbors)
            rng.shuffle(nbr_list)

            for npid in nbr_list:
                if npid not in pieces_dict:
                    continue
                combined = cur_area + piece_area[npid]
                score = abs(combined - target_area)
                if combined > target_area * 1.5:
                    score += combined  # penalise way-over-target
                if score < best_score:
                    best_score = score
                    best_nbr = npid

            if best_nbr is not None:
                _merge(smallest_pid, best_nbr)
                merged = True
                break

        if not merged:
            # No more merges possible (disconnected components)
            break

    # --- Build result: fixed solo pieces + merged pieces ---
    result_pieces: list[PuzzlePiece] = []

    for bid in sorted(fixed_ids):
        b = bricks_by_id[bid]
        result_pieces.append(PuzzlePiece(
            id=len(result_pieces),
            brick_ids=[bid],
            x=b.x, y=b.y, width=b.width, height=b.height,
        ))

    for pid, brick_ids in pieces_dict.items():
        x, y, w, h = compute_piece_bbox(brick_ids, bricks_by_id)
        result_pieces.append(PuzzlePiece(
            id=len(result_pieces),
            brick_ids=brick_ids,
            x=x, y=y, width=w, height=h,
        ))

    for i, piece in enumerate(result_pieces):
        piece.id = i

    return MergeResult(pieces=result_pieces)


def pieces_to_json(pieces: list[PuzzlePiece], bricks_by_id: dict[int, Brick]) -> list[dict]:
    """Convert pieces to JSON-serializable format."""
    result = []
    for piece in pieces:
        brick_data = []
        for bid in piece.brick_ids:
            b = bricks_by_id[bid]
            brick_data.append({
                "id": b.id,
                "x": b.x, "y": b.y,
                "width": b.width, "height": b.height,
                "type": b.brick_type,
            })
        result.append({
            "id": piece.id,
            "x": piece.x,
            "y": piece.y,
            "width": piece.width,
            "height": piece.height,
            "brick_ids": piece.brick_ids,
            "bricks": brick_data,
            "num_bricks": len(piece.brick_ids),
        })
    return result
