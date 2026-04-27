# Plan — AI file validator/normalizer

Phased build of the Illustrator script. Each phase finishes with a
specific, testable artefact; resist the temptation to start the next
phase before the current one passes its fixtures.

## Phase 0 — Scaffold + DOM snapshot (≤ half a day)

**Goal:** end-to-end dev loop is alive; `validate.jsx` walks the active
document and emits a JSON dump of every brick's sub-paths.

- [x] `run.sh` — osascript driver (open file → run script → cat report)
- [x] `validate.jsx` — entry point; reads `/tmp/ai-validate-mode.txt`,
      writes `/tmp/ai-validate-report.json` skeleton
- [x] `lib/walk_paths.jsx` — for each layer that holds bricks, emit
      `{ id, sub_paths: [{ closed, anchors: [[x, y], ...] }] }`
- [ ] Verify on `_NY9n.ai`: 4 sub-paths come back with `closed: false`.
- [ ] Verify on `_NY1.ai`: every brick layer is enumerated.
- [ ] Pin down the actual layer hierarchy. The Rust parser walks
      `%AI5_BeginLayer` markers; the Illustrator DOM exposes a layer
      tree. Confirm: are bricks nested under a single `bricks` layer,
      or top-level by name? Adjust `walk_paths.jsx` once.

**Done when:** `./run.sh ../../in/_NY9n.ai` produces a JSON snapshot in
which the four known-broken bricks have `closed: false`.

## Phase 1 — Read-only checks (1–2 days)

**Goal:** the script reports every AI-source bug from `todo.md`.
Never modifies the document.

Each check is a separate function returning  
`[{ severity: "error" | "warning", kind, brick, message, ... }]`.

Highest value first:

- [ ] **Layer structure** — `bricks`, `background`, `screen` layers exist
      and are non-empty.
- [ ] **Unclosed path** — start anchor ≠ last segment endpoint (gap > ε).
      Test fixture: `_NY9n.ai` (4 expected).
- [ ] **Degenerate path** — < 3 anchors, or signed area < 1 pymu².
- [ ] **Multi-grid drift across bricks** — cluster every anchor's x and
      y separately within `< 1 pymu`; flag any cluster spanning > 0.1
      pymu, listing every brick that contributed. Test fixtures:
      `_NY1.ai` (b172/b173 vs b154-159), `_NY5.ai` (y-grids).
- [ ] **Intra-brick cluster drift** — same idea, per brick. Catches
      NY7 b334's outer-vs-inner y mismatch (520.00 vs 519.14).
- [ ] **Adjacent-brick corner jitter** — vertices within 1 pymu of each
      other across two different bricks but not bit-identical (NY7 b334
      ↔ b366 v1, 0.29 pymu apart). Use vector adjacency to constrain;
      flag pairs whose nominal shared corner mismatches > 0.05 pymu.
- [ ] **Sub-pymu staircase edge inside a brick** — single sub-path
      contains an edge < 1 pymu long. Test fixture: `_NY5.ai` b022/b027.
- [ ] **Multi-object layer with independent sub-paths** — already in
      `todo.md` backlog; flag if a single brick layer contains
      sub-paths whose bboxes don't overlap.
- [ ] **Brick overlap / containment** — slowest, last. Pairwise polygon
      overlap (warn on bbox containment, error on > 1 pymu² polygon
      intersection).

**Done when:** each fixture file produces the exact set of expected
findings (no false positives, no missed cases).

## Phase 2 — Auto-fix for the deterministic cases (1–2 days)

`./run.sh <file> --fix` modifies the document in place. The artist
saves manually after reviewing.

Three safe fixes:

- [ ] **Close open paths.** Set `pathItem.closed = true` after appending
      a single straight closing line if start ≠ last anchor. Test:
      `_NY9n.ai` produces 0 findings after a `--fix` round-trip.
- [ ] **Snap drift clusters.** For every cluster identified in Phase 1's
      multi-grid check, pick the most-popular value as the anchor for
      the cluster (median if it ties), and shift every contributing
      vertex *and* its corresponding brick raster by the delta. Tests:
      `_NY1.ai`, `_NY5.ai`, `_NY7.ai`.
- [ ] **Snap adjacent-brick jitter.** Close vertices on a vector-shared
      edge (within 0.5 pymu) snap to the cluster's most-popular value.
      Same vector + raster move rule.

Hard rule: every other check from Phase 1 reports as warning only —
overlaps, containment, ambiguous staircases — never auto-fixed. The
artist resolves those by hand and re-runs.

**Done when:** running `--fix` on each test fixture clears every
`error`-level finding; the parent Rust project's testbed renders the
fixed file with the corresponding runtime workaround disabled.

## Phase 3 — Artist UX (optional, only if needed)

If the artist runs the script manually rather than as a CI step:

- [ ] One modal dialog at end of run: "N issues found, M auto-fixable.
      [Apply fixes] [Save report] [Cancel]"
- [ ] Click-to-navigate: report rows have a "go to brick" button that
      selects the offending `pathItem` in the active document.
- [ ] Per-issue accept/reject for ambiguous cases.

Skip until Phases 0–2 are stable on every fixture.

## Phase 4 — Wire into the Rust pipeline (½ day)

- [ ] CI step that fails the build if `validate.jsx` would emit any
      `error`-level finding for a tracked AI file.
- [ ] Remove the four runtime workarounds listed in `../../todo.md`
      § "Remove runtime workarounds once AI normalisation is in place"
      one at a time, regenerating the testbed snapshot/golden after
      each removal.

## Done criteria summary

| Phase | Pass condition                                                  |
|-------|------------------------------------------------------------------|
| 0     | JSON dump matches DOM for one known file.                       |
| 1     | Each fixture produces expected findings, no false positives.    |
| 2     | `--fix` clears all `error` findings, runtime workaround safe to remove. |
| 3     | Artist can run interactively without leaving Illustrator.       |
| 4     | All four runtime workarounds removed; testbed green.            |
