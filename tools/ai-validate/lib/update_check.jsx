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

// Returns the cached manifest if a newer version is available, else
// null. Side-effect: kicks off a background refresh if the cache is
// stale, so the next session sees a current manifest. Used by the
// panel to render an in-UI banner instead of the old modal alert.
function getAvailableUpdateManifest() {
    try {
        var cacheFile = updateCheckCacheFile();
        var current = (typeof AI_VALIDATE_VERSION === "string") ? AI_VALIDATE_VERSION : null;
        var manifestUrl = (typeof AI_VALIDATE_MANIFEST_URL === "string") ? AI_VALIDATE_MANIFEST_URL : null;
        if (!current || !manifestUrl) return null; // dev mode
        // Dev sentinel "(dev)" must never participate.
        if (!/^\d+\.\d+\.\d+$/.test(current)) return null;

        var cached = readManifestCache(cacheFile);

        if (cacheIsStale(cacheFile, UPDATE_CHECK_MIN_AGE_HOURS)) {
            refreshManifestInBackground(manifestUrl, cacheFile);
        }

        if (cached && cached.version && versionLessThan(current, cached.version)) {
            return cached;
        }
        return null;
    } catch (e) {
        try { logWarn("update_check failed", { error: String(e) }); } catch (eLog) {}
        return null;
    }
}

// Legacy entry — keeps any direct callers working. The panel now uses
// getAvailableUpdateManifest() and renders inline instead of alerting.
function maybeWarnAboutUpdate() {
    var m = getAvailableUpdateManifest();
    if (m) {
        try { showUpdateAlert(AI_VALIDATE_VERSION, m); } catch (e) {}
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
        // macOS: the script itself doesn't refresh — refresh is handled
        // out-of-band by a per-user launchd agent installed by the
        // .pkg's postinstall (com.alexeytuboltsev.ai-validate-updater,
        // fires once per 24 h via StartInterval). We can't do it from
        // here because Illustrator's ExtendScript engine has no silent
        // shell-out: File.execute on .sh opens Terminal, on .scpt
        // opens Script Editor (artist-reported on sv0.1.1), and
        // there's no $.system / system.callSystem to lean on.
        //
        // The cache file lives at the same Folder.userData path the
        // launchd refresh.sh writes to, so the read above transparently
        // sees whatever the agent fetched last.
        return;
    }
}

