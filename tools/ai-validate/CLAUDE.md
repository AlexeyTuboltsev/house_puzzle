# AI file validator/normalizer (Illustrator script)

ExtendScript (`.jsx`) for Adobe Illustrator that validates and (optionally)
normalises the `.ai` files in `house_puzzle/in/` before the Rust pipeline
ingests them.

## Tool Usage Rules — Always Follow

(Inherited from the parent project's `CLAUDE.md` — repeated here so a
fresh Claude Code session opened directly in `tools/ai-validate/` picks
them up without searching upward.)

### Code Analysis
- ALWAYS use LSP tools for code analysis, diagnostics, type checking, and symbol resolution.
- Never guess at types, definitions, or errors when LSP tools are available. Use them first.
- If LSP tools are unavailable or throw an auth error: STOP and ask the user what to do.
  Do not fall back to any other method.
- Caveat: ExtendScript (`.jsx`) has no first-class LSP. The editor's TypeScript
  resolver may flag spurious errors on `.jsx` files; ignore those. For real type
  checking, the only ground truth is running the script in Illustrator and
  reading the JSON report.

### Web Search
- ALWAYS use Firecrawl for any web search, URL fetching, or documentation lookup.
- Do not use generic Bash curl/wget for web content retrieval if Firecrawl is available.
- If Firecrawl is unavailable or throws an auth error: STOP and ask the user how to proceed.
  Do not fall back to any other method until explicitly told to do so.
- Useful targets when stuck on the Illustrator API: Adobe's
  [Illustrator Scripting Reference](https://ai-scripting.docsforadobe.dev/),
  the ExtendScript Toolkit docs, and Adobe Forum threads.

### Git Operations
- ALL git write operations (commits, push, PRs, issues, releases) go through the `github` MCP server.
- The github MCP server is pre-authenticated as the bot (`k5qkop-bot`) via `GIT_BOT_TOKEN`.
- Never use raw `git` bash commands or `gh` CLI for write operations unless MCP is unavailable.
- If you must fall back to bash git/gh: the PreToolUse hook will automatically inject bot identity.
  You do NOT need to set git config or switch credentials manually.
- All commits must appear as `k5qkop-bot`. Never commit under the user's personal identity.
- If the github MCP server is unavailable or throws an auth error: STOP and tell the user.
  Do not fall back to any other method without explicit permission.
- Before EVERY push: run `git fetch origin`, then `git branch --merged origin/main`.
  Block if the current branch is already merged. Do this IMMEDIATELY before pushing —
  never assume a branch is unmerged based on an earlier check, even from the same session.
- Before adding commits to an EXISTING PR branch: run `git fetch origin`, then
  `git ls-remote --heads origin <branch>`. If the remote branch is gone, the PR
  was merged and deleted — create a new branch from origin/main instead.
  Do this IMMEDIATELY before every push to an existing branch.
- NEVER force-push (`--force`, `--force-with-lease`) unless the user explicitly allows it.
  Always make new commits instead of amending/rebasing pushed branches.

### Bash Commands
- NEVER chain multiple commands with `&&`, `||`, or `;` in a single Bash tool call.
- Run each command as a separate Bash tool call so that whitelisted commands
  don't require manual approval.
- If commands are independent, run them as parallel tool calls in the same message.

## AWS Documentation

When working on any AWS-related tasks, always use the `awslabs-aws-documentation-mcp-server`
and `awslabs-core-mcp-server` MCP tools before responding. Use them to look up service
documentation, API references, and best practices rather than relying solely on training
knowledge — AWS APIs and features change frequently and the MCP servers always reflect
the latest guidance. (Unlikely to come up in this sub-project, but standing rule still
applies.)

## Environment & Installation Rules

### Containers do NOT apply to this sub-project
The parent project's standing rule is "default to running in a container, never
install on the host." That rule **does not apply here**: this script must run
inside Adobe Illustrator on the artist's macOS host. There is no container
form-factor for Illustrator. The whole point of `tools/ai-validate/` is to
drive the host's Illustrator process via `osascript`. So:

- Edit and run on the macOS host.
- Do NOT propose Dockerizing this script.
- Do NOT install ExtendScript-specific tooling system-wide; the script
  itself is a single `.jsx` plus a few helpers, no install step needed.
- The parent project's normal "no host installs" rule still applies for
  ANY OTHER side-tools you might be tempted to introduce — keep this
  sub-project to ExtendScript + a tiny shell wrapper.

These are standing instructions. Do not wait to be reminded. Apply them every session.

---


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
