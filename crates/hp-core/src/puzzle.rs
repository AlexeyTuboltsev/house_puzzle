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

/// Build a geo polygon from canvas-coord point coordinates.
fn brick_polygon(_brick: &Brick, polygon: &[[f64; 2]]) -> Option<Polygon<f64>> {
    if polygon.len() < 3 {
        return None;
    }
    let coords: Vec<Coord<f64>> = polygon
        .iter()
        .map(|p| Coord { x: p[0], y: p[1] })
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
    clip_x0: f64,
    clip_y0: f64,
    scale: f64,
) -> HashMap<String, HashSet<String>> {
    // All vector operations in AI pymu scale. Convert canvas-px params to AI units.
    let inv_scale = 1.0 / scale;
    let gap_ai = gap * inv_scale;
    let min_border_ai = min_border * inv_scale;
    let border_gap_ai = border_gap * inv_scale;

    // Polygons are in AI pymu coords — use directly.
    let polys: HashMap<&str, Polygon<f64>> = bricks
        .iter()
        .filter_map(|b| {
            let pts = brick_polygons.get(&b.id)?;
            if pts.len() < 3 { return None; }
            let coords: Vec<Coord<f64>> = pts.iter()
                .map(|p| Coord { x: p[0], y: p[1] })
                .collect();
            let poly = Polygon::new(LineString::new(coords), vec![]);
            if poly.unsigned_area() < 1.0 { return None; }
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

            // Bbox pre-filter — use polygon bboxes in AI units (brick.x is canvas px)
            let (abb_min, abb_max) = (pa.bounding_rect().map(|r| (r.min(), r.max()))
                .map(|(mn, mx)| ((mn.x, mn.y), (mx.x, mx.y)))
                .unwrap_or(((0.0, 0.0), (0.0, 0.0))));
            let (bbb_min, bbb_max) = (pb.bounding_rect().map(|r| (r.min(), r.max()))
                .map(|(mn, mx)| ((mn.x, mn.y), (mx.x, mx.y)))
                .unwrap_or(((0.0, 0.0), (0.0, 0.0))));
            if !((abb_min.0 - gap_ai) < bbb_max.0
                && (abb_max.0 + gap_ai) > bbb_min.0
                && (abb_min.1 - gap_ai) < bbb_max.1
                && (abb_max.1 + gap_ai) > bbb_min.1)
            {
                continue;
            }

            // Buffer in AI units, intersect, measure border_length in AI units.
            use geo_clipper::Clipper;
            let factor = 10000.0; // higher precision for AI-scale coords
            let buf_a = pa.offset(border_gap_ai, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);
            let buf_b = pb.offset(border_gap_ai, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);

            if buf_a.0.is_empty() || buf_b.0.is_empty() {
                continue;
            }

            let intersection = Clipper::intersection(&buf_a, &buf_b, factor);
            if intersection.0.is_empty() {
                continue;
            }

            let inter_area: f64 = intersection.0.iter().map(|p| p.unsigned_area()).sum();
            let border_length = if border_gap_ai > 0.0 {
                inter_area / (2.0 * border_gap_ai)
            } else {
                0.0
            };

            // Additional check: if intersection is small and roughly circular
            // (corner touch), the border length is misleadingly non-zero because
            // buffered corners overlap. Require the intersection BOUNDING BOX to
            // have at least one dimension >= min_border.
            let inter_bbox: (f64, f64, f64, f64) = intersection.0.iter()
                .map(|p| {
                    let mut x0 = f64::INFINITY;
                    let mut y0 = f64::INFINITY;
                    let mut x1 = f64::NEG_INFINITY;
                    let mut y1 = f64::NEG_INFINITY;
                    for c in p.exterior().0.iter() {
                        x0 = x0.min(c.x); y0 = y0.min(c.y);
                        x1 = x1.max(c.x); y1 = y1.max(c.y);
                    }
                    (x0, y0, x1, y1)
                })
                .fold((f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
                    |acc, b| (acc.0.min(b.0), acc.1.min(b.1), acc.2.max(b.2), acc.3.max(b.3)));
            let inter_w = (inter_bbox.2 - inter_bbox.0).max(0.0);
            let inter_h = (inter_bbox.3 - inter_bbox.1).max(0.0);
            let max_extent = inter_w.max(inter_h);

            let passed = border_length >= min_border && max_extent >= min_border;
            if border_length > 0.5 && !passed {
                eprintln!("[adj-reject] {} <-> {} bl={:.2} inter_area={:.2} extent={:.2}x{:.2}",
                    a.id, b.id, border_length, inter_area, inter_w, inter_h);
            }
            if passed {
                adj.entry(a.id.clone()).or_default().insert(b.id.clone());
                adj.entry(b.id.clone()).or_default().insert(a.id.clone());
            }
        }
    }

    adj
}

/// Compute polygon areas for all bricks.
pub fn compute_polygon_areas(
    bricks: &[Brick],
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
) -> HashMap<String, f64> {
    bricks
        .iter()
        .map(|b| {
            // Polygons are in AI pymu coords — compute area there.
            // Merge algorithm uses areas relatively, so the scale factor doesn't matter.
            let area = brick_polygons
                .get(&b.id)
                .and_then(|pts| {
                    if pts.len() < 3 { return None; }
                    let coords: Vec<Coord<f64>> = pts.iter()
                        .map(|p| Coord { x: p[0], y: p[1] })
                        .collect();
                    let poly = Polygon::new(LineString::new(coords), vec![]);
                    if poly.unsigned_area() < 0.001 { None } else { Some(poly.unsigned_area()) }
                })
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
        candidates.sort_by(|a, b| {
            piece_area
                .get(a)
                .unwrap_or(&0.0)
                .partial_cmp(piece_area.get(b).unwrap_or(&0.0))
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        let mut merged = false;
        for smallest_pid in &candidates {
            let neighbors = match piece_adj.get(smallest_pid) {
                Some(n) if !n.is_empty() => n.clone(),
                _ => continue,
            };

            let cur_area = piece_area.get(smallest_pid).copied().unwrap_or(0.0);
            let mut nbr_list: Vec<String> = neighbors.into_iter().collect();
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

/// Remove degenerate spikes from a polygon coordinate list.
///
/// Clipper union of polygons sharing edges in the same winding direction produces
/// spikes: the outline traces along the shared internal edge and comes back.
/// Pattern: `..., A, B, A, ...` where v[i] == v[i+2] — remove B and the second A.
/// Also removes consecutive duplicate points. Repeats until stable.
fn remove_polygon_spikes(coords: Vec<[f64; 2]>) -> Vec<[f64; 2]> {
    remove_polygon_spikes_named(coords, "?")
}

fn remove_polygon_spikes_named(mut coords: Vec<[f64; 2]>, piece_id: &str) -> Vec<[f64; 2]> {
    const EPS: f64 = 0.01;
    let eq = |a: [f64; 2], b: [f64; 2]| (a[0] - b[0]).abs() < EPS && (a[1] - b[1]).abs() < EPS;
    let input_len = coords.len();

    loop {
        let before = coords.len();
        if before < 3 { break; }

        // Remove consecutive duplicates (including wrap-around: last == first)
        let mut deduped = Vec::with_capacity(coords.len());
        for pt in &coords {
            if deduped.last().map_or(true, |prev: &[f64; 2]| !eq(*prev, *pt)) {
                deduped.push(*pt);
            }
        }
        while deduped.len() > 1 && eq(*deduped.first().unwrap(), *deduped.last().unwrap()) {
            deduped.pop();
        }
        coords = deduped;
        if coords.len() < 3 { break; }

        let n = coords.len();

        // Find a spike: v[i] == v[i+k] for small k AND the enclosed loop has
        // near-zero area (path goes out and comes back along nearly the same line).
        // Loop area threshold ~1.0 square pixel — too large = legitimate lobe, not a spike.
        let mut found: Option<(usize, usize)> = None;
        'outer: for i in 0..n {
            for k in 2..=6.min(n - 1) {
                let ik = (i + k) % n;
                if !eq(coords[i], coords[ik]) { continue; }
                let mut area = 0.0_f64;
                for j in 0..k {
                    let a = coords[(i + j) % n];
                    let b = coords[(i + j + 1) % n];
                    area += a[0] * b[1] - b[0] * a[1];
                }
                if area.abs() < 2.0 {
                    found = Some((i, k));
                    break 'outer;
                }
            }
        }

        if let Some((i, k)) = found {
            let to_remove: Vec<usize> = (1..=k).map(|j| (i + j) % n).collect();
            let removed_pts: Vec<[f64; 2]> = to_remove.iter().map(|&j| coords[j]).collect();
            eprintln!("[spike-detected] piece={} at v{} k={} DETECTED (not removing yet) pts: {}",
                piece_id, i, k,
                removed_pts.iter().map(|p| format!("({:.2},{:.2})", p[0], p[1])).collect::<Vec<_>>().join(" "));
            // NOT removing — just detecting for now
            break;
        }

        if coords.len() == before { break; }
    }
    let _ = input_len;
    coords
}

/// For each pair of polygons, find vertices of one that lie near edges of another
/// and insert them into the edge. Adjacent bricks sharing a curve but tessellated
/// differently end up with matching shared edges after this pass.
fn snap_vertices_to_edges(mut polys: Vec<Polygon<f64>>, tolerance: f64) -> Vec<Polygon<f64>> {
    let tol_sq = tolerance * tolerance;
    let n = polys.len();
    if n < 2 { return polys; }
    let mut total_snapped = 0;

    // STEP 1: snap near-duplicate vertices across polygons to a canonical value.
    // Collect all vertices, then for each vertex find the "representative" by
    // taking the first vertex within tolerance (deterministic).
    let all_verts: Vec<(f64, f64)> = polys.iter()
        .flat_map(|p| p.exterior().0.iter().map(|c| (c.x, c.y)))
        .collect();
    let mut canonical: Vec<(f64, f64)> = Vec::new();
    let mut snap_map: std::collections::HashMap<u64, (f64, f64)> = std::collections::HashMap::new();
    let key = |x: f64, y: f64| -> u64 {
        let ix = (x / tolerance).round() as i64;
        let iy = (y / tolerance).round() as i64;
        ((ix as u64) << 32) | (iy as u32 as u64)
    };
    for &(x, y) in &all_verts {
        let k = key(x, y);
        snap_map.entry(k).or_insert_with(|| {
            canonical.push((x, y));
            (x, y)
        });
    }
    // Apply canonical vertices to each polygon
    for p in polys.iter_mut() {
        let ring: Vec<Coord<f64>> = p.exterior().0.iter().map(|c| {
            let rep = snap_map.get(&key(c.x, c.y)).copied().unwrap_or((c.x, c.y));
            Coord { x: rep.0, y: rep.1 }
        }).collect();
        *p = Polygon::new(LineString::new(ring), vec![]);
    }

    // Extract all (vertex, owner_idx) pairs
    let mut all_vertices: Vec<(f64, f64, usize)> = Vec::new();
    for (i, p) in polys.iter().enumerate() {
        for c in p.exterior().0.iter() {
            all_vertices.push((c.x, c.y, i));
        }
    }

    // For each polygon, build a new vertex list where we insert any foreign vertex
    // that lies near one of its edges.
    let mut new_rings: Vec<Vec<Coord<f64>>> = Vec::with_capacity(n);
    for (i, p) in polys.iter().enumerate() {
        let ring: Vec<Coord<f64>> = p.exterior().0.clone();
        let mut new_ring: Vec<Coord<f64>> = Vec::with_capacity(ring.len() * 2);
        for seg_idx in 0..ring.len().saturating_sub(1) {
            let a = ring[seg_idx];
            let b = ring[seg_idx + 1];
            new_ring.push(a);

            // Find foreign vertices close to segment a→b
            let mut inserts: Vec<(f64, f64, f64)> = Vec::new(); // (t along seg, x, y)
            let ab_x = b.x - a.x;
            let ab_y = b.y - a.y;
            let ab_len_sq = ab_x * ab_x + ab_y * ab_y;
            if ab_len_sq > 1e-12 {
                for &(vx, vy, owner) in &all_vertices {
                    if owner == i { continue; }
                    // Skip if very close to endpoint a or b (already have it as vertex)
                    let da_sq = (vx - a.x).powi(2) + (vy - a.y).powi(2);
                    let db_sq = (vx - b.x).powi(2) + (vy - b.y).powi(2);
                    if da_sq < tol_sq || db_sq < tol_sq { continue; }
                    // Project onto segment
                    let t = ((vx - a.x) * ab_x + (vy - a.y) * ab_y) / ab_len_sq;
                    if t <= 0.0 || t >= 1.0 { continue; }
                    let proj_x = a.x + t * ab_x;
                    let proj_y = a.y + t * ab_y;
                    let d_sq = (vx - proj_x).powi(2) + (vy - proj_y).powi(2);
                    if d_sq < tol_sq {
                        // Use the foreign vertex's exact coords (not projected) so
                        // shared vertices become bit-identical across polys
                        inserts.push((t, vx, vy));
                    }
                }
            }
            // Sort by t and insert
            inserts.sort_by(|x, y| x.0.partial_cmp(&y.0).unwrap_or(std::cmp::Ordering::Equal));
            let mut last_t = -1.0;
            for (t, x, y) in inserts {
                if t - last_t < 1e-6 { continue; }
                last_t = t;
                new_ring.push(Coord { x, y });
                total_snapped += 1;
            }
        }
        // Close the ring
        if let Some(&first) = ring.first() {
            new_ring.push(first);
        }
        new_rings.push(new_ring);
    }

    // Rebuild polygons with the enriched rings
    for (i, ring) in new_rings.into_iter().enumerate() {
        polys[i] = Polygon::new(LineString::new(ring), vec![]);
    }
    if total_snapped > 0 {
        eprintln!("[snap] inserted {} vertices across {} polys", total_snapped, n);
    }
    polys
}

/// Compute merged polygon outlines for each piece.
///
/// Unions the original brick polygons (unbuffered) to preserve exact vector shapes.
/// Fills small interior holes.
pub fn compute_piece_polygons(
    pieces: &[PuzzlePiece],
    bricks_by_id: &HashMap<String, Brick>,
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
    clip_x0: f64,
    clip_y0: f64,
    scale: f64,
) -> HashMap<String, Vec<[f64; 2]>> {
    use geo::algorithm::bool_ops::BooleanOps;

    let mut result: HashMap<String, Vec<[f64; 2]>> = HashMap::new();

    for piece in pieces {
        let mut polys: Vec<Polygon<f64>> = Vec::new();
        // Debug: trace pieces that produce multiple components from plain union.
        // These are the "missing bricks" cases — find out why outlines don't merge.
        let debug_piece = false; // will enable below if multi-component detected

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

            // Polygons are stored in AI pymu coords — use directly, scale at end
            let _ = brick;
            let coords: Vec<Coord<f64>> = pts.iter()
                .map(|p| Coord { x: p[0], y: p[1] })
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
            eprintln!("[piece-debug] {} has {} brick polygons from {} bricks: {:?}",
                piece.id, polys.len(), piece.brick_ids.len(), piece.brick_ids);
        }

        if polys.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        // Snap vertices across bricks: where a vertex of one brick lies on (or near)
        // an edge of another brick, insert it into that edge. This fixes shared
        // curves tessellated with different vertex counts in different bricks.
        // Runs in AI pymu coords where tolerance matches original source precision.
        polys = snap_vertices_to_edges(polys, 0.5);

        // Plain union in AI pymu space. Shared edges (now bit-identical after snap)
        // fuse cleanly without any epsilon tricks.
        use geo_clipper::Clipper;
        let factor = 10000.0; // higher precision for AI-scale coords
        const HOLE_AREA_THRESHOLD: f64 = 100.0;

        let mut union = geo::MultiPolygon(vec![polys[0].clone()]);
        for poly in &polys[1..] {
            union = Clipper::union(&union, poly, factor);
        }
        union.0.retain(|p| p.unsigned_area() > 1.0);

        if debug_piece {
            eprintln!("[piece-debug] {} union produced {} components", piece.id, union.0.len());
            for (ci, comp) in union.0.iter().enumerate() {
                let bb = comp.bounding_rect();
                eprintln!("[piece-debug]   component {} area={:.0} pts={} holes={} bbox={:?}",
                    ci, comp.unsigned_area(), comp.exterior().0.len(), comp.interiors().len(),
                    bb.map(|r| (r.min().x as i32, r.min().y as i32, r.max().x as i32, r.max().y as i32)));
                if union.0.len() > 1 {
                    for (vi, c) in comp.exterior().0.iter().enumerate() {
                        eprintln!("[piece-debug]     v{}: ({:.2}, {:.2})", vi, c.x, c.y);
                    }
                }
            }
        }

        if union.0.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        // Dump details for pieces that didn't merge into a single component
        if union.0.len() > 1 {
            eprintln!("[piece-gap] {} has {} components from {} bricks",
                piece.id, union.0.len(), piece.brick_ids.len());
            // Dump input brick polygons
            for (pi, bp) in polys.iter().enumerate() {
                let bb = bp.bounding_rect();
                eprintln!("[piece-gap]   INPUT brick {} pts={} bbox={:?}",
                    pi, bp.exterior().0.len(),
                    bb.map(|r| (r.min().x, r.min().y, r.max().x, r.max().y)));
                for (vi, c) in bp.exterior().0.iter().enumerate() {
                    eprintln!("[piece-gap]     iv{}: ({:.4}, {:.4})", vi, c.x, c.y);
                }
            }
            // Dump output components
            for (ci, comp) in union.0.iter().enumerate() {
                let bb = comp.bounding_rect();
                eprintln!("[piece-gap]   OUTPUT component {} area={:.1} pts={} bbox={:?}",
                    ci, comp.unsigned_area(), comp.exterior().0.len(),
                    bb.map(|r| (r.min().x, r.min().y, r.max().x, r.max().y)));
            }
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

        let mut coords: Vec<[f64; 2]> = final_poly.exterior().0.iter()
            .map(|c| [c.x, c.y])
            .collect();

        // Check if this piece contains user-reported spike coordinates
        let has_target = coords.iter().any(|c| {
            (c[0] >= 277.0 && c[0] <= 280.0 && c[1] >= 550.0 && c[1] <= 551.0)
            || (c[0] >= 285.0 && c[0] <= 286.0 && c[1] >= 794.0 && c[1] <= 795.5)
            || (c[0] >= 144.5 && c[0] <= 145.5 && c[1] >= 792.0 && c[1] <= 795.0)
        });
        let debug_this = has_target;

        if debug_this {
            eprintln!("[target-piece] {} has {} pts before cleanup", piece.id, coords.len());
            for (vi, c) in coords.iter().enumerate() {
                eprintln!("[target-piece]   v{}: ({:.4}, {:.4})", vi, c[0], c[1]);
            }
        }

        // Clean up clipper union artifacts: degenerate spikes
        coords = remove_polygon_spikes_named(coords, &piece.id);

        // Scale from AI pymu coords to canvas coords (last step)
        let coords: Vec<[f64; 2]> = coords.into_iter()
            .map(|p| [(p[0] - clip_x0) * scale, (p[1] - clip_y0) * scale])
            .collect();

        if debug_piece {
            eprintln!("[piece-debug] {} FINAL polygon: {} pts", piece.id, coords.len());
            for (vi, c) in coords.iter().enumerate() {
                eprintln!("[piece-debug]   v{}: ({:.6}, {:.6})", vi, c[0], c[1]);
            }
        }
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

        let (placements, _meta, _ai_data) = ai_parser::parse_ai(&ai_path, 900).unwrap();

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
}
