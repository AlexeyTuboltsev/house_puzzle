// validate.jsx — entry point.
//
// Read first: ../CLAUDE.md.
//
// Hard rules (do not violate):
//   - report mode never modifies the document.
//   - fix mode only does deterministic fixes; ambiguous findings stay as
//     warnings.
//   - Vector and raster move together (see lib/raster_move.jsx in later
//     phases).
//   - One JSON report per run at /tmp/ai-validate-report.json.
//   - No alert() in headless mode — it blocks Illustrator.

#target illustrator

#include "lib/json2.jsx"
#include "lib/walk_paths.jsx"
#include "lib/checks.jsx"
#include "lib/render_md.jsx"

(function main() {
    var REPORT_VERSION = 1;
    var REPORT_PATH = "/tmp/ai-validate-report.json";
    var MODE_PATH   = "/tmp/ai-validate-mode.txt";
    var TARGET_PATH = "/tmp/ai-validate-target.txt";

    var report = {
        version: REPORT_VERSION,
        mode: readMode(MODE_PATH),
        file: null,
        findings: [],
        summary: null,
        snapshot: null,
        error: null
    };

    try {
        var target = readTrimmed(TARGET_PATH);
        var doc = pickDocument(target);
        if (!doc) {
            report.error = target
                ? ("could not find or open: " + target)
                : "no document open and no target file specified";
            writeReport(REPORT_PATH, report);
            return;
        }
        report.file = doc.fullName ? doc.fullName.fsName : doc.name;

        report.snapshot = walkPaths(doc);
        report.findings = runChecks(report.snapshot);
        report.summary  = summarize(report.findings);

        if (report.mode === "fix") {
            // Phase 2 will populate this; stays a no-op for now.
            report.error = "fix mode not implemented yet (phase 2)";
        }
    } catch (e) {
        report.error = String(e) + (e.line ? " (line " + e.line + ")" : "");
    }

    writeReport(REPORT_PATH, report);
    writeMarkdown(report);
})();

function writeMarkdown(report) {
    var md = renderMarkdown(report);
    // Always drop a /tmp copy so callers (run.sh, CI) have a stable path.
    writeText("/tmp/ai-validate-report.md", md);
    // Also drop one next to the source .ai so the artist can find it
    // in Finder without leaving the working folder. Filename:
    // "<basename>.report.md".
    if (report.file) {
        var dot = report.file.lastIndexOf(".");
        var stem = (dot > 0) ? report.file.substring(0, dot) : report.file;
        writeText(stem + ".report.md", md);
    }
}

function writeText(path, body) {
    var f = new File(path);
    f.encoding = "UTF-8";
    f.lineFeed = "Unix"; // default on Mac is "\r" (Classic); markdown needs "\n"
    f.open("w");
    f.write(body);
    f.close();
}

function summarize(findings) {
    var bySeverity = { error: 0, warning: 0 };
    var byKind = {};
    for (var i = 0; i < findings.length; i++) {
        var f = findings[i];
        if (bySeverity[f.severity] === undefined) bySeverity[f.severity] = 0;
        bySeverity[f.severity]++;
        byKind[f.kind] = (byKind[f.kind] || 0) + 1;
    }
    return { total: findings.length, by_severity: bySeverity, by_kind: byKind };
}

// Resolve the document the caller asked for. If no target path was
// passed, fall back to the currently active document (legacy behaviour).
function pickDocument(targetPath) {
    if (targetPath) {
        for (var i = 0; i < app.documents.length; i++) {
            var d = app.documents[i];
            try {
                if (d.fullName && d.fullName.fsName === targetPath) {
                    app.activeDocument = d;
                    return d;
                }
            } catch (e) { /* doc may not have a saved path; skip */ }
        }
        var f = new File(targetPath);
        if (f.exists) {
            return app.open(f);
        }
        return null;
    }
    return app.documents.length ? app.activeDocument : null;
}

function readMode(path) {
    var v = readTrimmed(path);
    return (v === "fix") ? "fix" : "report";
}

function readTrimmed(path) {
    try {
        var f = new File(path);
        if (!f.exists) return "";
        f.encoding = "UTF-8";
        f.open("r");
        var raw = f.read();
        f.close();
        return String(raw).replace(/^\s+|\s+$/g, "");
    } catch (e) {
        return "";
    }
}

function writeReport(path, report) {
    writeText(path, JSON.stringify(report, null, 2));
}
