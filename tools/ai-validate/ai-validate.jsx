// ai-validate.jsx — in-Illustrator entry point.
//
// Drop a copy / symlink of this file into Illustrator's Scripts folder
// (typically `/Applications/Adobe Illustrator 2022/Presets/<locale>/Scripts/`)
// and restart Illustrator. The script then appears under
//   File → Scripts → ai-validate
// and runs against the currently active document.
//
// You can also run it ad-hoc without installing: Illustrator's
//   File → Scripts → Other Script…  (Cmd+F12)
// and pick this file directly.
//
// The shell entry point (run.sh) and this in-Illustrator entry both
// hand control off to the same validate.jsx, so the report shape and
// fix behaviour are identical regardless of how you launched it.

#target illustrator

(function main() {
    if (app.documents.length === 0) {
        alert("ai-validate: open an .ai file first.");
        return;
    }
    var doc = app.activeDocument;
    var hasPath = false;
    try { hasPath = !!doc.fullName; } catch (e) { hasPath = false; }
    if (!hasPath) {
        alert("ai-validate: save the document first so it has a path on disk.");
        return;
    }

    // confirm() returns true on "Yes" / OK. Default to Yes (fix mode)
    // since that's the usual reason to invoke this script; pressing
    // No still gives the artist a read-only report next to the .ai.
    var goFix = confirm(
        "ai-validate\n\n" +
        "Apply auto-fixes?\n\n" +
        "  Yes — FIX mode (modifies the document; review then Save).\n" +
        "  No  — REPORT only (read-only).",
        true /* default Yes */
    );
    var mode = goFix ? "fix" : "report";

    writeText("/tmp/ai-validate-mode.txt", mode);
    writeText("/tmp/ai-validate-target.txt", doc.fullName.fsName);

    // Try the sibling location first (works when running this file
    // straight from the project tree, e.g. via File → Scripts →
    // Other Script…). If that fails, fall back to the absolute
    // project path so an installed copy in Illustrator's Scripts
    // folder still finds the rest of the codebase.
    var here = File($.fileName).parent;
    var sibling = File(here.fsName + "/validate.jsx");
    var fallback = File("/Users/varya/house_puzzle/tools/ai-validate/validate.jsx");
    var validateJsx = sibling.exists ? sibling : fallback;
    if (!validateJsx.exists) {
        alert("ai-validate: can't find validate.jsx.\n\nTried:\n  " +
              sibling.fsName + "\n  " + fallback.fsName);
        return;
    }
    $.evalFile(validateJsx);

    var stem = doc.fullName.fsName.replace(/\.[^.]+$/, "");
    var mdPath = stem + ".report.md";
    alert("ai-validate (" + mode + ") done.\n\nReport:\n" + mdPath);
})();

function writeText(path, body) {
    var f = new File(path);
    f.encoding = "UTF-8";
    f.lineFeed = "Unix";
    f.open("w");
    f.write(body);
    f.close();
}
