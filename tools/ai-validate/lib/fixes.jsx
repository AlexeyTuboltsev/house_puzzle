// lib/fixes.jsx — Phase 2 deterministic auto-fixes.
//
// MUTATES THE DOCUMENT. Only invoked when mode === "fix".
//
// runFixes(doc, snapshot, findings) -> { applied, skipped, error }
//
// Hard rules (see ../CLAUDE.md):
//   - Never auto-fix kinds the artist must judge:
//     brick_bbox_contained, multi_object_layer, missing_top_layer,
//     empty_top_layer, intra_brick_drift (judgement-call), corner_jitter
//     (Phase 2.4 only after we have vector-adjacency).
//   - Vector and raster move together for any anchor shifts
//     (Phase 2.3 onward — not relevant for close-path which doesn't
//     move anchors).
//   - Idempotent: running --fix twice in a row produces the same
//     end state as running it once.

function runFixes(doc, snapshot, findings) {
    var result = { applied: [], skipped: [], error: null };
    try {
        // Pass 1: close_path (no anchor changes).
        for (var i = 0; i < findings.length; i++) {
            var f = findings[i];
            if (f.kind === "unclosed_path" || f.kind === "unclosed_path_zero_gap") {
                applyClosePath(doc, f, i, result);
            }
        }
        // Pass 2: merge_subpymu (mutates pathPoints arrays — process in
        // reverse so later removals don't invalidate earlier indices).
        for (var j = findings.length - 1; j >= 0; j--) {
            var g = findings[j];
            if (g.kind === "sub_pymu_edge") {
                applyMergeSubPymu(doc, g, j, result);
            }
        }
        // Pass 3: snap_drift_clusters (position-based; immune to prior
        // index changes). Anchor shifts accumulated per brick, then
        // each brick's rasters move by the centroid shift.
        applySnapDriftClusters(doc, findings, result);

        // Anything else: warning-only or future phase. Mark skipped.
        for (var k = 0; k < findings.length; k++) {
            var h = findings[k];
            if (h.kind !== "unclosed_path" &&
                h.kind !== "unclosed_path_zero_gap" &&
                h.kind !== "sub_pymu_edge" &&
                h.kind !== "multi_grid_drift") {
                result.skipped.push({
                    finding_index: k,
                    kind: h.kind,
                    brick: h.brick || null,
                    reason: "fix not implemented for this kind (or kind is warning-only)"
                });
            }
        }
    } catch (e) {
        result.error = String(e) + (e.line ? " (line " + e.line + ")" : "");
    }
    return result;
}

// Close an open sub-path. Illustrator implicitly draws the closing
// segment from last anchor to first when `closed` is set to true,
// which matches the plan's "append a single straight closing line"
// requirement.
function applyClosePath(doc, finding, idx, result) {
    var layer = findLayer(doc, finding.layer_path);
    if (!layer) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "layer not found: " + finding.layer_path
        });
        return;
    }
    if (finding.sub_path == null ||
        finding.sub_path < 0 ||
        finding.sub_path >= layer.pathItems.length) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "sub_path index out of range"
        });
        return;
    }
    var p = layer.pathItems[finding.sub_path];
    if (p.closed) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "already closed (idempotent re-run)"
        });
        return;
    }
    p.closed = true;
    result.applied.push({
        finding_index: idx,
        kind: finding.kind,
        brick: finding.brick,
        layer_path: finding.layer_path,
        sub_path: finding.sub_path,
        action: "close_path",
        gap_pymu: finding.gap_pymu
    });
}

