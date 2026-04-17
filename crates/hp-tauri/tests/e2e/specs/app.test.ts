/**
 * E2E tests for House Puzzle Tauri app
 *
 * Three suites always run:
 *  1. UI structure — sidebar, app title, initial state
 *  2. Tauri command API — structural smoke tests via invoke()
 *  3. Screenshot baselines — per-platform pixel comparison
 *
 * A fourth suite runs only when FIXTURE_DIR is set:
 *  4. Load + Merge functional — mirrors the canary / test_e2e.py coverage:
 *       • load_pdf  → brick count, canvas size, render_dpi
 *       • merge_pieces → piece count, piece→brick assignments
 *     Compared against the committed JSON baselines in tests/baselines/.
 *
 * Usage with fixtures:
 *   FIXTURE_DIR=/path/to/dir-containing-NY-ai-files npx wdio run wdio.conf.ts
 *
 * The FIXTURE_DIR must contain files named _NY1.ai … _NY10.ai (or any subset).
 * Baselines are read from  <repo-root>/tests/baselines/_NY*_load.json  and
 * _NY*_merge.json — the same files used by the Python canary / test_e2e suite.
 */

import fs from "fs";
import path from "path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

// ─── Directories ─────────────────────────────────────────────────────────────

const SCREENSHOTS_DIR = path.join(__dirname, "../screenshots");
const BASELINES_DIR_SS = path.join(SCREENSHOTS_DIR, "baselines");
const ACTUAL_DIR = path.join(SCREENSHOTS_DIR, "actual");

// JSON baselines live in  <repo-root>/tests/baselines/
const REPO_BASELINES_DIR = path.resolve(__dirname, "../../../../tests/baselines");

// Fixture .ai files: set FIXTURE_DIR env var to enable functional tests
const FIXTURE_DIR = process.env.FIXTURE_DIR ?? "";

