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

// Kinds that runFixes will attempt to auto-fix. The actual fix may
// still skip individual instances at apply-time (locked layer, brick
// overlap above the auto-threshold, ...), but the kind-level set
// gives the artist a useful "this row could potentially be fixed"
// hint without us having to per-finding pre-validate.
//
// Kinds NOT in this set (intentionally unfixable):
//   - missing_top_layer / empty_top_layer (artist decision: rename
//     vs add — Phase B will let them pick a rename target inline)
//   - bezier_self_intersection (artist must redraw the corner)
//   - intra_brick_drift, multi_object_layer (warnings, not errors)
//   - brick_bbox_contained (intentional in 100% of observed cases)
var FIXABLE_KINDS = {
    unclosed_path: true,
    unclosed_path_zero_gap: true,
    sub_pymu_edge: true,
    tiny_brick: true,
    multi_grid_drift: true,
    corner_jitter: true,
    brick_overlap: true,
    path_spur: true,
    degenerate_path: true,
    degenerate_area: true
};

function isFindingFixable(f) {
    return !!(f && f.kind && FIXABLE_KINDS[f.kind] === true);
}

function countFixable(findings) {
    var n = 0;
    for (var i = 0; i < (findings ? findings.length : 0); i++) {
        if (isFindingFixable(findings[i])) n++;
    }
    return n;
}

function fillFindingsListbox(lb, findings) {
    for (var i = 0; i < findings.length; i++) {
        var f = findings[i];
        var d = describeFindingShort(f);
        var item = lb.add("item", f.severity);
        item.subItems[0].text = f.kind;
        item.subItems[1].text = d.brick + (d.detail ? "  " + d.detail : "");
        item.subItems[2].text = isFindingFixable(f) ? "✓" : "";
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

// Self-contained "navigate to layer + zoom + select" function. Sent
// to Illustrator's BridgeTalk runtime via toString() + BridgeTalk
// dispatch from the panel's row-click handler. MUST NOT reference any
// external symbol — when stringified and sent through BridgeTalk it
// runs in a fresh engine context that only has the standard
// Illustrator globals (`app`, `File`, ...). Direct calls to layer
// helpers / `doc` references from a palette callback throw "there is
// no document" — Illustrator's documented quirk; BridgeTalk-from-
// palette is the canonical workaround.
function showFindingInDocBT(layerPath) {
    var doc = null;
    try { doc = app.activeDocument; } catch (e) { return; }
    if (!doc || !layerPath) return;

    // Walk doc.layers by slash-separated parts.
    var parts = String(layerPath).split("/");
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
        } catch (eL) { return; }
        if (!found) return;
        current = found;
    }
    if (current === doc) return;
    var layer = current;

    // Set the active layer so the Layers panel highlights it.
    try { doc.activeLayer = layer; } catch (eAL) {}

    // Collect every PathItem under this layer (recursing into
    // CompoundPathItems and GroupItems).
    function collect(parent) {
        var out = [];
        var pageItems;
        try { pageItems = parent.pageItems; } catch (eP) { return out; }
        for (var k = 0; k < pageItems.length; k++) {
            var item = pageItems[k];
            var t;
            try { t = item.typename; } catch (eT) { continue; }
            if (t === "PathItem") {
                out.push(item);
            } else if (t === "CompoundPathItem") {
                try {
                    for (var c = 0; c < item.pathItems.length; c++) out.push(item.pathItems[c]);
                } catch (eC) {}
            } else if (t === "GroupItem") {
                var sub = collect(item);
                for (var s = 0; s < sub.length; s++) out.push(sub[s]);
            }
        }
        return out;
    }
    var paths = collect(layer);

    // Select all paths so Illustrator draws marquee handles around
    // them (the visual "this is the brick" cue).
    try { doc.selection = null; } catch (eS) {}
    for (var p = 0; p < paths.length; p++) {
        try { paths[p].selected = true; } catch (ePS) {}
    }

    // Union bounding box.
    var x0 = Infinity, y0 = Infinity, x1 = -Infinity, y1 = -Infinity, any = false;
    for (var b = 0; b < paths.length; b++) {
        var bb = null;
        try { bb = paths[b].geometricBounds; } catch (eB) { continue; }
        if (!bb || bb.length < 4) continue;
        var lx = Math.min(bb[0], bb[2]), rx = Math.max(bb[0], bb[2]);
        var ly = Math.min(bb[1], bb[3]), ry = Math.max(bb[1], bb[3]);
        if (lx < x0) x0 = lx;
        if (rx > x1) x1 = rx;
        if (ly < y0) y0 = ly;
        if (ry > y1) y1 = ry;
        any = true;
    }

    // Zoom-to-fit with padding, clamped.
    if (any) {
        var pad = 80;
        var w = (x1 - x0) + 2 * pad;
        var h = (y1 - y0) + 2 * pad;
        if (w > 0 && h > 0) {
            var view = doc.activeView;
            var vb = view.bounds;
            var vw = Math.abs(vb[2] - vb[0]);
            var vh = Math.abs(vb[3] - vb[1]);
            if (vw > 0 && vh > 0) {
                var newZoom = view.zoom * Math.min(vw / w, vh / h);
                if (newZoom < 0.05) newZoom = 0.05;
                if (newZoom > 32) newZoom = 32;
                try { view.zoom = newZoom; } catch (eZ) {}
                try { view.centerPoint = [(x0 + x1) / 2, (y0 + y1) / 2]; } catch (eC) {}
            }
        }
    }

    try { app.redraw(); } catch (eR) {}
}


