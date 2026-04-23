//! Vector merge pipeline for piece outlines.
//!
//! The testbed iterates on this module. Goal: take the bezier paths of every
//! brick in a piece and emit clean closed outlines for the piece — no spikes,
//! no crossings, no missing bricks, no extra loops.
//!
//! Current strategy (baseline, to be replaced):
//! 1. Tessellate each brick's bezier paths (AI-native scale).
//! 2. Snap near-duplicate vertices to canonical grid positions.
//! 3. Insert vertex-on-edge fixups so shared curves match vertex-for-vertex.
//! 4. Plain clipper union over the snapped polygons.
//! 5. Remove degenerate spike loops (v[i] == v[(i+k) % n] with ~zero area).
//! 6. Return the exterior ring(s) as bezier paths (lines only, post-tessellation).

use crate::bezier::{BezierPath, Segment};
use geo::algorithm::area::Area;
use geo::{Coord, LineString, MultiPolygon, Polygon};
use geo_clipper::Clipper;
use std::collections::BTreeMap;

/// Tessellation density for cubic beziers in the baseline algorithm.
pub const DEFAULT_SAMPLES: usize = 12;
/// Endpoint snap tolerance in AI pymu units.
pub const SNAP_TOL: f64 = 0.5;
/// Minimum polygon area to keep after union.
const MIN_AREA: f64 = 1.0;
/// Interior holes below this area are filled.
const HOLE_FILL_AREA: f64 = 100.0;

/// Input: all bezier paths that make up a single piece (one list per brick,
/// flattened). Output: list of closed bezier paths that form the piece outline.
pub fn merge_piece(brick_paths: &[BezierPath]) -> Vec<BezierPath> {
    if brick_paths.is_empty() {
        return Vec::new();
    }

    // 1. Tessellate each bezier path to a polyline.
    let tessellated: Vec<Vec<[f64; 2]>> = brick_paths
        .iter()
        .map(|p| p.tessellate(DEFAULT_SAMPLES))
        .collect();

    // 2. Snap near-duplicate vertices across all rings.
    let snapped = snap_vertices(tessellated, SNAP_TOL);

    // 3. Build geo::Polygons and run clipper union.
    let polys: Vec<Polygon<f64>> = snapped
        .into_iter()
        .filter_map(|ring| {
            if ring.len() < 3 { return None; }
            let mut coords: Vec<Coord<f64>> = ring.iter().map(|p| Coord { x: p[0], y: p[1] }).collect();
            if coords.first() != coords.last() { coords.push(coords[0]); }
            let poly = Polygon::new(LineString::new(coords), vec![]);
            if poly.unsigned_area() < MIN_AREA { None } else { Some(poly) }
        })
        .collect();
    if polys.is_empty() { return Vec::new(); }

    let factor = 10_000.0; // clipper scales to i64 via multiplier
    let mut union: MultiPolygon<f64> = MultiPolygon(vec![polys[0].clone()]);
    for p in &polys[1..] {
        union = Clipper::union(&union, p, factor);
    }
    union.0.retain(|p| p.unsigned_area() > MIN_AREA);
    if union.0.is_empty() { return Vec::new(); }

    // 4. Per component: drop tiny holes, remove spikes, emit as Line-only
    // bezier path.
    union
        .0
        .into_iter()
        .map(|poly| {
            let (exterior, holes) = poly.into_inner();
            let _ = holes.into_iter().filter(|h| {
                Polygon::new(h.clone(), vec![]).unsigned_area() >= HOLE_FILL_AREA
            });
            let mut pts: Vec<[f64; 2]> =
                exterior.0.iter().map(|c| [c.x, c.y]).collect();
            if pts.len() > 1 && pts.first() == pts.last() { pts.pop(); }
            pts = remove_spikes(pts);
            polyline_to_bezier(pts)
        })
        .collect()
}

/// Canonicalize vertices that are within `tol` of each other to a shared value.
/// Operates on all input rings together, so shared edges between rings become
/// bit-identical after snapping.
fn snap_vertices(rings: Vec<Vec<[f64; 2]>>, tol: f64) -> Vec<Vec<[f64; 2]>> {
    let key = |x: f64, y: f64| -> u64 {
        let ix = (x / tol).round() as i64;
        let iy = (y / tol).round() as i64;
        ((ix as u64) << 32) | (iy as u32 as u64)
    };
    let mut canon: BTreeMap<u64, [f64; 2]> = BTreeMap::new();
    for ring in &rings {
        for p in ring {
            canon.entry(key(p[0], p[1])).or_insert(*p);
        }
    }
    rings
        .into_iter()
        .map(|ring| {
            ring.into_iter()
                .map(|p| *canon.get(&key(p[0], p[1])).unwrap_or(&p))
                .collect()
        })
        .collect()
}