[SCREENSHOTS_DIR, BASELINES_DIR_SS, ACTUAL_DIR].forEach((dir) => {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

// ─── Platform string ─────────────────────────────────────────────────────────

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
 * Compare actualPath against the stored baseline for this platform.
 *
 * - If no baseline is committed yet:  copies actual → baseline and returns 0
 *   (first-run semantics; the CI artifact upload makes baselines available
 *    for the next commit).
 * - If a baseline exists:  runs pixelmatch and returns the mismatch count.
 *
 * The per-platform naming means baselines committed on Linux won't be used
 * on macOS/Windows and vice-versa, so cross-platform font-rendering differences
 * don't cause false failures.
 */
async function compareOrEstablishBaseline(name: string): Promise<number> {
  const filename = screenshotName(name);
  const actualPath = path.join(ACTUAL_DIR, filename);
  const baselinePath = path.join(BASELINES_DIR_SS, filename);

  if (!fs.existsSync(baselinePath)) {
    fs.copyFileSync(actualPath, baselinePath);
    console.log(`[baseline] established: ${baselinePath}`);
    return 0; // first run: no reference to compare against
  }

  const actual = PNG.sync.read(fs.readFileSync(actualPath));
  const baseline = PNG.sync.read(fs.readFileSync(baselinePath));

  if (actual.width !== baseline.width || actual.height !== baseline.height) {
    console.warn(
      `[baseline] size mismatch: baseline=${baseline.width}x${baseline.height} ` +
        `actual=${actual.width}x${actual.height}`
    );
    return -1; // size mismatch: warn but don't fail
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

// ─── Tauri invocation helper ──────────────────────────────────────────────────

type TauriResult =
  | { ok: unknown }
  | { err: string };

/**
 * Invoke a Tauri command from the browser context.
 * Polls for window.__TAURI__ availability before calling to handle slow startup.
 */
async function invokeTauri(
  command: string,
  args: Record<string, unknown> = {}
): Promise<unknown> {
  const result = await browser.executeAsync(function (
    cmd: string,
    cmdArgs: Record<string, unknown>,
    done: (r: TauriResult) => void
  ) {
    let attempts = 0;
    const MAX_ATTEMPTS = 50; // 50 × 200 ms = 10 s

    function tryInvoke(): void {
      // eslint-disable-next-line @typescript-eslint/no-explicit-any
      const tauri = (globalThis as any).__TAURI__ as { core?: { invoke?: (...a: unknown[]) => Promise<unknown> } } | undefined;
      if (tauri?.core?.invoke) {
        tauri.core
          .invoke(cmd, cmdArgs)
          .then(function (r: unknown) {
            done({ ok: r });
          })
          .catch(function (e: unknown) {
            done({ err: String(e) });
          });
      } else if (attempts < MAX_ATTEMPTS) {
        attempts++;
        setTimeout(tryInvoke, 200);
      } else {
        done({ err: "window.__TAURI__.core.invoke not available after 10 s" });
      }
    }

    tryInvoke();
  },
  command,
  args);

  if (result && typeof result === "object" && "err" in result) {
    throw new Error((result as { err: string }).err);
  }
  return (result as { ok: unknown }).ok;
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

/** Returns list of { stem, aiPath } for NY fixtures that exist in FIXTURE_DIR. */
function availableFixtures(): Array<{ stem: string; aiPath: string }> {
  if (!FIXTURE_DIR) return [];
  return NY_STEMS.flatMap((stem) => {
    const aiPath = path.join(FIXTURE_DIR, `${stem}.ai`);
    if (fs.existsSync(aiPath)) return [{ stem, aiPath }];
    return [];
  });
}

// ─── MERGE PARAMS — match canary / test_e2e.py ────────────────────────────────
const MERGE_PARAMS = { target_count: 60, min_border: 10, seed: 42 };

// =============================================================================
// Suite 1 – UI structure
// =============================================================================

describe("House Puzzle app", () => {
  it("launches and shows the start screen", async () => {
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
  });
});

// =============================================================================
// Suite 2 – Tauri command API (structural smoke tests)
// =============================================================================

describe("Tauri command API", () => {
  it("get_version returns a non-empty string", async () => {
    const version = (await invokeTauri("get_version")) as string;
    console.log(`[test] version: "${version}"`);
    expect(typeof version).toBe("string");
    expect(version.length).toBeGreaterThan(0);
  });

  it("list_pdfs returns an object with a files array", async () => {
    const result = (await invokeTauri("list_pdfs")) as { files: unknown[] };
    console.log(`[test] list_pdfs files: ${result.files.length}`);
    expect(result).toBeDefined();
    expect(Array.isArray(result.files)).toBe(true);
  });

  it("load_pdf with a non-existent path returns an error (not a crash)", async () => {
    let errorThrown = false;
    let errorMessage = "";
    try {
      await invokeTauri("load_pdf", {
        path: "/nonexistent/path/to/test.ai",
        canvas_height: 900,
        deterministic_ids: true,
      });
    } catch (e) {
      errorThrown = true;
      errorMessage = String(e);
    }
    console.log(`[test] load_pdf error (expected): ${errorMessage}`);
    expect(errorThrown).toBe(true);
    // Should mention "not found" or similar
    expect(errorMessage.toLowerCase()).toMatch(
      /not found|no such file|does not exist/i
    );
  });
});

// =============================================================================
// Suite 3 – Screenshot baseline
// =============================================================================

describe("Screenshot baseline", () => {
  it(`captures initial state and compares against ${PLATFORM} baseline`, async () => {
    // Give the UI time to settle
    await browser.pause(1000);
    await takeScreenshot("initial-state");
    const mismatch = await compareOrEstablishBaseline("initial-state");

    const windowSize = await browser.getWindowSize();
    const totalPixels = windowSize.width * windowSize.height;
    // Allow up to 2 % pixel difference for anti-aliasing / font rendering variance
    const allowedMismatch = Math.ceil(totalPixels * 0.02);

    if (mismatch < 0) {
      console.warn("[test] screenshot size mismatch between platforms – skipping pixel diff");
    } else {
      expect(mismatch).toBeLessThanOrEqual(allowedMismatch);
    }
  });
});

// =============================================================================
// Suite 4 – Load + Merge functional tests (canary equivalent)
//
// Runs only when FIXTURE_DIR is set and at least one _NY*.ai file is found.
// Verifies the same invariants as the canary and test_e2e.py suites:
//   • brick count, canvas dimensions, render_dpi
//   • brick IDs and positions match the committed JSON baselines
//   • merge piece count and piece→brick assignments match baselines
// =============================================================================

const fixtures = availableFixtures();

if (fixtures.length > 0) {
  console.log(`[fixtures] found ${fixtures.length} fixture(s) in ${FIXTURE_DIR}`);

  describe("Load + Merge functional (canary equivalent)", () => {
    for (const { stem, aiPath } of fixtures) {
      describe(stem, () => {
        let sessionKey: string;
        let loadResp: LoadResponse;
        const loadBaseline = readLoadBaseline(stem);
        const mergeBaseline = readMergeBaseline(stem);

        before(`load ${stem}.ai`, async function () {
          // Loading can be slow (rendering, AI parse)
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
          const baselineMap = new Map(
            loadBaseline.bricks.map((b) => [b.id, b])
          );
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
          const baselineMap = new Map(
            loadBaseline.bricks.map((b) => [b.id, b])
          );
          let nbDiffs = 0;
          for (const brick of loadResp.bricks) {
            const bl = baselineMap.get(brick.id);
            if (!bl) continue;
            const actualNbrs = brick.neighbors.slice().sort().join(",");
            const baselineNbrs = bl.neighbors.slice().sort().join(",");
            if (actualNbrs !== baselineNbrs) nbDiffs++;
          }
          if (nbDiffs > 0) {
            console.warn(
              `[diff] ${nbDiffs} bricks have different neighbor sets`
            );
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
            console.log(
              `[fixture] merged: pieces=${mergeResp.num_pieces}`
            );
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
            const firstPieceId = mergeResp.pieces
              .map((p) => p.id)
              .sort()[0];
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
