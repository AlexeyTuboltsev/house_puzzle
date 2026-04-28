// lib/fixes.jsx — Phase 2 deterministic auto-fixes.
//
// MUTATES THE DOCUMENT. Only invoked when mode === "fix".
//
// runFixes(doc, snapshot, findings) -> { applied, skipped, error }
//
// Hard rules (see ../CLAUDE.md):
//   - Never auto-fix kinds the artist must judge:
//     brick_bbox_contained, multi_object_layer, missing_top_layer,
//     empty_top_layer, intra_brick_drift, corner_jitter (Phase 2.4
//     only after vector-adjacency).
//   - Vector and raster move together for any anchor shifts
//     (snap_drift_cluster: brick raster shifts by centroid Δ).
//   - Idempotent: running --fix twice in a row produces the same
//     end state as running it once.
//
// Defensive coding (Adobe JS runtime is buggy):
//   - Each individual mutation is wrapped in a try/catch so one
//     failure (locked layer, hidden item, ...) doesn't kill the
//     whole pass.
//   - Every fix attempt and outcome is logged via lib/log.jsx so
//     a crash leaves a clear last-known-good marker on disk.

function runFixes(doc, snapshot, findings, failedFixes) {
    var result = { applied: [], skipped: [], error: null };
    failedFixes = failedFixes || {};  // cross-iteration blacklist
    logInfo("runFixes: begin", { findings: findings.length, blacklisted: countKeys(failedFixes) });

    try {
        // Pass 1 — close_path. No anchor changes.
        logDebug("pass 1: close_path");
        for (var i = 0; i < findings.length; i++) {
            var f = findings[i];
            if (f.kind === "unclosed_path" || f.kind === "unclosed_path_zero_gap") {
                applyClosePath(doc, f, i, result, failedFixes);
            }
        }

        // Pass 2 — merge_subpymu. Mutates pathPoints arrays; iterate
        // in reverse so later removals don't invalidate earlier indices.
        logDebug("pass 2: merge_subpymu");
        for (var j = findings.length - 1; j >= 0; j--) {
            var g = findings[j];
            if (g.kind === "sub_pymu_edge") {
                applyMergeSubPymu(doc, g, j, result, failedFixes);
            }
        }

        // Pass 3 — snap_drift_clusters. Position-based, immune to
        // index changes from pass 2.
        logDebug("pass 3: snap_drift_clusters");
        applySnapDriftClusters(doc, findings, result, failedFixes);

        // Mark warning-only / not-yet-implemented kinds as skipped.
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
        // Last-resort guard — individual fix functions should already
        // catch their own errors. Anything reaching here is a bug in
        // the orchestration code.
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        result.error = msg;
        logError("runFixes: orchestration crash", { error: msg });
    }

    logInfo("runFixes: end", {
        applied: result.applied.length,
        skipped: result.skipped.length,
        error: result.error
    });
    return result;
}

// ----- per-fix helpers --------------------------------------------------

function skipFinding(result, ctx, reason) {
    result.skipped.push({
        finding_index: ctx.idx,
        kind: ctx.kind,
        brick: ctx.brick,
        layer_path: ctx.layer_path,
        sub_path: ctx.sub_path,
        reason: reason
    });
    logDebug("skip", { reason: reason, ctx: ctx });
}

// A layer is editable iff it AND all its ancestor layers are unlocked.
// In Illustrator, locking a parent layer effectively locks all children
// regardless of the children's own .locked state.
function isLayerEditable(layer) {
    var L = layer;
    while (L && L.typename === "Layer") {
        try {
            if (L.locked) return false;
        } catch (e) {
            // If we can't even read .locked, treat as non-editable to
            // stay on the safe side.
            return false;
        }
        try { L = L.parent; }
        catch (e) { L = null; }
    }
    return true;
}

