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
//
// NB: #target / #targetengine directives are intentionally NOT in this
// file. The dev wrapper (ai-validate.jsx) and the production bundle
// own them, declared once at the very top. Re-declaring them here
// breaks the named-engine load when this file is reached via
// $.evalFile from the wrapper (Illustrator's parser silently aborts).

#include "lib/json2.jsx"
#include "lib/log.jsx"
#include "lib/walk_paths.jsx"
#include "lib/checks.jsx"
#include "lib/fixes.jsx"
#include "lib/render_md.jsx"
#include "lib/update_check.jsx"
#include "lib/ui.jsx"

// Captured at parse time so the panel's BT-dispatch handlers know
// which file to $.evalFile when the user clicks Refresh / Fix.
// In dev: the /Users/varya/.../validate.jsx path. In production
// bundle: the /Applications/.../Scripts/ai-validate.jsx path.
// Either way, re-evaluating this file via BridgeTalk re-runs all the
// helpers in the BT engine where doc access works from palette
// callbacks.
var AI_VALIDATE_ENTRY_PATH = null;
try { AI_VALIDATE_ENTRY_PATH = $.fileName; } catch (eP) {}

// Windows has no /tmp, so the bundled `.jsx` running inside
// Illustrator on Windows can't write the panel-handoff report to a
// hardcoded `/tmp/...` path — `File("/tmp/...")` resolves to
// `C:\tmp\...` which usually doesn't exist (and isn't writable from
// non-admin Illustrator). The panel then gets nothing back and looks
// like it "stops parsing the document." Use the OS temp dir on
// Windows; keep `/tmp/` on macOS / Linux so the dev `run.sh` driver
// (which hardcodes `/tmp/`) still works.
function aiValidateTempDir() {
    try {
        if ($.os.indexOf("Windows") >= 0) {
            return Folder.temp.fsName;
        }
    } catch (e) {}
    return "/tmp";
}

