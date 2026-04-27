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
#include "lib/log.jsx"
#include "lib/walk_paths.jsx"
#include "lib/checks.jsx"
#include "lib/fixes.jsx"
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
        fixes: null,
        snapshot: null,
        error: null
    };

    logReset();
    logInfo("validate.jsx: start", { mode: report.mode });

    try {
        var target = readTrimmed(TARGET_PATH);
        logInfo("target read", { target: target });
        var doc = pickDocument(target);
        if (!doc) {
            report.error = target
                ? ("could not find or open: " + target)
                : "no document open and no target file specified";
            logError("no document selected", { error: report.error });
            writeReport(REPORT_PATH, report);
            return;
        }
        report.file = doc.fullName ? doc.fullName.fsName : doc.name;
        logInfo("document selected", { file: report.file });

        logInfo("walk_paths: begin");
        report.snapshot = walkPaths(doc);
        logInfo("walk_paths: end", {
            bricks: report.snapshot.bricks ? report.snapshot.bricks.length : 0,
            top_layers: doc.layers.length
        });

        logInfo("runChecks: begin");
        report.findings = runChecks(report.snapshot);
        report.summary  = summarize(report.findings);
        logInfo("runChecks: end", {
            findings: report.findings.length,
            errors: report.summary.by_severity.error || 0,
            warnings: report.summary.by_severity.warning || 0
        });

        if (report.mode === "fix") {
            // Convergence loop: a single pass of fixes can create new
            // findings (e.g. snap_drift_cluster shortens an edge below
            // 1 pymu, which becomes a new sub_pymu_edge). Re-walk and
            // re-fix until no more fixes are applied, capped at 5
            // iterations as a safety against pathological cycles.
            var iterations = [];
            var MAX_ITER = 5;
            for (var iter = 0; iter < MAX_ITER; iter++) {
                logInfo("fix iter: begin", { iter: iter });
                var pass = runFixes(doc, report.snapshot, report.findings);
                iterations.push(pass);
                report.snapshot = walkPaths(doc);
                report.findings = runChecks(report.snapshot);
                logInfo("fix iter: end", {
                    iter: iter,
                    applied: pass.applied.length,
                    skipped: pass.skipped.length,
                    error: pass.error,
                    remaining_findings: report.findings.length
                });
                if (!pass.applied || pass.applied.length === 0) break;
            }
            report.summary = summarize(report.findings);
            report.fixes   = collapseIterations(iterations);
            logInfo("fix mode: converged", {
                iterations: report.fixes.iterations,
                total_applied: report.fixes.applied.length,
                remaining: report.findings.length
            });
        }
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        report.error = msg;
        logError("validate.jsx: top-level exception", { error: msg });
    }
    logInfo("validate.jsx: end", { error: report.error });

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

// Combine the per-iteration fix results into a single block. We keep
// every applied fix (so the artist sees the full chain of edits) but
// only the LAST iteration's skipped list (earlier skips often resolve
// in a later pass once a prerequisite fix has been applied).
function collapseIterations(iters) {
    var applied = [];
    var errors = [];
    var perIterCounts = [];
    for (var i = 0; i < iters.length; i++) {
        if (iters[i].applied) {
            for (var a = 0; a < iters[i].applied.length; a++) {
                applied.push(iters[i].applied[a]);
            }
            perIterCounts.push(iters[i].applied.length);
        } else {
            perIterCounts.push(0);
        }
        if (iters[i].error) errors.push("iter " + i + ": " + iters[i].error);
    }
    // Earlier iterations' skipped lists often resolve in later passes
    // once a prerequisite fix has run; the last iteration is the only
    // one whose skipped list reflects the converged state.
    var lastSkipped = iters.length > 0 ? (iters[iters.length - 1].skipped || []) : [];
    return {
        applied: applied,
        skipped: lastSkipped,
        iterations: iters.length,
        per_iteration_applied: perIterCounts,
        error: errors.length ? errors.join("; ") : null
    };
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