/// Remove degenerate spike loops where a sub-sequence returns to a previous vertex
/// with near-zero area. Passes until convergence.
fn remove_spikes(mut pts: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    loop {
        let n = pts.len();
        if n < 4 { return pts; }
        let mut removed = false;
        let mut out: Vec<[f64; 2]> = Vec::with_capacity(n);
        let mut i = 0;
        while i < n {
            let a = pts[i];
            let mut matched_k: Option<usize> = None;
            for k in 2..=6.min(n - 1) {
                let j = (i + k) % n;
                if pts[j] == a {
                    // zero-ish area over this slice?
                    let slice_area = ring_area(&pts[i..=(i + k).min(n - 1)]);
                    if slice_area.abs() < 0.5 {
                        matched_k = Some(k);
                        break;
                    }
                }
            }
            match matched_k {
                Some(k) => {
                    out.push(a);
                    i += k + 1;
                    removed = true;
                }
                None => {
                    out.push(a);
                    i += 1;
                }
            }
        }
        pts = out;
        if !removed { return pts; }
    }
}

fn ring_area(pts: &[[f64; 2]]) -> f64 {
    if pts.len() < 3 { return 0.0; }
    let mut a = 0.0;
    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        a += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    a.abs() / 2.0
}

fn polyline_to_bezier(pts: Vec<[f64; 2]>) -> BezierPath {
    let start = pts.first().copied().unwrap_or([0.0, 0.0]);
    let segments = pts
        .iter()
        .skip(1)
        .map(|p| Segment::Line { to: *p })
        .collect();
    BezierPath { start, segments }
}

// ═════════════════════════════════════════════════════════════════════
// Bezier-native merge (curves preserved)
// ═════════════════════════════════════════════════════════════════════
//
// Core idea: every brick edge that is shared with a neighbouring brick in
// the same piece is internal → drop it. Every remaining edge is part of
// the outer boundary. Walk the remaining edges to build closed loops.
//
// No tessellation, no polygon boolean ops. For cleanly tiled puzzle
// input (shared endpoints and, for cubics, shared control points), this
// preserves the original vector data exactly.

/// Tolerance (in AI pymu units) for considering two endpoints to be "the
/// same". Endpoints in the AI source are usually bit-identical between
/// adjacent bricks; this slack absorbs minor upstream wobble.
const BEZIER_TOL: f64 = 0.1;

/// Alias kept for local clarity where we want the same precision as
/// endpoints when de-duplicating anchors inside the split passes.
const SPLIT_TOL: f64 = BEZIER_TOL;

/// Perpendicular-distance tolerance for "vertex lies on a line" (line
/// T-junction test). Must be generous enough to absorb small endpoint
/// drift introduced by `canonicalize_endpoints` — otherwise a line whose
/// endpoint was nudged 0.3 pymu by clustering will no longer accept the
/// midway vertex that was originally exactly on it. 1.0 pymu ≈ 0.4
/// canvas-px, which is safely below any real perpendicular distance
/// between distinct lines in puzzle artwork.
const LINE_PROJECT_TOL: f64 = 1.0;

/// Maximum distance between two endpoint positions that should be treated
/// as the same vertex. Hand-drawn bricks often drift by up to ~1 pymu
/// (~0.4 canvas-px) between neighbours; fuse aggressively so the shared-
/// edge key matches anyway.
const VERTEX_FUSE_TOL: f64 = 1.5;

/// Tolerance for comparing a curve's midpoint-at-t=0.5 to another's. Two
/// adjacent arch bricks often describe the same arc with slightly different
/// cubic control points; their midpoints drift by a couple of pymu, so we
/// need a looser tolerance here than for endpoints. Also quantization
/// boundaries — two points within `x` of each other can land in adjacent
/// buckets when quantized at `x`, so we set the grid bigger than the
/// typical drift.
const MIDPOINT_TOL: f64 = 3.0;

fn q(v: f64, tol: f64) -> i64 {
    (v / tol).round() as i64
}
fn qp(p: [f64; 2], tol: f64) -> (i64, i64) {
    (q(p[0], tol), q(p[1], tol))
}

/// Walking-edge record. Carries both endpoints and original segment shape
/// so we can reconstruct the correct `Segment` (possibly reversed) when
/// emitting the final bezier path.
#[derive(Clone, Copy, Debug)]
struct Edge {
    from: [f64; 2],
    to: [f64; 2],
    /// `None` → straight line, `Some((cp1, cp2))` → cubic in the from→to direction.
    cp: Option<([f64; 2], [f64; 2])>,
}

impl Edge {
    fn reversed(&self) -> Edge {
        Edge {
            from: self.to,
            to: self.from,
            cp: self.cp.map(|(a, b)| (b, a)),
        }
    }

    /// Midpoint at t=0.5 — direction-insensitive (B(0.5) is the same for a
    /// cubic and its reverse).
    fn midpoint(&self) -> [f64; 2] {
        match self.cp {
            None => [
                (self.from[0] + self.to[0]) * 0.5,
                (self.from[1] + self.to[1]) * 0.5,
            ],
            Some((c1, c2)) => cubic_at(self.from, c1, c2, self.to, 0.5),
        }
    }

