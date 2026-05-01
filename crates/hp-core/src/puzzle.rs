//! Puzzle engine — brick merging via area-balanced adjacency grouping.
//!
//! Takes bricks with positions and vector polygons, builds an adjacency graph
//! using polygon proximity, then merges bricks into puzzle pieces targeting
//! a specified piece count.

use geo::algorithm::area::Area;
use geo::algorithm::bounding_rect::BoundingRect;
use geo::{Coord, LineString, Polygon};
use rand::seq::SliceRandom;
use rand::SeedableRng;
use rand_chacha::ChaCha8Rng;
use std::collections::{HashMap, HashSet};

use crate::types::{Brick, PuzzlePiece};

/// Adjacency threshold: bricks within this many pixels are candidates.
const ADJACENCY_THRESHOLD: f64 = 15.0;

/// Build a Shapely-equivalent polygon from brick-local point coordinates.
fn brick_polygon(brick: &Brick, polygon: &[[f64; 2]]) -> Option<Polygon<f64>> {
    if polygon.len() < 3 {
        return None;
    }
    let coords: Vec<Coord<f64>> = polygon
        .iter()
        .map(|p| Coord {
            x: p[0] + brick.x as f64,
            y: p[1] + brick.y as f64,
        })
        .collect();
    let ring = LineString::new(coords);
    let poly = Polygon::new(ring, vec![]);
    if poly.unsigned_area() < 1.0 {
        return None;
    }
    Some(poly)
}

/// Build vector-based adjacency graph.
///
/// Two bricks are adjacent if their polygons, each buffered by `border_gap`,
/// overlap with intersection area implying shared border >= `min_border`.
pub fn build_adjacency_vector(
    bricks: &[Brick],
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
    gap: f64,
    min_border: f64,
    border_gap: f64,
) -> HashMap<String, HashSet<String>> {
    // Build Shapely-equivalent polygons
    let polys: HashMap<&str, Polygon<f64>> = bricks
        .iter()
        .filter_map(|b| {
            let pts = brick_polygons.get(&b.id)?;
            let poly = brick_polygon(b, pts)?;
            Some((b.id.as_str(), poly))
        })
        .collect();

    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    let n = bricks.len();

    for i in 0..n {
        let a = &bricks[i];
        let pa = match polys.get(a.id.as_str()) {
            Some(p) => p,
            None => continue,
        };

        for j in (i + 1)..n {
            let b = &bricks[j];
            let pb = match polys.get(b.id.as_str()) {
                Some(p) => p,
                None => continue,
            };

            // Bbox pre-filter
            if !((a.x as f64 - gap) < b.right() as f64
                && (a.right() as f64 + gap) > b.x as f64
                && (a.y as f64 - gap) < b.bottom() as f64
                && (a.bottom() as f64 + gap) > b.y as f64)
            {
                continue;
            }

            // Match Python: buffer both polygons by border_gap, intersect,
            // measure border_length = intersection.area / (2 * border_gap)
            use geo_clipper::Clipper;
            let factor = 1000.0;
            let buf_a = pa.offset(border_gap, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);
            let buf_b = pb.offset(border_gap, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);

            if buf_a.0.is_empty() || buf_b.0.is_empty() {
                continue;
            }

            let intersection = Clipper::intersection(&buf_a, &buf_b, factor);
            if intersection.0.is_empty() {
                continue;
            }

            let inter_area: f64 = intersection.0.iter().map(|p| p.unsigned_area()).sum();
            let border_length = if border_gap > 0.0 {
                inter_area / (2.0 * border_gap)
            } else {
                0.0
            };

            if border_length >= min_border {
                adj.entry(a.id.clone()).or_default().insert(b.id.clone());
                adj.entry(b.id.clone()).or_default().insert(a.id.clone());
            }
        }
    }

    adj
}