(function main() {
    var REPORT_VERSION = 1;
    var TMP_DIR     = aiValidateTempDir();
    var REPORT_PATH = TMP_DIR + "/ai-validate-report.json";
    var MODE_PATH   = TMP_DIR + "/ai-validate-mode.txt";
    var TARGET_PATH = TMP_DIR + "/ai-validate-target.txt";

    // Bundled builds set AI_VALIDATE_VERSION via packaging/bundle.sh.
    // Its presence is the ONLY signal that we're talking to a human
    // through ScriptUI dialogs vs being driven by run.sh in dev. In
    // bundled mode we ignore the /tmp scaffolding files entirely —
    // that's where stale targets like _NY1.ai used to leak in (a
    // previous run.sh invocation wrote /tmp/ai-validate-target.txt
    // and we'd then app.open() that file even with no doc visible
    // to the artist).
    var INTERACTIVE = (typeof AI_VALIDATE_VERSION === "string");

    // BridgeTalk bootstrap. ScriptUI palette() windows die when their
    // host script's engine context tears down. File > Scripts and
    // Cmd+F12 both launch us in a default ephemeral engine where
    // palettes can't persist. So: if we haven't yet been routed
    // through BridgeTalk's persistent engine, send ourselves to it
    // and exit. The BT body sets AI_VALIDATE_BT_READY so this branch
    // is a no-op on the re-entry.
    //
    // Action-mode BT dispatches (Refresh / Fix from the panel) also
    // set AI_VALIDATE_BT_READY, so the action-mode branch below sees
    // it and skips this re-dispatch.
    //
    // Only applicable in interactive mode (i.e. when AI_VALIDATE_VERSION
    // is set — bundled or dev wrapper). The headless run.sh path
    // doesn't need a persistent engine.
    if (INTERACTIVE && typeof AI_VALIDATE_BT_READY === "undefined") {
        var selfPath = AI_VALIDATE_ENTRY_PATH;
        try { if (!selfPath) selfPath = $.fileName; } catch (eFn) {}
        if (selfPath) {
            // Forward the version + manifest constants through the BT
            // body. In the production bundle these are baked into the
            // bundle text at top, so the BT-eval'd copy already has
            // them — passing them again is a no-op overwrite. In dev,
            // the wrapper sets them in the default engine ONLY; the BT
            // engine is fresh, so without this propagation INTERACTIVE
            // would be false on the BT side and the panel would never
            // open.
            var verLit = (typeof AI_VALIDATE_VERSION === "string")
                ? JSON.stringify(AI_VALIDATE_VERSION) : "undefined";
            var manLit = (typeof AI_VALIDATE_MANIFEST_URL === "string")
                ? JSON.stringify(AI_VALIDATE_MANIFEST_URL) : "undefined";

            var btBoot = new BridgeTalk();
            btBoot.target = BridgeTalk.appSpecifier;
            btBoot.body =
                "AI_VALIDATE_BT_READY = true;\n" +
                "AI_VALIDATE_ACTION = undefined;\n" +
                "AI_VALIDATE_VERSION = " + verLit + ";\n" +
                "AI_VALIDATE_MANIFEST_URL = " + manLit + ";\n" +
                "$.evalFile(new File(" + JSON.stringify(selfPath) + "));\n";
            btBoot.onError = function (resp) {
                alert("ai-validate BT bootstrap failed:\n" +
                      (resp && resp.body ? resp.body : String(resp)));
            };
            btBoot.send();
        } else {
            alert("ai-validate: couldn't determine script path for BridgeTalk re-dispatch.");
        }
        return;
    }

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

    // Action mode: BT-dispatched fix/refresh from the panel's button
    // onClick handlers. Each BT message body $.evalFile()s this very
    // file with AI_VALIDATE_ACTION set ("report" or "fix") to trigger
    // the headless code path below — runs the requested action,
    // writes /tmp/ai-validate-report.json, returns. The panel's
    // onResult callback re-reads the file and updates its UI.
    //
    // Why a flag instead of a separate entry script: BT message
    // bodies appear to run in fresh engine contexts (writeReport et
    // al. defined elsewhere are not visible). Re-evaluating
    // validate.jsx in the BT body brings every helper back into scope
    // and reuses one source-of-truth for the report pipeline.
    if (typeof AI_VALIDATE_ACTION === "string") {
        // Capture the action and IMMEDIATELY clear the global. BT
        // engine state persists across messages, so leaving it set
        // would cause the NEXT validate.jsx eval (e.g. from re-
        // opening the panel via File > Scripts > ai-validate) to
        // re-enter this branch and skip the interactive path. Bug
        // observed in the wild as "I closed the panel, now the
        // script doesn't open again."
        var theAction = AI_VALIDATE_ACTION;
        AI_VALIDATE_ACTION = undefined;

        var REPORT_PATH_ACT = aiValidateTempDir() + "/ai-validate-report.json";
        var actionReport = {
            version: REPORT_VERSION, mode: theAction, file: null,
            findings: [], summary: null, fixes: null, snapshot: null, error: null
        };
        try {
            var actDoc = app.activeDocument;
            if (!actDoc) {
                actionReport.error = "no active document";
            } else {
                actionReport.file = actDoc.fullName ? actDoc.fullName.fsName : actDoc.name;
                runReportPass(actDoc, actionReport);
                if (theAction === "fix") {
                    runFixLoop(actDoc, actionReport);
                }
            }
        } catch (eAct) {
            actionReport.error = String(eAct) + (eAct.line ? " (line " + eAct.line + ")" : "");
        }
        writeReport(REPORT_PATH_ACT, actionReport);
        writeMarkdown(actionReport);
        return;
    }

    logReset();
    var engineName = "?";
    try { engineName = $.engineName; } catch (e) {}
    logInfo("validate.jsx: start", { interactive: INTERACTIVE, engine: engineName });

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

        if (INTERACTIVE) {
            // Hand off to the non-blocking panel WITHOUT pre-walking.
            // The panel paints empty/scanning, then BT-dispatches the
            // initial walk + check itself. This avoids the ~5s "menu
            // → panel" gap on large files where Illustrator looked
            // frozen with no UI feedback. All subsequent walks (refresh)
            // and fixes flow through the same BT-dispatched pipeline.
            showInteractivePanel(doc, report);
            return;
        }

        // Dev / run.sh path is still synchronous: walk + (optionally)
        // fix, write the report and exit.
        runReportPass(doc, report);
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
    // Always drop a temp-dir copy so dev callers (run.sh, CI) have a
    // stable path. Same `aiValidateTempDir()` rule: /tmp on macOS /
    // Linux, %TEMP% on Windows.
    writeText(aiValidateTempDir() + "/ai-validate-report.md", md);
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
