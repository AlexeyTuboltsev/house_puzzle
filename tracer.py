"""
Faithful Python port of the JS brick outline tracing pipeline:
  coarseTraceSnap -> autoSimplify (visvalingamWhyatt) -> refineCorners

All algorithms match the JS exactly so results can be compared side-by-side.
"""

import math
import numpy as np
from PIL import Image


def _load_alpha(png_path: str) -> np.ndarray:
    """Return (H, W) uint8 alpha channel array."""
    img = Image.open(png_path).convert("RGBA")
    return np.array(img)[:, :, 3]


def coarse_trace_snap(alpha: np.ndarray) -> list[tuple[float, float]]:
    """
    Port of JS coarseTraceSnap(comp).

    alpha: (H, W) uint8 array (alpha channel only).
    Returns list of (x, y) pixel coords as the traced outline.
    """
    H, W = alpha.shape
    CELL = 5
    PAD = 3
    GW = math.ceil(W / CELL) + PAD * 2
    GH = math.ceil(H / CELL) + PAD * 2

    # --- boundary pixels (alpha > 30, adjacent to transparent) ---
    opaque = alpha > 30  # (H, W) bool

    # 4-neighbour boundary check via shifted arrays
    top    = np.pad(opaque, ((1,0),(0,0)), mode='constant')[:-1, :]
    bot    = np.pad(opaque, ((0,1),(0,0)), mode='constant')[1:,  :]
    left   = np.pad(opaque, ((0,0),(1,0)), mode='constant')[:, :-1]
    right  = np.pad(opaque, ((0,0),(0,1)), mode='constant')[:, 1:]
    is_boundary = opaque & (~top | ~bot | ~left | ~right)
    # also any opaque pixel adjacent to out-of-bounds counts as boundary
    # (already handled: shifted arrays use constant=False = 0 = transparent)

    ys_b, xs_b = np.where(is_boundary)
    boundary_pts = list(zip(xs_b.tolist(), ys_b.tolist()))  # [(x,y), ...]

    # --- coarse grid ---
    grid = np.zeros((GH, GW), dtype=np.uint8)
    ys_o, xs_o = np.where(opaque)
    gxs = (xs_o // CELL) + PAD
    gys = (ys_o // CELL) + PAD
    grid[gys, gxs] = 1

    # --- 8-connected dilation (manual, matching JS 3x3 neighbourhood) ---
    dil_grid = np.zeros((GH, GW), dtype=np.uint8)
    src_rows, src_cols = np.where(grid)
    for dy in range(-1, 2):
        for dx in range(-1, 2):
            nrows = np.clip(src_rows + dy, 0, GH - 1)
            ncols = np.clip(src_cols + dx, 0, GW - 1)
            dil_grid[nrows, ncols] = 1

    # --- 4-connected exterior flood fill from border cells ---
    exterior = np.zeros((GH, GW), dtype=np.uint8)
    q = []

    def seed_border(idx_flat):
        if not dil_grid.flat[idx_flat]:
            exterior.flat[idx_flat] = 1
            q.append(idx_flat)

    for x in range(GW):
        seed_border(x)                    # top row
        seed_border((GH - 1) * GW + x)   # bottom row
    for y in range(GH):
        seed_border(y * GW)               # left col
        seed_border(y * GW + GW - 1)      # right col

    qi = 0
    while qi < len(q):
        idx = q[qi]; qi += 1
        gx = idx % GW; gy = idx // GW
        for dx, dy in ((-1,0),(1,0),(0,-1),(0,1)):
            nx, ny = gx + dx, gy + dy
            if 0 <= nx < GW and 0 <= ny < GH:
                ni = ny * GW + nx
                if not exterior.flat[ni] and not dil_grid.flat[ni]:
                    exterior.flat[ni] = 1
                    q.append(ni)

    # JS: solid[i] = exterior[i] ? 0 : 1
    solid = (1 - exterior).astype(np.uint8)

    # --- Moore neighbourhood contour trace ---
    moore_dx = [1, 1, 0, -1, -1, -1, 0, 1]
    moore_dy = [0, 1, 1,  1,  0, -1, -1, -1]

    # Find first solid cell (row-major)
    starts = np.argwhere(solid)
    if len(starts) == 0:
        return []
    start_y, start_x = starts[0]
    startX, startY = int(start_x), int(start_y)

    traced = []
    visited = set()
    curX, curY = startX, startY
    back_dir = 4

    while True:
        key = curY * GW + curX
        if key not in visited:
            traced.append((curX, curY))
            visited.add(key)

        found = False
        for i in range(1, 9):
            d = (back_dir + i) % 8
            nx, ny = curX + moore_dx[d], curY + moore_dy[d]
            if 0 <= nx < GW and 0 <= ny < GH and solid[ny, nx]:
                back_dir = (d + 4) % 8
                curX, curY = nx, ny
                found = True
                break

        if not found:
            break
        if curX == startX and curY == startY:
            break

    if len(traced) < 3:
        return []

    # Filter to cells that have at least one exterior (or OOB) 8-neighbour
    boundary_traced = []
    for gx, gy in traced:
        is_bnd = False
        for dx, dy in ((-1,0),(1,0),(0,-1),(0,1),(-1,-1),(1,-1),(-1,1),(1,1)):
            nx, ny = gx + dx, gy + dy
            if nx < 0 or nx >= GW or ny < 0 or ny >= GH or exterior[ny, nx]:
                is_bnd = True
                break
        if is_bnd:
            boundary_traced.append((gx, gy))

    if len(boundary_traced) < 3:
        return []

    # Snap each cell centre to nearest real boundary pixel (vectorized)
    if not boundary_pts:
        return []

    bpts = np.array(boundary_pts, dtype=np.float32)  # (P, 2)
    cell_centers = np.array(
        [[(gx - PAD + 0.5) * CELL, (gy - PAD + 0.5) * CELL]
         for gx, gy in boundary_traced],
        dtype=np.float32,
    )  # (C, 2)

    # (C, P) distance squared via broadcasting
    diff = cell_centers[:, None, :] - bpts[None, :, :]  # (C, P, 2)
    d2 = (diff * diff).sum(axis=2)                       # (C, P)
    nearest = d2.argmin(axis=1)                          # (C,)
    snapped_arr = bpts[nearest]                          # (C, 2)

    return [(float(snapped_arr[i, 0]), float(snapped_arr[i, 1]))
            for i in range(len(boundary_traced))]


def _hausdorff_to_poly(pts, poly):
    """
    Port of JS hausdorffToPoly(pts, poly).
    One-sided: max over pts of min distance to any segment in poly.
    Vectorized with numpy for speed.
    """
    if not pts or not poly:
        return 0.0
    p = np.asarray(pts, dtype=np.float64)   # (N, 2)
    q = np.asarray(poly, dtype=np.float64)  # (M, 2)
    a = q                                    # (M, 2) segment starts
    b = np.roll(q, -1, axis=0)              # (M, 2) segment ends
    ab = b - a                              # (M, 2)
    ab_len_sq = (ab * ab).sum(axis=1)      # (M,)

    # (N, M, 2)
    pa = p[:, None, :] - a[None, :, :]
    # dot(pa, ab) / len_sq, clamped -> (N, M)
    denom = np.where(ab_len_sq > 0, ab_len_sq, 1.0)
    t = np.clip((pa * ab[None, :, :]).sum(axis=2) / denom, 0.0, 1.0)
    # closest point on each segment -> (N, M, 2)
    closest = a[None, :, :] + t[:, :, None] * ab[None, :, :]
    # squared distance -> (N, M), then min over segments -> (N,)
    d_sq = ((p[:, None, :] - closest) ** 2).sum(axis=2)
    return float(np.sqrt(d_sq.min(axis=1).max()))


def _visvalingam_whyatt(points: list, target_count: int) -> list:
    """Port of JS visvalingamWhyatt(points, targetCount). Input: list of (x,y)."""
    if len(points) <= target_count:
        return list(points)

    class Pt:
        __slots__ = ['x', 'y', 'prev', 'next', 'removed']
        def __init__(self, x, y):
            self.x = x; self.y = y
            self.prev = self.next = 0
            self.removed = False

    pts = [Pt(p[0], p[1]) for p in points]
    n = len(pts)
    for i in range(n):
        pts[i].prev = (i - 1) % n
        pts[i].next = (i + 1) % n

    def triangle_area(a, b, c):
        return abs((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) / 2.0

    def get_area(i):
        return triangle_area(pts[pts[i].prev], pts[i], pts[pts[i].next])

    remaining = n
    while remaining > target_count:
        min_area = math.inf
        min_idx = -1
        for i in range(n):
            if pts[i].removed:
                continue
            a = get_area(i)
            if a < min_area:
                min_area = a
                min_idx = i
        if min_idx < 0:
            break
        pts[min_idx].removed = True
        prev_i = pts[min_idx].prev
        next_i = pts[min_idx].next
        pts[prev_i].next = next_i
        pts[next_i].prev = prev_i
        remaining -= 1

    return [(p.x, p.y) for p in pts if not p.removed]


def auto_simplify(outline: list, tolerance: float = 1.0) -> list:
    """Port of JS autoSimplify(outline, tolerance)."""
    if len(outline) <= 3:
        return list(outline)
    lo, hi = 3, len(outline)
    while lo < hi:
        mid = (lo + hi) >> 1
        simplified = _visvalingam_whyatt(outline, mid)
        dist = _hausdorff_to_poly(outline, simplified)
        if dist <= tolerance:
            hi = mid
        else:
            lo = mid + 1
    return _visvalingam_whyatt(outline, lo)


def refine_corners(simplified: list, outline: list,
                   threshold: float = 20.0, max_deviation: float = 1.0) -> list:
    """Port of JS refineCorners(simplified, outline, threshold, maxDeviation)."""
    if len(simplified) <= 4:
        return list(simplified)

    # Pre-convert outline to numpy array for vectorized search
    ol_arr = np.array(outline, dtype=np.float64)  # (P, 2)
    search_r2 = (threshold * 3) ** 2

    pts = [list(p) for p in simplified]
    changed = True
    while changed:
        changed = False
        i = 0
        while i < len(pts) and len(pts) > 4:
            j = (i + 1) % len(pts)
            dx = pts[i][0] - pts[j][0]
            dy = pts[i][1] - pts[j][1]
            if math.sqrt(dx * dx + dy * dy) > threshold:
                i += 1
                continue

            prev = pts[(i - 1) % len(pts)]
            next_ = pts[(j + 1) % len(pts)]

            mid = np.array([(pts[i][0] + pts[j][0]) / 2.0,
                             (pts[i][1] + pts[j][1]) / 2.0])

            lx = next_[0] - prev[0]
            ly = next_[1] - prev[1]
            line_len = math.sqrt(lx * lx + ly * ly)

            best_pt = mid.tolist()

            if line_len > 0:
                # Vectorized: filter by radius then find max perpendicular distance
                dm2 = ((ol_arr[:, 0] - mid[0]) ** 2 +
                       (ol_arr[:, 1] - mid[1]) ** 2)
                mask = dm2 <= search_r2
                if mask.any():
                    near = ol_arr[mask]  # (K, 2)
                    # signed perpendicular distance to line prev->next
                    d_vals = np.abs(lx * (prev[1] - near[:, 1]) -
                                    ly * (prev[0] - near[:, 0])) / line_len
                    best_k = int(np.argmax(d_vals))
                    best_pt = [float(near[best_k, 0]), float(near[best_k, 1])]

            candidate = [list(p) for p in pts]
            candidate[i] = best_pt
            if j > i:
                candidate.pop(j)
            else:
                candidate.pop(0)

            new_dist = _hausdorff_to_poly(outline, candidate)
            if new_dist > max_deviation:
                i += 1
                continue

            pts = candidate
            changed = True
            break

    return pts


def trace_brick_png(png_path: str) -> list[tuple[float, float]]:
    """
    Full pipeline: png_path -> polygon points (same as JS pipeline).
    Returns list of [x, y] pairs (absolute pixel coords within the brick PNG).
    """
    alpha = _load_alpha(png_path)
    outline = coarse_trace_snap(alpha)
    if len(outline) < 3:
        return []
    simplified = auto_simplify(outline, tolerance=1.0)
    refined = refine_corners(simplified, outline, threshold=20.0, max_deviation=1.0)
    return refined
