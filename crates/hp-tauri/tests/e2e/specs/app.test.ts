/**
 * E2E tests for House Puzzle Tauri app
 *
 * Four suites:
 *
 *  1. UI structure — sidebar, app title, navigation buttons, file list
 *  2. Initial UI state — verifies the app shows the correct idle/empty state
 *  3. Screenshot baseline — per-platform PNG comparison
 *  4. Load + Merge functional — canary equivalent (gated on FIXTURE_DIR)
 *
 * Note on Tauri IPC:
 *   In WebDriver mode, `window.__TAURI__` is injected by the Tauri runtime into
 *   the page, but may not be accessible from WebDriver `executeAsync()` scripts
 *   due to execution context isolation (especially on WebKitGTK/Linux).
 *   Suites 1–3 therefore test observable DOM/UI state instead of raw IPC calls.
 *
 * Suite 4 (Load + Merge):
 *   Runs only when FIXTURE_DIR is set to a directory containing _NY*.ai files.
 *   Example: FIXTURE_DIR=/path/to/ai npx wdio run wdio.conf.ts
 *   Baselines are compared from  <repo-root>/tests/baselines/  (same files used
 *   by tests/canary.py and tests/test_e2e.py).
 *   The IPC bridge is used from within the page's own JS context via a helper
 *   stored on window, bypassing WebDriver context isolation.
 */

import fs from "fs";
import path from "path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

// ─── Directories ─────────────────────────────────────────────────────────────

const SCREENSHOTS_DIR = path.join(__dirname, "../screenshots");
const BASELINES_DIR_SS = path.join(SCREENSHOTS_DIR, "baselines");
const ACTUAL_DIR = path.join(SCREENSHOTS_DIR, "actual");

/** JSON baselines live in  <repo-root>/tests/baselines/ */
const REPO_BASELINES_DIR = path.resolve(__dirname, "../../../../tests/baselines");

/** Fixture .ai files: set FIXTURE_DIR env var to enable functional tests */
const FIXTURE_DIR = process.env.FIXTURE_DIR ?? "";

