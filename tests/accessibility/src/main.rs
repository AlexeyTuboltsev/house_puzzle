//! E2E test runner for House Puzzle — uses the `tauri-ui-test` library.
//!
//! After driving the app through the load/generate flow, compares each
//! captured screenshot against a per-platform baseline PNG. A pixel is
//! "different" if any RGBA channel deviates by more than CHANNEL_TOL;
//! the run fails if more than PIXEL_PCT_TOL of pixels are different.

use std::path::{Path, PathBuf};
use std::{env, fs};
use tauri_ui_test::App;

/// Max per-channel delta (0..=255) for a pixel to be considered "same".
/// Gives some slack for compression artefacts and minor antialiasing
/// noise between identical runs.
const CHANNEL_TOL: u8 = 16;

/// Allowed fraction of differing pixels per screenshot before the
/// comparison is treated as a regression.
const PIXEL_PCT_TOL: f64 = 0.01; // 1 %

fn main() {
    let args: Vec<String> = env::args().collect();
    let binary = find_arg(&args, "--binary").expect("--binary <path> required");
    let fixture_dir = find_arg(&args, "--fixture-dir").expect("--fixture-dir <path> required");
    let screenshots_dir = find_arg(&args, "--screenshots").unwrap_or_else(|| "screenshots".into());
    let baselines_dir = find_arg(&args, "--baselines");
    let mask_bottom: u32 = find_arg(&args, "--mask-bottom")
        .and_then(|s| s.parse().ok())
        .unwrap_or(0);

    fs::create_dir_all(&screenshots_dir).expect("Failed to create screenshots dir");

    // Copy fixture to in/ (skip if already there)
    let project_root = env::current_dir().expect("Failed to get cwd");
    let in_dir = project_root.join("in");
    fs::create_dir_all(&in_dir).ok();
    let fixture_src = Path::new(&fixture_dir).join("_NY2.ai");
    let fixture_dst = in_dir.join("_NY2.ai");
    if fixture_src.exists() && fixture_src.canonicalize().ok() != fixture_dst.canonicalize().ok() {
        fs::copy(&fixture_src, &fixture_dst).ok();
        println!("[test] Copied _NY2.ai to in/");
    } else if fixture_dst.exists() {
        println!("[test] _NY2.ai already in in/");
    }

    // Debug: verify fixture is in place
    let in_contents: Vec<_> = fs::read_dir(&in_dir)
        .into_iter()
        .flatten()
        .flatten()
        .map(|e| e.file_name().to_string_lossy().to_string())
        .collect();
    println!("[test] CWD: {}", project_root.display());
    println!("[test] in/ contents: {:?}", in_contents);

    // Launch app
    let mut app = App::launch(&binary, &project_root);
    app.wait_for_window(60);
    // Pre-screenshot settle: `wait_for_window` only waits for the Tauri
    // window to exist, not for the Elm bundle to finish loading inside
    // the WebView. On a cold Windows runner the JS/wasm boot can take
    // 10+ s, and capturing at 5 s caught a blank grey window once
    // (Run #25226321551). Give it room.
    app.sleep(15);

    // === Test flow ===

    // 1. Initial state
    app.screenshot(&screenshots_dir, "initial-state");

    // 2. Load file
    println!("[test] Step: load _NY2.ai");
    app.click_button("_NY2");
    app.sleep(30);
    app.screenshot(&screenshots_dir, "house-loaded");

    // 3. Generate puzzle
    println!("[test] Step: generate puzzle");
    app.click_button("Import");
    app.sleep(2);
    app.click_button("Generate Puzzle");
    app.sleep(30);

    // 4. View pieces
    println!("[test] Step: view pieces");
    app.click_button("Pieces");
    app.sleep(3);
    app.screenshot(&screenshots_dir, "puzzle-generated");

    app.close();

    // === Verify ===
    let mut pass = true;
    let baselines = baselines_dir.as_ref().map(PathBuf::from);
    if baselines.is_none() {
        println!("[test] No --baselines provided; comparing only that screenshots are non-empty.");
    }

    for name in &["initial-state", "house-loaded", "puzzle-generated"] {
        let actual = Path::new(&screenshots_dir).join(format!("{name}.png"));
        if !actual.exists() || fs::metadata(&actual).map(|m| m.len() == 0).unwrap_or(true) {
            println!("  FAIL: {name}.png missing or empty");
            pass = false;
            continue;
        }
        let size = fs::metadata(&actual).unwrap().len();
        println!("  OK file: {name}.png ({size} bytes)");

        if let Some(base_dir) = &baselines {
            let baseline = base_dir.join(format!("{name}.png"));
            if !baseline.exists() {
                println!("  WARN: no baseline for {name} at {}", baseline.display());
                continue;
            }
            match compare_pngs(&actual, &baseline, &screenshots_dir, name, mask_bottom) {
                Ok(report) => {
                    let pct = report.diff_pct();
                    let status = if pct <= PIXEL_PCT_TOL { "PASS" } else { "FAIL" };
                    println!(
                        "  {status} compare: {name} -- diff {} / {} px ({:.3}% , tol {:.2}%)",
                        report.diff_pixels,
                        report.total_pixels,
                        pct * 100.0,
                        PIXEL_PCT_TOL * 100.0,
                    );
                    if pct > PIXEL_PCT_TOL {
                        pass = false;
                    }
                }
                Err(e) => {
                    println!("  FAIL compare: {name} -- {e}");
                    pass = false;
                }
            }
        }
    }

    if pass {
        println!("[test] PASSED");
    } else {
        println!("[test] FAILED");
        std::process::exit(1);
    }
}