    /// Canonical undirected key. Two edges that represent approximately
    /// the same curve (regardless of direction or slight control-point
    /// drift) compare equal via endpoint pair + midpoint.
    fn key(&self, tol: f64) -> EdgeKey {
        let a = qp(self.from, tol);
        let b = qp(self.to, tol);
        let (lo, hi) = if a <= b { (a, b) } else { (b, a) };
        let m = self.midpoint();
        let mid = qp(m, MIDPOINT_TOL);
        EdgeKey { lo, hi, mid, is_cubic: self.cp.is_some() }
    }

    fn as_segment(&self) -> Segment {
        match self.cp {
            None => Segment::Line { to: self.to },
            Some((cp1, cp2)) => Segment::Cubic { cp1, cp2, to: self.to },
        }
    }
}

#[derive(PartialEq, Eq, Hash, PartialOrd, Ord, Clone, Copy, Debug)]
struct EdgeKey {
    lo: (i64, i64),
    hi: (i64, i64),
    mid: (i64, i64),
    is_cubic: bool,
}

/// All 9 quantization-neighbour variants of an `EdgeKey` — the original plus
/// every mid-cell shifted by (±1, 0) and (±1, ±1). Used for shared-edge
/// lookup so that two near-duplicate curves whose midpoints sit across a
/// quantization boundary still pair up.
fn key_variants(k: &EdgeKey) -> Vec<EdgeKey> {
    let mut out = Vec::with_capacity(9);
    for dx in -1..=1 {
        for dy in -1..=1 {
            out.push(EdgeKey {
                lo: k.lo,
                hi: k.hi,
                mid: (k.mid.0 + dx, k.mid.1 + dy),
                is_cubic: k.is_cubic,
            });
        }
    }
    out
}

/// Merge endpoints that are within `tol` of each other to a common
/// canonical position. This fixes the common case of adjacent bricks that
/// ought to share a vertex but were drawn on slightly different sub-pixel
/// grids.
///
/// Only endpoints are fused — control points of cubics are left alone
/// (their precision doesn't participate in the shared-edge key).
fn canonicalize_endpoints(edges: Vec<Edge>, tol: f64) -> Vec<Edge> {
    if edges.is_empty() { return edges; }
    // Collect unique endpoints.
    let mut pts: Vec<[f64; 2]> = Vec::new();
    let push_unique = |pts: &mut Vec<[f64; 2]>, p: [f64; 2]| {
        if !pts.iter().any(|q| (q[0] - p[0]).abs() < 1e-9 && (q[1] - p[1]).abs() < 1e-9) {
            pts.push(p);
        }
    };
    for e in &edges {
        push_unique(&mut pts, e.from);
        push_unique(&mut pts, e.to);
    }

    // Union-find by proximity: O(n²) but n is small per piece.
    let n = pts.len();
    let mut parent: Vec<usize> = (0..n).collect();
    fn find(parent: &mut [usize], x: usize) -> usize {
        if parent[x] == x { return x; }
        let r = find(parent, parent[x]);
        parent[x] = r;
        r
    }
    for i in 0..n {
        for j in (i + 1)..n {
            let d2 = (pts[i][0] - pts[j][0]).powi(2) + (pts[i][1] - pts[j][1]).powi(2);
            if d2 <= tol * tol {
                let ri = find(&mut parent, i);
                let rj = find(&mut parent, j);
                if ri != rj { parent[ri] = rj; }
            }
        }
    }

    // Average cluster members to get a canonical position per root.
    let mut sums: Vec<[f64; 2]> = vec![[0.0, 0.0]; n];
    let mut counts: Vec<u32> = vec![0; n];
    for i in 0..n {
        let r = find(&mut parent, i);
        sums[r][0] += pts[i][0];
        sums[r][1] += pts[i][1];
        counts[r] += 1;
    }
    let mut canon: BTreeMap<(i64, i64), [f64; 2]> = BTreeMap::new();
    for i in 0..n {
        let r = find(&mut parent, i);
        if counts[r] == 0 { continue; }
        let avg = [sums[r][0] / counts[r] as f64, sums[r][1] / counts[r] as f64];
        let key = ((pts[i][0] * 1e6).round() as i64, (pts[i][1] * 1e6).round() as i64);
        canon.insert(key, avg);
    }

    let lookup = |p: [f64; 2]| -> [f64; 2] {
        let k = ((p[0] * 1e6).round() as i64, (p[1] * 1e6).round() as i64);
        canon.get(&k).copied().unwrap_or(p)
    };

    edges
        .into_iter()
        .map(|e| Edge {
            from: lookup(e.from),
            to: lookup(e.to),
            cp: e.cp,
        })
        .collect()
}