/// Build adjacency graph directly from AI-native bezier paths — no
/// polygon buffering. Two bricks are adjacent iff their combined shared
/// edge length ≥ `min_border` (in the same AI pymu units the beziers
/// are expressed in).
///
/// A "shared edge" counts:
///   * Line-line overlap: two line segments that are colinear (within
///     `colinear_tol` on perpendicular distance) and whose projections
///     onto the common line overlap. The overlap length is added.
///   * Cubic-cubic match: two cubics with the same unordered endpoint
///     pair (within `endpoint_tol`) AND whose midpoints-at-t=0.5 are
///     within `midpoint_tol`. The cubic's chord length (p0→p3) is added
///     as an approximation of arc length. Good enough for comparing
///     against a fixed threshold.
///
/// Vertex-only contacts naturally contribute 0 — no edges coincide —
/// and get rejected when the sum is below `min_border`.
pub fn build_adjacency_bezier(
    bricks: &[Brick],
    brick_beziers: &HashMap<String, Vec<crate::bezier::BezierPath>>,
    min_border: f64,
) -> HashMap<String, HashSet<String>> {
    use crate::bezier::Segment;

    // Per-edge representation: (from, to, Option<(cp1, cp2)>)
    struct Ed {
        from: [f64; 2],
        to: [f64; 2],
        cp: Option<([f64; 2], [f64; 2])>,
    }
    fn edges_of(paths: &[crate::bezier::BezierPath]) -> Vec<Ed> {
        let mut out = Vec::new();
        for p in paths {
            let mut prev = p.start;
            for s in &p.segments {
                let (to, cp) = match *s {
                    Segment::Line { to } => (to, None),
                    Segment::Cubic { cp1, cp2, to } => (to, Some((cp1, cp2))),
                };
                out.push(Ed { from: prev, to, cp });
                prev = to;
            }
        }
        out
    }

    // Bbox per brick for a cheap pre-filter.
    fn bbox(paths: &[crate::bezier::BezierPath]) -> (f64, f64, f64, f64) {
        let mut x0 = f64::INFINITY;
        let mut y0 = f64::INFINITY;
        let mut x1 = f64::NEG_INFINITY;
        let mut y1 = f64::NEG_INFINITY;
        let mut consume = |p: [f64; 2]| {
            if p[0] < x0 { x0 = p[0]; }
            if p[1] < y0 { y0 = p[1]; }
            if p[0] > x1 { x1 = p[0]; }
            if p[1] > y1 { y1 = p[1]; }
        };
        for p in paths {
            consume(p.start);
            for s in &p.segments { consume(s.end()); }
        }
        (x0, y0, x1, y1)
    }

    fn cubic_mid(p0: [f64; 2], c1: [f64; 2], c2: [f64; 2], p3: [f64; 2]) -> [f64; 2] {
        // de Casteljau at t=0.5
        let lerp = |a: [f64; 2], b: [f64; 2]| [(a[0]+b[0])*0.5, (a[1]+b[1])*0.5];
        let q0 = lerp(p0, c1);
        let q1 = lerp(c1, c2);
        let q2 = lerp(c2, p3);
        let r0 = lerp(q0, q1);
        let r1 = lerp(q1, q2);
        lerp(r0, r1)
    }

    // Tolerances — same scale as the bezier merge so values transfer.
    const ENDPOINT_TOL: f64 = 1.5;
    const MIDPOINT_TOL: f64 = 3.0;
    const COLINEAR_TOL: f64 = 1.0; // perpendicular distance of a line to another's axis

    fn near(a: [f64; 2], b: [f64; 2], tol: f64) -> bool {
        let dx = a[0] - b[0];
        let dy = a[1] - b[1];
        dx * dx + dy * dy <= tol * tol
    }

    /// Shared length between two line segments if they're colinear-with-overlap.
    fn line_line_overlap(a1: [f64; 2], a2: [f64; 2], b1: [f64; 2], b2: [f64; 2]) -> f64 {
        let adx = a2[0] - a1[0];
        let ady = a2[1] - a1[1];
        let bdx = b2[0] - b1[0];
        let bdy = b2[1] - b1[1];
        // Parallel? cross of direction vectors.
        let cross = adx * bdy - ady * bdx;
        let alen2 = adx * adx + ady * ady;
        let blen2 = bdx * bdx + bdy * bdy;
        if alen2 < 1e-9 || blen2 < 1e-9 { return 0.0; }
        // Cross-product magnitude ~ |a| * |b| * sin(theta). Normalise.
        let sin_theta_sq = cross * cross / (alen2 * blen2);
        if sin_theta_sq > (COLINEAR_TOL * COLINEAR_TOL) / alen2.min(blen2) {
            // Not parallel enough
            return 0.0;
        }
        // Check b1 lies near a's line (perpendicular distance).
        let dx = b1[0] - a1[0];
        let dy = b1[1] - a1[1];
        let perp = (dx * ady - dy * adx).abs() / alen2.sqrt();
        if perp > COLINEAR_TOL { return 0.0; }
        // Project b1, b2 onto a's parameterisation (0 at a1, 1 at a2).
        let tb1 = (dx * adx + dy * ady) / alen2;
        let bx2 = b2[0] - a1[0];
        let by2 = b2[1] - a1[1];
        let tb2 = (bx2 * adx + by2 * ady) / alen2;
        let (lo, hi) = (tb1.min(tb2), tb1.max(tb2));
        let lo = lo.max(0.0);
        let hi = hi.min(1.0);
        if hi <= lo { return 0.0; }
        (hi - lo) * alen2.sqrt()
    }

    /// Shared length between two cubic segments if they describe the same curve.
    fn cubic_cubic_shared(
        ap0: [f64; 2], ac1: [f64; 2], ac2: [f64; 2], ap3: [f64; 2],
        bp0: [f64; 2], bc1: [f64; 2], bc2: [f64; 2], bp3: [f64; 2],
    ) -> f64 {
        // Either-direction endpoint match + midpoint match.
        let a_mid = cubic_mid(ap0, ac1, ac2, ap3);
        let b_mid = cubic_mid(bp0, bc1, bc2, bp3);
        let endpoints_match = (near(ap0, bp0, ENDPOINT_TOL) && near(ap3, bp3, ENDPOINT_TOL))
            || (near(ap0, bp3, ENDPOINT_TOL) && near(ap3, bp0, ENDPOINT_TOL));
        if !endpoints_match { return 0.0; }
        if !near(a_mid, b_mid, MIDPOINT_TOL) { return 0.0; }
        // Approximate arc length by chord length (conservative).
        let dx = ap3[0] - ap0[0];
        let dy = ap3[1] - ap0[1];
        (dx * dx + dy * dy).sqrt()
    }

    // Pre-compute edges and bboxes.
    let mut brick_edges: HashMap<&str, Vec<Ed>> = HashMap::new();
    let mut brick_bbox: HashMap<&str, (f64, f64, f64, f64)> = HashMap::new();
    for b in bricks {
        if let Some(paths) = brick_beziers.get(&b.id) {
            brick_edges.insert(b.id.as_str(), edges_of(paths));
            brick_bbox.insert(b.id.as_str(), bbox(paths));
        }
    }

    use rayon::prelude::*;
    let n = bricks.len();
    // Parallel sweep: each row (i) computes its set of neighbours j > i,
    // then we sequentially fold the results into the adjacency map.
    let pairs: Vec<(String, String)> = (0..n)
        .into_par_iter()
        .flat_map_iter(|i| {
            let a_id = bricks[i].id.as_str();
            let a_edges = brick_edges.get(a_id);
            let abb = brick_bbox.get(a_id).copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
            let mut local: Vec<(String, String)> = Vec::new();
            if let Some(a_edges) = a_edges {
                for j in (i + 1)..n {
                    let b_id = bricks[j].id.as_str();
                    let b_edges = match brick_edges.get(b_id) {
                        Some(v) => v,
                        None => continue,
                    };
                    let bbb = brick_bbox.get(b_id).copied().unwrap_or((0.0, 0.0, 0.0, 0.0));
                    let slack = ENDPOINT_TOL;
                    if abb.2 + slack < bbb.0 || bbb.2 + slack < abb.0
                        || abb.3 + slack < bbb.1 || bbb.3 + slack < abb.1
                    {
                        continue;
                    }
                    let mut total_shared = 0.0;
                    for ea in a_edges {
                        for eb in b_edges {
                            let share = match (ea.cp, eb.cp) {
                                (None, None) => line_line_overlap(ea.from, ea.to, eb.from, eb.to),
                                (Some((ac1, ac2)), Some((bc1, bc2))) => cubic_cubic_shared(
                                    ea.from, ac1, ac2, ea.to,
                                    eb.from, bc1, bc2, eb.to,
                                ),
                                _ => 0.0,
                            };
                            total_shared += share;
                        }
                    }
                    if total_shared >= min_border {
                        local.push((a_id.to_string(), b_id.to_string()));
                    }
                }
            }
            local
        })
        .collect();

    let mut adj: HashMap<String, HashSet<String>> = HashMap::new();
    for (a, b) in pairs {
        adj.entry(a.clone()).or_default().insert(b.clone());
        adj.entry(b).or_default().insert(a);
    }
    adj
}

