// lib/ui.jsx — interactive ScriptUI dialogs.
//
// Used only when running from the bundled .jsx (the AI_VALIDATE_VERSION
// constant injected by packaging/bundle.sh switches validate.jsx into
// interactive mode). Headless dev runs from run.sh write the report
// files and skip these dialogs entirely.

function aiValidateVersionLabel() {
    return (typeof AI_VALIDATE_VERSION === "string") ? AI_VALIDATE_VERSION : "(dev)";
}

function basenameOf(p) {
    if (!p) return "(untitled)";
    var slash = p.lastIndexOf("/");
    var bs    = p.lastIndexOf("\\");
    var i = (slash > bs) ? slash : bs;
    return (i >= 0) ? p.substring(i + 1) : p;
}

function showNoDocumentAlert() {
    alert(
        "ai-validate " + aiValidateVersionLabel() + "\n\n" +
        "Open the .ai file you want to validate first, then run this script again."
    );
}

// Pretty short identifier for a finding row.
function describeFindingShort(f) {
    var brick = f.brick || (f.layer_path ? f.layer_path.replace(/^.*\//, "") : "—");
    var detail = "";
    if (f.kind === "brick_overlap" && f.brick_other) {
        detail = "↔ " + f.brick_other;
    } else if (f.kind === "brick_bbox_contained" && f.brick_other) {
        detail = "inside " + f.brick_other;
    } else if (f.kind === "intra_brick_drift" && f.axis) {
        detail = f.axis + "-axis drift";
        if (typeof f.span === "number") detail += " (" + f.span.toFixed(2) + " pymu)";
    } else if (f.kind === "multi_grid_drift") {
        brick = "(cluster)";
        detail = (f.axis || "") + "-axis";
        if (f.distinct_values && f.distinct_values.length) {
            detail += "  " + f.distinct_values.length + " values";
        }
    } else if (f.kind === "corner_jitter" && f.axis) {
        detail = f.axis + "-axis";
    } else if (f.kind === "bezier_self_intersection") {
        detail = "sub-path " + (f.sub_path == null ? "?" : f.sub_path);
    } else if (f.sub_path != null) {
        detail = "sub-path " + f.sub_path;
    }
    return { brick: brick, detail: detail };
}

function fillFindingsListbox(lb, findings) {
    for (var i = 0; i < findings.length; i++) {
        var f = findings[i];
        var d = describeFindingShort(f);
        var item = lb.add("item", f.severity);
        item.subItems[0].text = f.kind;
        item.subItems[1].text = d.brick + (d.detail ? "  " + d.detail : "");
    }
}

// Build a one-line summary string for the panel header from the
// summary object on `report.summary`.
function summaryHeadline(s) {
    if (!s) return "";
    var nErr  = s.by_severity.error   || 0;
    var nWarn = s.by_severity.warning || 0;
    if (s.total === 0) return "No issues — the file looks clean.";
    return s.total + " issue(s) — " + nErr + " error(s), " + nWarn + " warning(s).";
}

// Walk doc.layers (and sub-layers) and return the layer matching the
// given slash-separated path, e.g. "bricks/Layer 7". Returns null if
// no match (e.g. the layer was renamed or deleted between scans).
function findLayerByPath(doc, layerPath) {
    if (!layerPath) return null;
    var parts = layerPath.split("/");
    var current = doc;
    for (var i = 0; i < parts.length; i++) {
        var found = null;
        try {
            for (var j = 0; j < current.layers.length; j++) {
                if (current.layers[j].name === parts[i]) {
                    found = current.layers[j];
                    break;
                }
            }
        } catch (e) { return null; }
        if (!found) return null;
        current = found;
    }
    return (current === doc) ? null : current;
}

// Set the document's active layer so the Layers panel highlights it
// and Illustrator's selection focuses there. Tolerant: if the layer
// can't be found or activeLayer assignment fails, silently no-op.
function jumpToLayer(doc, layerPath) {
    var layer = findLayerByPath(doc, layerPath);
    if (!layer) return;
    try { doc.activeLayer = layer; } catch (e) { /* tolerate */ }
}

// Compute the union geometricBounds of an array of PathItems. Returns
// [x0, y0, x1, y1] in document coordinates, with x0 < x1 and y0 < y1
// regardless of Illustrator's coordinate orientation. Returns null if
// no bounds are obtainable (degenerate paths, etc.).
function unionBoundingBox(paths) {
    var x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity;
    var any = false;
    for (var i = 0; i < paths.length; i++) {
        var b = null;
        try { b = paths[i].geometricBounds; } catch (e) { continue; }
        if (!b || b.length < 4) continue;
        var lx = Math.min(b[0], b[2]);
        var rx = Math.max(b[0], b[2]);
        var ly = Math.min(b[1], b[3]);
        var ry = Math.max(b[1], b[3]);
        if (lx < x0) x0 = lx;
        if (rx > x1) x1 = rx;
        if (ly < y0) y0 = ly;
        if (ry > y1) y1 = ry;
        any = true;
    }
    return any ? [x0, y0, x1, y1] : null;
}

// Zoom the active view so `bbox` fits with some padding, centered.
// Illustrator caps zoom around 64x; clamp accordingly.
var ZOOM_PADDING_PYMU   = 80;   // breathing room around the brick
var ZOOM_MAX            = 32;   // soft cap so a 1-pymu artifact doesn't 64×
var ZOOM_MIN            = 0.05;

function zoomToBounds(doc, bbox) {
    if (!bbox) return;
    var view = doc.activeView;
    var w = (bbox[2] - bbox[0]) + 2 * ZOOM_PADDING_PYMU;
    var h = (bbox[3] - bbox[1]) + 2 * ZOOM_PADDING_PYMU;
    if (w <= 0 || h <= 0) return;

    var vb = view.bounds;
    var viewW = Math.abs(vb[2] - vb[0]);
    var viewH = Math.abs(vb[3] - vb[1]);
    if (viewW === 0 || viewH === 0) return;

    // view.bounds is in document coords at the CURRENT zoom. To compute
    // the new zoom level that fits `w`/`h` into the view rect, scale
    // current zoom by the ratio of view-extent-in-doc-coords to bbox.
    var newZoom = view.zoom * Math.min(viewW / w, viewH / h);
    if (newZoom < ZOOM_MIN) newZoom = ZOOM_MIN;
    if (newZoom > ZOOM_MAX) newZoom = ZOOM_MAX;

    try { view.zoom = newZoom; } catch (e) {}
    try {
        view.centerPoint = [(bbox[0] + bbox[2]) / 2,
                            (bbox[1] + bbox[3]) / 2];
    } catch (e) {}
}

// Bring the layer behind a finding into view: set as active layer,
// select all its path items so Illustrator draws marquee handles
// around them, and zoom-to-fit. The artist still has to click into
// the canvas to start editing, but they don't have to hunt.
//
// Tolerant of every failure mode (missing layer, locked path,
// unfindable bbox) — at worst the row click is a no-op.
function showFindingInDoc(doc, finding) {
    if (!finding || !finding.layer_path) return;
    var layer = findLayerByPath(doc, finding.layer_path);
    if (!layer) return;

    try { doc.activeLayer = layer; } catch (e) { return; }

    // Use walk_paths' collectPathItems so we follow the same
    // CompoundPathItem / GroupItem unwrapping the snapshot used.
    var paths = collectPathItems(layer);
    try { doc.selection = null; } catch (e) {}
    for (var i = 0; i < paths.length; i++) {
        try { paths[i].selected = true; } catch (e) {}
    }

    var bbox = unionBoundingBox(paths);
    zoomToBounds(doc, bbox);

    // app.redraw() after we mutate selection/zoom; without it Illustrator
    // sometimes batches the visual update until next user input, which
    // defeats the whole "click to see" UX.
    try { app.redraw(); } catch (e) {}
}

// Non-modal panel shown to the artist after the initial report walk.
// Keeps the artist in control: they look at the list, click a row to
// jump to that layer in Illustrator, fix manually, click Refresh to
// re-scan. Auto-fix is just one of three actions, not the only path.
//
// `runReport` and `runFix` are callbacks the caller (validate.jsx)
// passes in. They know how to mutate `report` in-place (re-walk,
// re-check, run the convergence loop). We drive them from button
// handlers.
//
// palette.show() blocks the calling script until the user clicks
// Close (or the X). While the panel is up, the artist can interact
// with the document freely — palette ≠ dialog modality-wise.
function showInteractivePanel(doc, report, runReport, runFix) {
    var basename = basenameOf(report.file);

    var w = new Window("palette", "ai-validate " + aiValidateVersionLabel());
    w.alignChildren = "fill";
    w.preferredSize.width = 760;
    w.spacing = 8;
    w.margins = 12;

    var fileLine = w.add("statictext", undefined, "File: " + basename);
    fileLine.graphics.font = ScriptUI.newFont(fileLine.graphics.font.name, "BOLD",
                                              fileLine.graphics.font.size);

    var summaryLine = w.add("statictext", undefined, summaryHeadline(report.summary));

    var lb = w.add("listbox", undefined, [], {
        multiselect: false,
        numberOfColumns: 3,
        showHeaders: true,
        columnTitles: ["Severity", "Issue", "Where"]
    });
    lb.preferredSize = [740, 380];
    fillFindingsListbox(lb, report.findings);

    // Click a row → jump to that layer in Illustrator. The active
    // layer change is reflected in the Layers panel; the artist still
    // has to click into the canvas to actually edit, but at least
    // they don't have to scroll the Layers panel by hand.
    lb.onChange = function () {
        if (!lb.selection) return;
        var idx = lb.selection.index;
        var f = report.findings[idx];
        if (f) showFindingInDoc(doc, f);
    };

    var hint = w.add("statictext", undefined,
        "Click a row to zoom in on that finding and select its paths. " +
        "Fix manually in Illustrator, then click Refresh to re-scan. " +
        "“Fix what’s fixable” applies deterministic auto-fixes; " +
        "manual-review issues stay in the list.",
        { multiline: true });
    hint.preferredSize.width = 740;

    var btns = w.add("group");
    btns.alignment = "right";
    var refreshBtn = btns.add("button", undefined, "Refresh");
    var fixBtn     = btns.add("button", undefined, "Fix what’s fixable");
    var closeBtn   = btns.add("button", undefined, "Close", { name: "cancel" });

    function repaintList() {
        lb.removeAll();
        fillFindingsListbox(lb, report.findings);
        summaryLine.text = summaryHeadline(report.summary);
        try { w.update(); } catch (e) {}
    }

    refreshBtn.onClick = function () {
        refreshBtn.enabled = false;
        fixBtn.enabled = false;
        try {
            runReport();
            repaintList();
        } finally {
            refreshBtn.enabled = true;
            fixBtn.enabled = true;
        }
    };

    fixBtn.onClick = function () {
        refreshBtn.enabled = false;
        fixBtn.enabled = false;
        try {
            runFix();
            repaintList();
            // Append a "Save your work" reminder to the summary line
            // — the panel stays open afterwards so the artist sees it.
            summaryLine.text = summaryHeadline(report.summary) +
                "  •  Save the document to keep changes.";
        } finally {
            refreshBtn.enabled = true;
            fixBtn.enabled = true;
        }
    };

    closeBtn.onClick = function () { w.close(); };

    w.show();
}

// Non-modal palette shown during the fix loop so the artist sees
// progress feedback. ScriptUI's progressbar can be updated between
// fix iterations; we drive it from validate.jsx.
function openProgressPalette(maxSteps) {
    var p = new Window("palette", "ai-validate — working…");
    p.alignChildren = "fill";
    p.margins = 12;
    p.spacing = 6;
    p.preferredSize.width = 360;

    var label = p.add("statictext", undefined, "Applying fixes…");
    label.preferredSize.width = 340;

    var bar = p.add("progressbar", undefined, 0, maxSteps || 8);
    bar.preferredSize = [340, 16];

    p.show();
    return { window: p, bar: bar, label: label };
}

function updateProgressPalette(handle, step, message) {
    if (!handle) return;
    if (typeof step === "number") handle.bar.value = step;
    if (message) handle.label.text = message;
    try { handle.window.update(); } catch (e) { /* tolerate older engines */ }
}

function closeProgressPalette(handle) {
    if (!handle) return;
    try { handle.window.close(); } catch (e) { /* already closed */ }
}