/// Flatten a brick's bezier paths into directed edges (one per segment).
fn edges_of(paths: &[BezierPath]) -> Vec<Edge> {
    let mut out = Vec::new();
    for path in paths {
        let mut prev = path.start;
        for seg in &path.segments {
            let (to, cp) = match *seg {
                Segment::Line { to } => (to, None),
                Segment::Cubic { cp1, cp2, to } => (to, Some((cp1, cp2))),
            };
            out.push(Edge { from: prev, to, cp });
            prev = to;
        }
    }
    out
}

// ── Cubic helpers ────────────────────────────────────────────────────
fn cubic_at(p0: [f64; 2], cp1: [f64; 2], cp2: [f64; 2], p3: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1.0 - t;
    [
        u.powi(3) * p0[0] + 3.0 * u.powi(2) * t * cp1[0]
            + 3.0 * u * t.powi(2) * cp2[0] + t.powi(3) * p3[0],
        u.powi(3) * p0[1] + 3.0 * u.powi(2) * t * cp1[1]
            + 3.0 * u * t.powi(2) * cp2[1] + t.powi(3) * p3[1],
    ]
}

/// De Casteljau subdivision of a cubic at `t`. Returns `(left, right)` as
/// `((p0, cp1, cp2, mid), (mid, cp1', cp2', p3))`.
fn cubic_split(
    p0: [f64; 2], cp1: [f64; 2], cp2: [f64; 2], p3: [f64; 2], t: f64,
) -> (([f64; 2], [f64; 2], [f64; 2], [f64; 2]), ([f64; 2], [f64; 2], [f64; 2], [f64; 2])) {
    let lerp = |a: [f64; 2], b: [f64; 2]| [a[0] + t * (b[0] - a[0]), a[1] + t * (b[1] - a[1])];
    let q0 = lerp(p0, cp1);
    let q1 = lerp(cp1, cp2);
    let q2 = lerp(cp2, p3);
    let r0 = lerp(q0, q1);
    let r1 = lerp(q1, q2);
    let mid = lerp(r0, r1);
    ((p0, q0, r0, mid), (mid, r1, q2, p3))
}

/// Cubic derivative at `t` — 3 * [(1-t)² (p1-p0) + 2(1-t)t(p2-p1) + t²(p3-p2)].
fn cubic_deriv(
    p0: [f64; 2], cp1: [f64; 2], cp2: [f64; 2], p3: [f64; 2], t: f64,
) -> [f64; 2] {
    let u = 1.0 - t;
    [
        3.0 * (u * u * (cp1[0] - p0[0]) + 2.0 * u * t * (cp2[0] - cp1[0]) + t * t * (p3[0] - cp2[0])),
        3.0 * (u * u * (cp1[1] - p0[1]) + 2.0 * u * t * (cp2[1] - cp1[1]) + t * t * (p3[1] - cp2[1])),
    ]
}

/// Unit tangent leaving the edge's `from` vertex (direction of travel at t=0).
fn edge_tangent_at_start(e: &Edge) -> [f64; 2] {
    let raw = match e.cp {
        None => [e.to[0] - e.from[0], e.to[1] - e.from[1]],
        Some((c1, c2)) => cubic_deriv(e.from, c1, c2, e.to, 0.0),
    };
    normalize(raw)
}

/// Unit tangent arriving at the edge's `to` vertex (direction of travel at t=1).
fn edge_tangent_at_end(e: &Edge) -> [f64; 2] {
    let raw = match e.cp {
        None => [e.to[0] - e.from[0], e.to[1] - e.from[1]],
        Some((c1, c2)) => cubic_deriv(e.from, c1, c2, e.to, 1.0),
    };
    normalize(raw)
}

fn normalize(v: [f64; 2]) -> [f64; 2] {
    let len = (v[0] * v[0] + v[1] * v[1]).sqrt();
    if len < 1e-12 { [1.0, 0.0] } else { [v[0] / len, v[1] / len] }
}

/// Counter-clockwise angle from `a` to `b`, in radians (−π, π].
fn turn_ccw(a: [f64; 2], b: [f64; 2]) -> f64 {
    let cross = a[0] * b[1] - a[1] * b[0];
    let dot = a[0] * b[0] + a[1] * b[1];
    cross.atan2(dot)
}

/// Find `t ∈ (0, 1)` at which the cubic is closest to `v`. Returns
/// `Some((t, dist_sq))` if a minimum is found in the interior.
fn closest_t_on_cubic(
    p0: [f64; 2], cp1: [f64; 2], cp2: [f64; 2], p3: [f64; 2], v: [f64; 2],
) -> (f64, f64) {
    // 3-level refinement: coarse grid → refine around best → refine again.
    let mut lo = 0.0;
    let mut hi = 1.0;
    let mut best_t = 0.5;
    let mut best_d = f64::INFINITY;
    for _ in 0..3 {
        let n = 40;
        let mut bt = lo;
        let mut bd = f64::INFINITY;
        for i in 0..=n {
            let t = lo + (hi - lo) * (i as f64 / n as f64);
            let pt = cubic_at(p0, cp1, cp2, p3, t);
            let d = (pt[0] - v[0]).powi(2) + (pt[1] - v[1]).powi(2);
            if d < bd { bd = d; bt = t; }
        }
        let step = (hi - lo) / n as f64;
        lo = (bt - step).max(0.0);
        hi = (bt + step).min(1.0);
        best_t = bt;
        best_d = bd;
    }
    (best_t, best_d)
}

