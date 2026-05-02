// lib/update_check.jsx — non-blocking script-version check.
//
// Why fire-and-forget?
//
//   ExtendScript's `Socket` object is plain TCP only — it can't do
//   TLS, so it can't talk to raw.githubusercontent.com (HTTPS-only).
//   We work around this by shelling out: a tiny helper script (VBScript
//   on Windows, bash+curl on macOS) does the HTTPS GET in the
//   background, writes the manifest to a cache file under
//   Folder.userData, and exits. The .jsx itself reads the PREVIOUS
//   run's cache and shows a warning if the cached manifest's version
//   beats AI_VALIDATE_VERSION (the constant baked into the bundle).
//
//   First run after install: cache empty, no warning. Helper kicks
//   off and populates cache.
//   Run 2+: cache exists, warning shows if newer.
//
//   The throttling is implicit (one refresh per script invocation)
//   plus an explicit floor: refreshes are skipped if the cache file's
//   modtime is < UPDATE_CHECK_MIN_AGE_HOURS old. Keeps GitHub happy.
//
// Cache file: <Folder.userData>/ai-validate/script-version-cache.json
//   - macOS: ~/Library/Application Support/ai-validate/...
//   - Windows: %APPDATA%\ai-validate\...
//
// Manifest schema (script-version.json on the repo):
//   {
//     "version": "1.0.0",
//     "released": "YYYY-MM-DD",
//     "release_notes_url": "...",
//     "platforms": { "windows": "url.exe", "macos": "url.pkg" }
//   }

var UPDATE_CHECK_MIN_AGE_HOURS = 12;

function maybeWarnAboutUpdate() {
    try {
        var cacheFile = updateCheckCacheFile();
        var current = (typeof AI_VALIDATE_VERSION === "string") ? AI_VALIDATE_VERSION : null;
        var manifestUrl = (typeof AI_VALIDATE_MANIFEST_URL === "string") ? AI_VALIDATE_MANIFEST_URL : null;
        if (!current || !manifestUrl) return; // dev mode (no bundle constants)

        // Read the previous run's cache and show warning if newer.
        var cached = readManifestCache(cacheFile);
        if (cached && cached.version && versionLessThan(current, cached.version)) {
            showUpdateAlert(current, cached);
        }

        // Kick off a background refresh if the cache is stale.
        if (cacheIsStale(cacheFile, UPDATE_CHECK_MIN_AGE_HOURS)) {
            refreshManifestInBackground(manifestUrl, cacheFile);
        }
    } catch (e) {
        try { logWarn("update_check failed", { error: String(e) }); } catch (eLog) {}
    }
}

function updateCheckCacheFile() {
    var dir = new Folder(Folder.userData.fsName + "/ai-validate");
    if (!dir.exists) dir.create();
    return new File(dir.fsName + "/script-version-cache.json");
}

function readManifestCache(file) {
    if (!file.exists) return null;
    try {
        file.encoding = "UTF-8";
        file.open("r");
        var raw = file.read();
        file.close();
        return JSON.parse(raw);
    } catch (e) {
        return null;
    }
}

function cacheIsStale(file, minAgeHours) {
    if (!file.exists) return true;
    try {
        var ageMs = (new Date()).getTime() - file.modified.getTime();
        return ageMs > (minAgeHours * 3600 * 1000);
    } catch (e) {
        return true;
    }
}

function versionLessThan(a, b) {
    var pa = parseSemver(a);
    var pb = parseSemver(b);
    for (var i = 0; i < 3; i++) {
        if (pa[i] < pb[i]) return true;
        if (pa[i] > pb[i]) return false;
    }
    return false;
}

function parseSemver(s) {
    var parts = String(s).split(".");
    return [parseInt(parts[0] || 0, 10) || 0,
            parseInt(parts[1] || 0, 10) || 0,
            parseInt(parts[2] || 0, 10) || 0];
}

function showUpdateAlert(current, manifest) {
    var url = "";
    if (manifest.platforms) {
        if ($.os.indexOf("Windows") >= 0 && manifest.platforms.windows) url = manifest.platforms.windows;
        else if (manifest.platforms.macos) url = manifest.platforms.macos;
    }
    if (!url) url = manifest.release_notes_url || "";
    var msg = "ai-validate update available\n\n" +
              "Installed: " + current + "\n" +
              "Available: " + manifest.version + "\n\n";
    if (url) msg += "Download:\n" + url;
    alert(msg);
}

// Write a tiny platform-specific helper to the temp dir and execute
// it. The helper does the HTTPS GET and writes manifest JSON to
// `cacheFile`. We don't wait for it — the result becomes visible on
// the NEXT run.
function refreshManifestInBackground(url, cacheFile) {
    var helperDir = Folder.temp;
    var cachePath = cacheFile.fsName;

    if ($.os.indexOf("Windows") >= 0) {
        var vbs = new File(helperDir.fsName + "/ai-validate-update.vbs");
        vbs.encoding = "UTF-8";
        vbs.open("w");
        vbs.write(
            "On Error Resume Next\r\n" +
            "Set http = CreateObject(\"MSXML2.ServerXMLHTTP.6.0\")\r\n" +
            "http.SetTimeouts 3000, 3000, 5000, 5000\r\n" +
            "http.Open \"GET\", \"" + url + "\", False\r\n" +
            "http.Send\r\n" +
            "If http.Status = 200 Then\r\n" +
            "  Set fso = CreateObject(\"Scripting.FileSystemObject\")\r\n" +
            "  Set f = fso.CreateTextFile(\"" + cachePath.replace(/\\/g, "\\\\") + "\", True, True)\r\n" +
            "  f.Write http.responseText\r\n" +
            "  f.Close\r\n" +
            "End If\r\n"
        );
        vbs.close();
        // wscript.exe runs .vbs silently. File.execute uses the file's
        // default handler — for .vbs that's wscript by default.
        vbs.execute();
    } else {
        // macOS: skip the background refresh. There is no way to spawn
        // a silent shell command from Illustrator ExtendScript — every
        // route either pops a window (File.execute on .sh opens
        // Terminal, on .scpt opens Script Editor as the user just
        // observed in the wild) or doesn't actually run (no $.system /
        // system.callSystem in Illustrator's ES engine).
        //
        // Until a proper macOS-side refresher ships (likely a launchd
        // plist installed by the .pkg's postinstall, doing curl once
        // a day in the user's session), the cache is populated only at
        // .pkg install time — see packaging/macos/postinstall. That's
        // good enough for the common case (artist re-installs every
        // few releases) and avoids the user-visible popup.
        return;
    }
}

