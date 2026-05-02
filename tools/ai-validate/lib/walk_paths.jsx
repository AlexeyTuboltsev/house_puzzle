// lib/walk_paths.jsx — DOM → JSON snapshot of every brick.
//
// Phase 0 helper. The Rust parser identifies brick layers by scanning
// the AI source's %AI5_BeginLayer markers; the Illustrator DOM exposes
// the same structure as Layer / SubLayer / PathItem objects.
//
// We don't yet know the artist's exact layer-naming convention, so this
// helper is intentionally permissive: it walks the entire layer tree
// and emits everything that looks like a brick (a layer whose direct
// children include at least one PathItem).
//
// Returns:
//   {
//     layer_tree: [{ name, depth, kind, children: [...] }],
//     bricks: [{ id, layer_path: "bricks/b042", sub_paths: [
//       { closed, anchor_count, anchors: [[x,y], ...] }
//     ]}]
//   }

function walkPaths(doc) {
    var out = {
        layer_tree: [],
        bricks: []
    };

    for (var i = 0; i < doc.layers.length; i++) {
        out.layer_tree.push(describeLayer(doc.layers[i], 0));
    }

    // Walk the tree and collect anything that looks like a brick.
    for (var j = 0; j < doc.layers.length; j++) {
        collectBricks(doc.layers[j], "", out.bricks);
    }

    return out;
}

function describeLayer(layer, depth) {
    var node = {
        name: layer.name,
        depth: depth,
        path_items: layer.pathItems.length,
        sub_layers: layer.layers.length,
        raster_items: 0,
        page_item_count: 0,
        children: []
    };
    try { node.raster_items = layer.placedItems.length + layer.rasterItems.length; }
    catch (e) { /* some doc versions lack placedItems */ }
    // pageItems counts EVERYTHING inside this layer (paths, rasters,
    // groups, compound paths, text frames, placed images, ...) at any
    // nesting depth within the layer but not crossing into sub-layers.
    // This is the canonical "is this layer empty" total.
    try { node.page_item_count = layer.pageItems.length; }
    catch (e) { node.page_item_count = node.path_items + node.raster_items; }

    for (var i = 0; i < layer.layers.length; i++) {
        node.children.push(describeLayer(layer.layers[i], depth + 1));
    }
    return node;
}

// "Brick-like" heuristic: a leaf layer (no sub-layers) with ≥ 1
// pathItem. The artist's convention may name them b001, b002, etc.;
// we record the full layer path so non-conforming names are still
// visible in the report.
function collectBricks(layer, parentPath, bricks) {
    var path = parentPath ? (parentPath + "/" + layer.name) : layer.name;

    if (layer.layers.length === 0 && layer.pathItems.length > 0) {
        bricks.push({
            id: layer.name,
            layer_path: path,
            sub_paths: extractSubPaths(layer)
        });
        return;
    }

    for (var i = 0; i < layer.layers.length; i++) {
        collectBricks(layer.layers[i], path, bricks);
    }
}

function extractSubPaths(layer) {
    var out = [];
    var EPS_H = 0.001;  // handle-coincident-with-anchor tolerance
    for (var i = 0; i < layer.pathItems.length; i++) {
        var p = layer.pathItems[i];
        var anchors = [];
        var path_points = [];
        var has_curves = false;
        for (var k = 0; k < p.pathPoints.length; k++) {
            var pt = p.pathPoints[k];
            var a = pt.anchor;
            anchors.push([a[0], a[1]]);
            var ld, rd;
            try {
                ld = pt.leftDirection;
                rd = pt.rightDirection;
            } catch (e) { ld = a; rd = a; }
            path_points.push({
                anchor: [a[0], a[1]],
                left:   [ld[0], ld[1]],
                right:  [rd[0], rd[1]]
            });
            // A handle that's not coincident with its anchor signals
            // an active Bezier curve at this anchor. Spur detection
            // must skip these.
            if (Math.abs(ld[0] - a[0]) > EPS_H || Math.abs(ld[1] - a[1]) > EPS_H ||
                Math.abs(rd[0] - a[0]) > EPS_H || Math.abs(rd[1] - a[1]) > EPS_H) {
                has_curves = true;
            }
        }

        // Some pathItems throw on .area / .geometricBounds (degenerate
        // shapes, single-anchor paths). Tolerate failures.
        var area = null;
        try { area = p.area; } catch (e) { area = null; }

        var bbox = null;
        try {
            var b = p.geometricBounds;
            var x0 = Math.min(b[0], b[2]);
            var x1 = Math.max(b[0], b[2]);
            var y0 = Math.min(b[1], b[3]);
            var y1 = Math.max(b[1], b[3]);
            bbox = [x0, y0, x1, y1];
        } catch (e) { bbox = null; }

        out.push({
            closed: !!p.closed,
            anchor_count: anchors.length,
            area: area,
            bbox: bbox,
            anchors: anchors,
            path_points: path_points,
            has_curves: has_curves
        });
    }
    return out;
}