/// Pre-pass: split every LINE edge at any vertex that lies on it (T-junctions).
///
/// Adjacent bricks frequently share only a PORTION of an edge — e.g. a small
/// brick's bottom edge lands midway along a bigger brick's top edge. To make
/// those pieces match in the edge-sharing walk, we insert the midway vertex
/// into the bigger brick's edge first so both now expose the same shorter
/// segment.
///
/// Only applies to `Line` edges. Cubics are left as-is: splitting a cubic at
/// a point requires evaluating t and de Casteljau subdivision, and for
/// puzzle art shared edges are almost always straight.
fn split_lines_at_vertices(edges: Vec<Edge>) -> Vec<Edge> {
    // Collect all unique vertices (canonical-quantized positions) with a
    // representative float coord. We reuse the canonical form so nearby
    // points fuse to a single anchor.
    use std::collections::BTreeMap;
    let mut anchors: BTreeMap<(i64, i64), [f64; 2]> = BTreeMap::new();
    for e in &edges {
        anchors.entry(qp(e.from, SPLIT_TOL)).or_insert(e.from);
        anchors.entry(qp(e.to, SPLIT_TOL)).or_insert(e.to);
    }
    let anchor_list: Vec<[f64; 2]> = anchors.values().copied().collect();

    let mut out: Vec<Edge> = Vec::with_capacity(edges.len());
    for e in edges {
        if e.cp.is_some() {
            out.push(e);
            continue;
        }
        // Find every anchor that lies strictly between e.from and e.to on the
        // segment. Project onto the line and check perpendicular distance.
        let dx = e.to[0] - e.from[0];
        let dy = e.to[1] - e.from[1];
        let len2 = dx * dx + dy * dy;
        if len2 < 1e-9 { out.push(e); continue; }

        let mut splits: Vec<(f64, [f64; 2])> = Vec::new();
        for a in &anchor_list {
            if qp(*a, BEZIER_TOL) == qp(e.from, SPLIT_TOL) { continue; }
            if qp(*a, BEZIER_TOL) == qp(e.to, SPLIT_TOL) { continue; }
            let ax = a[0] - e.from[0];
            let ay = a[1] - e.from[1];
            let t = (ax * dx + ay * dy) / len2;
            if t <= 1e-6 || t >= 1.0 - 1e-6 { continue; }
            // Perpendicular distance from anchor to line
            let px = e.from[0] + t * dx;
            let py = e.from[1] + t * dy;
            let d2 = (a[0] - px).powi(2) + (a[1] - py).powi(2);
            if d2 > LINE_PROJECT_TOL * LINE_PROJECT_TOL { continue; }
            splits.push((t, *a));
        }

        if splits.is_empty() {
            out.push(e);
            continue;
        }
        splits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        // Dedup by canonical position
        splits.dedup_by(|a, b| qp(a.1, BEZIER_TOL) == qp(b.1, BEZIER_TOL));

        let mut prev = e.from;
        for (_t, pt) in &splits {
            out.push(Edge { from: prev, to: *pt, cp: None });
            prev = *pt;
        }
        out.push(Edge { from: prev, to: e.to, cp: None });
    }
    out
}

