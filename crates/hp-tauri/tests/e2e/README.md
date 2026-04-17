# House Puzzle – Tauri E2E Tests

End-to-end test suite for the [House Puzzle](../../) Tauri desktop app.

**Stack:** [WebDriverIO](https://webdriver.io/) + [`tauri-driver`](https://crates.io/crates/tauri-driver) + [pixelmatch](https://github.com/mapbox/pixelmatch)

---

## Prerequisites

| Requirement | Install |
|---|---|
| Rust + Cargo | https://rustup.rs |
| `tauri-driver` | `cargo install tauri-driver` |
| Node.js ≥ 18 | https://nodejs.org |
| Linux: WebKitWebDriver | `sudo apt install webkit2gtk-driver xvfb` |
| macOS: SafariDriver | enabled via `safaridriver --enable` |
| Windows: msedgedriver | bundled with Edge; ensure on PATH |

---

## Running locally

```bash
# 1. Build the app (debug profile)
cd /path/to/house_puzzle
cargo build -p hp-tauri

# 2. Install test deps
cd crates/hp-tauri/tests/e2e
npm install

# 3. Run tests
#   Linux headless:
xvfb-run --auto-servernum --server-args="-screen 0 1280x800x24" npx wdio run wdio.conf.ts

#   macOS / Windows:
npx wdio run wdio.conf.ts
```

### Environment variables

| Variable | Default | Description |
|---|---|---|
| `TAURI_BINARY` | `target/debug/hp-tauri` | Path to the compiled app binary |
| `TAURI_DRIVER` | `~/.cargo/bin/tauri-driver` | Path to tauri-driver |
| `WEBKIT_DRIVER` | `/usr/bin/WebKitWebDriver` | Path to WebKitWebDriver (Linux) |

---

## Screenshot baselines

On first run a baseline PNG is captured per platform (`screenshots/baselines/<name>-<platform>.png`).  
Subsequent runs compare against the baseline with ≤ 2 % pixel tolerance.

To update a baseline, delete the file and re-run.

---

## Directory layout

```
tests/e2e/
├── wdio.conf.ts          # WebDriverIO config (tauri-driver glue)
├── specs/
│   └── app.test.ts       # Test specs
├── screenshots/
│   ├── baselines/        # Committed reference screenshots per platform
│   └── actual/           # Generated on each run (git-ignored)
├── package.json
└── tsconfig.json
```