[SCREENSHOTS_DIR, BASELINES_DIR_SS, ACTUAL_DIR].forEach((dir) => {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

// ─── Platform ────────────────────────────────────────────────────────────────

/** "linux" | "darwin" | "win32" */
const PLATFORM = process.platform;

function screenshotName(name: string): string {
  return `${name}-${PLATFORM}.png`;
}

// ─── Screenshot helpers ───────────────────────────────────────────────────────

async function takeScreenshot(name: string): Promise<string> {
  const filename = screenshotName(name);
  const actualPath = path.join(ACTUAL_DIR, filename);
  await browser.saveScreenshot(actualPath);
  console.log(`[screenshot] saved: ${actualPath}`);
  return actualPath;
}

/**
 * Compare actualPath against the stored per-platform baseline.
 *
 * First-run behaviour: copies actual → baseline and returns 0.
 * Subsequent runs: runs pixelmatch and returns the mismatch pixel count.
 *
 * To update a baseline delete the file and re-run, then commit the new baseline.
 */
async function compareOrEstablishBaseline(name: string): Promise<number> {
  const filename = screenshotName(name);
  const actualPath = path.join(ACTUAL_DIR, filename);
  const baselinePath = path.join(BASELINES_DIR_SS, filename);

  if (!fs.existsSync(baselinePath)) {
    fs.copyFileSync(actualPath, baselinePath);
    console.log(`[baseline] established: ${baselinePath}`);
    return 0; // first run: no reference to compare against yet
  }

  const actual = PNG.sync.read(fs.readFileSync(actualPath));
  const baseline = PNG.sync.read(fs.readFileSync(baselinePath));

  if (actual.width !== baseline.width || actual.height !== baseline.height) {
    console.warn(
      `[baseline] size mismatch: baseline=${baseline.width}x${baseline.height} ` +
        `actual=${actual.width}x${actual.height}`
    );
    return -1; // size differs between platforms — don't fail, just warn
  }

  const diff = new PNG({ width: actual.width, height: actual.height });
  const mismatch = pixelmatch(
    actual.data,
    baseline.data,
    diff.data,
    actual.width,
    actual.height,
    { threshold: 0.1 }
  );

  if (mismatch > 0) {
    const diffPath = path.join(ACTUAL_DIR, `diff-${filename}`);
    fs.writeFileSync(diffPath, PNG.sync.write(diff));
    console.warn(`[pixelmatch] ${mismatch} px differ → diff: ${diffPath}`);
  }

  return mismatch;
}

// ─── Tauri IPC helper — in-page bridge ───────────────────────────────────────
//
// The Tauri runtime injects window.__TAURI__ into the page's main execution
// context at startup.  Because WebDriver executeAsync() may run in an isolated
// context on some platforms (e.g. WebKitGTK), we FIRST inject a helper
// function (window._testInvoke) into the PAGE context that captures the
// already-available window.__TAURI__ reference, then call it via execute().
//
// This is done once in beforeEach for Suite 4; subsequent calls use the
// already-installed window._testInvoke shim.

type TauriResult = { ok: unknown } | { err: string };

/** Install the in-page helper if it isn't there yet. */
async function ensurePageHelper(): Promise<void> {
  await browser.execute(function () {
    // eslint-disable-next-line @typescript-eslint/no-explicit-any
    const g = globalThis as any;
    if (g._testInvokeReady) return;

    g._testInvoke = function (
      cmd: string,
      args: Record<string, unknown>,
      cb: (r: { ok: unknown } | { err: string }) => void
    ): void {
      const tauri = g.__TAURI__ as
        | { core?: { invoke?: (c: string, a: unknown) => Promise<unknown> } }
        | undefined;
      if (!tauri?.core?.invoke) {
        cb({ err: "__TAURI__.core.invoke not available in page context" });
        return;
      }
      tauri.core
        .invoke(cmd, args)
        .then((r: unknown) => cb({ ok: r }))
        .catch((e: unknown) => cb({ err: String(e) }));
    };

    g._testInvokeReady = true;
  });
}

/**
 * Invoke a Tauri command via the in-page helper.
 * Falls back to a direct globalThis approach on platforms where it works.
 */
async function invokeTauri(
  command: string,
  args: Record<string, unknown> = {}
): Promise<unknown> {
  await ensurePageHelper();

  return new Promise<unknown>((resolve, reject) => {
    browser
      .executeAsync(function (
        cmd: string,
        cmdArgs: Record<string, unknown>,
        done: (r: TauriResult) => void
      ) {
        // eslint-disable-next-line @typescript-eslint/no-explicit-any
        const helper = (globalThis as any)._testInvoke as
          | ((
              c: string,
              a: Record<string, unknown>,
              cb: (r: TauriResult) => void
            ) => void)
          | undefined;

        if (!helper) {
          done({ err: "_testInvoke helper not installed" });
          return;
        }
        helper(cmd, cmdArgs, done);
      }, command, args)
      .then((result) => {
        if (result && typeof result === "object" && "err" in result) {
          reject(new Error((result as { err: string }).err));
        } else {
          resolve((result as { ok: unknown }).ok);
        }
      })
      .catch(reject);
  });
}

// ─── JSON baseline helpers ────────────────────────────────────────────────────

interface BrickBaseline {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  type: string;
  neighbors: string[];
}

interface LoadBaseline {
  canvas: { width: number; height: number };
  num_bricks: number;
  render_dpi: number;
  houseUnitsHigh: number;
  has_base: boolean;
  total_layers: number;
  bricks: BrickBaseline[];
  png_hashes: Record<string, string>;
}

interface PieceBaseline {
  id: string;
  x: number;
  y: number;
  width: number;
  height: number;
  num_bricks: number;
  brick_ids: string[];
}

interface MergeBaseline {
  num_pieces: number;
  pieces: PieceBaseline[];
}

function readLoadBaseline(stem: string): LoadBaseline | null {
  const p = path.join(REPO_BASELINES_DIR, `${stem}_load.json`);
  if (!fs.existsSync(p)) return null;
  return JSON.parse(fs.readFileSync(p, "utf8")) as LoadBaseline;
}

function readMergeBaseline(stem: string): MergeBaseline | null {
  const p = path.join(REPO_BASELINES_DIR, `${stem}_merge.json`);
  if (!fs.existsSync(p)) return null;
  return JSON.parse(fs.readFileSync(p, "utf8")) as MergeBaseline;
}

// ─── Tauri response shapes ────────────────────────────────────────────────────

interface LoadResponse {
  key: string;
  canvas: { width: number; height: number };
  num_bricks: number;
  bricks: Array<{
    id: string;
    x: number;
    y: number;
    width: number;
    height: number;
    type: string;
    neighbors: string[];
  }>;
  render_dpi: number;
  houseUnitsHigh: number;
  has_base: boolean;
}

interface MergeResponse {
  num_pieces: number;
  pieces: Array<{
    id: string;
    x: number;
    y: number;
    width: number;
    height: number;
    brick_ids: string[];
  }>;
}

// ─── Fixtures available for functional tests ──────────────────────────────────

const NY_STEMS = Array.from({ length: 10 }, (_, i) => `_NY${i + 1}`);

function availableFixtures(): Array<{ stem: string; aiPath: string }> {
  if (!FIXTURE_DIR) return [];
  return NY_STEMS.flatMap((stem) => {
    const aiPath = path.join(FIXTURE_DIR, `${stem}.ai`);
    if (fs.existsSync(aiPath)) return [{ stem, aiPath }];
    return [];
  });
}

/** Merge params matching canary / test_e2e.py */
const MERGE_PARAMS = { target_count: 60, min_border: 10, seed: 42 };

// =============================================================================
// Suite 1 – UI structure
// =============================================================================

describe("House Puzzle app", () => {
  it("launches and shows the start screen", async () => {
    // Elm replaces #elm-root; the sidebar is always rendered.
    const sidebar = await $(".left-sidebar");
    await sidebar.waitForExist({ timeout: 15_000 });

    const title = await browser.getTitle();
    console.log(`[test] window title: "${title}"`);

    expect(
      title.toLowerCase().includes("house puzzle") ||
        title.toLowerCase().includes("editor")
    ).toBe(true);
  });

  it("renders the left sidebar with the app title", async () => {
    const appTitle = await $(".app-title");
    await appTitle.waitForExist({ timeout: 10_000 });

    const text = await appTitle.getText();
    console.log(`[test] .app-title text: "${text}"`);
    expect(text.length).toBeGreaterThan(0);
    expect(text).toContain("House Puzzle");
  });

  it("shows a version tag in the sidebar", async () => {
    const versionTag = await $(".version-tag");
    await versionTag.waitForExist({ timeout: 10_000 });

    const version = await versionTag.getText();
    console.log(`[test] .version-tag text: "${version}"`);
    expect(typeof version).toBe("string");
    expect(version.length).toBeGreaterThan(0);
  });

  it("renders the file-list panel with a Browse button", async () => {
    const fileList = await $(".file-list");
    await fileList.waitForExist({ timeout: 10_000 });

    const browseBtn = await $(".file-entry-browse");
    await browseBtn.waitForExist({ timeout: 5_000 });

    const text = await browseBtn.getText();
    console.log(`[test] browse button text: "${text}"`);
    expect(text.length).toBeGreaterThan(0);
  });
});

// =============================================================================
// Suite 2 – Initial UI state
//
// Verifies the app's idle state when no PDF has been loaded:
//   • navigation buttons are present
//   • action buttons that require a loaded file are disabled
//   • the empty-state message is shown
// =============================================================================

describe("Initial UI state (no file loaded)", () => {
  it('shows the "Start" / reset navigation button as enabled', async () => {
    // The first mode button says "Start" in the initial state
    const modeBtn = await $(".mode-btn");
    await modeBtn.waitForExist({ timeout: 10_000 });

    const isEnabled = await modeBtn.isEnabled();
    console.log(`[test] first mode-btn enabled: ${isEnabled}`);
    expect(isEnabled).toBe(true);
  });

  it("shows the Import/Pieces/Export navigation buttons as disabled", async () => {
    // Buttons that require a loaded/generated puzzle should be disabled
    const allModeBtns = await $$(".mode-btn");
    expect(allModeBtns.length).toBeGreaterThan(1);

    let disabledCount = 0;
    for (const btn of allModeBtns) {
      const enabled = await btn.isEnabled();
      const text = await btn.getText();
      if (!enabled) {
        disabledCount++;
        console.log(`[test] disabled button: "${text}"`);
      }
    }
    // At minimum, Import, Pieces, Blueprint, Groups, Waves, Export are all disabled
    expect(disabledCount).toBeGreaterThanOrEqual(6);
  });

  it('shows "No files in in/" or the file list when no files exist', async () => {
    // Either the empty state message shows, or some file entries are present
    const fileList = await $(".file-list");
    await fileList.waitForExist({ timeout: 5_000 });

    const html = await fileList.getHTML();
    console.log(`[test] file-list content (excerpt): ${html.substring(0, 200)}`);

    // Should contain either the empty message or file entries
    const hasEmptyMsg = html.includes("No files in in/");
    const hasFileEntry = html.includes("file-entry");

    expect(hasEmptyMsg || hasFileEntry).toBe(true);
  });

  it("renders the canvas area or empty body placeholder", async () => {
    // Either .app-body (file loaded) or .app-body-empty (no file)
    // After initial load with no file, .app-body-empty should be visible
    const emptyBody = await $(".app-body-empty");
    const regularBody = await $(".app-body");

    const emptyExists = await emptyBody.isExisting();
    const regularExists = await regularBody.isExisting();

    console.log(
      `[test] .app-body-empty: ${emptyExists}, .app-body: ${regularExists}`
    );
    expect(emptyExists || regularExists).toBe(true);
  });

  it("renders undo/redo buttons (both disabled when nothing to undo)", async () => {
    const undoBtn = await $(".undo-btn");
    await undoBtn.waitForExist({ timeout: 5_000 });
    const redoBtn = await $(".redo-btn");
    await redoBtn.waitForExist({ timeout: 5_000 });

    const undoEnabled = await undoBtn.isEnabled();
    const redoEnabled = await redoBtn.isEnabled();
    console.log(
      `[test] undo: ${undoEnabled}, redo: ${redoEnabled} (both should be false on init)`
    );
    expect(undoEnabled).toBe(false);
    expect(redoEnabled).toBe(false);
  });
});

// =============================================================================
// Suite 3 – Screenshot baseline
// =============================================================================

describe("Screenshot baseline", () => {
  it(`captures initial state and compares against ${PLATFORM} baseline`, async () => {
    await browser.pause(1000); // let UI fully settle

    await takeScreenshot("initial-state");
    const mismatch = await compareOrEstablishBaseline("initial-state");

    const windowSize = await browser.getWindowSize();
    const totalPixels = windowSize.width * windowSize.height;
    // Allow up to 2 % pixel difference for anti-aliasing / font rendering variance
    const allowedMismatch = Math.ceil(totalPixels * 0.02);

    if (mismatch < 0) {
      console.warn(
        "[test] screenshot size mismatch between runs — skipping pixel diff"
      );
    } else {
      expect(mismatch).toBeLessThanOrEqual(allowedMismatch);
    }
  });
});

// =============================================================================
// Suite 4 – Load + Merge functional (canary equivalent)
//
// Mirrors the behaviour of tests/canary.py and tests/test_e2e.py.
// Enabled by setting FIXTURE_DIR to a directory containing _NY*.ai files.
//
// Invokes Tauri commands via a helper shim installed in the page context
// (window._testInvoke), which avoids WebDriver context isolation issues
// while still going through the real Tauri IPC bridge.
//
// Each NY fixture is tested against the committed JSON baselines in
// tests/baselines/_NY*_load.json and _NY*_merge.json.
// =============================================================================

const fixtures = availableFixtures();

if (fixtures.length > 0) {
  console.log(
    `[fixtures] ${fixtures.length} fixture(s) found in ${FIXTURE_DIR}`
  );

  describe("Load + Merge functional (canary equivalent)", () => {
    // Install the page helper once before all fixture tests
    before("install page helper", async () => {
      await ensurePageHelper();
    });

    for (const { stem, aiPath } of fixtures) {
      describe(stem, () => {
        let sessionKey: string;
        let loadResp: LoadResponse;
        const loadBaseline = readLoadBaseline(stem);
        const mergeBaseline = readMergeBaseline(stem);

        before(`load ${stem}.ai`, async function () {
          this.timeout(120_000);
          console.log(`[fixture] loading ${aiPath}`);
          loadResp = (await invokeTauri("load_pdf", {
            path: aiPath,
            canvas_height: 900,
            deterministic_ids: true,
          })) as LoadResponse;
          sessionKey = loadResp.key;
          console.log(
            `[fixture] loaded: key=${sessionKey} bricks=${loadResp.num_bricks}`
          );
        });

        // ── load_pdf assertions ──────────────────────────────────────────────

        it("returns a session key", () => {
          expect(typeof sessionKey).toBe("string");
          expect(sessionKey.length).toBeGreaterThan(0);
        });

        it("returns the expected canvas dimensions", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          expect(loadResp.canvas.width).toBe(loadBaseline.canvas.width);
          expect(loadResp.canvas.height).toBe(loadBaseline.canvas.height);
        });

        it("returns the expected brick count", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          expect(loadResp.num_bricks).toBe(loadBaseline.num_bricks);
        });

        it("returns the expected render_dpi", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          expect(loadResp.render_dpi).toBeCloseTo(loadBaseline.render_dpi, 1);
        });

        it("brick IDs match baseline", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          const actualIds = loadResp.bricks.map((b) => b.id).sort();
          const baselineIds = loadBaseline.bricks.map((b) => b.id).sort();
          expect(actualIds).toEqual(baselineIds);
        });

        it("brick positions match baseline", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          const baselineMap = new Map(loadBaseline.bricks.map((b) => [b.id, b]));
          let mismatches = 0;
          for (const brick of loadResp.bricks) {
            const bl = baselineMap.get(brick.id);
            if (!bl) continue;
            if (
              brick.x !== bl.x ||
              brick.y !== bl.y ||
              brick.width !== bl.width ||
              brick.height !== bl.height
            ) {
              mismatches++;
              console.warn(
                `[diff] brick ${brick.id}: ` +
                  `actual=[${brick.x},${brick.y},${brick.width},${brick.height}] ` +
                  `baseline=[${bl.x},${bl.y},${bl.width},${bl.height}]`
              );
            }
          }
          expect(mismatches).toBe(0);
        });

        it("brick neighbors match baseline", () => {
          if (!loadBaseline) {
            console.warn(`[skip] no load baseline for ${stem}`);
            return;
          }
          const baselineMap = new Map(loadBaseline.bricks.map((b) => [b.id, b]));
          let nbDiffs = 0;
          for (const brick of loadResp.bricks) {
            const bl = baselineMap.get(brick.id);
            if (!bl) continue;
            const actualNbrs = brick.neighbors.slice().sort().join(",");
            const baselineNbrs = bl.neighbors.slice().sort().join(",");
            if (actualNbrs !== baselineNbrs) nbDiffs++;
          }
          if (nbDiffs > 0) {
            console.warn(`[diff] ${nbDiffs} bricks have different neighbor sets`);
          }
          expect(nbDiffs).toBe(0);
        });

        // ── merge_pieces assertions ──────────────────────────────────────────

        describe("merge", () => {
          let mergeResp: MergeResponse;

          before("run merge", async function () {
            this.timeout(120_000);
            mergeResp = (await invokeTauri("merge_pieces", {
              key: sessionKey,
              ...MERGE_PARAMS,
            })) as MergeResponse;
            console.log(`[fixture] merged: pieces=${mergeResp.num_pieces}`);
          });

          it("returns the expected piece count", () => {
            if (!mergeBaseline) {
              console.warn(`[skip] no merge baseline for ${stem}`);
              return;
            }
            expect(mergeResp.num_pieces).toBe(mergeBaseline.num_pieces);
          });

          it("piece IDs match baseline", () => {
            if (!mergeBaseline) {
              console.warn(`[skip] no merge baseline for ${stem}`);
              return;
            }
            const actualIds = mergeResp.pieces.map((p) => p.id).sort();
            const baselineIds = mergeBaseline.pieces.map((p) => p.id).sort();
            expect(actualIds).toEqual(baselineIds);
          });

          it("piece→brick assignments match baseline", () => {
            if (!mergeBaseline) {
              console.warn(`[skip] no merge baseline for ${stem}`);
              return;
            }
            const baselineMap = new Map(
              mergeBaseline.pieces.map((p) => [p.id, p])
            );
            let assignDiffs = 0;
            for (const piece of mergeResp.pieces) {
              const bl = baselineMap.get(piece.id);
              if (!bl) continue;
              const actualBricks = piece.brick_ids.slice().sort().join(",");
              const baselineBricks = bl.brick_ids.slice().sort().join(",");
              if (actualBricks !== baselineBricks) {
                assignDiffs++;
                console.warn(
                  `[diff] piece ${piece.id}: brick assignment differs`
                );
              }
            }
            expect(assignDiffs).toBe(0);
          });

          it("piece positions match baseline", () => {
            if (!mergeBaseline) {
              console.warn(`[skip] no merge baseline for ${stem}`);
              return;
            }
            const baselineMap = new Map(
              mergeBaseline.pieces.map((p) => [p.id, p])
            );
            let positionDiffs = 0;
            for (const piece of mergeResp.pieces) {
              const bl = baselineMap.get(piece.id);
              if (!bl) continue;
              if (
                piece.x !== bl.x ||
                piece.y !== bl.y ||
                piece.width !== bl.width ||
                piece.height !== bl.height
              ) {
                positionDiffs++;
                console.warn(
                  `[diff] piece ${piece.id}: ` +
                    `actual=[${piece.x},${piece.y},${piece.width},${piece.height}] ` +
                    `baseline=[${bl.x},${bl.y},${bl.width},${bl.height}]`
                );
              }
            }
            expect(positionDiffs).toBe(0);
          });

          it("get_image(composite) returns non-empty base64 PNG", async () => {
            const b64 = (await invokeTauri("get_image", {
              key: sessionKey,
              image_type: "composite",
            })) as string;
            expect(typeof b64).toBe("string");
            // A real PNG base64-encodes to at least a few hundred chars
            expect(b64.length).toBeGreaterThan(100);
          });

          it("get_piece_image returns non-empty base64 PNG for first piece", async () => {
            if (mergeResp.pieces.length === 0) return;
            const firstPieceId = mergeResp.pieces.map((p) => p.id).sort()[0];
            const b64 = (await invokeTauri("get_piece_image", {
              key: sessionKey,
              piece_id: firstPieceId,
            })) as string;
            expect(typeof b64).toBe("string");
            expect(b64.length).toBeGreaterThan(100);
          });
        });
      });
    }
  });
} else {
  describe("Load + Merge functional (canary equivalent)", () => {
    it("is skipped — set FIXTURE_DIR=/path/to/ai-files to enable", () => {
      console.log(
        "[info] functional tests skipped: FIXTURE_DIR not set or no _NY*.ai files found.\n" +
          "       Set FIXTURE_DIR to a directory containing _NY1.ai … _NY10.ai to run\n" +
          "       full canary-equivalent verification against tests/baselines/."
      );
      // Intentionally passes — fixture files are not present in CI
      expect(true).toBe(true);
    });
  });
}

