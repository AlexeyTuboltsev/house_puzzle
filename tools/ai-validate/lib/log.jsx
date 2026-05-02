// lib/log.jsx — append-mode, flush-per-event debug log.
//
// Adobe's JS runtime crashes often (locked layers, deep recursion,
// memory pressure, etc.). Every log event is opened-written-closed
// individually so that if Illustrator dies mid-script, the log on
// disk shows exactly the last operation attempted.
//
// API:
//   logReset()                — wipe the log file at the start of a run
//   logInfo(msg, data?)       — milestones the artist might care about
//   logWarn(msg, data?)       — recoverable anomalies
//   logError(msg, data?)      — caught exceptions, abort markers
//   logDebug(msg, data?)      — high-volume per-fix tracing
//
// `data` is optional; if present it's JSON.stringify'd onto the line.

var LOG_PATH = "/tmp/ai-validate-debug.log";
var __LOG_SEQ = 0;
var __LOG_START = null;

function logReset() {
    try {
        var f = new File(LOG_PATH);
        if (f.exists) f.remove();
    } catch (e) { /* if we can't reset, append instead */ }
    __LOG_SEQ = 0;
    __LOG_START = new Date();
}

function logEvent(level, message, data) {
    if (__LOG_START === null) __LOG_START = new Date();
    __LOG_SEQ++;
    var t = ((new Date() - __LOG_START) / 1000).toFixed(3);
    var line = "[" + padLeft(t, 7) + "s #" + padLeft(__LOG_SEQ, 4) + " " + level + "] " + message;
    if (data !== undefined && data !== null) {
        try { line += " " + JSON.stringify(data); }
        catch (e) { line += " (data unstringifiable: " + String(data) + ")"; }
    }
    try {
        var f = new File(LOG_PATH);
        f.encoding = "UTF-8";
        f.lineFeed = "Unix";
        f.open("a");
        f.writeln(line);
        f.close();
    } catch (e) {
        // Logging is best-effort. We can't alert() in headless mode.
    }
}

function logInfo (m, d) { logEvent("INFO ", m, d); }
function logWarn (m, d) { logEvent("WARN ", m, d); }
function logError(m, d) { logEvent("ERROR", m, d); }
function logDebug(m, d) { logEvent("DEBUG", m, d); }

function padLeft(s, n) {
    s = String(s);
    while (s.length < n) s = " " + s;
    return s;
}