/// Pre-pass: split CUBIC edges at any vertex that lies ON the curve
/// (curve T-junctions). Same spirit as `split_lines_at_vertices` but uses
/// `closest_t_on_cubic` + de Casteljau to subdivide the cubic into two
/// cubics joined at the intruding vertex.
fn split_cubics_at_vertices(edges: Vec<Edge>) -> Vec<Edge> {
    use std::collections::BTreeMap;
    // Collect anchor vertices (quantized, with a representative coord).
    let mut anchors: BTreeMap<(i64, i64), [f64; 2]> = BTreeMap::new();
    for e in &edges {
        anchors.entry(qp(e.from, SPLIT_TOL)).or_insert(e.from);
        anchors.entry(qp(e.to, SPLIT_TOL)).or_insert(e.to);
    }
    let anchor_list: Vec<[f64; 2]> = anchors.values().copied().collect();

    // Perpendicular-distance tolerance for "vertex lies on curve".
    // Real T-junctions between adjacent arch bricks drift by ≲2 pymu due
    // to independently-chosen control points. A vertex merely *near* a
    // curve (4+ pymu away) typically belongs to a different neighbour
    // and must NOT be snapped onto this curve — snapping fabricates a
    // bogus edge via de Casteljau.
    const CURVE_TOL: f64 = 2.5;

    let mut out: Vec<Edge> = Vec::with_capacity(edges.len());
    for e in edges {
        let Some((cp1, cp2)) = e.cp else {
            out.push(e);
            continue;
        };
        let p0 = e.from;
        let p3 = e.to;

        // Collect candidate (t, vertex) splits.
        let mut splits: Vec<(f64, [f64; 2])> = Vec::new();
        for a in &anchor_list {
            let qa = qp(*a, BEZIER_TOL);
            if qa == qp(p0, BEZIER_TOL) { continue; }
            if qa == qp(p3, BEZIER_TOL) { continue; }
            let (t, d2) = closest_t_on_cubic(p0, cp1, cp2, p3, *a);
            if t <= 1e-4 || t >= 1.0 - 1e-4 { continue; }
            if d2 > CURVE_TOL * CURVE_TOL { continue; }
            splits.push((t, *a));
        }

        if splits.is_empty() {
            out.push(e);
            continue;
        }
        splits.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal));
        splits.dedup_by(|a, b| qp(a.1, BEZIER_TOL) == qp(b.1, BEZIER_TOL));

        if std::env::var("HP_DEBUG_CUBIC_SPLIT").is_ok() {
            eprintln!(
                "[cubic-split] ({:.3},{:.3})->({:.3},{:.3}) has {} split(s):",
                p0[0], p0[1], p3[0], p3[1], splits.len()
            );
            for (t, v) in &splits {
                eprintln!("  t={:.4} @ ({:.3},{:.3})", t, v[0], v[1]);
            }
        }

        // Subdivide the cubic at each t in increasing order. After each
        // subdivision the remaining (right) piece has its own [0,1] range,
        // so we remap subsequent t's relative to the new parameter space.
        let mut cur_p0 = p0;
        let mut cur_cp1 = cp1;
        let mut cur_cp2 = cp2;
        let mut cur_p3 = p3;
        let mut prev_t = 0.0;
        for (t, pt) in &splits {
            let local_t = (t - prev_t) / (1.0 - prev_t);
            let ((lp0, lcp1, lcp2, lp3), (rp0, rcp1, rcp2, rp3)) =
                cubic_split(cur_p0, cur_cp1, cur_cp2, cur_p3, local_t);
            // Emit left piece. Snap its end to the actual vertex so
            // adjacent bricks share bit-identical endpoints.
            out.push(Edge { from: lp0, to: *pt, cp: Some((lcp1, lcp2)) });
            let _ = lp3;
            // Right piece becomes the current cubic for next iteration.
            cur_p0 = *pt;
            cur_cp1 = rcp1;
            cur_cp2 = rcp2;
            cur_p3 = rp3;
            let _ = rp0;
            prev_t = *t;
        }
        // Emit the final right piece.
        out.push(Edge {
            from: cur_p0,
            to: cur_p3,
            cp: Some((cur_cp1, cur_cp2)),
        });
    }
    out
}

/// Bezier-native piece merge. Preserves cubic curves — no tessellation.
///
/// Returns one `BezierPath` per outer-boundary component of the piece.
/// Diagnostic: return (total_edges, unique_keys, shared_key_count).
pub fn merge_piece_bezier_stats(brick_paths: &[BezierPath]) -> (usize, usize, usize) {
    let all: Vec<Edge> = edges_of(brick_paths);
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<EdgeKey, usize> = BTreeMap::new();
    for e in &all {
        *buckets.entry(e.key(BEZIER_TOL)).or_insert(0) += 1;
    }
    let shared = buckets.values().filter(|&&c| c >= 2).count();
    (all.len(), buckets.len(), shared)
}

/// Diagnostic: enumerate edges with counts per key. Sorted by count desc.
pub fn merge_piece_bezier_edges(brick_paths: &[BezierPath])
    -> Vec<(([f64;2], [f64;2]), usize, bool /* has_cp */)>
{
    let all: Vec<Edge> = edges_of(brick_paths);
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<EdgeKey, (usize, [f64;2], [f64;2], bool)> = BTreeMap::new();
    for e in &all {
        let k = e.key(BEZIER_TOL);
        let entry = buckets.entry(k).or_insert((0, e.from, e.to, e.cp.is_some()));
        entry.0 += 1;
    }
    let mut out: Vec<_> = buckets.into_iter().map(|(_, (c, f, t, cp))| ((f, t), c, cp)).collect();
    out.sort_by(|a, b| b.1.cmp(&a.1));
    out
}

