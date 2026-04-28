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
        // Pass 0 — delete_tiny_brick. Run BEFORE everything else so
        // we never waste work snapping anchors on a layer we're about
        // to delete; more importantly, snapping a 0.1-pymu² triangle
        // collapses its 3 anchors onto one point, and that's what
        // hung Illustrator's renderer on _NY7.ai.
        logDebug("pass 0: delete_tiny_brick");
        for (var t = 0; t < findings.length; t++) {
            var ft = findings[t];
            if (ft.kind === "tiny_brick") {
                applyDeleteTinyBrick(doc, ft, t, result, failedFixes);
            }
        }

        // Pass 1 — close_path. No anchor changes.
        logDebug("pass 1: close_path");
        for (var i = 0; i < findings.length; i++) {
            var f = findings[i];
            if (f.kind === "unclosed_path" || f.kind === "unclosed_path_zero_gap") {
                applyClosePath(doc, f, i, result, failedFixes);
            }
        }

        // Pass 2 — merge_subpymu. Mutates pathPoints arrays (not
        // pathItem indices), so subsequent passes' sub_path indices
        // stay valid. Iterate in reverse so later removals don't
        // invalidate earlier pathPoint indices in the same path.
        logDebug("pass 2: merge_subpymu");
        for (var j = findings.length - 1; j >= 0; j--) {
            var g = findings[j];
            if (g.kind === "sub_pymu_edge") {
                applyMergeSubPymu(doc, g, j, result, failedFixes);
            }
        }

        // Pass 2.5 — remove_spur. Same kind of mutation as
        // merge_subpymu (drops a single pathPoint). Group findings
        // by (layer_path, sub_path) and process highest anchor_index
        // first within each so earlier removals don't shift later
        // targets in the same path.
        logDebug("pass 2.5: remove_spur");
        applyRemoveSpurs(doc, findings, result, failedFixes);

        // Pass 3 — snap_drift_clusters. Position-based, immune to
        // index changes from pass 2.
        logDebug("pass 3: snap_drift_clusters");
        applySnapDriftClusters(doc, findings, result, failedFixes);

        // Pass 4 — snap_corner_jitter. 2D version of pass 3: two
        // anchors on near-coincident corners across different bricks
        // get snapped to whichever position is most popular across
        // the whole document. Position-based, also immune to prior
        // index changes.
        logDebug("pass 4: snap_corner_jitter");
        applySnapCornerJitter(doc, snapshot, findings, result, failedFixes);

        // Pass 5 — delete_degenerate. Removes pathItems with < 3
        // anchors (degenerate_path, error) or area < MIN_AREA_PYMU2
        // (also degenerate_path post-unification). Runs LAST among
        // mutations because removing pathItems shifts sub_path
        // indices for every other finding in the same layer; nothing
        // after this pass cares about those indices. Within this
        // pass, group by layer and process highest sub_path first so
        // earlier deletes in the same layer don't shift later targets.
        logDebug("pass 5: delete_degenerate");
        applyDeleteDegenerate(doc, findings, result, failedFixes);

        // Mark warning-only / not-yet-implemented kinds as skipped.
        for (var k = 0; k < findings.length; k++) {
            var h = findings[k];
            if (h.kind !== "unclosed_path" &&
                h.kind !== "unclosed_path_zero_gap" &&
                h.kind !== "sub_pymu_edge" &&
                h.kind !== "multi_grid_drift" &&
                h.kind !== "tiny_brick" &&
                h.kind !== "degenerate_path" &&
                h.kind !== "degenerate_area" &&
                h.kind !== "corner_jitter" &&
                h.kind !== "path_spur") {
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

// ----- 1.5. delete_tiny_brick -----------------------------------------
//
// Whole-layer removal for bricks whose total geometry is so small it
// can only be an artist artifact (stray click, anchor merge gone
// wrong). The artist confirmed the threshold MIN_BRICK_TOTAL_AREA in
// checks.jsx — anything below it is wrong.
//
// Run BEFORE snap_drift_clusters: snapping the 3 anchors of a 0.1-
// pymu² triangle to a single grid value collapses them onto one
// point, which appeared to be what crashed Illustrator's renderer
// on _NY7.ai.

function applyDeleteTinyBrick(doc, finding, idx, result, failedFixes) {
    var ctx = {
        idx: idx, kind: finding.kind, brick: finding.brick,
        layer_path: finding.layer_path, sub_path: null
    };
    var bk = fixKey(finding);
    if (failedFixes[bk]) {
        skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
        return;
    }
    logDebug("delete_tiny_brick: begin", ctx);

    try {
        var layer = findLayer(doc, finding.layer_path);
        if (!layer) {
            skipFinding(result, ctx, "layer not found (already deleted?)");
            return;
        }
        if (!isLayerEditable(layer)) {
            failedFixes[bk] = "layer or ancestor is locked";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        var ok = tryMutate("layer.remove()", ctx, function () { layer.remove(); });
        if (!ok) {
            failedFixes[bk] = "layer.remove() threw";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        result.applied.push({
            finding_index: idx,
            kind: "tiny_brick",
            brick: finding.brick,
            layer_path: finding.layer_path,
            action: "delete_tiny_brick",
            total_area_pymu2: finding.total_area_pymu2,
            sub_path_count: finding.sub_path_count,
            anchor_counts: finding.anchor_counts
        });
        logDebug("delete_tiny_brick: applied", ctx);
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        failedFixes[bk] = "outer exception: " + msg;
        skipFinding(result, ctx, failedFixes[bk]);
        logError("delete_tiny_brick: outer exception", { error: msg, ctx: ctx });
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

// ----- 2.5. remove_spur -----------------------------------------------
//
// A spur anchor (collinear with neighbors, polygon backtracks through
// it) gets removed via pathPoint.remove(). Removing the anchor
// preserves the polygon's shape — the adjacent edges B-C and C-D
// collapse into a single forward edge B-D plus the remaining path.
//
// Group findings by (layer_path, sub_path) and process the highest
// anchor_index first within each path so earlier removals don't
// invalidate later anchor_indices in the same path.

function applyRemoveSpurs(doc, findings, result, failedFixes) {
    var perPath = {};
    for (var i = 0; i < findings.length; i++) {
        var f = findings[i];
        if (f.kind !== "path_spur") continue;
        var key = f.layer_path + "|" + f.sub_path;
        if (!perPath[key]) perPath[key] = [];
        perPath[key].push({ idx: i, finding: f });
    }
    for (var k in perPath) {
        if (!perPath.hasOwnProperty(k)) continue;
        perPath[k].sort(function (a, b) {
            return (b.finding.anchor_index || 0) - (a.finding.anchor_index || 0);
        });
        for (var m = 0; m < perPath[k].length; m++) {
            applyRemoveSpurOne(doc, perPath[k][m].finding, perPath[k][m].idx, result, failedFixes);
        }
    }
}

function applyRemoveSpurOne(doc, finding, idx, result, failedFixes) {
    var ctx = {
        idx: idx, kind: finding.kind, brick: finding.brick,
        layer_path: finding.layer_path, sub_path: finding.sub_path
    };
    var bk = fixKey(finding) + "|" + (finding.anchor_index == null ? "?" : finding.anchor_index);
    if (failedFixes[bk]) {
        skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
        return;
    }
    logDebug("remove_spur: begin", ctx);

    try {
        var layer = findLayer(doc, finding.layer_path);
        if (!layer) {
            skipFinding(result, ctx, "layer not found");
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
            skipFinding(result, ctx, "sub_path out of range");
            return;
        }
        var p = layer.pathItems[finding.sub_path];
        if (!isPathItemEditable(p)) {
            failedFixes[bk] = "pathItem.editable is false";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }
        if (p.pathPoints.length < 4) {
            // Removing an anchor would leave < 3, no longer a polygon.
            skipFinding(result, ctx, "path has < 4 anchors; remove would leave it degenerate");
            return;
        }
        var ai = finding.anchor_index;
        if (ai == null || ai < 0 || ai >= p.pathPoints.length) {
            skipFinding(result, ctx, "anchor_index out of range");
            return;
        }
        // Confirm the live pathPoint still matches the spur position
        // — if a prior fix already moved it, skip.
        var pp = p.pathPoints[ai];
        var live;
        try { live = pp.anchor; } catch (e) {
            skipFinding(result, ctx, "could not read anchor: " + String(e));
            return;
        }
        var EPS = 0.01;
        if (Math.abs(live[0] - finding.anchor[0]) > EPS ||
            Math.abs(live[1] - finding.anchor[1]) > EPS) {
            skipFinding(result, ctx, "live anchor moved away from spur position");
            return;
        }

        var ok = tryMutate("pathPoint.remove() (spur)", ctx, function () { pp.remove(); });
        if (!ok) {
            failedFixes[bk] = "pathPoint.remove() threw";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        result.applied.push({
            finding_index: idx,
            kind: "path_spur",
            brick: finding.brick,
            layer_path: finding.layer_path,
            sub_path: finding.sub_path,
            anchor_index: finding.anchor_index,
            action: "remove_spur",
            removed_anchor: finding.anchor
        });
        logDebug("remove_spur: applied", ctx);
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        failedFixes[bk] = "outer exception: " + msg;
        skipFinding(result, ctx, failedFixes[bk]);
        logError("remove_spur: outer exception", { error: msg, ctx: ctx });
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

// ----- 4. snap_corner_jitter -------------------------------------------
//
// Two near-coincident corners across different bricks get snapped to
// whichever position is most popular across the whole snapshot.
// Position-based, immune to prior index shifts. Same vector + raster
// move-together rule as snap_drift_cluster: each affected brick's
// rasters track the centroid shift.
//
// Pairwise resolution (per-finding). When the two anchors have equal
// popularity, the convergence loop relies on the second iteration's
// freshly-recomputed findings to settle any remainder; one iteration
// is rarely enough for transitive jitter (A near B near C).

function applySnapCornerJitter(doc, snapshot, findings, result, failedFixes) {
    var EPS = 0.001;
    var freq = buildAnchorFrequency(snapshot);
    var brickDeltaSum = {};

    for (var f = 0; f < findings.length; f++) {
        var fnd = findings[f];
        if (fnd.kind !== "corner_jitter") continue;

        var ctx = {
            idx: f, kind: fnd.kind, brick: fnd.brick,
            layer_path: fnd.layer_path, sub_path: fnd.sub_path
        };
        var bk = fixKey(fnd);
        if (failedFixes[bk]) {
            skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
            continue;
        }
        logDebug("snap_corner_jitter: begin", ctx);

        try {
            var ka = anchorKey(fnd.anchor);
            var kb = anchorKey(fnd.other_anchor);
            var fa = freq[ka] || 0;
            var fb = freq[kb] || 0;

            var loserPath, loserAnchor, target;
            if (fa > fb) {
                loserPath = fnd.other_layer_path;
                loserAnchor = fnd.other_anchor;
                target = fnd.anchor;
            } else if (fb > fa) {
                loserPath = fnd.layer_path;
                loserAnchor = fnd.anchor;
                target = fnd.other_anchor;
            } else {
                // Tied frequencies — pick `other_anchor` as loser by
                // convention. Convergence iteration handles the rest.
                loserPath = fnd.other_layer_path;
                loserAnchor = fnd.other_anchor;
                target = fnd.anchor;
            }

            var loserLayer = findLayer(doc, loserPath);
            if (!loserLayer) {
                skipFinding(result, ctx, "loser layer not found: " + loserPath);
                continue;
            }
            if (!isLayerEditable(loserLayer)) {
                failedFixes[bk] = "loser layer locked";
                skipFinding(result, ctx, failedFixes[bk]);
                continue;
            }

            var dx = target[0] - loserAnchor[0];
            var dy = target[1] - loserAnchor[1];
            var moved = 0;

            for (var pi = 0; pi < loserLayer.pathItems.length; pi++) {
                var path = loserLayer.pathItems[pi];
                if (!isPathItemEditable(path)) continue;
                for (var pt = 0; pt < path.pathPoints.length; pt++) {
                    var pp = path.pathPoints[pt];
                    var pa;
                    try { pa = pp.anchor; } catch (e) { continue; }
                    if (Math.abs(pa[0] - loserAnchor[0]) >= EPS) continue;
                    if (Math.abs(pa[1] - loserAnchor[1]) >= EPS) continue;
                    var ok = tryMutate(
                        "snap_corner_jitter anchor+handles",
                        { layer_path: loserPath, pi: pi, pt: pt, dx: dx, dy: dy },
                        function () {
                            pp.anchor = [target[0], target[1]];
                            var ld = pp.leftDirection;
                            var rd = pp.rightDirection;
                            pp.leftDirection = [ld[0] + dx, ld[1] + dy];
                            pp.rightDirection = [rd[0] + dx, rd[1] + dy];
                        }
                    );
                    if (ok) moved++;
                }
            }

            if (moved === 0) {
                skipFinding(result, ctx, "loser anchor not found in layer (already moved?)");
                continue;
            }

            if (!brickDeltaSum[loserPath]) {
                brickDeltaSum[loserPath] = { sx: 0, sy: 0, anchors_moved: 0 };
            }
            brickDeltaSum[loserPath].sx += dx * moved;
            brickDeltaSum[loserPath].sy += dy * moved;
            brickDeltaSum[loserPath].anchors_moved += moved;

            result.applied.push({
                finding_index: f,
                kind: "corner_jitter",
                action: "snap_corner_jitter",
                loser_brick: loserPath.substring(loserPath.lastIndexOf("/") + 1),
                loser_layer_path: loserPath,
                target: target,
                jitter_pymu: fnd.jitter_pymu,
                anchors_moved: moved
            });
            logDebug("snap_corner_jitter: applied", ctx);
        } catch (e) {
            var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
            failedFixes[bk] = "outer exception: " + msg;
            skipFinding(result, ctx, failedFixes[bk]);
            logError("snap_corner_jitter: outer exception", { error: msg, ctx: ctx });
        }
    }

    // Per-brick raster move (centroid shift). Same rule as
    // snap_drift_cluster. Tag with source so the report can
    // attribute which fix caused which raster movement.
    for (var lpKey in brickDeltaSum) {
        if (!brickDeltaSum.hasOwnProperty(lpKey)) continue;
        try {
            var L = findLayer(doc, lpKey);
            if (!L || L.rasterItems.length === 0) continue;
            if (!isLayerEditable(L)) {
                logWarn("move_raster (corner): layer locked", { layer_path: lpKey });
                continue;
            }
            var totalAnchors = 0;
            for (var p2 = 0; p2 < L.pathItems.length; p2++) {
                totalAnchors += L.pathItems[p2].pathPoints.length;
            }
            if (totalAnchors === 0) continue;
            var rdx = brickDeltaSum[lpKey].sx / totalAnchors;
            var rdy = brickDeltaSum[lpKey].sy / totalAnchors;
            if (Math.abs(rdx) < 1e-9 && Math.abs(rdy) < 1e-9) continue;

            var rasterFailures = 0;
            for (var ri = 0; ri < L.rasterItems.length; ri++) {
                var r = L.rasterItems[ri];
                var ok = tryMutate(
                    "raster.position += delta (corner)",
                    { layer_path: lpKey, ri: ri, dx: rdx, dy: rdy },
                    function () {
                        var pos = r.position;
                        r.position = [pos[0] + rdx, pos[1] + rdy];
                    }
                );
                if (!ok) rasterFailures++;
            }
            result.applied.push({
                finding_index: null,
                kind: "raster_track",
                action: "move_raster",
                source: "corner_jitter",
                brick: lpKey.substring(lpKey.lastIndexOf("/") + 1),
                layer_path: lpKey,
                delta_x: rdx,
                delta_y: rdy,
                anchors_moved: brickDeltaSum[lpKey].anchors_moved,
                total_anchors: totalAnchors,
                rasters_moved: L.rasterItems.length - rasterFailures,
                raster_failures: rasterFailures
            });
        } catch (e) {
            logError("move_raster (corner): outer exception", { error: String(e), layer_path: lpKey });
        }
    }
}

function buildAnchorFrequency(snapshot) {
    var freq = {};
    for (var bi = 0; bi < snapshot.bricks.length; bi++) {
        var b = snapshot.bricks[bi];
        if (!b.layer_path || b.layer_path.indexOf("bricks/") !== 0) continue;
        for (var sp = 0; sp < b.sub_paths.length; sp++) {
            var anchors = b.sub_paths[sp].anchors;
            for (var ai = 0; ai < anchors.length; ai++) {
                var k = anchorKey(anchors[ai]);
                freq[k] = (freq[k] || 0) + 1;
            }
        }
    }
    return freq;
}

function anchorKey(p) {
    return p[0].toFixed(4) + "," + p[1].toFixed(4);
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

// Per-sub-path deletion for degenerate sub-paths. Targets findings of
// kind `degenerate_path` (anchor_count < 3 OR area < MIN_AREA_PYMU2).
// Runs LAST among mutating passes because removing pathItems shifts
// sub_path indices for every other finding in the same layer.
//
// Within a single pass, group findings by layer and process highest
// sub_path first so earlier deletes in the same layer don't shift
// later targets.
function applyDeleteDegenerate(doc, findings, result, failedFixes) {
    var perLayer = {};
    for (var i = 0; i < findings.length; i++) {
        var f = findings[i];
        if (f.kind !== "degenerate_path") continue;
        if (!perLayer[f.layer_path]) perLayer[f.layer_path] = [];
        perLayer[f.layer_path].push({ idx: i, finding: f });
    }
    for (var lp in perLayer) {
        if (!perLayer.hasOwnProperty(lp)) continue;
        perLayer[lp].sort(function (a, b) {
            var sa = a.finding.sub_path == null ? -1 : a.finding.sub_path;
            var sb = b.finding.sub_path == null ? -1 : b.finding.sub_path;
            return sb - sa;
        });
        for (var k = 0; k < perLayer[lp].length; k++) {
            applyDeleteDegenerateOne(doc, perLayer[lp][k].finding, perLayer[lp][k].idx, result, failedFixes);
        }
    }
}

function applyDeleteDegenerateOne(doc, finding, idx, result, failedFixes) {
    var ctx = {
        idx: idx, kind: finding.kind, brick: finding.brick,
        layer_path: finding.layer_path, sub_path: finding.sub_path
    };
    var bk = fixKey(finding);
    if (failedFixes[bk]) {
        skipFinding(result, ctx, "blacklisted (prior iteration failed: " + failedFixes[bk] + ")");
        return;
    }
    logDebug("delete_degenerate: begin", ctx);

    try {
        var layer = findLayer(doc, finding.layer_path);
        if (!layer) {
            skipFinding(result, ctx, "layer not found (already deleted by tiny_brick?)");
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
            skipFinding(result, ctx, "sub_path index out of range (layer changed?)");
            return;
        }
        var p = layer.pathItems[finding.sub_path];
        if (!isPathItemEditable(p)) {
            failedFixes[bk] = "pathItem.editable is false";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        var ok = tryMutate("pathItem.remove()", ctx, function () { p.remove(); });
        if (!ok) {
            failedFixes[bk] = "pathItem.remove() threw";
            skipFinding(result, ctx, failedFixes[bk]);
            return;
        }

        result.applied.push({
            finding_index: idx,
            kind: "degenerate_path",
            brick: finding.brick,
            layer_path: finding.layer_path,
            sub_path: finding.sub_path,
            action: "delete_degenerate",
            anchor_count: finding.anchor_count,
            area_pymu2: finding.area_pymu2,
            reason: finding.reason
        });
        logDebug("delete_degenerate: applied", ctx);
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        failedFixes[bk] = "outer exception: " + msg;
        skipFinding(result, ctx, failedFixes[bk]);
        logError("delete_degenerate: outer exception", { error: msg, ctx: ctx });
    }
}

// Unlock every locked layer AND every locked pageItem (pathItems,
// rasterItems, ...) in the document. Lock state at the pageItem
// level is independent of layer-level lock — Illustrator's
// `pathItem.editable` flag returns false if EITHER is set. Also
// makes hidden layers visible, since invisible layers also report
// `editable = false` even if everything else is fine.
//
// Called once before the convergence loop so fixes can reach
// everything. Per artist instruction we do not restore the original
// lock / visibility state — they can re-lock / re-hide manually
// after review.
function unlockAllLayers(doc) {
    var changes = { layers: [], page_items: 0, hidden_layers_shown: [] };
    unlockLayerColl(doc.layers, changes);
    return changes;
}

function unlockLayerColl(coll, changes) {
    for (var i = 0; i < coll.length; i++) {
        var L = coll[i];
        var lp = layerPathOf(L);
        try {
            if (L.locked) {
                L.locked = false;
                changes.layers.push(lp);
                logInfo("unlocked layer", { layer: lp });
            }
        } catch (e) {
            logError("unlockAllLayers: layer failed", { layer: lp, error: String(e) });
        }
        try {
            if (!L.visible) {
                L.visible = true;
                changes.hidden_layers_shown.push(lp);
                logInfo("made layer visible", { layer: lp });
            }
        } catch (e) {
            logError("unlockAllLayers: visibility failed", { layer: lp, error: String(e) });
        }
        // Walk this layer's pageItems and unlock any individually-
        // locked items. pageItems includes pathItems, rasterItems,
        // groupItems, compoundPathItems, and so on — and crucially
        // includes items at any depth of group nesting inside this
        // layer (Illustrator returns them flat).
        try {
            for (var j = 0; j < L.pageItems.length; j++) {
                var item = L.pageItems[j];
                try {
                    if (item.locked) {
                        item.locked = false;
                        changes.page_items++;
                    }
                } catch (eItem) {
                    logError("unlockAllLayers: pageItem failed", {
                        layer: lp, item_index: j, error: String(eItem)
                    });
                }
                try {
                    if (item.hidden) {
                        item.hidden = false;
                    }
                } catch (eHidden) { /* not all pageItems have .hidden */ }
            }
        } catch (eItems) {
            logError("unlockAllLayers: pageItems iteration failed", {
                layer: lp, error: String(eItems)
            });
        }
        try {
            if (L.layers && L.layers.length > 0) {
                unlockLayerColl(L.layers, changes);
            }
        } catch (eSub) { /* ignore */ }
    }
}

function layerPathOf(layer) {
    var parts = [];
    var L = layer;
    while (L && L.typename === "Layer") {
        parts.unshift(L.name);
        try { L = L.parent; } catch (e) { L = null; }
    }
    return parts.join("/");
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
