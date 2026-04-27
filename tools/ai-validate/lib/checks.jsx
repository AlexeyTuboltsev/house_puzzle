// lib/checks.jsx — Phase 1 read-only checks.
//
// Pure functions over a snapshot produced by walk_paths.jsx.
// Each check appends 0+ findings to the shared array.
//
// Finding shape (uniform across kinds):
//   {
//     severity: "error" | "warning",
//     kind: <string identifier>,
//     brick: <Illustrator layer name, e.g. "Layer 81"> | null,
//     layer_path: "bricks/Layer 81" | null,
//     sub_path: <int index into brick.sub_paths> | null,
//     message: <human-readable summary>,
//     ...kind-specific fields
//   }
//
// `severity`:
//   - "error"  : runtime would render visibly wrong / parser may crash;
//                CI must fail.
//   - "warning": ambiguous or artist-judgement; report only, never
//                auto-fixed.
//
// All distance/area thresholds are in document ruler units. The Rust
// pipeline calls these "pymu"; for these AI files 1 pymu = 1 point.

// --- thresholds ---------------------------------------------------------

var EPS_CLOSED         = 0.001;  // first==last considered "zero gap"
var MIN_ANCHORS        = 3;      // <3 anchors → degenerate
var MIN_BRICK_AREA     = 1.0;    // pymu² — degenerate area
var MIN_EDGE_LEN       = 1.0;    // pymu  — sub-pymu staircase
var MULTI_GRID_TOL     = 0.1;    // pymu  — cluster span tolerance
var GRID_CLUSTER_RADIUS = 1.0;   // pymu  — single-link cluster radius
var CORNER_JITTER_MIN  = 0.05;   // pymu  — below this, considered "matched"
var CORNER_JITTER_MAX  = 1.0;    // pymu  — above this, not the same corner

var REQUIRED_TOP_LAYERS = ["bricks", "screen", "background"];
var BRICK_CONTAINER_PREFIX = "bricks/";

// --- entry point --------------------------------------------------------

function runChecks(snapshot) {
    var findings = [];
    var bricks = brickLayers(snapshot);
    checkLayerStructure(snapshot, findings);
    checkUnclosedAndDegenerate(bricks, findings);
    checkSubPymuEdges(bricks, findings);
    checkMultiObjectLayer(bricks, findings);
    checkMultiGridDrift(bricks, findings);
    checkIntraBrickDrift(bricks, findings);
    checkCornerJitter(bricks, findings);
    checkBboxContainment(bricks, findings);
    return findings;
}

// Filter snapshot.bricks down to actual brick layers (children of the
// "bricks/" container). Other path-bearing top-level layers like
// `screen` are excluded — they are the playfield outline / chrome,
// not bricks, and their bbox would otherwise dominate brick-on-brick
// checks.
function brickLayers(snapshot) {
    var out = [];
    for (var i = 0; i < snapshot.bricks.length; i++) {
        var b = snapshot.bricks[i];
        if (b.layer_path && b.layer_path.indexOf(BRICK_CONTAINER_PREFIX) === 0) {
            out.push(b);
        }
    }
    return out;
}

// --- helpers ------------------------------------------------------------

function countKeys(obj) {
    var n = 0;
    for (var k in obj) {
        if (obj.hasOwnProperty(k)) n++;
    }
    return n;
}

function objectKeys(obj) {
    var out = [];
    for (var k in obj) {
        if (obj.hasOwnProperty(k)) out.push(k);
    }
    return out;
}

function dist(a, b) {
    var dx = a[0] - b[0];
    var dy = a[1] - b[1];
    return Math.sqrt(dx * dx + dy * dy);
}

// --- 1. Layer structure -------------------------------------------------

function checkLayerStructure(snapshot, findings) {
    var top = snapshot.layer_tree;
    var byName = {};
    for (var i = 0; i < top.length; i++) byName[top[i].name] = top[i];

    for (var j = 0; j < REQUIRED_TOP_LAYERS.length; j++) {
        var name = REQUIRED_TOP_LAYERS[j];
        var L = byName[name];
        if (!L) {
            findings.push({
                severity: "warning",
                kind: "missing_top_layer",
                brick: null,
                layer_path: name,
                sub_path: null,
                message: "expected top-level layer '" + name + "' is missing"
            });
            continue;
        }
        if (L.path_items === 0 && L.sub_layers === 0 && L.raster_items === 0) {
            findings.push({
                severity: "warning",
                kind: "empty_top_layer",
                brick: null,
                layer_path: name,
                sub_path: null,
                message: "top-level layer '" + name + "' is empty"
            });
        }
    }
}

