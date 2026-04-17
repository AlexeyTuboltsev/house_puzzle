import os from "os";
import path from "path";
import { spawn } from "child_process";

// ─── Platform helpers ────────────────────────────────────────────────────────

function getAppBinary(): string {
  const platform = process.platform;
  const projectRoot = path.resolve(__dirname, "../../../..");

  if (platform === "linux") {
    return (
      process.env.TAURI_BINARY ||
      path.join(projectRoot, "target/debug/hp-tauri")
    );
  } else if (platform === "darwin") {
    return (
      process.env.TAURI_BINARY ||
      path.join(
        projectRoot,
        "target/debug/bundle/macos/House Puzzle.app/Contents/MacOS/House Puzzle"
      )
    );
  } else if (platform === "win32") {
    return (
      process.env.TAURI_BINARY ||
      path.join(projectRoot, "target/debug/hp-tauri.exe")
    );
  }
  throw new Error(`Unsupported platform: ${platform}`);
}

function getTauriDriverPath(): string {
  return (
    process.env.TAURI_DRIVER ||
    path.join(os.homedir(), ".cargo", "bin", "tauri-driver")
  );
}

/**
 * On Linux, tauri-driver proxies to WebKitWebDriver.
 * Pass the path via --native-driver so it is always found, even in CI
 * where it may not be on PATH.
 *
 * On macOS, Safari / WebKit is used (no extra flag needed).
 * On Windows, tauri-driver proxies to msedgedriver automatically.
 */
function getTauriDriverArgs(): string[] {
  if (process.platform === "linux") {
    const nativeDriver =
      process.env.WEBKIT_DRIVER ||
      "/usr/bin/WebKitWebDriver";
    return ["--native-driver", nativeDriver];
  }
  return [];
}

// ─── Process management ──────────────────────────────────────────────────────

let tauriDriver: ReturnType<typeof spawn> | undefined;
let driverShuttingDown = false;

function startTauriDriver() {
  driverShuttingDown = false;
  const driverPath = getTauriDriverPath();
  const driverArgs = getTauriDriverArgs();
  console.log(`[wdio] starting tauri-driver: ${driverPath} ${driverArgs.join(" ")}`);

  tauriDriver = spawn(driverPath, driverArgs, {
    stdio: [null, process.stdout, process.stderr],
  });

  tauriDriver.on("error", (error) => {
    console.error("[wdio] tauri-driver error:", error);
    process.exit(1);
  });

  tauriDriver.on("exit", (code) => {
    if (!driverShuttingDown) {
      console.error("[wdio] tauri-driver exited unexpectedly, code:", code);
      process.exit(1);
    }
  });
}

function stopTauriDriver() {
  driverShuttingDown = true;
  tauriDriver?.kill();
}

// ─── WebDriverIO configuration ───────────────────────────────────────────────

export const config: WebdriverIO.Config = {
  host: "127.0.0.1",
  port: 4444,
  specs: ["./specs/**/*.ts"],
  maxInstances: 1,

  capabilities: [
    {
      maxInstances: 1,
      "tauri:options": {
        application: getAppBinary(),
      },
    } as WebdriverIO.Capabilities,
  ],

  reporters: ["spec"],
  framework: "mocha",
  mochaOpts: {
    ui: "bdd",
    timeout: 60000,
  },

  beforeSession: () => startTauriDriver(),
  afterSession: () => stopTauriDriver(),
};

// Graceful shutdown on signals
["SIGINT", "SIGTERM", "exit"].forEach((sig) => {
  process.on(sig as NodeJS.Signals, stopTauriDriver);
});