// Merge two anchors on a sub-pymu edge. The survivor is the anchor
// whose (x, y) coordinates appear most often elsewhere in the same
// brick — that's the canonical position the artist meant to put the
// extra anchor on. Tie → keep the first anchor.
//
// Skip if the path has < 4 anchors (merge would leave a degenerate
// shape) or if the live edge length is no longer < 1 pymu (a previous
// run already fixed it, or the finding is stale).
function applyMergeSubPymu(doc, finding, idx, result) {
    var layer = findLayer(doc, finding.layer_path);
    if (!layer) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "layer not found: " + finding.layer_path
        });
        return;
    }
    if (finding.sub_path == null ||
        finding.sub_path < 0 ||
        finding.sub_path >= layer.pathItems.length) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "sub_path index out of range"
        });
        return;
    }
    var p = layer.pathItems[finding.sub_path];
    if (p.pathPoints.length < 4) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "path has < 4 anchors; merge would leave it degenerate"
        });
        return;
    }
    var i = finding.edge_index;
    if (i == null || i < 0 || i >= p.pathPoints.length) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "edge_index out of range"
        });
        return;
    }
    var nextI = (i + 1 < p.pathPoints.length) ? i + 1 : 0;

    var aA = p.pathPoints[i].anchor;
    var aB = p.pathPoints[nextI].anchor;
    var dx = aB[0] - aA[0];
    var dy = aB[1] - aA[1];
    var len = Math.sqrt(dx * dx + dy * dy);
    if (len >= 1.0) {
        result.skipped.push({
            finding_index: idx, kind: finding.kind, brick: finding.brick,
            reason: "edge already >= 1.0 pymu (idempotent re-run?)"
        });
        return;
    }

    var scoreA = anchorPopularity(aA, layer);
    var scoreB = anchorPopularity(aB, layer);

    // Subtract self-counts so each anchor is compared on its
    // popularity *elsewhere* in the brick.
    var dropIndex, droppedAnchor, keptAnchor;
    if ((scoreA - 1) >= (scoreB - 1)) {
        dropIndex = nextI;
        droppedAnchor = [aB[0], aB[1]];
        keptAnchor = [aA[0], aA[1]];
    } else {
        dropIndex = i;
        droppedAnchor = [aA[0], aA[1]];
        keptAnchor = [aB[0], aB[1]];
    }

    p.pathPoints[dropIndex].remove();

    result.applied.push({
        finding_index: idx,
        kind: "sub_pymu_edge",
        brick: finding.brick,
        layer_path: finding.layer_path,
        sub_path: finding.sub_path,
        action: "merge_subpymu",
        edge_len_pymu: len,
        kept_anchor: keptAnchor,
        dropped_anchor: droppedAnchor
    });
}

// Count anchors anywhere in `layer` whose (x, y) match the given
// position within EPS_MATCH. Includes the anchor itself, so callers
// subtract 1 when comparing.
function anchorPopularity(anchor, layer) {
    var EPS = 0.001;
    var n = 0;
    for (var s = 0; s < layer.pathItems.length; s++) {
        var p = layer.pathItems[s];
        for (var k = 0; k < p.pathPoints.length; k++) {
            var a = p.pathPoints[k].anchor;
            if (Math.abs(a[0] - anchor[0]) < EPS && Math.abs(a[1] - anchor[1]) < EPS) {
                n++;
            }
        }
    }
    return n;
}

