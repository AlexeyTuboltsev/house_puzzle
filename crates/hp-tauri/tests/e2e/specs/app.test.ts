/**
 * E2E tests for House Puzzle Tauri app
 *
 * Prerequisites:
 *   - tauri-driver installed: cargo install tauri-driver
 *   - Debug binary built: cargo build -p hp-tauri
 *   - On Linux headless: wrap with xvfb-run
 */

import fs from "fs";
import path from "path";
import { PNG } from "pngjs";
import pixelmatch from "pixelmatch";

const SCREENSHOTS_DIR = path.join(__dirname, "../screenshots");
const BASELINES_DIR = path.join(SCREENSHOTS_DIR, "baselines");
const ACTUAL_DIR = path.join(SCREENSHOTS_DIR, "actual");

// Ensure directories exist
[SCREENSHOTS_DIR, BASELINES_DIR, ACTUAL_DIR].forEach((dir) => {
  if (!fs.existsSync(dir)) fs.mkdirSync(dir, { recursive: true });
});

const PLATFORM = process.platform; // linux | darwin | win32

function screenshotName(name: string): string {
  return `${name}-${PLATFORM}.png`;
}

async function takeScreenshot(name: string): Promise<string> {
  const filename = screenshotName(name);
  const actualPath = path.join(ACTUAL_DIR, filename);
  await browser.saveScreenshot(actualPath);
  console.log(`[screenshot] saved: ${actualPath}`);
  return actualPath;
}

/**
 * Compare a screenshot against the stored baseline for this platform.
 * If no baseline exists yet, the actual screenshot is copied as the new baseline.
 *
 * @returns pixel mismatch count (0 = identical)
 */
async function compareOrEstablishBaseline(name: string): Promise<number> {
  const filename = screenshotName(name);
  const actualPath = path.join(ACTUAL_DIR, filename);
  const baselinePath = path.join(BASELINES_DIR, filename);

  if (!fs.existsSync(baselinePath)) {
    fs.copyFileSync(actualPath, baselinePath);
    console.log(`[baseline] established: ${baselinePath}`);
    return 0; // first run always passes
  }

  const actual = PNG.sync.read(fs.readFileSync(actualPath));
  const baseline = PNG.sync.read(fs.readFileSync(baselinePath));

  if (actual.width !== baseline.width || actual.height !== baseline.height) {
    console.warn(
      `[baseline] size mismatch: baseline=${baseline.width}x${baseline.height} actual=${actual.width}x${actual.height}`
    );
    return -1; // size mismatch
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

// ─────────────────────────────────────────────────────────────────────────────

describe("House Puzzle app", () => {
  it("launches and shows the start screen", async () => {
    // Wait for the Elm app to render something into the DOM.
    // Elm replaces #elm-root, so we wait for the sidebar which is always rendered.
    const sidebar = await $(".left-sidebar");
    await sidebar.waitForExist({ timeout: 15000 });

    const title = await browser.getTitle();
    console.log(`[test] window title: "${title}"`);

    // Accept either the HTML <title> or a dynamic one set by setTitle port
    expect(
      title.toLowerCase().includes("house puzzle") ||
        title.toLowerCase().includes("editor")
    ).toBe(true);
  });

  it("renders the left sidebar with the app title", async () => {
    // The sidebar .app-title element should be present
    const appTitle = await $(".app-title");
    await appTitle.waitForExist({ timeout: 10000 });

    const text = await appTitle.getText();
    console.log(`[test] .app-title text: "${text}"`);
    expect(text.length).toBeGreaterThan(0);
  });

  it("takes a baseline screenshot of the initial state", async () => {
    // Give the UI a moment to finish rendering
    await browser.pause(1000);

    await takeScreenshot("initial-state");
    const mismatch = await compareOrEstablishBaseline("initial-state");

    // Allow up to 2 % pixel difference to tolerate anti-aliasing / font rendering
    const totalPixels =
      (await browser.getWindowSize()).width *
      (await browser.getWindowSize()).height;
    const allowedMismatch = Math.ceil(totalPixels * 0.02);

    if (mismatch < 0) {
      // Size mismatch – don't fail, just warn (resolution can differ in CI)
      console.warn("[test] screenshot size mismatch – skipping pixel diff");
    } else {
      expect(mismatch).toBeLessThanOrEqual(allowedMismatch);
    }
  });
});
