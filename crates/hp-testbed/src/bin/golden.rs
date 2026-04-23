//! Capture or verify the golden per-piece merge output for the NY8 snapshot.
//!
//! Usage:
//!   hp-golden capture  — write `testbed/golden.json` from the current algo
//!   hp-golden verify   — re-run the algo, diff against `testbed/golden.json`,
//!                         exit non-zero on any difference. Prints a summary.

use anyhow::{Context, Result, bail};
use hp_core::bezier::BezierPath;
use hp_core::bezier_merge;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize, Serialize)]
struct BrickIn {
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    layer_type: String,
    beziers: Vec<BezierPath>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct PieceIn {
    id: String,
    brick_ids: Vec<String>,
    #[serde(default)]
    x: i32,
    #[serde(default)]
    y: i32,
    #[serde(default)]
    width: i32,
    #[serde(default)]
    height: i32,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    bricks: Vec<BrickIn>,
    pieces: Vec<PieceIn>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Golden {
    /// Ordered by piece id for stable diffs.
    pieces: BTreeMap<String, Vec<BezierPath>>,
}

fn compute_golden(snapshot_path: &PathBuf) -> Result<Golden> {
    let raw = std::fs::read(snapshot_path)
        .with_context(|| format!("reading {}", snapshot_path.display()))?;
    let snap: Snapshot = serde_json::from_slice(&raw).context("parsing snapshot")?;
    let bricks: BTreeMap<&str, &BrickIn> =
        snap.bricks.iter().map(|b| (b.id.as_str(), b)).collect();

    let mut pieces: BTreeMap<String, Vec<BezierPath>> = BTreeMap::new();
    for piece in &snap.pieces {
        let mut input: Vec<BezierPath> = Vec::new();
        for bid in &piece.brick_ids {
            if let Some(b) = bricks.get(bid.as_str()) {
                input.extend(b.beziers.iter().cloned());
            }
        }
        pieces.insert(piece.id.clone(), bezier_merge::merge_piece_bezier(&input));
    }
    Ok(Golden { pieces })
}

fn main() -> Result<()> {
    let mut args = std::env::args().skip(1);
    let cmd = args.next().unwrap_or_else(|| "verify".to_string());
    let snapshot_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/hp-testbed/testbed/snapshot.json"));
    let golden_path = args
        .next()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("crates/hp-testbed/testbed/golden.json"));

    match cmd.as_str() {
        "capture" => {
            let g = compute_golden(&snapshot_path)?;
            let body = serde_json::to_vec_pretty(&g)?;
            std::fs::write(&golden_path, &body)?;
            eprintln!(
                "[golden] wrote {} ({} pieces, {} bytes)",
                golden_path.display(),
                g.pieces.len(),
                body.len()
            );
            Ok(())
        }
        "verify" => {
            let current = compute_golden(&snapshot_path)?;
            let raw = std::fs::read(&golden_path)
                .with_context(|| format!("reading {}", golden_path.display()))?;
            let expected: Golden = serde_json::from_slice(&raw)?;
            let diffs = diff_golden(&expected, &current);
            if diffs.is_empty() {
                eprintln!(
                    "[golden] OK — {} pieces match",
                    current.pieces.len()
                );
                Ok(())
            } else {
                for d in &diffs {
                    eprintln!("[golden] DIFF {d}");
                }
                bail!("{} piece(s) differ from golden", diffs.len());
            }
        }
        other => bail!("unknown subcommand: {other} (want 'capture' or 'verify')"),
    }
}

fn diff_golden(expected: &Golden, actual: &Golden) -> Vec<String> {
    let mut out = Vec::new();
    for (pid, exp) in &expected.pieces {
        match actual.pieces.get(pid) {
            None => out.push(format!("{pid}: missing in current output")),
            Some(cur) if cur != exp => {
                out.push(format!(
                    "{pid}: {} paths in golden, {} in current (segments: {} vs {})",
                    exp.len(),
                    cur.len(),
                    exp.iter().map(|p| p.segments.len()).sum::<usize>(),
                    cur.iter().map(|p| p.segments.len()).sum::<usize>()
                ));
            }
            Some(_) => {}
        }
    }
    for pid in actual.pieces.keys() {
        if !expected.pieces.contains_key(pid) {
            out.push(format!("{pid}: new piece (not in golden)"));
        }
    }
    out
}