/// Areas computed from bezier paths directly — matches `build_adjacency_bezier`.
pub fn compute_bezier_areas(
    bricks: &[Brick],
    brick_beziers: &HashMap<String, Vec<crate::bezier::BezierPath>>,
) -> HashMap<String, f64> {
    bricks
        .iter()
        .map(|b| {
            let area = brick_beziers
                .get(&b.id)
                .map(|paths| {
                    paths.iter()
                        .map(|bp| {
                            // Shoelace on tessellated ring (chord polygon — sufficient
                            // for relative area comparisons).
                            let pts = bp.tessellate(8);
                            if pts.len() < 3 { return 0.0; }
                            let mut a = 0.0;
                            let n = pts.len();
                            for i in 0..n {
                                let j = (i + 1) % n;
                                a += pts[i][0] * pts[j][1] - pts[j][0] * pts[i][1];
                            }
                            a.abs() / 2.0
                        })
                        .sum::<f64>()
                })
                .unwrap_or(b.area() as f64);
            (b.id.clone(), area)
        })
        .collect()
}

/// Compute polygon areas for all bricks.
pub fn compute_polygon_areas(
    bricks: &[Brick],
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
) -> HashMap<String, f64> {
    bricks
        .iter()
        .map(|b| {
            let area = brick_polygons
                .get(&b.id)
                .and_then(|pts| brick_polygon(b, pts))
                .map(|p| p.unsigned_area())
                .unwrap_or(b.area() as f64);
            (b.id.clone(), area)
        })
        .collect()
}