pub fn merge_piece_bezier(brick_paths: &[BezierPath]) -> Vec<BezierPath> {
    if brick_paths.is_empty() {
        return Vec::new();
    }

    let raw: Vec<Edge> = edges_of(brick_paths);
    if raw.is_empty() {
        return Vec::new();
    }
    // Canonicalize near-duplicate vertices (hand-drawn bricks frequently
    // have sub-pixel drift between neighbours — e.g. one at y=.59, the
    // next at y=.35). Up to VERTEX_FUSE_TOL apart → fused to one position.
    let all = canonicalize_endpoints(raw, VERTEX_FUSE_TOL);

    // T-junctions: split LINE edges at midway vertices (staircase tiling),
    // then CUBIC edges at vertices lying on the curve (arch-on-arch cases).
    // Two-pass so that a split-line can introduce a new vertex that later
    // lands on a cubic, and a split-cubic can introduce a vertex that
    // another cubic passes through.
    let mut all = split_lines_at_vertices(all);
    all = split_cubics_at_vertices(all);
    all = split_lines_at_vertices(all);
    all = split_cubics_at_vertices(all);

    // Bucket by canonical key; any bucket of size ≥ 2 is internal. For
    // each edge we also probe the 8 neighbour midpoint cells so that two
    // near-duplicate curves whose midpoints sit just across a quantization
    // boundary still pair up.
    use std::collections::BTreeMap;
    let mut buckets: BTreeMap<EdgeKey, usize> = BTreeMap::new();
    for e in &all {
        *buckets.entry(e.key(BEZIER_TOL)).or_insert(0) += 1;
    }

    let is_internal = |e: &Edge| -> bool {
        let primary = e.key(BEZIER_TOL);
        let total: usize = key_variants(&primary)
            .into_iter()
            .map(|k| buckets.get(&k).copied().unwrap_or(0))
            .sum();
        // `key_variants` always includes `primary`, which counted this edge.
        // Shared → at least one other edge contributes.
        total >= 2
    };

    let outer: Vec<Edge> = all
        .into_iter()
        .filter(|e| !is_internal(e))
        .collect();
    if outer.is_empty() {
        return Vec::new();
    }

    // Adjacency: canonical vertex key → list of outer-edge indices starting
    // at that vertex (using either original or reversed orientation).
    let mut out_edges: BTreeMap<(i64, i64), Vec<(usize, bool)>> = BTreeMap::new();
    for (i, e) in outer.iter().enumerate() {
        out_edges.entry(qp(e.from, SPLIT_TOL)).or_default().push((i, false));
        out_edges.entry(qp(e.to, SPLIT_TOL)).or_default().push((i, true));
    }

    let mut used = vec![false; outer.len()];
    let mut loops: Vec<BezierPath> = Vec::new();

    for start_idx in 0..outer.len() {
        if used[start_idx] {
            continue;
        }
        // Start the walk from the "from" endpoint of this edge.
        let start_edge = outer[start_idx];
        let start_vertex = qp(start_edge.from, BEZIER_TOL);
        let mut current = start_edge;
        let mut current_idx = start_idx;
        used[current_idx] = true;

        let mut segs: Vec<Segment> = Vec::new();
        let loop_start_pt = current.from;
        segs.push(current.as_segment());

        loop {
            let cur_end = qp(current.to, BEZIER_TOL);
            if cur_end == start_vertex {
                break; // closed
            }
            let candidates = match out_edges.get(&cur_end) {
                Some(v) => v,
                None => break,
            };

            // Face-tracing rule: at the junction, sort outgoing edges by
            // their angle and pick the NEXT one CCW from the direction we
            // arrived along. This hugs the outer face counter-clockwise.
            let in_dir = edge_tangent_at_end(&current);
            let ang_in = (-in_dir[1]).atan2(-in_dir[0]);
            let mut best: Option<(usize, bool, f64)> = None;
            for &(idx, reversed) in candidates {
                if used[idx] { continue; }
                let raw = outer[idx];
                let candidate = if reversed { raw.reversed() } else { raw };
                let out_dir = edge_tangent_at_start(&candidate);
                let ang_out = out_dir[1].atan2(out_dir[0]);
                let mut delta = ang_out - ang_in;
                while delta <= 1e-9 { delta += std::f64::consts::TAU; }
                if best.map_or(true, |(_, _, a)| delta > a) {
                    best = Some((idx, reversed, delta));
                }
            }
            let Some((next_idx, next_reversed, _)) = best else { break };
            let raw = outer[next_idx];
            let next = if next_reversed { raw.reversed() } else { raw };
            used[next_idx] = true;
            segs.push(next.as_segment());
            current = next;
            current_idx = next_idx;
            let _ = current_idx;
        }

        // Drop degenerate tiny loops (e.g. from a pair of edges that
        // happen to connect at both ends but enclose no area).
        if segs.len() >= 2 {
            loops.push(BezierPath {
                start: loop_start_pt,
                segments: segs,
            });
        }
    }

    // If two loops share a vertex (vertex-only contact between pieces that
    // otherwise don't share an edge — e.g. two bricks touching at a corner),
    // concatenate them at that vertex so the outline is a single figure-8
    // path rather than two disjoint loops.
    let loops = merge_loops_at_shared_vertex(loops);

    // Drop interior loops (compound-path holes / cut-outs). A loop whose
    // bbox is strictly contained in another loop's bbox is treated as an
    // inner loop. For disconnected-but-separate components, bboxes don't
    // nest, so both are kept.
    drop_contained_loops(loops)
}