// --- 2. Unclosed + degenerate ------------------------------------------

function checkUnclosedAndDegenerate(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        for (var s = 0; s < b.sub_paths.length; s++) {
            var sp = b.sub_paths[s];

            if (!sp.closed && sp.anchors.length >= 2) {
                var first = sp.anchors[0];
                var last  = sp.anchors[sp.anchors.length - 1];
                var gap = dist(first, last);
                if (gap < EPS_CLOSED) {
                    findings.push({
                        severity: "warning",
                        kind: "unclosed_path_zero_gap",
                        brick: b.id,
                        layer_path: b.layer_path,
                        sub_path: s,
                        gap_pymu: gap,
                        anchor_count: sp.anchor_count,
                        first: first,
                        last: last,
                        message: "sub-path is visually closed (first == last) but pathItem.closed is false"
                    });
                } else {
                    findings.push({
                        severity: "error",
                        kind: "unclosed_path",
                        brick: b.id,
                        layer_path: b.layer_path,
                        sub_path: s,
                        gap_pymu: gap,
                        anchor_count: sp.anchor_count,
                        first: first,
                        last: last,
                        message: "open sub-path with " + sp.anchor_count +
                                 " anchors; gap " + gap.toFixed(3) + " pymu"
                    });
                }
            }

            if (sp.anchor_count < MIN_ANCHORS) {
                findings.push({
                    severity: "error",
                    kind: "degenerate_path",
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor_count: sp.anchor_count,
                    message: "sub-path has " + sp.anchor_count + " anchors (< " + MIN_ANCHORS + ")"
                });
            } else if (sp.area !== null && Math.abs(sp.area) < MIN_BRICK_AREA) {
                findings.push({
                    severity: "warning",
                    kind: "degenerate_area",
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    area_pymu2: sp.area,
                    message: "sub-path signed area " + sp.area.toFixed(3) +
                             " pymu² is below " + MIN_BRICK_AREA + " pymu²"
                });
            }
        }
    }
}

// --- 3. Sub-pymu staircase edge ----------------------------------------

function checkSubPymuEdges(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        for (var s = 0; s < b.sub_paths.length; s++) {
            var sp = b.sub_paths[s];
            var anchors = sp.anchors;
            for (var k = 0; k < anchors.length; k++) {
                var nextK = k + 1;
                if (nextK >= anchors.length) {
                    if (!sp.closed) break;
                    nextK = 0;
                }
                var len = dist(anchors[k], anchors[nextK]);
                if (len > 0 && len < MIN_EDGE_LEN) {
                    findings.push({
                        severity: "warning",
                        kind: "sub_pymu_edge",
                        brick: b.id,
                        layer_path: b.layer_path,
                        sub_path: s,
                        edge_index: k,
                        edge_len_pymu: len,
                        from: anchors[k],
                        to: anchors[nextK],
                        message: "sub-pymu edge " + len.toFixed(3) +
                                 " pymu between anchor " + k + " and " + nextK
                    });
                }
            }
        }
    }
}

// --- 4. Multi-object layer ---------------------------------------------

function bboxesOverlap(a, b) {
    if (a[2] < b[0] || b[2] < a[0]) return false; // x-disjoint
    if (a[3] < b[1] || b[3] < a[1]) return false; // y-disjoint
    return true;
}

function checkMultiObjectLayer(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        if (b.sub_paths.length < 2) continue;

        var bboxes = [];
        for (var s = 0; s < b.sub_paths.length; s++) {
            if (b.sub_paths[s].bbox) bboxes.push({ idx: s, bbox: b.sub_paths[s].bbox });
        }

        var disjoint = [];
        for (var a = 0; a < bboxes.length; a++) {
            for (var c = a + 1; c < bboxes.length; c++) {
                if (!bboxesOverlap(bboxes[a].bbox, bboxes[c].bbox)) {
                    disjoint.push([bboxes[a].idx, bboxes[c].idx]);
                }
            }
        }
        if (disjoint.length > 0) {
            findings.push({
                severity: "warning",
                kind: "multi_object_layer",
                brick: b.id,
                layer_path: b.layer_path,
                sub_path: null,
                sub_path_count: b.sub_paths.length,
                disjoint_pairs: disjoint,
                message: "brick has " + b.sub_paths.length + " sub-paths with " +
                         disjoint.length + " bbox-disjoint pair(s) — likely multiple bricks merged"
            });
        }
    }
}