// Canonical "can I touch this pageItem" check. The DOM's `editable`
// flag reflects ALL non-mutability conditions: own lock, ancestor
// layer lock, template, clip path membership, hidden ancestor, etc.
// Use this BEFORE every mutation to keep Illustrator from raising
// "Target layer cannot be modified" — even when caught, those raises
// appear to leave the renderer / AppleScript bridge in a broken
// state on some files (observed on _NY7.ai).
function isPathItemEditable(p) {
    try { return !!p.editable; }
    catch (e) { return false; }
}

// Cross-iteration blacklist key. Convergence retries the same
// findings each iteration; once a fix has provably failed (e.g.
// because of a deep lock condition), don't keep poking it.
function fixKey(finding) {
    return finding.kind + "|" + (finding.layer_path || "") +
           "|" + (finding.sub_path == null ? "?" : finding.sub_path) +
           "|" + (finding.edge_index == null ? "" : finding.edge_index);
}

function countKeys(obj) {
    var n = 0;
    for (var k in obj) { if (obj.hasOwnProperty(k)) n++; }
    return n;
}

// Wrap a single in-Illustrator mutation with try/catch + logging.
// Returns true on success, false (with a log line) on failure.
function tryMutate(label, ctx, fn) {
    try {
        fn();
        return true;
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        logError("mutate failed: " + label, { error: msg, ctx: ctx });
        return false;
    }
}

// ----- 1. close_path ---------------------------------------------------