fn find_arg(args: &[String], flag: &str) -> Option<String> {
    args.windows(2).find(|w| w[0] == flag).map(|w| w[1].clone())
}

struct DiffReport {
    diff_pixels: u64,
    total_pixels: u64,
}

impl DiffReport {
    fn diff_pct(&self) -> f64 {
        if self.total_pixels == 0 {
            0.0
        } else {
            self.diff_pixels as f64 / self.total_pixels as f64
        }
    }
}

fn compare_pngs(
    actual_path: &Path,
    baseline_path: &Path,
    out_dir: &str,
    step_name: &str,
    mask_bottom: u32,
) -> Result<DiffReport, String> {
    let actual = image::open(actual_path)
        .map_err(|e| format!("read {}: {e}", actual_path.display()))?
        .to_rgba8();
    let baseline = image::open(baseline_path)
        .map_err(|e| format!("read {}: {e}", baseline_path.display()))?
        .to_rgba8();

    if actual.dimensions() != baseline.dimensions() {
        return Err(format!(
            "size mismatch: actual {:?} vs baseline {:?}",
            actual.dimensions(),
            baseline.dimensions()
        ));
    }

    let (w, h) = actual.dimensions();
    // `mask_bottom` rows at the bottom are excluded from the diff —
    // used on Windows to skip the OS taskbar (the clock keeps
    // ticking between runs and would flake the comparison).
    let mask_bottom = mask_bottom.min(h);
    let compare_h = h - mask_bottom;
    let total_pixels = w as u64 * compare_h as u64;
    let mut diff_pixels: u64 = 0;
    let mut diff_img = image::RgbaImage::new(w, h);

    for y in 0..h {
        let in_mask = y >= compare_h;
        for x in 0..w {
            let a = actual.get_pixel(x, y).0;
            let b = baseline.get_pixel(x, y).0;
            let max_delta = a
                .iter()
                .zip(b.iter())
                .map(|(av, bv)| av.abs_diff(*bv))
                .max()
                .unwrap_or(0);
            if in_mask {
                // Visualise the masked region as a grey strip in the
                // diff PNG so it's obvious from the artefact what was
                // skipped.
                diff_img.put_pixel(x, y, image::Rgba([60, 60, 60, 255]));
                continue;
            }
            if max_delta > CHANNEL_TOL {
                diff_pixels += 1;
                // Bright magenta on diff so it's eye-catching in artefacts.
                diff_img.put_pixel(x, y, image::Rgba([255, 0, 255, 255]));
            } else {
                // Dim the unchanged actual pixel for context.
                let p = actual.get_pixel(x, y).0;
                diff_img.put_pixel(
                    x,
                    y,
                    image::Rgba([p[0] / 4, p[1] / 4, p[2] / 4, 255]),
                );
            }
        }
    }

    let diff_path = Path::new(out_dir).join(format!("{step_name}.diff.png"));
    diff_img
        .save(&diff_path)
        .map_err(|e| format!("save diff {}: {e}", diff_path.display()))?;

    // Also drop the baseline next to the actual + diff so the gh-pages
    // grid can show an "expected / received / diff" trio per step.
    let expected_path = Path::new(out_dir).join(format!("{step_name}.expected.png"));
    fs::copy(baseline_path, &expected_path)
        .map_err(|e| format!("copy baseline -> {}: {e}", expected_path.display()))?;

    Ok(DiffReport {
        diff_pixels,
        total_pixels,
    })
}