// Non-modal panel shown to the artist. Opens immediately with empty
// content + a "Scanning…" status, then auto-triggers a refresh that
// BT-dispatches the walk + check; when results land the listbox
// populates. The artist clicks rows to navigate, [Fix what's fixable]
// to apply auto-fixes, [Close] to dismiss. While a refresh or fix is
// in flight, all interactive UI is disabled and the summary line
// reflects what's running.
//
// All document-touching work runs through BridgeTalk dispatch — direct
// doc access from a palette callback throws "there is no document" in
// Illustrator (a documented quirk). The BT body re-evaluates
// validate.jsx with AI_VALIDATE_ACTION set to trigger the headless
// action-mode branch; the panel reads the resulting /tmp report file
// in the BT onResult callback.
function showInteractivePanel(doc, report) {
    var basename = basenameOf(report.file);
    try { logInfo("ui: building panel", { findings: report.findings.length }); } catch (e) {}

    var w = new Window("palette", "ai-validate " + aiValidateVersionLabel());
    w.alignChildren = "fill";
    w.preferredSize.width = 760;
    w.spacing = 8;
    w.margins = 12;

    var fileLine = w.add("statictext", undefined, "File: " + basename);
    fileLine.graphics.font = ScriptUI.newFont(fileLine.graphics.font.name, "BOLD",
                                              fileLine.graphics.font.size);

    var summaryLine = w.add("statictext", undefined, summaryHeadline(report.summary));

    // Update-available banner: rendered as a single static line just
    // under the summary. Hidden (zero-height) when no update is
    // available. Replaces the old modal-alert behavior — modal alerts
    // interrupted the artist on every panel-open; an inline banner
    // is informational, dismissable, and never blocks editing.
    var updateBanner = w.add("statictext", undefined, "", { multiline: true });
    updateBanner.preferredSize.width = 740;
    updateBanner.visible = false;
    var updateManifest = null;
    try { updateManifest = getAvailableUpdateManifest(); } catch (eU) {}
    if (updateManifest && updateManifest.version) {
        var url = "";
        if (updateManifest.platforms) {
            url = ($.os.indexOf("Windows") >= 0)
                ? (updateManifest.platforms.windows || "")
                : (updateManifest.platforms.macos   || "");
        }
        if (!url) url = updateManifest.release_notes_url || "";
        updateBanner.text =
            "⚠  ai-validate " + updateManifest.version +
            " is available (you have " + aiValidateVersionLabel() + "). " +
            (url ? "Download: " + url : "");
        updateBanner.visible = true;
    }

    var lb = w.add("listbox", undefined, [], {
        multiselect: false,
        numberOfColumns: 4,
        showHeaders: true,
        columnTitles: ["Severity", "Issue", "Where", "Auto-fix"]
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
        if (!f || !f.layer_path) return;
        // Direct doc access from palette onChange throws "there is no
        // document" — even fresh app.activeDocument fails. Adobe's
        // documented workaround is BridgeTalk: dispatch a stringified
        // function to the persistent BT engine where doc access works
        // normally. See community thread + oshatrk's working example.
        var bt = new BridgeTalk();
        bt.target = BridgeTalk.appSpecifier;
        bt.body = "(" + showFindingInDocBT.toString() + ")(" +
                  JSON.stringify(f.layer_path) + ");";
        bt.send();
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
        // Fix button only enabled when there's something it can do.
        // Keeps the artist from clicking it on a clean file (or one
        // whose remaining findings are all manual-review issues like
        // brick_overlap > threshold or bezier_self_intersection).
        fixBtn.enabled = countFixable(report.findings) > 0;
        try { w.update(); } catch (e) {}
    }

    // Toggle interactive UI during long-running BT dispatches. While
    // busy, the listbox and the action buttons are disabled (so the
    // artist can't queue up a second action mid-scan), and the
    // summary line shows the current status. ScriptUI in Illustrator
    // has no async timer — we can't actually animate a spinner —
    // but the disabled-UI + status-text pair is a clear "working"
    // signal. Close stays enabled so the artist can always abort.
    function setBusy(running, statusText) {
        refreshBtn.enabled = !running;
        // Fix is more nuanced: while busy → forced disabled; not busy
        // → enabled only if the current report has at least one
        // fixable finding. repaintList() also adjusts this after a
        // refresh/fix completes.
        fixBtn.enabled = !running && countFixable(report.findings) > 0;
        try { lb.enabled = !running; } catch (eL) {}
        // Disable Close during long-running BT actions. ExtendScript
        // bodies are synchronous and uninterruptible — pretending to
        // "cancel" mid-action just queues the close behind the
        // unstoppable work. Disabled is honest.
        closeBtn.enabled = !running;
        if (running) {
            summaryLine.text = "⏳  " + (statusText || "Working…");
            try { w.update(); } catch (e) {}
        }
    }

    // Helper: read the post-action /tmp/ai-validate-report.json and
    // mutate the closed-over `report` object in place so repaintList()
    // picks up the new state. Used by both Refresh and Fix after their
    // BT bodies write back to the canonical /tmp file.
    function loadReportFromTmp() {
        try {
            var f = new File("/tmp/ai-validate-report.json");
            if (!f.exists) return;
            f.encoding = "UTF-8";
            f.open("r");
            var raw = f.read();
            f.close();
            var fresh = JSON.parse(raw);
            for (var k in fresh) {
                if (fresh.hasOwnProperty(k)) report[k] = fresh[k];
            }
        } catch (eR) { /* tolerate */ }
    }

    // Refresh and Fix touch doc.layers, doc.activeView etc., which
    // throws "there is no document" when called directly from the
    // palette's onClick context (Illustrator quirk). BridgeTalk-
    // dispatch the work to the persistent BT engine where doc access
    // works. The dispatched body writes the new report state to
    // /tmp/ai-validate-report.json; onResult re-reads it and updates
    // the panel UI.
    function dispatchAction(action, statusText, doneSuffix) {
        setBusy(true, statusText);
        try { logInfo("ui: dispatch (BT)", { action: action }); } catch (eL) {}

        // Resolve the file to re-evalFile in the BT engine.
        // AI_VALIDATE_ENTRY_PATH is captured at parse-time in
        // validate.jsx (or in the bundle's emulated validate.jsx
        // section). Works for both dev (points at the source) and
        // production (points at the bundled .jsx in Illustrator's
        // Scripts folder).
        var entryPath = (typeof AI_VALIDATE_ENTRY_PATH === "string")
            ? AI_VALIDATE_ENTRY_PATH : null;

        var bt = new BridgeTalk();
        bt.target = BridgeTalk.appSpecifier;
        bt.body =
            "AI_VALIDATE_BT_READY = true;\n" +
            "AI_VALIDATE_ACTION = " + JSON.stringify(action) + ";\n" +
            "$.evalFile(new File(" + JSON.stringify(entryPath) + "));\n";
        bt.onResult = function () {
            loadReportFromTmp();
            repaintList();
            if (doneSuffix) summaryLine.text = summaryHeadline(report.summary) + doneSuffix;
            try {
                logInfo("ui: dispatch done", {
                    action: action,
                    remaining: report.findings.length,
                    applied: (report.fixes && report.fixes.applied) ? report.fixes.applied.length : 0,
                    error: report.error
                });
            } catch (eL) {}
            if (report.error) alert("ai-validate " + action + " error:\n\n" + report.error);
            setBusy(false);
        };
        bt.onError = function (resp) {
            alert("ai-validate " + action + " failed\n\n" + (resp && resp.body ? resp.body : String(resp)));
            setBusy(false);
        };
        bt.send();
    }

    refreshBtn.onClick = function () {
        dispatchAction("report", "Scanning the document — please wait", null);
    };

    fixBtn.onClick = function () {
        dispatchAction("fix",
            "Applying fixes — this can take a minute on a large file",
            "  •  Save the document to keep changes.");
    };

    closeBtn.onClick = function () { w.close(); };

    // Initial state: empty list, "Scanning…" status. The auto-refresh
    // below triggers the first walk + check via BridgeTalk so the user
    // sees the panel within ~1s of clicking File > Scripts > ai-validate
    // instead of staring at a blank menu while walk_paths + runChecks
    // run synchronously.
    setBusy(true, "Scanning the document — please wait");

    try { logInfo("ui: about to show panel"); } catch (e) {}
    w.show();

    // Kick off the initial scan. BT-dispatched so the panel paints
    // first; populates when results land.
    dispatchAction("report", "Scanning the document — please wait", null);

    try { logInfo("ui: panel closed"); } catch (e) {}
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
