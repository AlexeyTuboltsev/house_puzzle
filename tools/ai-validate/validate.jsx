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

#include "lib/walk_paths.jsx"

(function main() {
    var REPORT_VERSION = 0;
    var REPORT_PATH = "/tmp/ai-validate-report.json";
    var MODE_PATH   = "/tmp/ai-validate-mode.txt";

    var report = {
        version: REPORT_VERSION,
        mode: readMode(MODE_PATH),
        file: null,
        findings: [],
        snapshot: null,
        error: null
    };

    try {
        if (!app.documents.length) {
            report.error = "no document open";
            writeReport(REPORT_PATH, report);
            return;
        }
        var doc = app.activeDocument;
        report.file = doc.fullName ? doc.fullName.fsName : doc.name;

        // Phase 0: dump the structure so callers can see what we're
        // working with. Phase 1+ adds findings to report.findings.
        report.snapshot = walkPaths(doc);

        if (report.mode === "fix") {
            // Phase 2 will populate this; stays a no-op for now.
            report.error = "fix mode not implemented yet (phase 2)";
        }
    } catch (e) {
        report.error = String(e) + (e.line ? " (line " + e.line + ")" : "");
    }

    writeReport(REPORT_PATH, report);
})();

function readMode(path) {
    try {
        var f = new File(path);
        if (!f.exists) return "report";
        f.encoding = "UTF-8";
        f.open("r");
        var raw = f.read();
        f.close();
        var v = String(raw).replace(/[\r\n\s]+/g, "");
        return (v === "fix") ? "fix" : "report";
    } catch (e) {
        return "report";
    }
}

function writeReport(path, report) {
    var f = new File(path);
    f.encoding = "UTF-8";
    f.open("w");
    f.write(JSON.stringify(report, null, 2));
    f.close();
}
