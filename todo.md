## Open

### ~~AI unscaled vector data as source of truth~~
~~Refactor to keep AI native coords (pymu units) and bezier curves as the
single source of truth for every vector op.~~ Done — vector pipeline
operates on AI-native bezier paths via `bezier_merge`/`build_adjacency_bezier`;
canvas scaling happens only at raster-output time.

### ~~Piece 1px gaps/bleeding — rasterization pipeline redesign~~
~~Polygon edges didn't align with pixel boundaries because we rasterized
the full page at low DPI then masked.~~ Obsolete — current pipeline renders
per-brick OCG layers via MuPDF at the right DPI/clip rect; no more cross-brick
bleeding or 1px gaps.

### ~~macOS double-click binary (PR #44)~~
~~Binary name with dots breaks Finder double-click.~~
Resolved: Tauri app bundle handles this natively.

## Features

### Programmatic export API
Full /api/export without browser — server-side outline path generation.

### Extensive logging + remote error reporting
Add structured logging throughout the pipeline (parse, render, merge,
export). Today the code uses `eprintln!` ad-hoc, which works in dev
but vanishes in distributed builds — when a user reports a bug we
have nothing to look at.

**Local logging — `tauri-plugin-log`.**
Cross-stack structured logging that writes to a rotating file under
the OS app-data dir AND streams to the WebView console + stdout.
Both the Rust side and the Elm side log through the same sink, so
"piece dropped, wave 4 -> wave 2" from Elm and "compose_clipped:
canvas=450x820 offset=(-24,0)" from Rust end up in one chronologically
ordered file. Configurable per-target log levels, redacts paths to
`~` by default. Once integrated, an "Export logs" / "Copy logs"
button is trivial — opens the log file in the OS file manager or
copies the last N lines to clipboard.

**Remote error reporting — Sentry-style.**
For client-reported bugs we want the log + a stack trace + redacted
system info (OS, app version, AI file metadata — never the file
itself) sent to a remote endpoint we can grep. Options:

- **Sentry** — has Rust SDK (`sentry`) and JS SDK. Free tier covers
  5k errors/month, enough for a small desktop tool. Plugs into
  `tauri-plugin-log` as an additional sink for `error!` and above.
- **GlitchTip** — open-source, Sentry-compatible API. Self-host or
  use their hosted free tier. Same SDKs as Sentry work against it.
- **DIY webhook** — `tauri-plugin-log` + a small Rust command that
  POSTs `error!` lines to a private endpoint (Cloudflare Worker,
  GitHub Issue API with a bot, etc.). Cheapest, lowest fidelity.

Privacy: must be opt-in (toggle in settings, default off in EU
distributions), and payload must be transparent — show the user
what we'd send before they confirm. AI file content is sensitive;
ship metadata only.

**User-facing "Report bug" flow.**
Button somewhere accessible (Help menu, About panel, error toast).
Bundles last N log lines + redacted system info into a single
payload. Two mode options:

1. Pre-fill a GitHub issue with the log inlined (opens the user's
   browser via `tauri-plugin-shell::shell.open`). User reviews,
   submits.
2. Direct POST to the configured remote sink, with a confirmation
   dialog showing the payload first.

