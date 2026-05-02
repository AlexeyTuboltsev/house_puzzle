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

// Returns true if user clicked "Fix what's fixable", false on Cancel
// or if there's nothing to fix and the user dismissed the dialog.
function showPreviewDialog(report) {
    var basename = basenameOf(report.file);
    var s = report.summary || { total: 0, by_severity: {} };
    var nErr  = s.by_severity.error   || 0;
    var nWarn = s.by_severity.warning || 0;

    var w = new Window("dialog", "ai-validate " + aiValidateVersionLabel());
    w.alignChildren = "fill";
    w.preferredSize.width = 720;
    w.spacing = 8;
    w.margins = 16;

    var fileLine = w.add("statictext", undefined, "File: " + basename);
    fileLine.graphics.font = ScriptUI.newFont(fileLine.graphics.font.name, "BOLD",
                                              fileLine.graphics.font.size);

    var summaryText;
    if (s.total === 0) {
        summaryText = "No issues found — the file looks clean.";
    } else {
        summaryText = "Found " + s.total + " issue(s) — " +
                      nErr + " error(s), " + nWarn + " warning(s).";
    }
    w.add("statictext", undefined, summaryText);

    if (s.total > 0) {
        var lb = w.add("listbox", undefined, [], {
            multiselect: false,
            numberOfColumns: 3,
            showHeaders: true,
            columnTitles: ["Severity", "Issue", "Where"]
        });
        lb.preferredSize = [700, 320];
        fillFindingsListbox(lb, report.findings);

        var note = w.add("statictext", undefined,
            "“Fix what’s fixable” applies deterministic auto-fixes " +
            "(snap drift, close paths, delete tiny stray geometry, ...). Issues that " +
            "need manual review (overlaps, bezier loops, ...) will remain in the " +
            "report.",
            { multiline: true });
        note.preferredSize.width = 700;
    }

    var btns = w.add("group");
    btns.alignment = "right";
    var cancelBtn = btns.add("button", undefined, "Cancel", { name: "cancel" });
    var fixBtn    = btns.add("button", undefined, "Fix what’s fixable", { name: "ok" });
    fixBtn.enabled = (s.total > 0);
    if (s.total === 0) fixBtn.text = "OK";

    var result = w.show();
    return result === 1; // OK button → fix
}

function showSummaryDialog(report) {
    var basename = basenameOf(report.file);
    var fixes = report.fixes || { applied: [], skipped: [], iterations: 0 };
    var nApplied   = fixes.applied  ? fixes.applied.length  : 0;
    var nRemaining = report.findings ? report.findings.length : 0;

    var w = new Window("dialog", "ai-validate " + aiValidateVersionLabel() + " — done");
    w.alignChildren = "fill";
    w.preferredSize.width = 720;
    w.spacing = 8;
    w.margins = 16;

    var fileLine = w.add("statictext", undefined, "File: " + basename);
    fileLine.graphics.font = ScriptUI.newFont(fileLine.graphics.font.name, "BOLD",
                                              fileLine.graphics.font.size);

    var headline = "Applied " + nApplied + " fix(es). ";
    if (nRemaining === 0) {
        headline += "No issues remain.";
    } else {
        headline += nRemaining + " issue(s) remain — manual review needed.";
    }
    w.add("statictext", undefined, headline);

    if (report.error) {
        var errLine = w.add("statictext", undefined, "⚠ " + report.error,
                            { multiline: true });
        errLine.preferredSize.width = 700;
    }

    if (nRemaining > 0) {
        var lb = w.add("listbox", undefined, [], {
            multiselect: false,
            numberOfColumns: 3,
            showHeaders: true,
            columnTitles: ["Severity", "Issue", "Where"]
        });
        lb.preferredSize = [700, 280];
        fillFindingsListbox(lb, report.findings);
    }

    if (report.file) {
        var stem = report.file.replace(/\.[^.]+$/, "");
        var mdPath = stem + ".report.md";
        w.add("statictext", undefined, "Detailed report saved to:");
        var pathTxt = w.add("edittext", undefined, mdPath, { readonly: true });
        pathTxt.preferredSize.width = 700;
        // The artist needs to Save the document for vector edits to
        // persist on disk; remind them in the dialog.
        w.add("statictext", undefined,
            "Don’t forget to Save the document — auto-fixes are still in-memory until you do.",
            { multiline: true });
    }

    var btns = w.add("group");
    btns.alignment = "right";
    btns.add("button", undefined, "OK", { name: "ok" });
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
