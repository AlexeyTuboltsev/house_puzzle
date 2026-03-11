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


# Default size constraints (pixels) — configurable via merge params
DEFAULT_MAX_WIDTH = 800
DEFAULT_MAX_HEIGHT = 600

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


def would_exceed_limits(piece_bricks: list[int], candidate_id: int,
                        bricks_by_id: dict[int, Brick],
                        max_w: int = DEFAULT_MAX_WIDTH,
                        max_h: int = DEFAULT_MAX_HEIGHT) -> bool:
    """Check if adding candidate brick would make the piece exceed max size."""
    test_ids = piece_bricks + [candidate_id]
    _, _, w, h = compute_piece_bbox(test_ids, bricks_by_id)
    return w > max_w or h > max_h


@dataclass
class MergeResult:
    pieces: list["PuzzlePiece"]
    oversized: list[int]  # piece IDs that exceed pixel dimensions


def merge_bricks(bricks: list[Brick], target_piece_count: int | None = None,
                 seed: int = 42, windows_separate: bool = True,
                 max_width: int = DEFAULT_MAX_WIDTH,
                 max_height: int = DEFAULT_MAX_HEIGHT,
                 min_bricks: int = 1,
                 max_bricks: int = 0,
                 min_border: int = 5,
                 border_pixels: dict[int, set] | None = None,
                 attempts: int = 5) -> MergeResult:
    """
    Merge bricks into puzzle pieces using random adjacency-based grouping.

    Tries *attempts* seed variations and picks the result with the fewest
    pieces that exceed the pixel dimension limits.  Brick-count constraints
    (min_bricks / max_bricks) are always enforced; pixel dimensions are
    soft — the best-effort result is returned with a list of oversized
    piece IDs so the UI can warn the user.
    """
    bricks_by_id = {b.id: b for b in bricks}
    adj = build_adjacency(bricks, min_border=min_border, border_pixels=border_pixels)

    # Separate windows/doors if requested
    fixed_pieces = []
    mergeable_ids = set()

    for b in bricks:
        if windows_separate and b.brick_type in ("window", "door"):
            fixed_pieces.append(PuzzlePiece(
                id=len(fixed_pieces),
                brick_ids=[b.id],
                x=b.x, y=b.y, width=b.width, height=b.height,
            ))
        else:
            mergeable_ids.add(b.id)

    # Target count for mergeable pieces
    initial_count = len(mergeable_ids)
    if target_piece_count is not None:
        target_mergeable = max(1, target_piece_count - len(fixed_pieces))
    else:
        target_mergeable = max(1, initial_count // 3)

    best_result: MergeResult | None = None

    for attempt in range(attempts):
        rng = random.Random(seed + attempt)

        # Initialize: each mergeable brick is its own piece
        piece_of = {}
        pieces_dict: dict[int, list[int]] = {}
        for bid in mergeable_ids:
            piece_of[bid] = bid
            pieces_dict[bid] = [bid]

        # --- Phase 1: merge toward target, respecting max_bricks + pixel dims ---
        for _ in range(50):
            if len(pieces_dict) <= target_mergeable:
                break

            all_edges = []
            for bid in mergeable_ids:
                for nbr in adj.get(bid, set()):
                    if nbr in mergeable_ids and bid < nbr:
                        pa, pb = piece_of[bid], piece_of[nbr]
                        if pa != pb:
                            all_edges.append((bid, nbr))
            if not all_edges:
                break

            rng.shuffle(all_edges)
            merged_any = False

            for a_id, b_id in all_edges:
                if len(pieces_dict) <= target_mergeable:
                    break
                pa, pb = piece_of[a_id], piece_of[b_id]
                if pa == pb:
                    continue

                merged_ids = pieces_dict[pa] + pieces_dict[pb]
                if max_bricks > 0 and len(merged_ids) > max_bricks:
                    continue
                _, _, w, h = compute_piece_bbox(merged_ids, bricks_by_id)
                if w > max_width or h > max_height:
                    continue

                pieces_dict[pa] = merged_ids
                for bid in pieces_dict[pb]:
                    piece_of[bid] = pa
                del pieces_dict[pb]
                merged_any = True

            if not merged_any:
                break

        # --- Phase 2: enforce min_bricks (ignore pixel dims, only max_bricks) ---
        if min_bricks > 1:
            changed = True
            while changed:
                changed = False
                for pid in list(pieces_dict.keys()):
                    if pid not in pieces_dict:
                        continue
                    brick_ids = pieces_dict[pid]
                    if len(brick_ids) >= min_bricks:
                        continue

                    # Find neighboring pieces
                    neighbor_pids = set()
                    for bid in brick_ids:
                        for nbr in adj.get(bid, set()):
                            if nbr in mergeable_ids and piece_of.get(nbr) != pid:
                                neighbor_pids.add(piece_of[nbr])

                    # Pick smallest neighbor that fits max_bricks; if none,
                    # pick the smallest neighbor anyway (min_bricks is hard)
                    best_target = None
                    best_size = float('inf')
                    fallback_target = None
                    fallback_size = float('inf')
                    for npid in neighbor_pids:
                        if npid not in pieces_dict:
                            continue
                        nsize = len(pieces_dict[npid])
                        candidate_size = nsize + len(brick_ids)
                        if max_bricks > 0 and candidate_size > max_bricks:
                            # Track as fallback
                            if nsize < fallback_size:
                                fallback_size = nsize
                                fallback_target = npid
                            continue
                        if nsize < best_size:
                            best_size = nsize
                            best_target = npid
                    if best_target is None:
                        best_target = fallback_target

                    # If no adjacent mergeable piece found (e.g. only
                    # neighbors are fixed windows), find the nearest
                    # piece by minimum brick-edge distance — but only
                    # if close enough (3× adjacency gap) to avoid
                    # creating disjoint pieces.
                    if best_target is None and not neighbor_pids:
                        max_dist = ADJACENCY_THRESHOLD * 3
                        best_dist = float('inf')
                        for opid, obids in pieces_dict.items():
                            if opid == pid:
                                continue
                            for bid_a in brick_ids:
                                ba = bricks_by_id[bid_a]
                                for bid_b in obids:
                                    bb = bricks_by_id[bid_b]
                                    dx = max(0, max(ba.x, bb.x) - min(ba.right, bb.right))
                                    dy = max(0, max(ba.y, bb.y) - min(ba.bottom, bb.bottom))
                                    d = (dx ** 2 + dy ** 2) ** 0.5
                                    if d < best_dist:
                                        best_dist = d
                                        best_target = opid
                        if best_dist > max_dist:
                            best_target = None  # too far — leave undersized

                    if best_target is not None:
                        pieces_dict[best_target] = pieces_dict[best_target] + brick_ids
                        for bid in brick_ids:
                            piece_of[bid] = best_target
                        del pieces_dict[pid]
                        changed = True

        # --- Build result and score it ---
        result_pieces = list(fixed_pieces)
        for pid, brick_ids in pieces_dict.items():
            x, y, w, h = compute_piece_bbox(brick_ids, bricks_by_id)
            result_pieces.append(PuzzlePiece(
                id=len(result_pieces),
                brick_ids=brick_ids,
                x=x, y=y, width=w, height=h,
            ))
        for i, piece in enumerate(result_pieces):
            piece.id = i

        oversized = [
            p.id for p in result_pieces
            if p.width > max_width or p.height > max_height
        ]

        candidate = MergeResult(pieces=result_pieces, oversized=oversized)

        if best_result is None or len(oversized) < len(best_result.oversized):
            best_result = candidate
            if not oversized:
                break  # perfect — no need to try more

    return best_result


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
