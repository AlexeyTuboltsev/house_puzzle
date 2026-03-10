"""
Puzzle piece merging engine.

Takes a list of bricks with positions/sizes and merges them into
puzzle pieces based on adjacency and size constraints.
"""

import random
from collections import defaultdict
from dataclasses import dataclass, field


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


def _edges_overlap(a_start, a_end, b_start, b_end, threshold=0):
    """Check if two 1D ranges overlap by more than threshold."""
    overlap = min(a_end, b_end) - max(a_start, b_start)
    return overlap > threshold


def build_adjacency(bricks: list[Brick], gap: int = ADJACENCY_THRESHOLD) -> dict[int, set[int]]:
    """Build adjacency graph: two bricks are neighbors if their bounding boxes
    overlap or are within `gap` pixels of each other."""
    adj = defaultdict(set)
    n = len(bricks)

    for i in range(n):
        a = bricks[i]
        for j in range(i + 1, n):
            b = bricks[j]

            # Check if bboxes are close (overlap or within gap)
            if (a.x - gap < b.right and a.right + gap > b.x and
                a.y - gap < b.bottom and a.bottom + gap > b.y):
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


def merge_bricks(bricks: list[Brick], target_piece_count: int | None = None,
                 seed: int = 42, windows_separate: bool = True,
                 max_width: int = DEFAULT_MAX_WIDTH,
                 max_height: int = DEFAULT_MAX_HEIGHT) -> list[PuzzlePiece]:
    """
    Merge bricks into puzzle pieces using random adjacency-based grouping.

    Args:
        bricks: List of Brick objects with positions
        target_piece_count: Desired number of pieces (None = auto)
        seed: Random seed for reproducibility
        windows_separate: If True, windows/doors are always separate pieces
    """
    rng = random.Random(seed)
    bricks_by_id = {b.id: b for b in bricks}
    adj = build_adjacency(bricks)

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

    # Initialize: each mergeable brick is its own piece
    # piece_of[brick_id] -> piece_id
    piece_of = {}
    pieces_dict = {}  # piece_id -> list of brick_ids

    for bid in mergeable_ids:
        pid = bid
        piece_of[bid] = pid
        pieces_dict[pid] = [bid]

    # Determine how many merge operations to do
    current_count = len(pieces_dict)
    if target_piece_count is not None:
        target_mergeable = max(1, target_piece_count - len(fixed_pieces))
    else:
        # Auto: aim for roughly total_bricks / 3
        target_mergeable = max(1, current_count // 3)

    merges_needed = current_count - target_mergeable

    # Iteratively merge until we reach target or can't merge anymore
    max_iterations = 50
    for iteration in range(max_iterations):
        current_count = len(pieces_dict)
        if current_count <= target_mergeable:
            break

        # Build list of candidate merge edges between different pieces
        all_edges = []
        for bid in mergeable_ids:
            for nbr in adj.get(bid, set()):
                if nbr in mergeable_ids and bid < nbr:
                    pa = piece_of[bid]
                    pb = piece_of[nbr]
                    if pa != pb:
                        all_edges.append((bid, nbr))

        if not all_edges:
            break

        rng.shuffle(all_edges)

        merged_any = False
        for a_id, b_id in all_edges:
            if len(pieces_dict) <= target_mergeable:
                break

            pa = piece_of[a_id]
            pb = piece_of[b_id]
            if pa == pb:
                continue

            piece_a = pieces_dict[pa]
            piece_b = pieces_dict[pb]

            merged_ids = piece_a + piece_b
            _, _, w, h = compute_piece_bbox(merged_ids, bricks_by_id)
            if w > max_width or h > max_height:
                continue

            # Merge b into a
            pieces_dict[pa] = merged_ids
            for bid in piece_b:
                piece_of[bid] = pa
            del pieces_dict[pb]
            merged_any = True

        if not merged_any:
            break

    # Build final pieces list
    result = list(fixed_pieces)
    for pid, brick_ids in pieces_dict.items():
        x, y, w, h = compute_piece_bbox(brick_ids, bricks_by_id)
        result.append(PuzzlePiece(
            id=len(result),
            brick_ids=brick_ids,
            x=x, y=y, width=w, height=h,
        ))

    # Re-index
    for i, piece in enumerate(result):
        piece.id = i

    return result


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
