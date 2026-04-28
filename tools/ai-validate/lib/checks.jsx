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

// Project floor for any geometry — both for whole-brick total area
// (tiny_brick check) and for per-sub-path area (degenerate_path
// check). Empirical distribution across 10 fixtures showed an
// unambiguous cliff: 3 artifacts at 0.000–0.091 pymu² (NY8 Layer 353,
// NY7 Layer 376/379), then a 5,000× gap before real bricks start at
// 486.9 pymu² (intentional 37×29 decorative triangles, confirmed by
// artist). 100 pymu² sits ~1100× above the artifacts and ~4.87×
// below the smallest legitimate brick.
var MIN_AREA_PYMU2     = 100.0;
var MIN_BRICK_AREA     = MIN_AREA_PYMU2;  // alias kept for backwards compat
var MIN_BRICK_TOTAL_AREA = MIN_AREA_PYMU2;
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
    checkTinyBricks(bricks, findings);
    checkPathSpurs(bricks, findings);
    checkUnclosedAndDegenerate(bricks, findings);
    checkSubPymuEdges(bricks, findings);
    checkMultiObjectLayer(bricks, findings);
    checkMultiGridDrift(bricks, findings);
    checkIntraBrickDrift(bricks, findings);
    checkCornerJitter(bricks, findings);
    checkBboxContainment(bricks, findings);
    checkBrickOverlap(bricks, findings);
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
        // page_item_count covers paths, rasters, groups, compound
        // paths, text frames, placed images — everything in the
        // layer except sub-layer structure. A layer is empty iff
        // it has no items AND no sub-layers.
        var pageCount = (L.page_item_count != null) ? L.page_item_count
                      : (L.path_items + L.raster_items);
        if (pageCount === 0 && L.sub_layers === 0) {
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

// --- 1.5 Tiny brick (whole-layer area below threshold) ----------------

function checkTinyBricks(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        var total = 0;
        var hasArea = false;
        for (var s = 0; s < b.sub_paths.length; s++) {
            if (b.sub_paths[s].area === null) continue;
            total += Math.abs(b.sub_paths[s].area);
            hasArea = true;
        }
        if (!hasArea) continue;
        if (total >= MIN_BRICK_TOTAL_AREA) continue;
        findings.push({
            severity: "error",
            kind: "tiny_brick",
            brick: b.id,
            layer_path: b.layer_path,
            sub_path: null,
            total_area_pymu2: total,
            sub_path_count: b.sub_paths.length,
            anchor_counts: anchorCounts(b),
            message: "brick total area " + total.toFixed(3) +
                     " pymu² is below " + MIN_BRICK_TOTAL_AREA +
                     " — almost certainly an artist artifact"
        });
    }
}

function anchorCounts(b) {
    var out = [];
    for (var s = 0; s < b.sub_paths.length; s++) out.push(b.sub_paths[s].anchor_count);
    return out;
}

// --- 1.6 Path spurs (collinear back-track anchors) --------------------
//
// A "spur" is an anchor C such that the polygon goes from B to C and
// then immediately back along the same line (C→D, with B-C-D
// collinear and the C→D vector antiparallel to B→C). The polygon
// visits C and comes back; C is a degenerate vertex that creates an
// out-and-back protrusion in the rendered shape (or a self-overlap
// in fill).
//
// Auto-fixable by removing C — the polygon's shape doesn't change
// after removal.

var SPUR_COLLINEAR_EPS = 1.0;  // |cross product| upper bound. For
                                // 50-pymu edges this corresponds to
                                // ~0.02 pymu off the prev/next line.

