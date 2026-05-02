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
//   - No alert() in headless mode — it blocks Illustrator. Interactive
//     mode (bundled .jsx) does use ScriptUI dialogs; that's fine
//     because the artist just clicked File > Scripts > ai-validate.

#target illustrator

#include "lib/json2.jsx"
#include "lib/log.jsx"
#include "lib/walk_paths.jsx"
#include "lib/checks.jsx"
#include "lib/fixes.jsx"
#include "lib/render_md.jsx"
#include "lib/update_check.jsx"
#include "lib/ui.jsx"

(function main() {
    var REPORT_VERSION = 1;
    var REPORT_PATH = "/tmp/ai-validate-report.json";
    var MODE_PATH   = "/tmp/ai-validate-mode.txt";
    var TARGET_PATH = "/tmp/ai-validate-target.txt";

    // Bundled builds set AI_VALIDATE_VERSION via packaging/bundle.sh.
    // Its presence is the ONLY signal that we're talking to a human
    // through ScriptUI dialogs vs being driven by run.sh in dev. In
    // bundled mode we ignore the /tmp scaffolding files entirely —
    // that's where stale targets like _NY1.ai used to leak in (a
    // previous run.sh invocation wrote /tmp/ai-validate-target.txt
    // and we'd then app.open() that file even with no doc visible
    // to the artist).
    var INTERACTIVE = (typeof AI_VALIDATE_VERSION === "string");

    var report = {
        version: REPORT_VERSION,
        mode: "report",
        file: null,
        findings: [],
        summary: null,
        fixes: null,
        snapshot: null,
        error: null
    };

    logReset();
    logInfo("validate.jsx: start", { interactive: INTERACTIVE });

    // Non-blocking: alerts only if a previous run cached a newer
    // manifest. Refresh of the cache itself is handled out-of-band on
    // macOS (launchd plist) and silently in-process on Windows
    // (VBScript via wscript). Dev runs without the bundle constants
    // skip the check.
    maybeWarnAboutUpdate();

    var doc = null;

    try {
        if (INTERACTIVE) {
            if (app.documents.length === 0) {
                showNoDocumentAlert();
                return;
            }
            doc = app.activeDocument;
            report.mode = "report"; // start with report; preview decides whether to fix
        } else {
            // Dev / run.sh path — read /tmp scaffolding so the shell
            // driver can target a specific file.
            report.mode = readMode(MODE_PATH);
            var target = readTrimmed(TARGET_PATH);
            doc = pickDocument(target);
            if (!doc) {
                report.error = target
                    ? ("could not find or open: " + target)
                    : "no document open and no target file specified";
                logError("no document selected", { error: report.error });
                writeReport(REPORT_PATH, report);
                return;
            }
        }

        report.file = doc.fullName ? doc.fullName.fsName : doc.name;
        logInfo("document selected", { file: report.file });

        // Always start with a read-only walk + checks. In interactive
        // mode the panel takes over from here; in dev mode the result
        // is either written straight out (report mode) or used as the
        // baseline before the fix loop's unlock pre-pass overwrites it
        // (fix mode).
        runReportPass(doc, report);

        if (INTERACTIVE) {
            // Hand off to the non-blocking panel. It stays open while
            // the artist navigates the document and fixes things by
            // hand or via auto-fix; the script blocks here until the
            // user closes the panel.
            showInteractivePanel(doc, report,
                function () {
                    // refresh callback — re-walk + re-check.
                    runReportPass(doc, report);
                    writeReport(REPORT_PATH, report);
                    writeMarkdown(report);
                },
                function () {
                    // fix callback — run convergence loop, persist the
                    // post-fix report. report.mode flips to "fix"
                    // permanently once auto-fix runs at least once.
                    report.mode = "fix";
                    runFixLoop(doc, report);
                    writeReport(REPORT_PATH, report);
                    writeMarkdown(report);
                });
            return;
        }

        if (report.mode === "fix") {
            runFixLoop(doc, report);
        }
    } catch (e) {
        var msg = String(e) + (e.line ? " (line " + e.line + ")" : "");
        report.error = msg;
        logError("validate.jsx: top-level exception", { error: msg });
    }
    logInfo("validate.jsx: end", { error: report.error });

    writeReport(REPORT_PATH, report);
    writeMarkdown(report);

    if (INTERACTIVE && report.error) {
        // Interactive catastrophic-failure path (e.g. exception
        // before the panel opened). Surface to the artist directly.
        alert("ai-validate: " + report.error);
    }
})();

// Walk + check the document without mutating anything. Populates
// report.snapshot, report.findings, report.summary.
function runReportPass(doc, report) {
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
}

// Pre-unlock + iterative fix loop. Mutates the document. Populates
// report.fixes, report.unlocked, and refreshes report.snapshot /
// report.findings / report.summary to the post-fix state.
function runFixLoop(doc, report) {
    var INTERACTIVE = (typeof AI_VALIDATE_VERSION === "string");
    var progress = INTERACTIVE ? openProgressPalette(8) : null;

    try {
        // Pre-pass: unlock every locked layer AND every locked
        // pageItem AND make hidden layers visible — Illustrator's
        // pathItem.editable is false if any of those are set. Per
        // artist instruction, we do NOT re-lock / re-hide afterwards.
        // Recorded in report.unlocked so the .md surfaces what changed.
        report.unlocked = unlockAllLayers(doc);
        logInfo("fix mode: pre-unlock", {
            layers: report.unlocked.layers.length,
            page_items: report.unlocked.page_items,
            hidden_shown: report.unlocked.hidden_layers_shown.length
        });
        // Re-walk + re-check after the unlock so subsequent findings
        // reflect the now-editable layers.
        report.snapshot = walkPaths(doc);
        report.findings = runChecks(report.snapshot);

        // Convergence loop: a single pass of fixes can create new
        // findings (e.g. snap_drift_cluster shortens an edge below
        // 1 pymu, which becomes a new sub_pymu_edge). Re-walk and
        // re-fix until no more fixes are applied, capped at MAX_ITER.
        var iterations = [];
        var MAX_ITER = 8;
        // Cross-iteration blacklist — once a fix has provably failed
        // (locked layer, non-editable path, ...), don't retry it.
        var failedFixes = {};
        for (var iter = 0; iter < MAX_ITER; iter++) {
            updateProgressPalette(progress, iter,
                "Fix pass " + (iter + 1) + " of up to " + MAX_ITER + "…");
            logInfo("fix iter: begin", { iter: iter, blacklisted: countKeys(failedFixes) });
            var pass = runFixes(doc, report.snapshot, report.findings, failedFixes);
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
    } finally {
        closeProgressPalette(progress);
    }
}

function writeMarkdown(report) {
    var md = renderMarkdown(report);
    // Always drop a /tmp copy so dev callers (run.sh, CI) have a stable path.
    writeText("/tmp/ai-validate-report.md", md);
    // Drop one next to the source .ai so the artist can find it
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

// Resolve the document the caller asked for (dev / run.sh path only).
// If no target path was passed, fall back to the currently active
// document.
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
