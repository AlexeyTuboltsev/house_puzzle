//! Regression test: re-run the bezier merge over the NY8 snapshot and assert
//! every piece's output is byte-identical to the captured golden.
//!
//! Refresh the golden when the algorithm intentionally changes:
//!   cargo run -p hp-testbed --bin hp-golden -- capture

use hp_core::bezier::BezierPath;
use hp_core::bezier_merge;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
struct BrickIn {
    id: String,
    #[serde(default)]
    name: String,
    beziers: Vec<BezierPath>,
}

#[derive(Debug, Clone, Deserialize)]
struct PieceIn {
    id: String,
    brick_ids: Vec<String>,
}

#[derive(Debug, Deserialize)]
struct Snapshot {
    bricks: Vec<BrickIn>,
    pieces: Vec<PieceIn>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Golden {
    pieces: BTreeMap<String, Vec<BezierPath>>,
}

fn repo_root() -> PathBuf {
    // CARGO_MANIFEST_DIR is the hp-testbed crate dir.
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn load_snapshot() -> Snapshot {
    let path = repo_root().join("testbed").join("snapshot.json");
    let raw = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}. Run `cargo run -p hp-testbed --bin hp-snapshot <NY8.ai>` first.", path.display()));
    serde_json::from_slice(&raw).expect("parsing snapshot.json")
}

fn compute_current(snap: &Snapshot) -> Golden {
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
    Golden { pieces }
}

#[test]
fn merge_piece_bezier_matches_golden_ny8() {
    let snap = load_snapshot();
    let current = compute_current(&snap);
    let current_bytes = serde_json::to_vec_pretty(&current).expect("serialize current");
    let path = repo_root().join("testbed").join("golden.json");
    let golden_bytes = std::fs::read(&path)
        .unwrap_or_else(|e| panic!("reading {}: {e}. Capture it with `cargo run -p hp-testbed --bin hp-golden -- capture`.", path.display()));
    if current_bytes == golden_bytes {
        return;
    }

    // Bytes differ — deserialize for a precise per-piece diff.
    let expected: Golden = serde_json::from_slice(&golden_bytes).expect("parsing golden.json");

    // Build a concise diff so failures are actionable.
    let mut diffs: Vec<String> = Vec::new();
    for (pid, exp) in &expected.pieces {
        match current.pieces.get(pid) {
            None => diffs.push(format!("{pid}: missing")),
            Some(cur) if cur != exp => {
                let mut first_diff = String::new();
                'find: for (pi, (ep, cp)) in exp.iter().zip(cur.iter()).enumerate() {
                    if ep.start != cp.start {
                        first_diff = format!(
                            "path{pi} start: golden={:?} now={:?}",
                            ep.start, cp.start
                        );
                        break 'find;
                    }
                    for (si, (es, cs)) in ep.segments.iter().zip(cp.segments.iter()).enumerate() {
                        if es != cs {
                            first_diff = format!(
                                "path{pi} seg{si}: golden={:?} now={:?}",
                                es, cs
                            );
                            break 'find;
                        }
                    }
                }
                diffs.push(format!(
                    "{pid}: {} path(s) / {} seg(s). First diff: {}",
                    exp.len(),
                    exp.iter().map(|p| p.segments.len()).sum::<usize>(),
                    first_diff
                ));
            }
            _ => {}
        }
    }
    for pid in current.pieces.keys() {
        if !expected.pieces.contains_key(pid) {
            diffs.push(format!("{pid}: new piece"));
        }
    }
    panic!(
        "{} piece(s) differ from golden:\n  {}",
        diffs.len(),
        diffs.join("\n  ")
    );
}
