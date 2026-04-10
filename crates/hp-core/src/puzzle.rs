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

    // Phase 0: exclude oversized bricks
    let total_area: f64 = all_ids.iter().map(|id| brick_areas.get(id).copied().unwrap_or(0.0)).sum();
    let mut fixed_ids: HashSet<String> = HashSet::new();
    for _ in 0..10 {
        let target_area = total_area / target_count.max(1) as f64;
        let new_fixed: HashSet<String> = all_ids
            .iter()
            .filter(|id| brick_areas.get(*id).copied().unwrap_or(0.0) >= target_area)
            .cloned()
            .collect();
        if new_fixed == fixed_ids {
            break;
        }
        fixed_ids = new_fixed;
    }

    let mergeable_ids: HashSet<String> = all_ids.difference(&fixed_ids).cloned().collect();
    let target_mergeable = target_count.saturating_sub(fixed_ids.len()).max(1);
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
    while pieces_dict.len() > target_mergeable {
        let mut candidates: Vec<String> = pieces_dict.keys().cloned().collect();
        candidates.sort_by(|a, b| {
            piece_area
                .get(a)
                .unwrap_or(&0.0)
                .partial_cmp(piece_area.get(b).unwrap_or(&0.0))
                .unwrap()
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
                let absorbed_bricks = pieces_dict.remove(&absorb_pid).unwrap();
                pieces_dict.get_mut(&keep_pid).unwrap().extend(absorbed_bricks.iter().cloned());
                *piece_area.get_mut(&keep_pid).unwrap() +=
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

        if !merged {
            break;
        }
    }

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

    result
}

/// Compute merged polygon outlines for each piece.
///
/// For each piece, converts brick polygons to canvas coords, buffers by BRIDGE px,
/// computes union, then erodes back. Returns the largest polygon if the union
/// produces a MultiPolygon.
pub fn compute_piece_polygons(
    pieces: &[PuzzlePiece],
    bricks_by_id: &HashMap<String, Brick>,
    brick_polygons: &HashMap<String, Vec<[f64; 2]>>,
) -> HashMap<String, Vec<[f64; 2]>> {
    use geo::algorithm::bool_ops::BooleanOps;

    const BRIDGE: f64 = 3.0; // px buffer to bridge shared edges (matches Python)

    let mut result: HashMap<String, Vec<[f64; 2]>> = HashMap::new();

    for piece in pieces {
        let mut polys: Vec<Polygon<f64>> = Vec::new();

        for bid in &piece.brick_ids {
            let brick = match bricks_by_id.get(bid) {
                Some(b) => b,
                None => continue,
            };
            let pts = match brick_polygons.get(bid) {
                Some(p) if p.len() >= 3 => p,
                _ => {
                    // Fallback: bounding box rectangle
                    let bx = brick.x as f64;
                    let by = brick.y as f64;
                    let bw = brick.width as f64;
                    let bh = brick.height as f64;
                    let coords = vec![
                        Coord { x: bx, y: by },
                        Coord { x: bx + bw, y: by },
                        Coord { x: bx + bw, y: by + bh },
                        Coord { x: bx, y: by + bh },
                        Coord { x: bx, y: by },
                    ];
                    polys.push(Polygon::new(LineString::new(coords), vec![]));
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
            polys.push(poly);
        }

        if polys.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        // Match Python's Shapely: buffer(BRIDGE) → unary_union → buffer(-BRIDGE).
        use geo_clipper::Clipper;
        use geo::algorithm::convex_hull::ConvexHull;

        let factor = 1000.0; // internal i64 precision

        // Buffer each polygon by BRIDGE px, collect all expanded polygons
        let mut buffered: Vec<Polygon<f64>> = Vec::new();
        for poly in &polys {
            let expanded = poly.offset(BRIDGE, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);
            for p in expanded.0 {
                if p.unsigned_area() > 1.0 {
                    buffered.push(p);
                }
            }
        }

        if buffered.is_empty() {
            let all_points: Vec<Coord<f64>> = polys.iter()
                .flat_map(|p| p.exterior().0.iter().cloned())
                .collect();
            let hull = LineString::new(all_points).convex_hull();
            result.insert(piece.id.clone(), hull.exterior().0.iter().map(|c| [c.x, c.y]).collect());
            continue;
        }

        // Union all buffered polygons at once, keeping the full MultiPolygon
        // result (matching Python's unary_union behavior).
        let mut union = geo::MultiPolygon(vec![buffered[0].clone()]);
        for poly in &buffered[1..] {
            union = Clipper::union(&union, poly, factor);
        }

        // Erode back by BRIDGE. Take the largest polygon from the result.
        if union.0.is_empty() {
            result.insert(piece.id.clone(), vec![]);
            continue;
        }

        let largest = union.0.into_iter()
            .max_by(|a, b| a.unsigned_area().partial_cmp(&b.unsigned_area()).unwrap())
            .unwrap();

        let eroded = largest.offset(-BRIDGE, geo_clipper::JoinType::Round(0.25), geo_clipper::EndType::ClosedPolygon, factor);
        let final_poly = if !eroded.0.is_empty() {
            eroded.0.into_iter()
                .max_by(|a, b| a.unsigned_area().partial_cmp(&b.unsigned_area()).unwrap())
                .unwrap()
        } else {
            largest
        };

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
