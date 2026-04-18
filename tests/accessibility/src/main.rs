//! E2E test runner for House Puzzle — uses the `tauri-ui-test` library.

use std::path::Path;
use std::{env, fs};
use tauri_ui_test::App;

fn main() {
    let args: Vec<String> = env::args().collect();
    let binary = find_arg(&args, "--binary").expect("--binary <path> required");
    let fixture_dir = find_arg(&args, "--fixture-dir").expect("--fixture-dir <path> required");
    let screenshots_dir = find_arg(&args, "--screenshots").unwrap_or_else(|| "screenshots".into());

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
    app.sleep(5);

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

    // === Verify ===
    let mut pass = true;
    for name in &["initial-state", "house-loaded", "puzzle-generated"] {
        let path = Path::new(&screenshots_dir).join(format!("{name}.png"));
        if path.exists() && fs::metadata(&path).map(|m| m.len() > 0).unwrap_or(false) {
            let size = fs::metadata(&path).unwrap().len();
            println!("  OK: {name}.png ({size} bytes)");
        } else {
            println!("  FAIL: {name}.png missing or empty");
            pass = false;
        }
    }

    app.close();

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
