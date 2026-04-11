## Tool Usage Rules — Always Follow

### Code Analysis
- ALWAYS use LSP tools for code analysis, diagnostics, type checking, and symbol resolution.
- Never guess at types, definitions, or errors when LSP tools are available. Use them first.
- If LSP tools are unavailable or throw an auth error: STOP and ask the user what to do.
  Do not fall back to any other method.

### Web Search
- ALWAYS use Firecrawl for any web search, URL fetching, or documentation lookup.
- Do not use generic Bash curl/wget for web content retrieval if Firecrawl is available.
- If Firecrawl is unavailable or throws an auth error: STOP and ask the user how to proceed.
  Do not fall back to any other method until explicitly told to do so.

### Git Operations
- ALL git write operations (commits, push, PRs, issues, releases) go through the `github` MCP server.
- The github MCP server is pre-authenticated as the bot (`k5qkop-bot`) via GIT_BOT_TOKEN.
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
the latest guidance. For any task involving AWS services, infrastructure, SDKs, or CLI
commands, consult these tools first, even if you believe you already know the answer.

## Environment & Installation Rules

### Never install directly on the host system
- If ANY task requires installing packages, runtimes, compilers, dependencies, or system tools,
  ALWAYS assume the work should happen inside a container (Docker or similar).
- Do NOT run `apt install`, `brew install`, `npm install -g`, `pip install` (system-wide),
  or any other system-level installation directly on the host machine.
- Instead: automatically propose a Dockerfile or docker-compose.yml that covers the requirement,
  and wait for approval before proceeding.
- This applies even if the install command looks harmless or temporary.
- When in doubt, ask "should this go in a container?" — the default answer is YES.

### Detect and respect existing container setup
- At the start of any task, check if a Dockerfile, docker-compose.yml, or .dockerignore
  exists in the repo root or any parent directory.
- If found AND the task involves running, building, installing, or testing anything:
  STOP and ask before proceeding.
- Do not assume the answer is yes automatically — always ask explicitly, every time.
- Only proceed after receiving a clear answer.
- If the answer is yes: all commands, builds, installs, and test runs must happen
  inside that container, not on the host.


## Unity is NOT Under Your Full Control

Unity Editor is an external system. Commands can fail silently, state can be stale,
and actions may not take effect when you expect. ALWAYS follow this pattern for
EVERY Unity interaction:

1. **CHECK STATE BEFORE** — Before any Unity action, verify the current state.
   Is the game running? Is a compile pending? Is the editor responsive?
   Use `manage_editor`, `read_console`, `find_gameobjects`, `manage_scene` to check.
   NEVER assume state based on what you did earlier — always verify NOW.

2. **DO THE ACTION** — Execute the action (import, play, stop, etc.)

3. **VERIFY THE ACTION COMPLETED** — Poll until you have proof it worked.
   Check console logs, check scene hierarchy, check for errors.
   If it didn't work, diagnose and retry. Do NOT proceed on assumption.

This applies to: starting/stopping play mode, importing assets, triggering recompiles,
modifying scenes, writing gameData.json (game overwrites it on exit!), and ANY other
Unity interaction. If you skip step 1 or 3, you WILL end up with stale state and
waste time debugging phantom issues.

## Unity Import & Verification — MUST complete without user intervention

When asked to import a house, execute this ENTIRE procedure uninterrupted:

### Step 0: Check state
1. Check if game is in play mode (`manage_editor` or `find_gameobjects`)
2. If running, STOP it first and wait for it to fully exit
3. Verify editor is idle and responsive before proceeding

### Step 1: Import
1. Copy export ZIP: `cp <path> /tmp/house_puzzle_export.zip`
2. Do NOT delete existing sprite folders or house data — the importer handles re-import
3. Run: `mcp__unityMCP__execute_menu_item` → `Tools/House/Import From Temp ZIP`
4. Wait 30s, then check console for import success log (`Import from temp ZIP completed!`)
5. If import fails, diagnose and fix — do NOT ask the user

### Step 2: Set game state
1. **Stop play mode first** — the running game overwrites gameData.json on exit
2. Read the exported `house_data.json` to get location name and wave count
3. Find the location index (Tutorial=0, Rome=1, Athens=2, Amsterdam=3, Paris=4, Palermo=5, Venice=6, Frankfurt=7, New York=8, Prague=9)
4. **Read the existing gameData.json** and MODIFY it (do not overwrite from scratch):
   - Set `CurrentLocation` to the target location index
   - Add/update the target location in `Locations` with `CurrentHouse: 0` and `isStageCompleted` matching wave count
   - Ensure `Locations["1"]` (Rome) exists with at least one house where `IsCompleted: true` (progression gate)
   - Preserve all other existing location data
5. If gameData.json doesn't exist, first start the game with Rome, let it load fully, stop, THEN modify the save
6. Clear the Unity console

### Step 3: Start and verify
1. Start play mode
2. Poll `find_gameobjects(PosContainer)` every 60s until found (up to 10 minutes)
3. Verify the correct house is loaded: `find_gameobjects(<houseName>)` must exist
4. Clear console
5. Poll for NEW errors every 30-60s. Keep polling until no new errors appear for 2 consecutive checks.
   The ONLY acceptable errors are:
   - `Assembly '...Roslyn...' will not be loaded` (Linux platform)
   - `Assembly '...Firebase...' will not be loaded` (Linux platform)
   - `MCP-FOR-UNITY: Client handler exited` (MCP session management)
   - `The referenced script (Unknown) on this Behaviour is missing!` (pre-existing prefab)
6. ANY other error (NullReferenceException, etc.) means something is WRONG — investigate and fix, then re-run the full procedure from Step 1
7. After errors stabilize, interact with the game: use `manage_gameobject` or similar to touch/move something, then check console again for new errors

### Step 4: Report
Only after ALL of the above passes, tell the user the house is ready to check.
If errors are found, investigate and fix without asking the user. Re-run the full procedure.
NEVER report partial success. NEVER ask the user to check if there are unresolved errors.

## Unity C# Script Recompilation

Unity does NOT auto-recompile C# scripts edited externally (via CLI/Claude Code).
The editor only triggers recompilation when the Unity window **gains focus**.

After editing any `.cs` file in the VanityLane project:
1. Run `wmctrl -a Unity` to bring Unity to foreground (triggers auto-refresh + recompile)
2. Wait ~15 seconds, then verify: `ls -la Library/ScriptAssemblies/Assembly-CSharp-Editor.dll`
3. If the DLL timestamp hasn't changed, delete it and run `wmctrl -a Unity` again — Unity rebuilds from scratch
4. Check `~/.config/unity3d/Editor.log` for `error CS` if the DLL is missing after rebuild

Do NOT rely on `Assets/Refresh` menu, `manage_asset import`, or `touch` to trigger recompilation — they don't work.


These are standing instructions. Do not wait to be reminded. Apply them every session.
