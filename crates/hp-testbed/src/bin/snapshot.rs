//! One-time snapshot: parse the supplied AI file, run the puzzle merge at
//! fixed seed/target, and dump the data the testbed needs.
//!
//! Usage: `hp-snapshot <path-to-ai> [--out ./testbed] [--target 120] [--seed 42]`.

use anyhow::{Context, Result, bail};
use hp_testbed::build_snapshot;
use std::path::PathBuf;

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let ai_path = PathBuf::from(args.next().context("missing AI file argument")?);
    let mut out_dir = PathBuf::from("testbed");
    let mut target: usize = 120;
    let mut seed: u64 = 42;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--out" => out_dir = PathBuf::from(args.next().context("--out needs a value")?),
            "--target" => target = args.next().context("--target needs value")?.parse()?,
            "--seed" => seed = args.next().context("--seed needs value")?.parse()?,
            other => bail!("unknown arg: {other}"),
        }
    }
    std::fs::create_dir_all(&out_dir).ok();

    eprintln!("[snapshot] parsing {:?}", ai_path);
    let snap = build_snapshot(&ai_path, target, seed)?;
    eprintln!(
        "[snapshot] {} bricks, {} pieces, canvas {}x{}",
        snap.bricks.len(),
        snap.pieces.len(),
        snap.transform.canvas_width,
        snap.transform.canvas_height
    );

    let name = ai_path.file_stem().and_then(|s| s.to_str()).unwrap_or("snapshot");
    let out_path = out_dir.join(format!("{name}.json"));
    std::fs::write(&out_path, serde_json::to_vec_pretty(&snap)?)?;
    eprintln!("[snapshot] wrote {}", out_path.display());

    // Also write a stable alias if the repo already expects `snapshot.json`
    // (regression tests + golden.json were built against it).
    let legacy = out_dir.join("snapshot.json");
    if !legacy.exists() || name == "_NY8" {
        std::fs::write(&legacy, serde_json::to_vec_pretty(&snap)?)?;
    }
    Ok(())
}
