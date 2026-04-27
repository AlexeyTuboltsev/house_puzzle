# AI file validator/normalizer (Illustrator script)

ExtendScript (`.jsx`) for Adobe Illustrator that validates and (optionally)
normalises the `.ai` files in `house_puzzle/in/` before the Rust pipeline
ingests them.

The Rust runtime currently masks several AI-source bugs with workarounds
(see `../../todo.md` § "Remove runtime workarounds once AI normalisation
is in place"). This script is the upstream fix — once it ships and every
shipped `.ai` has been run through it, those workarounds get torn out.

## Environment

- macOS (Intel)
- Adobe Illustrator **26.0.2** (CC 2022). Other CC/CS6 versions also work.
- ExtendScript engine (legacy JSX, ES3-ish — see *Conventions* below).
- Driven from the shell via `osascript`. Illustrator must be running.

## Dev loop (the whole iteration cycle)

```sh
./run.sh ../../in/_NY9n.ai            # report mode (read-only)
./run.sh ../../in/_NY9n.ai --fix       # fix mode (modifies the document)
```

`run.sh`:
1. Tells Illustrator to open the file (if not already).
2. Writes the requested mode to `/tmp/ai-validate-mode.txt`.
3. Runs `validate.jsx` via `do javascript file`.
4. Pretty-prints `/tmp/ai-validate-report.json`.

`do javascript file` re-reads the JSX from disk on every invocation, so
edit any `.jsx` and rerun — no Illustrator restart, no panel reload, no
manual import.

## Coding conventions (ExtendScript engine)

ExtendScript is ES3-ish with a few CC-era extensions. **Test your code
runs at the top level of `validate.jsx` before assuming a feature works.**

- ✅ `var`, classic `function` declarations, `for (var i=0; i<arr.length; i++)`
- ✅ `try / catch / throw`
- ✅ `JSON.stringify` / `JSON.parse` (CC 2014+, fine for 26)
- ✅ `File` / `Folder` for I/O (encoding defaults to UTF-8 if you set it)
- ❌ `let`, `const`, arrow functions, template literals, `for...of`
- ❌ `Array.prototype.includes`, `Array.from`, `Object.assign`, spread
- ❌ `Promise`, `async/await`, anything event-loop-shaped
- ❌ `alert()` in headless mode — it blocks Illustrator and breaks the loop

Use `indexOf`, `concat`, plain object literals, classic for-loops.

## Hard rules

- **`report` mode never modifies the document.** Read-only. Even when a
  finding looks safe, no in-document mutations until `--fix` is on.
- **`fix` mode only does deterministic fixes.** Anything ambiguous (overlap,
  containment, decisions that need artist judgement) stays in the report
  as a warning.
- **Vector and raster move together.** When `fix` mode snaps a vertex,
  the corresponding brick raster image must shift by the same delta in the
  same brick layer (`RasterItem.position`). Vectors-only is the bug we're
  trying to fix; never reintroduce it.
- **One JSON report per run** at `/tmp/ai-validate-report.json`. No
  alerts, no console spam. The report is the API.

## File layout

```
tools/ai-validate/
├── CLAUDE.md       # this file — read first
├── plan.md         # phased build plan
├── run.sh          # osascript driver (./run.sh <ai-file> [--fix])
├── validate.jsx    # entry point
└── lib/
    └── walk_paths.jsx   # DOM → JSON snapshot helper
```

## Test fixtures

The known-bad files (in `house_puzzle/in/`):

| File       | Bug class                       | Bricks                       |
|------------|---------------------------------|------------------------------|
| `_NY9n.ai` | Unclosed sub-paths              | b012, b020, b022, b026       |
| `_NY1.ai`  | Multi-grid x-drift              | b172/b173 (.7462) vs b154-159 (.0085) |
| `_NY5.ai`  | Multi-grid y-drift + staircase  | b022/b027 etc                |
| `_NY7.ai`  | Intra-brick cluster drift       | b334 (outer y=520, inner y=519.14)<br>b366 v1 vs b334 (0.29 pymu jitter) |

Each phase is "done" when the corresponding test fixtures produce the
expected report (and `--fix` heals them, in Phase 2+). Reach over to
`../../crates/hp-testbed/` and rerun the testbed against the fixed file
to confirm the runtime workaround can be safely removed.

## Reading the report

The script writes to `/tmp/ai-validate-report.json`. Schema (subject to
change as phases land):

```json
{
  "file": "/abs/path/to/_NY9n.ai",
  "mode": "report",
  "version": 0,
  "findings": [
    { "severity": "error", "kind": "unclosed_path", "brick": "b012",
      "sub_path": 0, "gap_pymu": 50.0,
      "start": [359.75, 5718.53], "end": [359.75, 5668.53] }
  ],
  "snapshot": { ... raw DOM dump for debugging ... }
}
```

## Pointers into the parent project

- `../../todo.md` § Adobe Illustrator validation script — full backlog
- `../../crates/hp-core/src/ai_parser.rs` — Rust parser the script must
  produce inputs that survive
- `../../crates/hp-core/src/bezier_merge.rs` — runtime merge whose
  workarounds we're targeting