function checkPathSpurs(bricks, findings) {
    var EPS_H = 0.001;
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];
        for (var s = 0; s < b.sub_paths.length; s++) {
            var sp = b.sub_paths[s];
            if (!sp.closed) continue; // spurs are a closed-path concept
            var anchors = sp.anchors;
            if (!anchors || anchors.length < 4) continue;
            var pps = sp.path_points;  // null on older snapshots
            var n = anchors.length;
            for (var k = 0; k < n; k++) {
                var pi = (k - 1 + n) % n, ni = (k + 1) % n;
                var prev = anchors[pi], curr = anchors[k], next = anchors[ni];
                var v1x = curr[0] - prev[0], v1y = curr[1] - prev[1];
                var v2x = next[0] - curr[0], v2y = next[1] - curr[1];
                var cross = v1x * v2y - v1y * v2x;
                if (Math.abs(cross) >= SPUR_COLLINEAR_EPS) continue;
                var dot = v1x * v2x + v1y * v2y;
                if (dot >= 0) continue; // not antiparallel

                // Bezier safety net. A linear "B-C-D collinear and
                // antiparallel" pattern can still be a real curve if
                // either segment uses Bezier handles — the rendered
                // shape is determined by the handles, not the
                // anchor positions. Only flag the spur if BOTH
                // adjacent segments are straight, i.e. the relevant
                // handles coincide with their anchors:
                //   - B's right handle ≈ B (B→C is straight outbound)
                //   - C's left handle  ≈ C (B→C is straight inbound)
                //   - C's right handle ≈ C (C→D is straight outbound)
                //   - D's left handle  ≈ D (C→D is straight inbound)
                if (pps && pps.length === n) {
                    var prevPP = pps[pi], currPP = pps[k], nextPP = pps[ni];
                    if (!handleAtAnchor(prevPP.right, prev, EPS_H)) continue;
                    if (!handleAtAnchor(currPP.left,  curr, EPS_H)) continue;
                    if (!handleAtAnchor(currPP.right, curr, EPS_H)) continue;
                    if (!handleAtAnchor(nextPP.left,  next, EPS_H)) continue;
                }

                findings.push({
                    severity: "error",
                    kind: "path_spur",
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor_index: k,
                    anchor: [curr[0], curr[1]],
                    prev: [prev[0], prev[1]],
                    next: [next[0], next[1]],
                    message: "anchor " + k + " is a spur — collinear with " +
                             "neighbors, polygon backtracks through it, " +
                             "both adjacent segments are straight"
                });
            }
        }
    }
}

function handleAtAnchor(h, a, eps) {
    return Math.abs(h[0] - a[0]) < eps && Math.abs(h[1] - a[1]) < eps;
}

// --- 2. Unclosed + degenerate ------------------------------------------