function applyClosePath(doc, finding, idx, result, failedFixes) {
    var ctx = {
        idx: idx, kind: finding.kind, brick: finding.brick,
        layer_path: finding.layer_path, sub_path: finding.sub_path
    };
    var bk = fixKey(finding);
    if (failedFixes[bk]) {
        skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
        return;
    }
    logDebug("close_path: begin", ctx);

    try {
        var layer = findLayer(doc, finding.layer_path);
        if (!layer) {
            skipFinding(result, ctx, "layer not found: " + finding.layer_path);
            return;
        }
        if (!isLayerEditable(layer)) {
            failedFixes[bk] = "layer or ancestor is locked";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }
        if (finding.sub_path == null ||
            finding.sub_path < 0 ||
            finding.sub_path >= layer.pathItems.length) {
            skipFinding(result, ctx, "sub_path index out of range");
            return;
        }
        var p = layer.pathItems[finding.sub_path];
        if (p.closed) {
            skipFinding(result, ctx, "already closed (idempotent re-run)");
            return;
        }
        if (!isPathItemEditable(p)) {
            // Pre-flight: avoid even attempting the mutation. Caught
            // throws appear to leave Illustrator unstable on some files.
            failedFixes[bk] = "pathItem.editable is false";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        var ok = tryMutate("p.closed = true", ctx, function () { p.closed = true; });
        if (!ok) {
            failedFixes[bk] = "set closed flag threw";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        result.applied.push({
            finding_index: idx,
            kind: finding.kind,
            brick: finding.brick,
            layer_path: finding.layer_path,
            sub_path: finding.sub_path,
            action: "close_path",
            gap_pymu: finding.gap_pymu
        });
        logDebug("close_path: applied", ctx);
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        failedFixes[bk] = "outer exception: " + msg;
        skipFinding(result, ctx, failedFixes[bk]);
        logError("close_path: outer exception", { error: msg, ctx: ctx });
    }
}

// ----- 2. merge_subpymu ------------------------------------------------

function applyMergeSubPymu(doc, finding, idx, result, failedFixes) {
    var ctx = {
        idx: idx, kind: finding.kind, brick: finding.brick,
        layer_path: finding.layer_path, sub_path: finding.sub_path
    };
    var bk = fixKey(finding);
    if (failedFixes[bk]) {
        skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
        return;
    }
    logDebug("merge_subpymu: begin", ctx);

    try {
        var layer = findLayer(doc, finding.layer_path);
        if (!layer) {
            skipFinding(result, ctx, "layer not found: " + finding.layer_path);
            return;
        }
        if (!isLayerEditable(layer)) {
            failedFixes[bk] = "layer or ancestor is locked";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }
        if (finding.sub_path == null ||
            finding.sub_path < 0 ||
            finding.sub_path >= layer.pathItems.length) {
            skipFinding(result, ctx, "sub_path index out of range");
            return;
        }
        var p = layer.pathItems[finding.sub_path];
        if (!isPathItemEditable(p)) {
            failedFixes[bk] = "pathItem.editable is false";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }
        if (p.pathPoints.length < 4) {
            failedFixes[bk] = "path has < 4 anchors; merge would leave it degenerate";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }
        var i = finding.edge_index;
        if (i == null || i < 0 || i >= p.pathPoints.length) {
            skipFinding(result, ctx, "edge_index out of range");
            return;
        }
        var nextI = (i + 1 < p.pathPoints.length) ? i + 1 : 0;

        var aA = p.pathPoints[i].anchor;
        var aB = p.pathPoints[nextI].anchor;
        var dx = aB[0] - aA[0];
        var dy = aB[1] - aA[1];
        var len = Math.sqrt(dx * dx + dy * dy);
        if (len >= 1.0) {
            skipFinding(result, ctx, "edge already >= 1.0 pymu (idempotent re-run?)");
            return;
        }

        var scoreA = anchorPopularity(aA, layer);
        var scoreB = anchorPopularity(aB, layer);

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

        var pp = p.pathPoints[dropIndex];
        var ok = tryMutate("pathPoint.remove()", ctx, function () { pp.remove(); });
        if (!ok) {
            failedFixes[bk] = "pathPoint.remove() threw";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

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
        logDebug("merge_subpymu: applied", ctx);
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        skipFinding(result, ctx, "outer exception: " + msg);
        logError("merge_subpymu: outer exception", { error: msg, ctx: ctx });
    }
}

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

// ----- 3. snap_drift_cluster (+ raster centroid track) ----------------

function applySnapDriftClusters(doc, findings, result, failedFixes) {
    var EPS = 0.001;
    var brickDeltaSum = {}; // layer_path -> { sx, sy, shifted_x, shifted_y, anchor_failures }

    for (var f = 0; f < findings.length; f++) {
        var fnd = findings[f];
        if (fnd.kind !== "multi_grid_drift") continue;

        var ctxFinding = {
            idx: f, kind: fnd.kind, brick: null,
            layer_path: null, sub_path: null,
            axis: fnd.axis, cluster_min: fnd.cluster_min, cluster_max: fnd.cluster_max
        };
        var fbk = fixKey(fnd);
        if (failedFixes[fbk]) {
            skipFinding(result, ctxFinding, "blacklisted (prior iteration failed: " + failedFixes[fbk] + ")");
            continue;
        }
        logDebug("snap_drift_cluster: begin", ctxFinding);

        try {
            if (!fnd.member_layer_paths || fnd.member_layer_paths.length === 0) {
                skipFinding(result, ctxFinding, "no member_layer_paths recorded on finding");
                continue;
            }
            var winner = pickWinner(fnd.distinct_values);
            if (winner == null) {
                skipFinding(result, ctxFinding, "could not determine cluster winner");
                continue;
            }
            var axisIdx = (fnd.axis === "x") ? 0 : 1;
            var lo = fnd.cluster_min - EPS;
            var hi = fnd.cluster_max + EPS;

            var anchorsShifted = 0;
            var bricksTouched = 0;
            var lockedSkips = 0;
            var anchorFailures = 0;

            for (var bi = 0; bi < fnd.member_layer_paths.length; bi++) {
                var lp = fnd.member_layer_paths[bi];
                var layer = findLayer(doc, lp);
                if (!layer) {
                    logWarn("snap_drift_cluster: brick layer missing", { layer_path: lp });
                    continue;
                }
                if (!isLayerEditable(layer)) {
                    lockedSkips++;
                    logWarn("snap_drift_cluster: brick layer locked, skipping", { layer_path: lp });
                    continue;
                }
                var brickShiftedHere = 0;

                for (var pi = 0; pi < layer.pathItems.length; pi++) {
                    var path = layer.pathItems[pi];
                    if (!isPathItemEditable(path)) {
                        // Per-path lock check inside the inner loop —
                        // a brick layer can be unlocked overall while
                        // a single pathItem inside it is locked or
                        // template'd. Skip silently; the brick-level
                        // skip already noted any layer-wide locks.
                        logWarn("snap_drift_cluster: pathItem not editable, skipping", { layer_path: lp, pi: pi });
                        continue;
                    }
                    for (var pt = 0; pt < path.pathPoints.length; pt++) {
                        var pp = path.pathPoints[pt];
                        var anchor;
                        try { anchor = pp.anchor; }
                        catch (e) {
                            anchorFailures++;
                            logError("snap_drift_cluster: read anchor failed", { layer_path: lp, pi: pi, pt: pt, error: String(e) });
                            continue;
                        }
                        var v = anchor[axisIdx];
                        if (v < lo || v > hi) continue;
                        if (Math.abs(v - winner) < EPS) continue;

                        var delta = winner - v;
                        var ok = tryMutate(
                            "snap_drift anchor+handles",
                            { layer_path: lp, pi: pi, pt: pt, axis: fnd.axis, delta: delta },
                            function () {
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
                            }
                        );
                        if (!ok) { anchorFailures++; continue; }

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
                bricks_touched: bricksTouched,
                locked_layers_skipped: lockedSkips,
                anchor_failures: anchorFailures
            });
            logDebug("snap_drift_cluster: end", { idx: f, anchors_shifted: anchorsShifted, bricks_touched: bricksTouched, locked_skips: lockedSkips, anchor_failures: anchorFailures });
        } catch (e) {
            var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
            skipFinding(result, ctxFinding, "outer exception: " + msg);
            logError("snap_drift_cluster: outer exception", { error: msg, ctx: ctxFinding });
        }
    }

    // ---- Raster centroid track --------------------------------------
    for (var lpKey in brickDeltaSum) {
        if (!brickDeltaSum.hasOwnProperty(lpKey)) continue;
        var ctxRaster = { idx: null, kind: "raster_track", brick: null, layer_path: lpKey };
        try {
            var layer2 = findLayer(doc, lpKey);
            if (!layer2 || layer2.rasterItems.length === 0) continue;
            if (!isLayerEditable(layer2)) {
                logWarn("move_raster: layer locked, skipping", { layer_path: lpKey });
                continue;
            }

            var totalAnchors = 0;
            for (var p2 = 0; p2 < layer2.pathItems.length; p2++) {
                totalAnchors += layer2.pathItems[p2].pathPoints.length;
            }
            if (totalAnchors === 0) continue;

            var dx = brickDeltaSum[lpKey].sx / totalAnchors;
            var dy = brickDeltaSum[lpKey].sy / totalAnchors;
            if (Math.abs(dx) < 1e-9 && Math.abs(dy) < 1e-9) continue;

            var rasterFailures = 0;
            for (var ri = 0; ri < layer2.rasterItems.length; ri++) {
                var r = layer2.rasterItems[ri];
                var ok = tryMutate(
                    "raster.position += delta",
                    { layer_path: lpKey, ri: ri, dx: dx, dy: dy },
                    function () {
                        var pos = r.position;
                        r.position = [pos[0] + dx, pos[1] + dy];
                    }
                );
                if (!ok) rasterFailures++;
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
                rasters_moved: layer2.rasterItems.length - rasterFailures,
                raster_failures: rasterFailures
            });
            logDebug("move_raster: applied", { layer_path: lpKey, dx: dx, dy: dy });
        } catch (e) {
            var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
            logError("move_raster: outer exception", { error: msg, ctx: ctxRaster });
        }
    }
}

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
