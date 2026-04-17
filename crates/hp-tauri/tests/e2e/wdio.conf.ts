/**
 * WebDriverIO configuration for House Puzzle E2E tests.
 *
 * Uses tauri-plugin-webdriver (https://github.com/Choochmeque/tauri-plugin-webdriver),
 * a Tauri plugin that embeds a W3C WebDriver server (port 4445) directly in the
 * debug build of the app.  This approach works on all platforms including macOS,
 * where the standalone `tauri-driver` binary is not supported.
 *
 * Flow:
 *  1. beforeSession – spawn the debug app binary directly; it starts the
 *     embedded WebDriver server on 127.0.0.1:4445.
 *  2. WebDriverIO connects to port 4445 and creates a session.
 *  3. before (browser hook) – navigate to tauri://localhost so the WebView is
 *     attached to the session.
 *  4. Tests run.
 *  5. afterSession – kill the app process.
 */

import path from "path";
import http from "http";
import { spawn, ChildProcess } from "child_process";

// ─── Platform helpers ────────────────────────────────────────────────────────

function getAppBinary(): string {
  const platform = process.platform;
  const projectRoot = path.resolve(__dirname, "../../../..");

  // All platforms: use the raw debug binary produced by `cargo build`.
  // tauri-plugin-webdriver embeds the WebDriver server, so no bundle is needed.
  if (platform === "linux") {
    return (
      process.env.TAURI_BINARY ||
      path.join(projectRoot, "target/debug/hp-tauri")
    );
  } else if (platform === "darwin") {
    return (
      process.env.TAURI_BINARY ||
      path.join(projectRoot, "target/debug/hp-tauri")
    );
  } else if (platform === "win32") {
    return (
      process.env.TAURI_BINARY ||
      path.join(projectRoot, "target/debug/hp-tauri.exe")
    );
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

// ─── App process management ──────────────────────────────────────────────────

/** Port used by tauri-plugin-webdriver (default, overridable via env). */
const WEBDRIVER_PORT = parseInt(
  process.env.TAURI_WEBDRIVER_PORT ?? "4445",
  10
);

let appProcess: ChildProcess | undefined;
let appShuttingDown = false;

function startApp(): void {
  appShuttingDown = false;
  const binary = getAppBinary();
  console.log(`[wdio] launching app: ${binary}`);

  appProcess = spawn(binary, [], {
    stdio: [null, process.stdout, process.stderr],
    env: {
      ...process.env,
      // Ensure the plugin WebDriver port is set in case the app reads it
      TAURI_WEBDRIVER_PORT: String(WEBDRIVER_PORT),
    },
  });

  appProcess.on("error", (err) => {
    console.error("[wdio] app process error:", err);
    process.exit(1);
  });

  appProcess.on("exit", (code) => {
    if (!appShuttingDown) {
      console.error("[wdio] app exited unexpectedly, code:", code);
      process.exit(1);
    }
  });
}

function stopApp(): void {
  appShuttingDown = true;
  appProcess?.kill();
  appProcess = undefined;
}

/**
 * Poll the embedded WebDriver /status endpoint until it responds (or timeout).
 * The plugin starts the HTTP server almost immediately, but the app itself may
 * take a moment to initialise.
 */
function waitForWebDriver(
  port: number,
  timeoutMs = 30_000,
  intervalMs = 500
): Promise<void> {
  return new Promise((resolve, reject) => {
    const deadline = Date.now() + timeoutMs;

    const check = () => {
      const req = http.get(
        { hostname: "127.0.0.1", port, path: "/status", timeout: 1000 },
        (res) => {
          res.resume(); // discard body
          if (res.statusCode !== undefined && res.statusCode < 500) {
            resolve();
          } else {
            retry();
          }
        }
      );
      req.on("error", retry);
      req.on("timeout", () => { req.destroy(); retry(); });
    };

    const retry = () => {
      if (Date.now() >= deadline) {
        reject(new Error(`WebDriver server did not become ready on port ${port} within ${timeoutMs} ms`));
      } else {
        setTimeout(check, intervalMs);
      }
    };

    check();
  });
}

// ─── WebDriverIO configuration ───────────────────────────────────────────────

export const config: WebdriverIO.Config = {
  hostname: "127.0.0.1",
  port: WEBDRIVER_PORT,
  path: "/",

  specs: ["./specs/**/*.ts"],
  maxInstances: 1,

  // tauri-plugin-webdriver accepts any capabilities (they are not processed).
  // Do NOT use `tauri:options` – that was specific to the standalone tauri-driver.
  capabilities: [
    {
      maxInstances: 1,
    } as WebdriverIO.Capabilities,
  ],

  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  // Start the app before the WebDriver session is established.
  beforeSession: async () => {
    startApp();
    await waitForWebDriver(WEBDRIVER_PORT);
    console.log(`[wdio] WebDriver server ready on port ${WEBDRIVER_PORT}`);
  },

  // Navigate to the Tauri app URL once the session is open so that
  // the WebView is fully attached before any test assertions run.
  //
  // URL scheme differs by platform:
  //   - Linux (WebKitGTK) / macOS (WebKit): tauri://localhost
  //   - Windows (Edge WebView2): https://tauri.localhost
  //     WebView2 maps the custom protocol under https://tauri.localhost/
  before: async () => {
    const appUrl =
      process.platform === "win32"
        ? "https://tauri.localhost"
        : "tauri://localhost";
    await browser.url(appUrl);
    // Give the Elm SPA a moment to bootstrap
    await browser.pause(2000);
  },

  afterSession: () => stopApp(),
};

// Graceful shutdown on signals
["SIGINT", "SIGTERM", "exit"].forEach((sig) => {
  process.on(sig as NodeJS.Signals, stopApp);
});