/// Compute bounding box for a set of bricks.
fn compute_piece_bbox(brick_ids: &[String], bricks_by_id: &HashMap<&str, &Brick>) -> (i32, i32, i32, i32) {
    let mut x0 = i32::MAX;
    let mut y0 = i32::MAX;
    let mut x1 = i32::MIN;
    let mut y1 = i32::MIN;
    for bid in brick_ids {
        if let Some(b) = bricks_by_id.get(bid.as_str()) {
            x0 = x0.min(b.x);
            y0 = y0.min(b.y);
            x1 = x1.max(b.right());
            y1 = y1.max(b.bottom());
        }
    }
    (x0, y0, x1 - x0, y1 - y0)
}

/// Merge bricks into puzzle pieces using area-balanced adjacency grouping.
pub fn merge_bricks(
    bricks: &[Brick],
    target_piece_count: usize,
    seed: u64,
    adjacency: &HashMap<String, HashSet<String>>,
    brick_areas: &HashMap<String, f64>,
) -> Vec<PuzzlePiece> {
    let bricks_by_id: HashMap<&str, &Brick> = bricks.iter().map(|b| (b.id.as_str(), b)).collect();
    let all_ids: HashSet<String> = bricks.iter().map(|b| b.id.clone()).collect();
    let target_count = target_piece_count.max(1);
    eprintln!("[puzzle] merge_bricks: target_piece_count={target_piece_count} total_bricks={}", all_ids.len());

    // Phase 0: exclude oversized bricks
    let total_area: f64 = all_ids.iter().map(|id| brick_areas.get(id).copied().unwrap_or(0.0)).sum();
    eprintln!("[puzzle] total_area={total_area:.0}");
    let mut fixed_ids: HashSet<String> = HashSet::new();
    for iter in 0..10 {
        let target_area = total_area / target_count.max(1) as f64;
        let new_fixed: HashSet<String> = all_ids
            .iter()
            .filter(|id| brick_areas.get(*id).copied().unwrap_or(0.0) >= target_area)
            .cloned()
            .collect();
        eprintln!("[puzzle] phase0 iter={iter} target_area={target_area:.0} fixed={}", new_fixed.len());
        if new_fixed == fixed_ids {
            break;
        }
        fixed_ids = new_fixed;
    }

    let mergeable_ids: HashSet<String> = all_ids.difference(&fixed_ids).cloned().collect();
    let target_mergeable = target_count.saturating_sub(fixed_ids.len()).max(1);
    eprintln!("[puzzle] fixed_ids={} mergeable_ids={} target_mergeable={target_mergeable}", fixed_ids.len(), mergeable_ids.len());
    let mergeable_area: f64 = mergeable_ids
        .iter()
        .map(|id| brick_areas.get(id).copied().unwrap_or(0.0))
        .sum();
    let target_area = mergeable_area / target_mergeable as f64;

    let mut rng = ChaCha8Rng::seed_from_u64(seed);

    // Initialize: each mergeable brick is its own piece
    let mut piece_of: HashMap<String, String> = HashMap::new();
    let mut pieces_dict: HashMap<String, Vec<String>> = HashMap::new();
    let mut piece_area: HashMap<String, f64> = HashMap::new();

    for bid in &mergeable_ids {
        piece_of.insert(bid.clone(), bid.clone());
        pieces_dict.insert(bid.clone(), vec![bid.clone()]);
        piece_area.insert(bid.clone(), brick_areas.get(bid).copied().unwrap_or(0.0));
    }

    // Build piece-level adjacency
    let mut piece_adj: HashMap<String, HashSet<String>> = HashMap::new();
    for bid in &mergeable_ids {
        let pid = &piece_of[bid];
        if let Some(neighbors) = adjacency.get(bid) {
            for nbr in neighbors {
                if !mergeable_ids.contains(nbr) {
                    continue;
                }
                let npid = &piece_of[nbr];
                if npid != pid {
                    piece_adj.entry(pid.clone()).or_default().insert(npid.clone());
                    piece_adj.entry(npid.clone()).or_default().insert(pid.clone());
                }
            }
        }
    }

    // Phase 1: greedy merge
    eprintln!("[puzzle] phase1 start: pieces_dict.len()={} target_mergeable={target_mergeable}", pieces_dict.len());
    let mut merge_iter = 0usize;
    while pieces_dict.len() > target_mergeable {
        let mut candidates: Vec<String> = pieces_dict.keys().cloned().collect();
        // Sort by area, breaking ties on the piece id so the order is
        // deterministic. `pieces_dict.keys()` iterates the HashMap in
        // a per-process-random order, and `sort_by` is stable, so without
        // the id tiebreaker, two pieces with identical area would keep
        // their (random) input order and the merge would pick different
        // neighbours each run.
        candidates.sort_by(|a, b| {
            piece_area
                .get(a)
                .unwrap_or(&0.0)
                .partial_cmp(piece_area.get(b).unwrap_or(&0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
                .then_with(|| a.cmp(b))
        });

        let mut merged = false;
        for smallest_pid in &candidates {
            let neighbors = match piece_adj.get(smallest_pid) {
                Some(n) if !n.is_empty() => n.clone(),
                _ => continue,
            };

            let cur_area = piece_area.get(smallest_pid).copied().unwrap_or(0.0);
            // `neighbors` is a HashSet — its iteration order is
            // per-process-random. Sort before the seeded shuffle so
            // the shuffle input is deterministic; otherwise the same
            // (seed, brick set, adjacency) trio produces different
            // merges across runs.
            let mut nbr_list: Vec<String> = neighbors.into_iter().collect();
            nbr_list.sort();
            nbr_list.shuffle(&mut rng);

            let mut best_nbr: Option<String> = None;
            let mut best_score = f64::INFINITY;

            for npid in &nbr_list {
                if !pieces_dict.contains_key(npid) {
                    continue;
                }
                let combined = cur_area + piece_area.get(npid).copied().unwrap_or(0.0);
                let mut score = (combined - target_area).abs();
                if combined > target_area * 1.5 {
                    score += combined;
                }
                // Compactness penalty
                let merged_ids: Vec<String> = pieces_dict[smallest_pid]
                    .iter()
                    .chain(pieces_dict[npid].iter())
                    .cloned()
                    .collect();
                let (_, _, bw, bh) = compute_piece_bbox(&merged_ids, &bricks_by_id);
                let aspect = bw.max(bh) as f64 / bw.min(bh).max(1) as f64;
                score += target_area * (aspect - 1.0) * 0.3;

                if score < best_score {
                    best_score = score;
                    best_nbr = Some(npid.clone());
                }
            }

            if let Some(absorb_pid) = best_nbr {
                let keep_pid = smallest_pid.clone();
                // Merge absorb into keep
                let absorbed_bricks = pieces_dict.remove(&absorb_pid)
                    .expect("absorb_pid must exist in pieces_dict during merge");
                pieces_dict.get_mut(&keep_pid)
                    .expect("keep_pid must exist in pieces_dict during merge")
                    .extend(absorbed_bricks.iter().cloned());
                *piece_area.get_mut(&keep_pid)
                    .expect("keep_pid must exist in piece_area during merge") +=
                    piece_area.remove(&absorb_pid).unwrap_or(0.0);
                for bid in &absorbed_bricks {
                    piece_of.insert(bid.clone(), keep_pid.clone());
                }

                // Update adjacency
                let absorb_neighbors = piece_adj.remove(&absorb_pid).unwrap_or_default();
                for neighbor_pid in &absorb_neighbors {
                    if *neighbor_pid == keep_pid {
                        continue;
                    }
                    if let Some(ns) = piece_adj.get_mut(neighbor_pid) {
                        ns.remove(&absorb_pid);
                        ns.insert(keep_pid.clone());
                    }
                    piece_adj.entry(keep_pid.clone()).or_default().insert(neighbor_pid.clone());
                }
                if let Some(ns) = piece_adj.get_mut(&keep_pid) {
                    ns.remove(&absorb_pid);
                }

                merged = true;
                break;
            }
        }

        merge_iter += 1;
        if !merged {
            eprintln!("[puzzle] phase1 stuck at iter={merge_iter} pieces_dict.len()={}", pieces_dict.len());
            break;
        }
    }
    eprintln!("[puzzle] phase1 done: iters={merge_iter} mergeable_pieces={} fixed_pieces={}", pieces_dict.len(), fixed_ids.len());

    // Build result
    let mut result: Vec<PuzzlePiece> = Vec::new();

    // Fixed solo pieces first
    let mut sorted_fixed: Vec<&String> = fixed_ids.iter().collect();
    sorted_fixed.sort();
    for bid in sorted_fixed {
        let b = bricks_by_id[bid.as_str()];
        result.push(PuzzlePiece {
            id: format!("p{}", result.len()),
            brick_ids: vec![bid.clone()],
            x: b.x,
            y: b.y,
            width: b.width,
            height: b.height,
        });
    }

    // Merged pieces
    for (_, brick_ids) in &pieces_dict {
        let (x, y, w, h) = compute_piece_bbox(brick_ids, &bricks_by_id);
        result.push(PuzzlePiece {
            id: format!("p{}", result.len()),
            brick_ids: brick_ids.clone(),
            x,
            y,
            width: w,
            height: h,
        });
    }

    // Re-assign IDs
    for (i, piece) in result.iter_mut().enumerate() {
        piece.id = format!("p{i}");
    }

    eprintln!("[puzzle] final result: {} pieces (target was {target_count})", result.len());
    result
}

/// Find the nearest points between two polygon rings.
/// Returns (distance, point_on_a, point_on_b).
pub fn nearest_edge_points(
    ring_a: &LineString<f64>,
    ring_b: &LineString<f64>,
) -> (f64, Coord<f64>, Coord<f64>) {
    let mut best_dist = f64::INFINITY;
    let mut best_a = ring_a.0[0];
    let mut best_b = ring_b.0[0];

    // Check each vertex of A against each edge of B, and vice versa
    for &pa in &ring_a.0 {
        for seg in ring_b.lines() {
            let (dist, closest) = point_to_segment_dist(pa, seg.start, seg.end);
            if dist < best_dist {
                best_dist = dist;
                best_a = pa;
                best_b = closest;
            }
        }
    }
    for &pb in &ring_b.0 {
        for seg in ring_a.lines() {
            let (dist, closest) = point_to_segment_dist(pb, seg.start, seg.end);
            if dist < best_dist {
                best_dist = dist;
                best_a = closest;
                best_b = pb;
            }
        }
    }

    (best_dist, best_a, best_b)
}

/// Distance from point P to segment AB, and the closest point on AB.
pub fn point_to_segment_dist(p: Coord<f64>, a: Coord<f64>, b: Coord<f64>) -> (f64, Coord<f64>) {
    let ab = Coord { x: b.x - a.x, y: b.y - a.y };
    let ap = Coord { x: p.x - a.x, y: p.y - a.y };
    let len_sq = ab.x * ab.x + ab.y * ab.y;
    if len_sq < 1e-12 {
        let d = ((p.x - a.x).powi(2) + (p.y - a.y).powi(2)).sqrt();
        return (d, a);
    }
    let t = (ap.x * ab.x + ap.y * ab.y) / len_sq;
    let t = t.clamp(0.0, 1.0);
    let closest = Coord { x: a.x + t * ab.x, y: a.y + t * ab.y };
    let d = ((p.x - closest.x).powi(2) + (p.y - closest.y).powi(2)).sqrt();
    (d, closest)
}

/// Create a thin rectangle bridging two points.
pub fn make_bridge_rect(a: Coord<f64>, b: Coord<f64>, width: f64) -> Polygon<f64> {
    let dx = b.x - a.x;
    let dy = b.y - a.y;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 1e-12 {
        // Degenerate: points are the same, make a tiny square
        let hw = width / 2.0;
        return Polygon::new(
            LineString::new(vec![
                Coord { x: a.x - hw, y: a.y - hw },
                Coord { x: a.x + hw, y: a.y - hw },
                Coord { x: a.x + hw, y: a.y + hw },
                Coord { x: a.x - hw, y: a.y + hw },
                Coord { x: a.x - hw, y: a.y - hw },
            ]),
            vec![],
        );
    }
    // Normal perpendicular to the line AB
    let nx = -dy / len * width / 2.0;
    let ny = dx / len * width / 2.0;
    Polygon::new(
        LineString::new(vec![
            Coord { x: a.x + nx, y: a.y + ny },
            Coord { x: b.x + nx, y: b.y + ny },
            Coord { x: b.x - nx, y: b.y - ny },
            Coord { x: a.x - nx, y: a.y - ny },
            Coord { x: a.x + nx, y: a.y + ny },
        ]),
        vec![],
    )
}

/// Compute merged polygon outlines for each piece.
///
/// Unions the original brick polygons (unbuffered) to preserve exact vector shapes.
/// Bridges gaps between disconnected components with thin rectangles.
/// Fills small interior holes.
pub fn compute_piece_polygons(
    pieces: &[PuzzlePiece],
    bricks_by_id: &HashMap<String, Brick>,
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
) -> HashMap<String, Vec<[f64; 2]>> {
    use geo::algorithm::bool_ops::BooleanOps;

    let mut result: HashMap<String, Vec<[f64; 2]>> = HashMap::new();

    for piece in pieces {
        let mut polys: Vec<Polygon<f64>> = Vec::new();
        let debug_piece = piece.id == "p12" || piece.id == "p11" || piece.id == "p13";

        for bid in &piece.brick_ids {
            let brick = match bricks_by_id.get(bid) {
                Some(b) => b,
                None => continue,
            };
            let pts = match brick_polygons.get(bid) {
                Some(p) if p.len() >= 3 => p,
                _ => {
                    // No vector polygon — skip this brick
                    continue;
                }
            };

            // Brick-local → canvas coords
            let coords: Vec<Coord<f64>> = pts.iter()
                .map(|p| Coord {
                    x: p[0] + brick.x as f64,
                    y: p[1] + brick.y as f64,
                })
                .collect();

            if coords.len() < 3 {
                continue;
            }

            let mut ring_coords = coords;
            // Close the ring if needed
            if ring_coords.first() != ring_coords.last() {
                ring_coords.push(ring_coords[0]);
            }

            let poly = Polygon::new(LineString::new(ring_coords), vec![]);
            if poly.unsigned_area() < 1.0 {
                continue;
            }
            if debug_piece {
                let bb = poly.bounding_rect();
                eprintln!("[piece-debug] {} brick {} area={:.0} pts={} bbox={:?}",
                    piece.id, bid, poly.unsigned_area(), poly.exterior().0.len(),
                    bb.map(|r| (r.min().x as i32, r.min().y as i32, r.max().x as i32, r.max().y as i32)));
                // Dump all vertices
                for (vi, c) in poly.exterior().0.iter().enumerate() {
                    eprintln!("[piece-debug]   v{}: ({:.1}, {:.1})", vi, c.x, c.y);
                }
            }
            polys.push(poly);
        }

        if debug_piece {
            eprintln!("[piece-debug] {} has {} brick polygons", piece.id, polys.len());
        }

        if polys.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        // Union brick polygons with a tiny expansion (0.1px) to merge polygons
        // that touch at single vertices. This is invisible but ensures the clipper
        // can fuse vertex-touching polygons into one shape.
        use geo_clipper::Clipper;

        let factor = 1000.0;
        const EPSILON: f64 = 0.1; // sub-pixel expansion to fuse touching vertices
        const HOLE_AREA_THRESHOLD: f64 = 100.0;

        // Expand each polygon by a tiny epsilon, union, then erode back
        let mut expanded: Vec<Polygon<f64>> = Vec::new();
        for poly in &polys {
            let exp = poly.offset(EPSILON, geo_clipper::JoinType::Miter(2.0),
                                  geo_clipper::EndType::ClosedPolygon, factor);
            for p in exp.0 {
                if p.unsigned_area() > 1.0 {
                    expanded.push(p);
                }
            }
        }
        if expanded.is_empty() {
            expanded = polys.clone();
        }

        let mut union = geo::MultiPolygon(vec![expanded[0].clone()]);
        for poly in &expanded[1..] {
            union = Clipper::union(&union, poly, factor);
        }
        union.0.retain(|p| p.unsigned_area() > 1.0);

        // Erode back by epsilon to restore original shape
        let mut eroded_union: Vec<Polygon<f64>> = Vec::new();
        for poly in &union.0 {
            let er = poly.offset(-EPSILON, geo_clipper::JoinType::Miter(2.0),
                                 geo_clipper::EndType::ClosedPolygon, factor);
            for p in er.0 {
                if p.unsigned_area() > 1.0 {
                    eroded_union.push(p);
                }
            }
        }
        if !eroded_union.is_empty() {
            union = geo::MultiPolygon(eroded_union);
        }

        if debug_piece {
            eprintln!("[piece-debug] {} union produced {} components", piece.id, union.0.len());
            for (ci, comp) in union.0.iter().enumerate() {
                let bb = comp.bounding_rect();
                eprintln!("[piece-debug]   component {} area={:.0} pts={} holes={} bbox={:?}",
                    ci, comp.unsigned_area(), comp.exterior().0.len(), comp.interiors().len(),
                    bb.map(|r| (r.min().x as i32, r.min().y as i32, r.max().x as i32, r.max().y as i32)));
            }
        }

        if union.0.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        // Log if still disconnected after epsilon union (shouldn't happen normally)
        if union.0.len() > 1 {
            eprintln!("[piece-gap] {} still has {} components after epsilon union ({} bricks)",
                piece.id, union.0.len(), piece.brick_ids.len());
        }

        // Take the largest polygon
        let mut final_poly = union.0.into_iter()
            .max_by(|a, b| a.unsigned_area().partial_cmp(&b.unsigned_area())
                .unwrap_or(std::cmp::Ordering::Equal))
            .expect("union is non-empty");

        // Fill small interior holes
        let (exterior, interiors) = final_poly.into_inner();
        let kept_holes: Vec<LineString<f64>> = interiors.into_iter()
            .filter(|hole| {
                let hole_poly = Polygon::new(hole.clone(), vec![]);
                hole_poly.unsigned_area() >= HOLE_AREA_THRESHOLD
            })
            .collect();
        final_poly = Polygon::new(exterior, kept_holes);

        let coords: Vec<[f64; 2]> = final_poly.exterior().0.iter()
            .map(|c| [c.x, c.y])
            .collect();
        result.insert(piece.id.clone(), coords);
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ai_parser;

    #[test]
    fn test_merge_ny1() {
        let ai_path = std::path::PathBuf::from("../../in/_NY1.ai");
        if !ai_path.exists() {
            eprintln!("Skipping: in/_NY1.ai not found");
            return;
        }

        let (placements, _meta, _ai_data) =
            ai_parser::parse_ai(&ai_path, crate::CANVAS_HEIGHT_PX as i32).unwrap();

        // Convert to Brick + polygons
        let mut bricks: Vec<Brick> = Vec::new();
        let mut polygons: HashMap<String, Vec<[f64; 2]>> = HashMap::new();
        for (i, p) in placements.iter().enumerate() {
            let id = format!("b{i}");
            bricks.push(Brick {
                id: id.clone(),
                x: p.x,
                y: p.y,
                width: p.width,
                height: p.height,
                brick_type: p.layer_type.clone(),
            });
            if let Some(poly) = &p.polygon {
                polygons.insert(id, poly.clone());
            }
        }

        let adj = build_adjacency_vector(&bricks, &polygons, ADJACENCY_THRESHOLD, 10.0, 2.0);
        let areas = compute_polygon_areas(&bricks, &polygons);

        eprintln!("Adjacency: {} bricks have neighbors", adj.len());

        let pieces = merge_bricks(&bricks, 60, 42, &adj, &areas);
        eprintln!("Pieces: {}", pieces.len());
        assert_eq!(pieces.len(), 60, "Expected 60 pieces");
    }

    // ── Bezier-native adjacency + areas (synthetic geometry) ───────────

    use crate::bezier::{BezierPath, Segment};

    /// Build a bezier path that traces an axis-aligned rectangle (x..x+w, y..y+h)
    /// in pymu coordinates. Helper for the synthetic adjacency tests.
    fn rect(x: f64, y: f64, w: f64, h: f64) -> BezierPath {
        BezierPath {
            start: [x, y],
            segments: vec![
                Segment::Line { to: [x + w, y] },
                Segment::Line { to: [x + w, y + h] },
                Segment::Line { to: [x, y + h] },
                Segment::Line { to: [x, y] },
            ],
        }
    }

    fn brick(id: &str, x: i32, y: i32, w: i32, h: i32) -> Brick {
        Brick {
            id: id.to_string(),
            x, y, width: w, height: h,
            brick_type: "vector_brick".to_string(),
        }
    }

    #[test]
    fn build_adjacency_bezier_two_rectangles_sharing_edge() {
        // Two unit rectangles touching at x=10 — A on the left, B on the right.
        // Shared edge length = 10 → above the 5-pymu min_border floor.
        let bricks = vec![brick("a", 0, 0, 10, 10), brick("b", 10, 0, 10, 10)];
        let mut beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
        beziers.insert("a".into(), vec![rect(0.0, 0.0, 10.0, 10.0)]);
        beziers.insert("b".into(), vec![rect(10.0, 0.0, 10.0, 10.0)]);

        let adj = build_adjacency_bezier(&bricks, &beziers, 5.0);
        assert!(
            adj.get("a").map(|s| s.contains("b")).unwrap_or(false),
            "a → b adjacency missing: {:?}", adj
        );
        assert!(
            adj.get("b").map(|s| s.contains("a")).unwrap_or(false),
            "b → a adjacency missing: {:?}", adj
        );
    }

    #[test]
    fn build_adjacency_bezier_separated_rectangles_are_not_adjacent() {
        // 100-unit gap between A and B → no shared edge, no adjacency.
        let bricks = vec![brick("a", 0, 0, 10, 10), brick("b", 110, 0, 10, 10)];
        let mut beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
        beziers.insert("a".into(), vec![rect(0.0, 0.0, 10.0, 10.0)]);
        beziers.insert("b".into(), vec![rect(110.0, 0.0, 10.0, 10.0)]);

        let adj = build_adjacency_bezier(&bricks, &beziers, 5.0);
        assert!(adj.get("a").map(|s| !s.contains("b")).unwrap_or(true));
        assert!(adj.get("b").map(|s| !s.contains("a")).unwrap_or(true));
    }

    #[test]
    fn compute_bezier_areas_rectangle_matches_w_times_h() {
        let bricks = vec![brick("r", 0, 0, 50, 30)];
        let mut beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
        beziers.insert("r".into(), vec![rect(0.0, 0.0, 50.0, 30.0)]);
        let areas = compute_bezier_areas(&bricks, &beziers);
        let a = areas.get("r").copied().unwrap_or(0.0);
        // Allow 1% slack — areas computed via shoelace on a coarse
        // tessellated ring; an exact rectangle is a degenerate case
        // where the chord polygon equals the analytical shape.
        assert!((a - 1500.0).abs() < 15.0, "expected ~1500, got {}", a);
    }

    #[test]
    fn compute_bezier_areas_triangle_matches_half_base_times_height() {
        // Right triangle with legs 100 along x and y → area = 5000.
        let tri = BezierPath {
            start: [0.0, 0.0],
            segments: vec![
                Segment::Line { to: [100.0, 0.0] },
                Segment::Line { to: [0.0, 100.0] },
                Segment::Line { to: [0.0, 0.0] },
            ],
        };
        let bricks = vec![brick("t", 0, 0, 100, 100)];
        let mut beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
        beziers.insert("t".into(), vec![tri]);
        let areas = compute_bezier_areas(&bricks, &beziers);
        let a = areas.get("t").copied().unwrap_or(0.0);
        assert!((a - 5000.0).abs() < 50.0, "expected ~5000, got {}", a);
    }

    #[test]
    fn compute_bezier_areas_missing_brick_falls_back_to_bbox() {
        // Brick declared without a bezier path: the merge / sort step
        // shouldn't panic. The function falls back to bbox area
        // (`b.area()`) so adjacency never sees a zero divisor.
        let bricks = vec![brick("ghost", 0, 0, 10, 10)];
        let beziers: HashMap<String, Vec<BezierPath>> = HashMap::new();
        let areas = compute_bezier_areas(&bricks, &beziers);
        // 10 × 10 = 100 (b.area() returns w*h).
        assert_eq!(areas.get("ghost").copied(), Some(100.0));
    }
}