### Update checker — doesn't fire (confirmed broken v0.4.0 → v0.4.1)
tauri-plugin-updater integrated (PR #56), `check_for_updates` command
wired, but the banner never appears even when a newer release exists.

**Three root causes:**

1. **No `latest.json` manifest.** The updater endpoint
   `https://github.com/AlexeyTuboltsev/house_puzzle/releases/latest/download/latest.json`
   returns HTTP 404. Tauri v2 updater needs this JSON to compare versions
   and locate the per-platform update bundle.

2. **No signing set up.** `tauri.conf.json` has `"pubkey": ""`; CI
   (`build-tauri.yml`) has no `TAURI_SIGNING_PRIVATE_KEY` /
   `TAURI_SIGNING_PRIVATE_KEY_PASSWORD` env vars passed to `tauri-action`,
   so the bundler never emits `.sig` files. Tauri v2 updater rejects
   unsigned updates by design.

3. **Platform-prefixed artifact renames.** Release assets end up with names
   like `linux-House.Puzzle_0.4.1_amd64.AppImage` (done in a later CI step).
   Even if a `latest.json` existed, the URLs baked into it by `tauri-action`
   would point at the original names and 404 on download.

**To fix end-to-end:**

1. Generate a Tauri signing keypair: `cargo tauri signer generate`.
2. Paste the public key into `tauri.conf.json` → `plugins.updater.pubkey`.
3. Add the private key + password to repo secrets:
   `TAURI_SIGNING_PRIVATE_KEY`, `TAURI_SIGNING_PRIVATE_KEY_PASSWORD`.
4. Wire both env vars into the `tauri-apps/tauri-action` step so the
   bundler signs each artifact and produces `latest.json`.
5. Either stop renaming artifacts with the platform prefix, or regenerate
   `latest.json` after renames so its URLs match the uploaded filenames.

### Extract test harness behind feature flag
The `--test-mode` file watcher and JS eval for clicks live in production
`main.rs`. Move behind `#[cfg(feature = "e2e-test")]` so they're only
compiled in test builds. Also move `save_screenshot` command.

### Evaluate tauri-webdriver for e2e testing
Replace our custom test harness with proper WebDriver-based testing:
- `tauri-plugin-webdriver-automation` (danielraffel) — JS bridge plugin
  for macOS WKWebView, speaks W3C WebDriver. Solves the context isolation
  issue we hit. https://github.com/danielraffel/tauri-webdriver
- `tauri-plugin-webdriver` (Choochmeque) — embedded WebDriver server,
  cross-platform. The one we tried but had issues with.
- `tauri-plugin-screenshots` — Tauri v2 plugin for window/monitor
  screenshots. Could replace our WKWebView.takeSnapshot code.
- CrabNebula WebDriver — commercial hosted service with macOS support.

### Piece editor adjacency check
When combining pieces in the editor, verify they are adjacent before
allowing the merge. Currently not enforced.

### ~~OS file picker — remember last location~~
~~The native open dialog should reopen at the last directory the user
picked from.~~ Done — picker persists last directory in app data.

### ~~Waves — "Last wave" button~~
~~Create a new wave and assign every currently unassigned piece.~~
Done — "Last wave" button next to "New wave".

### ~~"Big wave" needs a scrollbar~~
Done — horizontal scrollbar on the bottom tray is now 12px and
visually prominent.

### ~~Stronger selected-piece highlight (canvas + strips)~~
Done — selected piece gets a glowing yellow stroke + bright fill on
the canvas and a glowing border on every matching strip thumb.

### ~~Selected piece auto-scrolls into view in every strip~~
Done — `scrollPieceIntoView` port calls `el.scrollIntoView` on every
`[data-piece-id]` match, including the canvas overlay.

### ~~Wave number badge on each wave~~
Done — 1-based ordinal badge in the wave row header.

### ~~Groups + waves: "Show only blueprint" checkbox~~
Done — per-group and per-wave "BP" checkbox swaps thumbnails to
`piece.outlineUrl`.

### ~~Numeric input next to "Pieces" and "Min border" sliders~~
Done — paired number inputs share the slider's handler/value.

### ~~Parse cache — cache busting, cleanup, versioning~~
~~Cache under `<temp_dir>/house_puzzle_parse_cache/` saved ~6s on
re-opens but needed cleanup, versioning, and a clear-button.~~ Decided
2026-05-07 to remove the cache entirely — re-opening unchanged files
isn't a hot workflow path, and the hygiene cost + risk of stale
matches (mtime edge cases, `cp -p`) outweighed the savings. Removal
also closes the "versioning is fragile" todo below.

### Test coverage — unit + regression suite
The parser, merge, and render pipelines have ~3 ad-hoc smoke tests and a
just-added regression for NY8 'Layer 320'. We need real coverage so
regressions stop being shipped to release. Two tracks:

**Unit tests** (per-function, no fixtures):
- `parse_path_lines` / `parse_path_lines_bezier`: open vs closed
  sub-paths, multiple sub-paths, cubic-bezier tessellation, malformed
  input. The auto-close behaviour now has two unit tests (open +
  explicitly closed) — same pattern should cover the remaining edge
  cases.
- `compute_pdf_offset`: offset detection on synthetic pixmaps with
  known opaque-pixel positions.
- `compose_ocg_canvas`: output dimensions + overlay position for
  various clip / offset combinations.
- `bezier_merge::merge_piece_bezier`: pairs and triples of bricks
  with known shared edges, no-overlap pairs, contained pairs.
- `find_covered_bricks`: detect a fully-covered brick, leave a
  protected vector brick alone, ignore small bricks under the size
  floor.
- `parse_cache_key`: same file → same key; different mtime / size /
  canvas_height → different keys; `PARSE_CACHE_VERSION` bump
  invalidates.

**Regression tests** (per-house, fixture-based):
- For each `_NY*.ai` fixture, assert known-good brick count,
  layer-name presence (`Layer 320` in NY8 — already done),
  metadata.warnings fingerprint, and a stable hash of the full
  bricks Vec serialized via bincode.
- Run a deterministic merge with `seed=42, target=120, deterministic_ids=true`
  and assert the produced piece count + a stable hash of the
  per-piece brick assignment.
- Run the full export pipeline against a fixture and diff
  `house_data.json` against a checked-in golden file. Fail the test
  if any field shifts unexpectedly. Refresh the golden via a
  flagged subcommand (`cargo run -p hp-testbed --bin hp-golden`).

CI:
- Wire `cargo test --workspace` into the existing CI workflow, with
  the AI fixtures present (the `in/` dir is committed).
- Make the regression suite part of `ultrareview` so PR reviewers
  see the diffs against goldens before merge.

### ~~Parse cache — versioning is fragile~~
Closed by the removal above (2026-05-07). No cache, no versioning
problem.

### ~~Adobe Illustrator validation script~~
~~Create a standalone validation script that runs inside Adobe Illustrator
(ExtendScript / JSX) to check `.ai` files before export.~~ Done — lives
under `tools/ai-validate/`, ships as a bundled `.jsx` via per-platform
installer (Inno on Windows, .pkg on macOS). Detects missing/empty layers,
unclosed paths, overlaps, containment, multi-object layers, degenerate
paths, snap drift, corner jitter, etc.

### ~~Illustrator script — release / distribution pipeline~~
~~Need a real distribution path.~~ Done — `release-ai-validate.yml`
GitHub Actions workflow builds the bundle on every `sv*` tag push,
produces signed Inno `.exe` + macOS `.pkg`, attaches to a GitHub
release, and the script self-checks `script-version.json` on launch
to nag the artist about updates. Versioning is independent (`svX.Y.Z`
prefix) from the editor's `vX.Y.Z`. Lessons learned around tag-rebuild
gotchas captured in `feedback_tag_rebuild.md`.

Still open as a *next-step nice-to-have* (not blocking): the Tauri
app could host a one-click "Install Illustrator script" action that
copies the bundled `.jsx` into the user's Illustrator scripts folder
without going through the per-platform installer. See "auto-update
shell for ai-validate" discussion (2026-05-05).

### Persist user settings + window state between sessions
Several UI knobs live in the DOM/Elm only and reset on every launch:

- Window size + position (currently `tauri.conf.json` hard-codes
  `width: 1280, height: 800, center: true` and ignores how the user
  last sized the window).
- `--tools-width` (right panel width — set by dragging `.resize-handle`,
  default now 40vw, never persisted).
- Zoom level / pan position on the canvas.
- "Show lights / grid / piece outlines / blueprint" checkboxes.
- Target pieces / min border slider values.
- Last picked AI file (already persisted via the OS picker, but only
  the directory; the actual selected file isn't restored).
- Selected wave / piece editor mode state.

Two Tauri plugins together handle this cleanly:

- `tauri-plugin-window-state` — drop-in window position/size restore.
  Three lines (`Cargo.toml` dep + `.plugin(...)` registration +
  capability entry); no app code change beyond that. Window state
  saves to `<app_data>/.window-state.json` automatically.
- `tauri-plugin-store` — typed key/value JSON for everything else.
  Survives reboots. Wire it up so:
  1. On change, debounce-save each setting to the store.
  2. On startup, read the store and seed the model + CSS custom
     properties before the first render (avoid layout flash).
  3. Keep the schema versioned so adding/removing fields stays safe.

Could also consider per-AI-file overrides (e.g. zoom level remembered
per file content hash) once the basic global settings work.

### Tauri warning / error UX — currently a raw dump
`load_pdf` returns `metadata.warnings` as `Vec<String>` and the Elm
side displays them as a flat list under the canvas. Some cases:

- `Layer 'X': N unclosed path(s) — discarded` (now auto-closed but
  still flagged so the artist can fix the source)
- `Layer 'A' is fully contained within Layer 'B' (95% overlap) — discarded`
- `Layer 'A' overlaps Layer 'B' (45% of smaller area) — Layer 'A' discarded`
- `MULTI_OBJECT: layer 'X' has 3 polygons, discarded 1 independent objects`
- `SKIPPED: 'Y' has no vector polygon`
- `COVERED: 'Z' removed (hidden under another brick)`

Today the user gets a wall of these on every load, can't sort, can't
filter, can't open the offending layer in Illustrator with one click,
can't tell "this is a real problem" from "the parser handled it". We
should:

1. **Structured warnings on the wire** — replace `Vec<String>` with
   `Vec<{severity, kind, layer_name, related_layer, message}>` so
   the frontend can group / sort / filter by severity (info /
   warning / error) and kind (unclosed / overlap / containment /
   skipped / covered).
2. **Collapsible panel in Elm** — group by severity, default
   collapse "info" (auto-fixed), expand "warning" / "error". Show a
   count badge per group. Click a row → highlight that layer on the
   canvas.
3. **"Open in Illustrator" link** per warning row — copy a
   `aiFile://...?layer=X` URL to clipboard (or open it via a Tauri
   command if Illustrator's URL scheme works) so the artist can
   jump straight to the broken layer.
4. **Suppress duplicates** — many warnings repeat the same layer
   name; collapse identical messages with a count.
5. **Persist dismissals** — once the artist acknowledges "I know
   about Layer 320", don't surface it on every reload of the same
   AI file. Tied to the AI file's content hash; a re-export by
   the artist invalidates dismissals.

## ~~Nice-to-have~~

### ~~Tauri desktop app~~
~~Wrap existing server+webview for native app bundle, Gatekeeper signing, dock icon.~~
Done: Tauri migration is now the mainline (PR #57).