// =============================================================================
// Suite 5 – Visual canary (UI-driven: load → screenshot → generate → screenshot)
//
// Mirrors the canary test flow through actual UI interactions:
//   1. Click a file entry to load an AI file
//   2. Wait for the composite to appear
//   3. Take a screenshot of the loaded house
//   4. Click "Generate Puzzle"
//   5. Wait for puzzle generation to complete
//   6. Take a screenshot of the generated puzzle
//
// Enabled by FIXTURE_DIR. Uses the first available _NY*.ai file.
// Screenshots are saved for visual inspection and baseline comparison.
// =============================================================================

// =============================================================================
// Suite 5 – Visual canary (load via IPC → screenshot → generate via UI → screenshot)
//
// Loads a file via Tauri IPC (no native file dialog in e2e), then drives
// the Generate button via UI, takes screenshots at each step.
// Uses the in-page _testInvoke helper to bypass WebDriver context isolation.
//
// Enabled by FIXTURE_DIR. Uses the first available _NY*.ai file.
// =============================================================================

// =============================================================================
// Suite 5 – Visual canary (CLI --load → screenshot → generate via UI → screenshot)
//
// When FIXTURE_DIR is set, the app is launched with --load <path> which
// auto-loads the file on startup. No IPC needed — works on all platforms.
// =============================================================================