// --- 5. Multi-grid drift across bricks ---------------------------------

function gatherAxisSamples(bricks, axisIdx) {
    var samples = [];
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        for (var s = 0; s < b.sub_paths.length; s++) {
            var anchors = b.sub_paths[s].anchors;
            for (var k = 0; k < anchors.length; k++) {
                samples.push({
                    v: anchors[k][axisIdx],
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor: k
                });
            }
        }
    }
    samples.sort(function (x, y) { return x.v - y.v; });
    return samples;
}

function singleLinkCluster(samples) {
    var clusters = [];
    var current = null;
    for (var j = 0; j < samples.length; j++) {
        var sm = samples[j];
        if (!current || sm.v - current.last > GRID_CLUSTER_RADIUS) {
            current = { min: sm.v, last: sm.v, members: [sm] };
            clusters.push(current);
        } else {
            current.last = sm.v;
            current.members.push(sm);
        }
    }
    return clusters;
}

function checkMultiGridDrift(bricks, findings) {
    flagAxisDrift(bricks, 0, "x", findings);
    flagAxisDrift(bricks, 1, "y", findings);
}

function flagAxisDrift(bricks, axisIdx, axisName, findings) {
    var samples = gatherAxisSamples(bricks, axisIdx);
    var clusters = singleLinkCluster(samples);

    for (var c = 0; c < clusters.length; c++) {
        var cl = clusters[c];
        var span = cl.last - cl.min;
        if (span <= MULTI_GRID_TOL) continue;

        var bricksInCluster = {};
        var pathsInCluster = {};
        var distinctVals = {};
        for (var m = 0; m < cl.members.length; m++) {
            bricksInCluster[cl.members[m].brick] = true;
            pathsInCluster[cl.members[m].layer_path] = true;
            var key = cl.members[m].v.toFixed(4);
            distinctVals[key] = (distinctVals[key] || 0) + 1;
        }
        var bricksList = objectKeys(bricksInCluster);
        var pathsList  = objectKeys(pathsInCluster);
        if (bricksList.length < 2) continue; // intra-brick drift handles this

        findings.push({
            severity: "error",
            kind: "multi_grid_drift",
            brick: null,
            layer_path: null,
            sub_path: null,
            axis: axisName,
            cluster_min: cl.min,
            cluster_max: cl.last,
            span_pymu: span,
            bricks: bricksList,
            member_layer_paths: pathsList,
            distinct_values: distinctVals,
            message: axisName + "-axis cluster spans " + span.toFixed(3) +
                     " pymu (" + cl.min.toFixed(3) + ".." + cl.last.toFixed(3) +
                     ") across " + bricksList.length + " bricks"
        });
    }
}

// --- 6. Intra-brick cluster drift --------------------------------------

function checkIntraBrickDrift(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        flagBrickAxisDrift(bricks[i], 0, "x", findings);
        flagBrickAxisDrift(bricks[i], 1, "y", findings);
    }
}

function flagBrickAxisDrift(b, axisIdx, axisName, findings) {
    var values = [];
    for (var s = 0; s < b.sub_paths.length; s++) {
        var anchors = b.sub_paths[s].anchors;
        for (var k = 0; k < anchors.length; k++) {
            values.push(anchors[k][axisIdx]);
        }
    }
    if (values.length < 2) return;
    values.sort(function (a, b) { return a - b; });

    var clusters = [];
    var current = null;
    for (var j = 0; j < values.length; j++) {
        if (!current || values[j] - current.last > GRID_CLUSTER_RADIUS) {
            current = { min: values[j], last: values[j], values: [values[j]] };
            clusters.push(current);
        } else {
            current.last = values[j];
            current.values.push(values[j]);
        }
    }

    for (var c = 0; c < clusters.length; c++) {
        var cl = clusters[c];
        var span = cl.last - cl.min;
        if (span <= MULTI_GRID_TOL) continue;
        var distinct = {};
        for (var m = 0; m < cl.values.length; m++) {
            var key = cl.values[m].toFixed(4);
            distinct[key] = (distinct[key] || 0) + 1;
        }
        if (countKeys(distinct) < 2) continue;
        findings.push({
            severity: "warning",
            kind: "intra_brick_drift",
            brick: b.id,
            layer_path: b.layer_path,
            sub_path: null,
            axis: axisName,
            cluster_min: cl.min,
            cluster_max: cl.last,
            span_pymu: span,
            distinct_values: distinct,
            message: "intra-brick " + axisName + "-drift in '" + b.id +
                     "': cluster span " + span.toFixed(3) + " pymu"
        });
    }
}

