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
        children: []
    };
    try { node.raster_items = layer.placedItems.length + layer.rasterItems.length; }
    catch (e) { /* some doc versions lack placedItems */ }

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
    for (var i = 0; i < layer.pathItems.length; i++) {
        var p = layer.pathItems[i];
        var anchors = [];
        for (var k = 0; k < p.pathPoints.length; k++) {
            var pt = p.pathPoints[k];
            anchors.push([pt.anchor[0], pt.anchor[1]]);
        }

        // Some pathItems throw on .area / .geometricBounds (degenerate
        // shapes, single-anchor paths). Tolerate failures — Phase 1
        // checks treat null as "unknown" and skip that branch.
        var area = null;
        try { area = p.area; } catch (e) { area = null; }

        var bbox = null;
        try {
            var b = p.geometricBounds;
            // Normalise to [xmin, ymin, xmax, ymax] regardless of
            // ruler orientation (y-up vs y-down).
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
            anchors: anchors
        });
    }
    return out;
}