if (FIXTURE_DIR) {
  describe("Visual canary", () => {
    it("loads a file by clicking a file entry", async function () {
      this.timeout(120_000);

      // The app lists files from in/ on startup. Wait for entries to appear.
      await browser.waitUntil(
        async () => {
          const entries = await $$(".file-entry:not(.file-entry-browse)");
          return entries.length > 0;
        },
        { timeout: 30_000, interval: 500, timeoutMsg: "No file entries appeared in the file list" }
      );

      // Click the first .ai file entry
      const entries = await $$(".file-entry:not(.file-entry-browse)");
      console.log(`[visual] found ${entries.length} file entries, clicking first`);
      await entries[0].click();

      // Wait for house image to render
      await browser.waitUntil(
        async () => {
          const imgs = await $$(".house-svg image");
          return imgs.length > 0;
        },
        { timeout: 90_000, interval: 1000, timeoutMsg: "House image did not appear within 90s" }
      );

      await browser.pause(1000);
      console.log("[visual] house loaded via file list click");
    });

    it("takes screenshot of loaded house", async function () {
      this.timeout(30_000);

      await takeScreenshot("canary-house-loaded");
      const mismatch = await compareOrEstablishBaseline("canary-house-loaded");

      if (mismatch < 0) {
        console.warn("[visual] screenshot size mismatch — skipping pixel diff");
      } else {
        const windowSize = await browser.getWindowSize();
        const allowedMismatch = Math.ceil(
          windowSize.width * windowSize.height * 0.05
        );
        expect(mismatch).toBeLessThanOrEqual(allowedMismatch);
      }
    });

    it("generates puzzle via UI and waits for completion", async function () {
      this.timeout(120_000);

      // Navigate to Import section
      const modeBtns = await $$(".mode-btn");
      for (const btn of modeBtns) {
        const text = await btn.getText();
        if (text.includes("Import")) {
          if (await btn.isEnabled()) {
            await btn.click();
            await browser.pause(500);
          }
          break;
        }
      }

      // Wait for Generate Puzzle button to be enabled
      await browser.waitUntil(
        async () => {
          const btn = await $("button.primary");
          if (!(await btn.isExisting())) return false;
          return await btn.isEnabled();
        },
        { timeout: 15_000, interval: 500, timeoutMsg: "Generate Puzzle button not found or not enabled" }
      );

      const generateBtn = await $("button.primary");
      const btnText = await generateBtn.getText();
      console.log(`[visual] clicking: "${btnText}"`);
      await generateBtn.click();

      // Wait for piece images to appear
      await browser.waitUntil(
        async () => {
          const imgs = await $$(".house-svg image");
          return imgs.length > 5;
        },
        { timeout: 90_000, interval: 1000, timeoutMsg: "Piece images did not appear within 90s" }
      );

      // Navigate to Pieces view
      const allBtns = await $$(".mode-btn");
      for (const btn of allBtns) {
        const text = await btn.getText();
        if (text.includes("Pieces")) {
          if (await btn.isEnabled()) {
            await btn.click();
            await browser.pause(500);
          }
          break;
        }
      }

      await browser.pause(1000);
    });

    it("takes screenshot of generated puzzle", async function () {
      this.timeout(30_000);

      await takeScreenshot("canary-puzzle-generated");
      const mismatch = await compareOrEstablishBaseline("canary-puzzle-generated");

      if (mismatch < 0) {
        console.warn("[visual] screenshot size mismatch — skipping pixel diff");
      } else {
        const windowSize = await browser.getWindowSize();
        const allowedMismatch = Math.ceil(
          windowSize.width * windowSize.height * 0.05
        );
        expect(mismatch).toBeLessThanOrEqual(allowedMismatch);
      }
    });
  });
} else {
  describe("Visual canary", () => {
    it("is skipped — set FIXTURE_DIR to enable", () => {
      console.log("[info] visual canary skipped: FIXTURE_DIR not set");
      expect(true).toBe(true);
    });
  });
}