// --- 7. Adjacent-brick corner jitter -----------------------------------
//
// Emit when two anchors from different bricks lie within
// CORNER_JITTER_MAX of each other but more than CORNER_JITTER_MIN
// apart — i.e. they nominally share a corner, but the artist's
// values disagree by enough to drift the runtime mesh.
//
// O(n²) over all anchors, but n ~ 2k for these files; fine.

function checkCornerJitter(bricks, findings) {
    var anchors = [];
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        for (var s = 0; s < b.sub_paths.length; s++) {
            var ap = b.sub_paths[s].anchors;
            for (var k = 0; k < ap.length; k++) {
                anchors.push({
                    v: ap[k],
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor: k
                });
            }
        }
    }

    var seen = {}; // dedupe symmetric pairs
    for (var a = 0; a < anchors.length; a++) {
        for (var c = a + 1; c < anchors.length; c++) {
            if (anchors[a].brick === anchors[c].brick) continue;
            var d = dist(anchors[a].v, anchors[c].v);
            if (d <= CORNER_JITTER_MIN) continue;
            if (d >= CORNER_JITTER_MAX) continue;
            // Dedupe: same brick-pair + same rounded position only once.
            var k1 = anchors[a].brick + "|" + anchors[c].brick + "|" +
                     anchors[a].v[0].toFixed(1) + "," + anchors[a].v[1].toFixed(1);
            if (seen[k1]) continue;
            seen[k1] = true;

            findings.push({
                severity: "warning",
                kind: "corner_jitter",
                brick: anchors[a].brick,
                layer_path: anchors[a].layer_path,
                sub_path: anchors[a].sub_path,
                other_brick: anchors[c].brick,
                other_layer_path: anchors[c].layer_path,
                other_sub_path: anchors[c].sub_path,
                anchor: anchors[a].v,
                other_anchor: anchors[c].v,
                jitter_pymu: d,
                message: "corner jitter " + d.toFixed(3) + " pymu between '" +
                         anchors[a].brick + "' and '" + anchors[c].brick + "'"
            });
        }
    }
}

// --- 8. Bbox containment (cheap pre-pass for overlap) ------------------
//
// Full polygon overlap is the slowest plan-listed check — left for a
// later pass once we see how many bbox-containment hits there are.
// This pre-pass surfaces the obvious cases: one brick's bbox lies
// fully inside another's.

function bboxContains(outer, inner) {
    return outer[0] <= inner[0] && outer[1] <= inner[1] &&
           outer[2] >= inner[2] && outer[3] >= inner[3];
}

function brickBbox(b) {
    var bb = null;
    for (var s = 0; s < b.sub_paths.length; s++) {
        var sb = b.sub_paths[s].bbox;
        if (!sb) continue;
        if (!bb) {
            bb = [sb[0], sb[1], sb[2], sb[3]];
        } else {
            if (sb[0] < bb[0]) bb[0] = sb[0];
            if (sb[1] < bb[1]) bb[1] = sb[1];
            if (sb[2] > bb[2]) bb[2] = sb[2];
            if (sb[3] > bb[3]) bb[3] = sb[3];
        }
    }
    return bb;
}

function checkBboxContainment(bricks, findings) {
    var bbs = [];
    for (var i = 0; i < bricks.length; i++) {
        var bb = brickBbox(bricks[i]);
        if (bb) bbs.push({ idx: i, bbox: bb });
    }
    for (var a = 0; a < bbs.length; a++) {
        for (var c = 0; c < bbs.length; c++) {
            if (a === c) continue;
            if (bboxContains(bbs[a].bbox, bbs[c].bbox)) {
                var ba = bricks[bbs[a].idx];
                var bc = bricks[bbs[c].idx];
                findings.push({
                    severity: "warning",
                    kind: "brick_bbox_contained",
                    brick: bc.id,
                    layer_path: bc.layer_path,
                    sub_path: null,
                    container_brick: ba.id,
                    container_layer_path: ba.layer_path,
                    message: "'" + bc.id + "' bbox is fully inside '" + ba.id + "'"
                });
            }
        }
    }
}
