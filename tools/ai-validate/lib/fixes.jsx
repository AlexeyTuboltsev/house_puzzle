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
        // Order matters: close_path first (no anchor changes), then
        // merge_subpymu (mutates pathPoints arrays). Within each pass,
        // fix in reverse-index order so removing an anchor doesn't
        // invalidate later findings that target the same pathItem.
        for (var i = 0; i < findings.length; i++) {
            var f = findings[i];
            if (f.kind === "unclosed_path" || f.kind === "unclosed_path_zero_gap") {
                applyClosePath(doc, f, i, result);
            }
        }
        for (var j = findings.length - 1; j >= 0; j--) {
            var g = findings[j];
            if (g.kind === "sub_pymu_edge") {
                applyMergeSubPymu(doc, g, j, result);
            }
        }
        // Anything else: warning-only or future phase. Mark skipped.
        for (var k = 0; k < findings.length; k++) {
            var h = findings[k];
            if (h.kind !== "unclosed_path" &&
                h.kind !== "unclosed_path_zero_gap" &&
                h.kind !== "sub_pymu_edge") {
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