/// Post-process: concatenate loops that share a vertex. The output can
/// legitimately have multiple loops for a piece with holes or genuinely
/// disconnected components — but for vertex-only contacts between two
/// bricks, we want one continuous outline (figure-8 at the shared vertex).
fn merge_loops_at_shared_vertex(mut loops: Vec<BezierPath>) -> Vec<BezierPath> {
    loop {
        let mut merged = false;
        'outer: for i in 0..loops.len() {
            for j in (i + 1)..loops.len() {
                if let Some((vi, vj)) = shared_vertex(&loops[i], &loops[j]) {
                    let a = rotate_loop(&loops[i], vi);
                    let b = rotate_loop(&loops[j], vj);
                    // Concatenate: walk a from its (rotated) start back to
                    // start, then walk b from its start back to start. Since
                    // both start at the shared vertex, a's final `to` equals
                    // the shared vertex, which is also b's start. So append
                    // b's segments after a's.
                    let mut new_segs = a.segments;
                    new_segs.extend(b.segments);
                    let new_loop = BezierPath { start: a.start, segments: new_segs };
                    // Replace loop i, remove loop j.
                    loops[i] = new_loop;
                    loops.remove(j);
                    merged = true;
                    break 'outer;
                }
            }
        }
        if !merged { break; }
    }
    loops
}

/// Find a vertex that appears in both loops. Returns the start-indices
/// (0 = `start`, k = after segment k-1) in each loop.
fn shared_vertex(a: &BezierPath, b: &BezierPath) -> Option<(usize, usize)> {
    let va = a.vertices();
    let vb = b.vertices();
    for (i, pa) in va.iter().enumerate() {
        for (j, pb) in vb.iter().enumerate() {
            let dx = pa[0] - pb[0];
            let dy = pa[1] - pb[1];
            if dx * dx + dy * dy <= BEZIER_TOL * BEZIER_TOL {
                // Skip the implicit closing vertex (last == first).
                if i + 1 == va.len() { continue; }
                if j + 1 == vb.len() { continue; }
                return Some((i, j));
            }
        }
    }
    None
}

/// Rotate a closed loop so that its `start` is the vertex at
/// index `vertex_idx` (0 = current start, k = end of segment k-1).
fn rotate_loop(loop_: &BezierPath, vertex_idx: usize) -> BezierPath {
    if vertex_idx == 0 {
        return loop_.clone();
    }
    let n = loop_.segments.len();
    let new_start = loop_.segments[vertex_idx - 1].end();
    // New segments: starting from vertex_idx, wrap around, then up to vertex_idx - 1.
    let mut new_segs = Vec::with_capacity(n);
    for k in 0..n {
        new_segs.push(loop_.segments[(vertex_idx + k) % n]);
    }
    BezierPath { start: new_start, segments: new_segs }
}

/// Approximate signed area of a closed bezier path via the chord polygon.
fn bezier_signed_area(bp: &BezierPath) -> f64 {
    let mut pts: Vec<[f64; 2]> = Vec::with_capacity(bp.segments.len() + 1);
    pts.push(bp.start);
    for s in &bp.segments {
        pts.push(s.end());
    }
    if pts.len() < 3 { return 0.0; }
    let mut a = 0.0;
    let n = pts.len();
    for i in 0..n {
        let j = (i + 1) % n;
        a += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
    }
    a / 2.0
}

fn loop_bbox(bp: &BezierPath) -> [f64; 4] {
    let mut mn = [f64::INFINITY; 2];
    let mut mx = [f64::NEG_INFINITY; 2];
    let mut upd = |p: [f64; 2]| {
        if p[0] < mn[0] { mn[0] = p[0]; }
        if p[1] < mn[1] { mn[1] = p[1]; }
        if p[0] > mx[0] { mx[0] = p[0]; }
        if p[1] > mx[1] { mx[1] = p[1]; }
    };
    upd(bp.start);
    for s in &bp.segments { upd(s.end()); }
    [mn[0], mn[1], mx[0], mx[1]]
}

fn drop_contained_loops(loops: Vec<BezierPath>) -> Vec<BezierPath> {
    if loops.len() < 2 { return loops; }
    let bboxes: Vec<[f64; 4]> = loops.iter().map(loop_bbox).collect();
    let eps = BEZIER_TOL;
    let mut keep: Vec<bool> = vec![true; loops.len()];
    for i in 0..loops.len() {
        if !keep[i] { continue; }
        let ai = &bboxes[i];
        for j in 0..loops.len() {
            if i == j || !keep[j] { continue; }
            let aj = &bboxes[j];
            // Is aj strictly inside ai (with slack)?
            if aj[0] >= ai[0] - eps
                && aj[1] >= ai[1] - eps
                && aj[2] <= ai[2] + eps
                && aj[3] <= ai[3] + eps
                && (aj[0] > ai[0] + eps
                    || aj[1] > ai[1] + eps
                    || aj[2] < ai[2] - eps
                    || aj[3] < ai[3] - eps)
            {
                keep[j] = false;
            }
        }
    }
    loops.into_iter().zip(keep.into_iter()).filter_map(|(bp, k)| k.then_some(bp)).collect()
}