function checkUnclosedAndDegenerate(bricks, findings) {
    for (var i = 0; i < bricks.length; i++) {
        var b = bricks[i];

        // If the whole brick is below the floor, tiny_brick will
        // handle it via whole-layer delete. Don't double-flag every
        // sub-path with a redundant degenerate_path warning.
        var brickTotalArea = 0;
        var allHaveArea = true;
        for (var ts = 0; ts < b.sub_paths.length; ts++) {
            if (b.sub_paths[ts].area === null) { allHaveArea = false; break; }
            brickTotalArea += Math.abs(b.sub_paths[ts].area);
        }
        var brickIsTiny = allHaveArea && brickTotalArea < MIN_BRICK_TOTAL_AREA;

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

            if (brickIsTiny) {
                // tiny_brick will delete the whole layer; nothing to
                // add per sub-path.
                continue;
            }
            if (sp.anchor_count < MIN_ANCHORS) {
                findings.push({
                    severity: "error",
                    kind: "degenerate_path",
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor_count: sp.anchor_count,
                    area_pymu2: sp.area,
                    reason: "anchor_count < " + MIN_ANCHORS,
                    message: "sub-path has " + sp.anchor_count + " anchors (< " + MIN_ANCHORS + ")"
                });
            } else if (sp.area !== null && Math.abs(sp.area) < MIN_AREA_PYMU2) {
                findings.push({
                    severity: "error",
                    kind: "degenerate_path",
                    brick: b.id,
                    layer_path: b.layer_path,
                    sub_path: s,
                    anchor_count: sp.anchor_count,
                    area_pymu2: sp.area,
                    reason: "area < " + MIN_AREA_PYMU2,
                    message: "sub-path area " + sp.area.toFixed(3) +
                             " pymu² is below " + MIN_AREA_PYMU2 + " pymu²"
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

// Polygon-level overlap check. For every pair of bricks whose bboxes
// overlap (and where one doesn't fully contain the other — that's
// brick_bbox_contained's job), test for *proper* segment crossings:
// the segments of two edges have to cross in their interiors. This
// excludes shared edges and corner-touching adjacency.
//
// Severity: error. Polygon overlap is the biggest source of
// downstream render bugs and the artist must resolve it by hand.

function checkBrickOverlap(bricks, findings) {
    var preps = [];
    for (var i = 0; i < bricks.length; i++) {
        var bb = brickBbox(bricks[i]);
        if (!bb) continue;
        preps.push({
            idx: i,
            brick: bricks[i],
            bbox: bb,
            edges: brickEdges(bricks[i])
        });
    }

    for (var a = 0; a < preps.length; a++) {
        for (var c = a + 1; c < preps.length; c++) {
            var pA = preps[a], pB = preps[c];
            // Bbox quick reject — most pairs are bbox-disjoint.
            if (!bboxesOverlap(pA.bbox, pB.bbox)) continue;
            // Skip pure containment cases — brick_bbox_contained
            // already flags them at warning severity.
            if (bboxContains(pA.bbox, pB.bbox) || bboxContains(pB.bbox, pA.bbox)) {
                continue;
            }
            var crossing = findFirstCrossing(pA.edges, pB.edges);
            if (!crossing) continue;
            findings.push({
                severity: "error",
                kind: "brick_overlap",
                brick: pA.brick.id,
                layer_path: pA.brick.layer_path,
                other_brick: pB.brick.id,
                other_layer_path: pB.brick.layer_path,
                sub_path: null,
                edge_a: crossing.a,
                edge_b: crossing.b,
                message: "bricks '" + pA.brick.id + "' and '" + pB.brick.id +
                         "' have crossing edges"
            });
        }
    }
}

function brickEdges(b) {
    var edges = [];
    for (var s = 0; s < b.sub_paths.length; s++) {
        var anchors = b.sub_paths[s].anchors;
        if (!anchors || anchors.length < 2) continue;
        for (var i = 0; i < anchors.length; i++) {
            var p1 = anchors[i];
            var p2 = anchors[(i + 1) % anchors.length];
            edges.push([p1, p2]);
        }
    }
    return edges;
}

// Anything deeper than this in BOTH segments counts as real overlap.
// Effectively a floating-point noise filter only — every overlap of
// any meaningful magnitude is reported. The artist resolves them by
// hand; this kind is not auto-fixable.
var MIN_OVERLAP_DEPTH_PYMU = 1e-6;

function findFirstCrossing(edgesA, edgesB) {
    for (var i = 0; i < edgesA.length; i++) {
        var ea = edgesA[i];
        for (var j = 0; j < edgesB.length; j++) {
            var eb = edgesB[j];
            if (!segmentsCrossProper(ea[0], ea[1], eb[0], eb[1])) continue;
            var xp = segmentIntersection(ea[0], ea[1], eb[0], eb[1]);
            if (!xp) continue;
            var dA = minDistanceToEnds(xp, ea[0], ea[1]);
            var dB = minDistanceToEnds(xp, eb[0], eb[1]);
            if (dA < MIN_OVERLAP_DEPTH_PYMU || dB < MIN_OVERLAP_DEPTH_PYMU) continue;
            return { a: ea, b: eb, point: xp, depth_a: dA, depth_b: dB };
        }
    }
    return null;
}

function segmentIntersection(p1, p2, p3, p4) {
    var x1 = p1[0], y1 = p1[1];
    var x2 = p2[0], y2 = p2[1];
    var x3 = p3[0], y3 = p3[1];
    var x4 = p4[0], y4 = p4[1];
    var denom = (x1 - x2) * (y3 - y4) - (y1 - y2) * (x3 - x4);
    if (Math.abs(denom) < 1e-12) return null;
    var t = ((x1 - x3) * (y3 - y4) - (y1 - y3) * (x3 - x4)) / denom;
    return [x1 + t * (x2 - x1), y1 + t * (y2 - y1)];
}

function minDistanceToEnds(p, a, b) {
    var dxA = p[0] - a[0], dyA = p[1] - a[1];
    var dxB = p[0] - b[0], dyB = p[1] - b[1];
    var dA = Math.sqrt(dxA * dxA + dyA * dyA);
    var dB = Math.sqrt(dxB * dxB + dyB * dyB);
    return dA < dB ? dA : dB;
}

// Strict (proper) segment crossing: each segment's endpoints lie on
// opposite sides of the other segment. Endpoint-touching and
// collinear-overlap return false on purpose so adjacency between
// bricks isn't flagged as overlap.
//
// Floating-point note: ccw values for a real geometric crossing are
// at least edge_length × crossing_depth, so even sub-pymu real
// crossings produce values >> 0.01 (e.g. 50 × 0.005 = 0.25). Pairs
// where ALL ccw values are below SEGMENT_COLLINEAR_EPS_CCW are
// numerically near-collinear; calling them a "crossing" is just
// float noise and produces phantom brick_overlap findings on
// adjacent bricks that share an edge.
var SEGMENT_COLLINEAR_EPS_CCW = 0.01;

function segmentsCrossProper(p1, p2, p3, p4) {
    var d1 = ccw(p3, p4, p1);
    var d2 = ccw(p3, p4, p2);
    var d3 = ccw(p1, p2, p3);
    var d4 = ccw(p1, p2, p4);

    // Both endpoints of one segment within EPS of the other segment's
    // line ⇒ near-collinear. Skip — these aren't real crossings.
    var e = SEGMENT_COLLINEAR_EPS_CCW;
    if (Math.abs(d1) < e && Math.abs(d2) < e) return false;
    if (Math.abs(d3) < e && Math.abs(d4) < e) return false;

    if (((d1 > 0 && d2 < 0) || (d1 < 0 && d2 > 0)) &&
        ((d3 > 0 && d4 < 0) || (d3 < 0 && d4 > 0))) {
        return true;
    }
    return false;
}

function ccw(a, b, c) {
    return (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
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
