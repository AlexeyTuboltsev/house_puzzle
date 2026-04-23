//! Determinism check — run the same merge twice, compare byte-for-byte.

use hp_core::bezier::BezierPath;
use hp_core::bezier_merge;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Deserialize)]
struct BrickIn {
    id: String,
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

fn run() -> BTreeMap<String, Vec<BezierPath>> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("testbed")
        .join("snapshot.json");
    let snap: Snapshot = serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
    let bricks: BTreeMap<&str, &BrickIn> =
        snap.bricks.iter().map(|b| (b.id.as_str(), b)).collect();
    let mut pieces = BTreeMap::new();
    for piece in &snap.pieces {
        let mut input: Vec<BezierPath> = Vec::new();
        for bid in &piece.brick_ids {
            if let Some(b) = bricks.get(bid.as_str()) {
                input.extend(b.beziers.iter().cloned());
            }
        }
        pieces.insert(piece.id.clone(), bezier_merge::merge_piece_bezier(&input));
    }
    pieces
}

#[test]
fn merge_is_deterministic() {
    let a = run();
    let b = run();
    let mut diffs = Vec::new();
    for (pid, va) in &a {
        let vb = b.get(pid).unwrap();
        if va != vb {
            diffs.push(pid.clone());
        }
    }
    assert!(
        diffs.is_empty(),
        "merge output differs between two runs for {} pieces: {:?}",
        diffs.len(),
        &diffs[..diffs.len().min(10)]
    );
}