// Snap multi-grid-drift clusters. For each cluster, the "winner" is
// the most-popular value (median on tie). Any anchor whose value falls
// in the cluster range but isn't at the winner gets shifted to the
// winner — anchor + leftDirection + rightDirection all by the same Δ.
//
// After all anchor shifts, each affected brick's rasters move by the
// brick's centroid shift: sum of per-anchor deltas / total anchor count.
// For "1 corner drifted in a 4-corner brick" this is Δ/4 (raster
// tracks centroid). For "whole brick shifted" it's Δ (raster moves
// with the brick).
//
// Position-based: doesn't use sub_path / anchor indices, so prior
// merge_subpymu fixes don't invalidate the inputs.
function applySnapDriftClusters(doc, findings, result) {
    var EPS = 0.001;
    var brickDeltaSum = {}; // layer_path -> { sx: 0, sy: 0, shifted_x: 0, shifted_y: 0 }

    for (var f = 0; f < findings.length; f++) {
        var fnd = findings[f];
        if (fnd.kind !== "multi_grid_drift") continue;
        if (!fnd.member_layer_paths || fnd.member_layer_paths.length === 0) {
            result.skipped.push({
                finding_index: f, kind: fnd.kind, brick: null,
                reason: "no member_layer_paths recorded on finding"
            });
            continue;
        }

        var winner = pickWinner(fnd.distinct_values);
        if (winner == null) {
            result.skipped.push({
                finding_index: f, kind: fnd.kind, brick: null,
                reason: "could not determine cluster winner"
            });
            continue;
        }
        var axisIdx = (fnd.axis === "x") ? 0 : 1;
        var lo = fnd.cluster_min - EPS;
        var hi = fnd.cluster_max + EPS;

        var anchorsShifted = 0;
        var bricksTouched = 0;

        for (var bi = 0; bi < fnd.member_layer_paths.length; bi++) {
            var lp = fnd.member_layer_paths[bi];
            var layer = findLayer(doc, lp);
            if (!layer) continue;
            var brickShiftedHere = 0;

            for (var pi = 0; pi < layer.pathItems.length; pi++) {
                var path = layer.pathItems[pi];
                for (var pt = 0; pt < path.pathPoints.length; pt++) {
                    var pp = path.pathPoints[pt];
                    var anchor = pp.anchor;
                    var v = anchor[axisIdx];
                    if (v < lo || v > hi) continue;
                    if (Math.abs(v - winner) < EPS) continue;

                    var delta = winner - v;
                    var newAnchor = [anchor[0], anchor[1]];
                    newAnchor[axisIdx] = winner;
                    pp.anchor = newAnchor;

                    var ld = pp.leftDirection;
                    var rd = pp.rightDirection;
                    var newLd = [ld[0], ld[1]];
                    var newRd = [rd[0], rd[1]];
                    newLd[axisIdx] += delta;
                    newRd[axisIdx] += delta;
                    pp.leftDirection = newLd;
                    pp.rightDirection = newRd;

                    if (!brickDeltaSum[lp]) {
                        brickDeltaSum[lp] = { sx: 0, sy: 0, shifted_x: 0, shifted_y: 0 };
                    }
                    if (axisIdx === 0) {
                        brickDeltaSum[lp].sx += delta;
                        brickDeltaSum[lp].shifted_x++;
                    } else {
                        brickDeltaSum[lp].sy += delta;
                        brickDeltaSum[lp].shifted_y++;
                    }
                    anchorsShifted++;
                    brickShiftedHere++;
                }
            }
            if (brickShiftedHere > 0) bricksTouched++;
        }

        result.applied.push({
            finding_index: f,
            kind: "multi_grid_drift",
            action: "snap_drift_cluster",
            axis: fnd.axis,
            winner: winner,
            cluster_range: [fnd.cluster_min, fnd.cluster_max],
            anchors_shifted: anchorsShifted,
            bricks_touched: bricksTouched
        });
    }

    // Move rasters by per-brick centroid shift.
    for (var lpKey in brickDeltaSum) {
        if (!brickDeltaSum.hasOwnProperty(lpKey)) continue;
        var layer2 = findLayer(doc, lpKey);
        if (!layer2 || layer2.rasterItems.length === 0) continue;

        var totalAnchors = 0;
        for (var p2 = 0; p2 < layer2.pathItems.length; p2++) {
            totalAnchors += layer2.pathItems[p2].pathPoints.length;
        }
        if (totalAnchors === 0) continue;

        var dx = brickDeltaSum[lpKey].sx / totalAnchors;
        var dy = brickDeltaSum[lpKey].sy / totalAnchors;
        if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) continue;

        for (var ri = 0; ri < layer2.rasterItems.length; ri++) {
            var r = layer2.rasterItems[ri];
            var pos = r.position;
            r.position = [pos[0] + dx, pos[1] + dy];
        }
        result.applied.push({
            finding_index: null,
            kind: "raster_track",
            action: "move_raster",
            brick: lpKey.substring(lpKey.lastIndexOf("/") + 1),
            layer_path: lpKey,
            delta_x: dx,
            delta_y: dy,
            shifted_x_count: brickDeltaSum[lpKey].shifted_x,
            shifted_y_count: brickDeltaSum[lpKey].shifted_y,
            total_anchors: totalAnchors,
            rasters_moved: layer2.rasterItems.length
        });
    }
}

// Pick the most-popular value from a {valueString: count} map. Ties
// resolve to the median (matching plan.md's "median if it ties" rule).
function pickWinner(distinct) {
    var entries = [];
    var maxCount = 0;
    for (var k in distinct) {
        if (!distinct.hasOwnProperty(k)) continue;
        var n = distinct[k];
        entries.push({ value: parseFloat(k), count: n });
        if (n > maxCount) maxCount = n;
    }
    if (entries.length === 0) return null;
    var winners = [];
    for (var i = 0; i < entries.length; i++) {
        if (entries[i].count === maxCount) winners.push(entries[i].value);
    }
    if (winners.length === 1) return winners[0];
    winners.sort(function (a, b) { return a - b; });
    var mid = Math.floor(winners.length / 2);
    if (winners.length % 2 === 1) return winners[mid];
    return (winners[mid - 1] + winners[mid]) / 2;
}

// Walk doc.layers by name, descending into sub-layers. Returns the
// layer object or null if any segment isn't found.
function findLayer(doc, layerPath) {
    var parts = layerPath.split("/");
    var coll = doc.layers;
    var layer = null;
    for (var i = 0; i < parts.length; i++) {
        layer = null;
        for (var j = 0; j < coll.length; j++) {
            if (coll[j].name === parts[i]) {
                layer = coll[j];
                break;
            }
        }
        if (!layer) return null;
        if (i < parts.length - 1) coll = layer.layers;
    }
    return layer;
}
