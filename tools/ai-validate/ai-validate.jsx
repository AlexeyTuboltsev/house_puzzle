//@target illustrator
// ai-validate.jsx — DEV-MODE in-Illustrator entry point.
//
// Install with one of:
//   sudo cp /Users/varya/house_puzzle/tools/ai-validate/ai-validate.jsx \
//       "/Applications/Adobe Illustrator 2022/Presets.localized/en_US/Scripts/ai-validate.jsx"
//   (real-file copy — re-cp this wrapper when ITS contents change;
//    edits to validate.jsx + lib/*.jsx are picked up live since this
//    wrapper just $.evalFile()s the dev source.)
//
// Works from BOTH:
//   - File > Scripts > ai-validate (menu entry)
//   - File > Scripts > Other Script... (Cmd+F12) → pick this file
//
// What this wrapper does:
//   Sets AI_VALIDATE_VERSION = "(dev)" so validate.jsx flips into
//   interactive mode (panel UI), then $.evalFile()s the dev source.
//   The validate.jsx bootstrap then handles BridgeTalk dispatch into
//   the persistent engine (where ScriptUI palette()s survive across
//   the script's natural end).

var AI_VALIDATE_VERSION = "(dev)";
var AI_VALIDATE_MANIFEST_URL = "";

(function () {
    var validateJsxPath = "/Users/varya/house_puzzle/tools/ai-validate/validate.jsx";
    var f = new File(validateJsxPath);
    if (!f.exists) {
        alert("ai-validate (dev): can't find validate.jsx at\n  " + validateJsxPath);
        return;
    }
    $.evalFile(f);
})();
