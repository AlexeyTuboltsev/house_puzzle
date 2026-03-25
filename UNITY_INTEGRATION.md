# Unity Integration — Problems, Solutions & Rules

This document tracks issues found during Unity import/export integration.
**Read this before making ANY changes to the exporter or importer.**

---

## Hard Rules (DO NOT violate)

1. **Colliders come from JSON only.** The Python exporter (`unity_export.py`) generates collider
   polygons. The importer (`HousePuzzleImporter.cs`) reads these via `ConvertJsonColliders()`.
   **Never** generate colliders from sprites via PixelCollider2D. If JSON has no colliders →
   error, not fallback. (This was forgotten and re-introduced TWICE.)

2. **Cannot modify game code.** Only the exporter (Python) and importer (HousePuzzleImporter.cs)
   can be changed. Game systems (CheckTruePositionSystem, SetBlockSystem, CreateHouses, etc.)
   are read-only. Debug logging in game systems is allowed temporarily but must be removed after.

---

## Solved Problems

### 1. Importer generated colliders from PixelCollider2D instead of JSON (FIXED 2026-03-25)
- **Symptom**: Colliders looked wrong, didn't match Python exporter output
- **Root cause**: Importer called `GenerateCollidersFromSprites()` using PixelCollider2D,
  completely ignoring the `colliders` array in house_data.json
- **Fix**: Added `ConvertJsonColliders()` to parse JSON colliders. Removed PixelCollider2D fallback.
  Error if JSON has no colliders.

### 2. Scheme.Position.y mismatch (FIXED 2026-03-25)
- **Symptom**: House sunk into ground, blocks couldn't reach targets
- **Root cause**: Export scales sprites (e.g., 600px target width). Scheme.Position.y was
  calculated from original canvas height instead of scaled height.
- **Fix**: Importer calculates from `json.canvas.height` (scaled):
  ```csharp
  var schemeCenter = new Vector3(0, json.canvas.height / 2f / ppu, 0);
  ```

### 3. Importer corrupted HousesData.asset on re-import (FIXED 2026-03-25)
- **Symptom**: Null entries in HousesData, house inserted into wrong location (Tutorial),
  duplicate entries, NullReferenceException in Addressables/CreateHousesSystem
- **Root cause**: `InsertArrayElementAtIndex` bugs + no check for existing entry
- **Fix**: Re-import detection (check existing asset → reuse sprite folder, update in-place,
  skip location insertion). Check if house already present before inserting.

### 4. Sprite names didn't match expected names (FIXED 2026-03-25)
- **Symptom**: Missing overlay sprites in game
- **Fix**: Changed from "blueprint"/"composite" to "scheme"/"light"/"blue"/"flat"

### 5. Blocks drift during placement and get rejected (FIXED 2026-03-25)

**Root cause**: Collider polygon simplification was too aggressive. Douglas-Peucker with
epsilon=2.0 reduced contours to 7-16 vertices, creating massive overlaps between adjacent
pieces (up to 1.7 Unity units across 114 overlapping pairs). When a block was placed, its
Dynamic collider overlapped Static neighbors, Unity physics pushed it out, and the block
drifted past the 0.1-unit rejection threshold in SetBlockSystem's 0.4s check.

Additionally, bottom pieces didn't touch the ground (Y=0) — artwork doesn't reach the canvas
bottom edge and contour tracing at pixel centers adds ~0.02-0.04 units of gap.

**Fix (two parts)**:
1. **Reduced DP epsilon from 2.0 to 0.5** — polygons now average 22 vertices with only 17
   tiny overlap pairs (vs 114 massive ones). Physics push-out is negligible.
2. **Added `groundOffset`** — exporter computes how far the lowest collider point sits above
   Y=0 in world space. Importer shifts the house down by this amount so bottom pieces touch
   the ground. For the current house: groundOffset = 0.043 units.

**Result**: All blocks snap in place with zero drift.

### 6. Addressables not loading at runtime (FIXED 2026-03-25)
- **Symptom**: `ArgumentNullException` in Addressables at startup
- **Root cause**: Stale Addressables catalog after importing new assets
- **Fix**: Importer now calls `SetFastMode.Set()` to use "Use Asset Database" mode,
  bypassing the need for a full Addressables build. This is set automatically on each import.

### 7. Null entry in HousesData.asset causing NullReferenceException (FIXED 2026-03-25)
- **Symptom**: `NullReferenceException` at `AtlasLoadingStage.cs:50` — `house.SpriteAtlasPath`
  crashes because Rome's HousesData array had `{fileID: 0}` (null) at index 0
- **Root cause**: Deleting the old test house assets left a null slot in the array; the importer
  re-created the house at a new index but didn't clean up the old null slot
- **Fix**: Manually removed the null entry from HousesData.asset

### 8. House overlapping in scrollable list — Spacing=0 (FIXED 2026-03-25)
- **Symptom**: NewHouse appeared directly on top of the previous house in the
  game's scrollable house list (both at x≈86)
- **Root cause**: The `Spacing` field in HouseData was exported as 0. The exporter
  computed spacing from `position_in_location`: position=0 → spacing=0 (intended
  for the first house in a location). But the default position was 0, so every
  export got spacing=0 regardless of actual insertion point.
- **Fix**: Made `spacing` an independent parameter (default 12.0) in both
  `unity_export.py` and `server.py`. It no longer depends on position.

---

## Architecture Reference

### Block placement pipeline (game code, read-only)
1. User drags block → DragSystem drops → Rigidbody Dynamic, gravityScale=10
2. **CheckTruePositionSystem**: match by SameBlocks + HouseNum + StepNum + proximity (< 1.5 units)
3. Snap to target position, enable collider, start 0.4s PositionCheckComponent
4. **SetBlockSystem**: check drift each frame. If drift > 0.1 → FallEvent. If 0.4s passes → Static.
5. Block needs physical support (ground collider or other set blocks) during the 0.4s check.
6. Residual velocity from drop is NOT zeroed on snap — only immediate physical contact prevents drift.

### House position formula (game code)
```
housePosition.y = schemeSprite.bounds.size.y / 2 - SchemeData.Scheme.Position.y
```
For our house: 897px/50ppu/2 - 8.97 = 8.97 - 8.97 = 0.0 (house container at Y=0).

### Collider pipeline (our code, current state)
```
unity_export.py:
  trace_alpha_contours(alpha_threshold=191)  →  contour pixels
  douglas_peucker_closed(epsilon=0.5)        →  simplified polygon (~22 vertices avg)
  contour_to_collider_path(ppu=50)           →  Unity local coords (center-pivot, Y-flip)
  groundOffset = min world-space collider bottom across all pieces
  → JSON: colliders[i].paths[j].points[k] = {x, y}

HousePuzzleImporter.cs:
  ConvertJsonColliders()  →  PolygonColliderData[]  →  houseData.Colliders
  schemeCenter.y = canvas.height / 2 / ppu + groundOffset  →  shifts house down
  SetFastMode.Set()  →  Addressables uses Asset Database (no build needed)
```

### Ground collider (runtime, verified via Unity MCP)
- Position: Y=-2.84, BoxCollider2D size.y=5.685 → **top edge at Y=0**
- Width scaled to cover all houses (scale.x=10.42)

### Key files
- `unity_export.py` — collider tracing, simplification, coordinate conversion
- `HousePuzzleImporter.cs` — Unity importer, reads JSON colliders
- `server.py` — `/api/export` endpoint
- `UNITY_INTEGRATION.md` — this file (read before making changes!)

---

## Import / Re-import Procedure

**Follow these steps exactly when importing or re-importing a house.**

### Prerequisites
- Unity Editor open, NOT in Play mode
- Docker container running (`docker compose up` in house_puzzle/)
- TIF loaded and pieces merged in the web UI

### Step-by-step

1. **Export from web UI**: Click "Export" in the puzzle editor → produces ZIP at
   `in/house_export.zip`. Export config (placement) is sent via the API and
   controls location, position, spacing, and house name.

2. **Copy ZIP to import location**:
   ```
   cp in/house_export.zip /tmp/house_puzzle_export.zip
   ```

3. **If re-importing (house already exists)**: The importer detects existing
   `NewHouse.asset` and reuses the sprite folder. It updates the asset in place
   and skips the HousesData insert. No manual cleanup needed.
   If you want a clean slate, delete:
   - `Assets/Data/Houses/Rome/NewHouse.asset`
   - `Assets/Visual/Sprites/Houses/Rome/11/` (the sprite folder)
   - The NewHouse entry from `HousesData.asset` (Rome's HousesData array)

4. **Trigger import**: In Unity menu: `Tools > House > Import From Temp ZIP`
   (or via MCP: `execute_menu_item("Tools/House/Import From Temp ZIP")`)

5. **Verify HousesData.asset**: Check that:
   - NewHouse was appended at the END of the location's HousesData array
   - NO existing houses were removed, reordered, or duplicated
   - The GUID count matches expectations (count entries, compare with before)
   **NEVER manually edit the HousesData order** — the game's save data
   (gameData.json) maps indices 1:1 to this array. Changing order breaks saves.

6. **Reset gameData.json** (ONLY while game is STOPPED — the running game
   overwrites this file!):
   - Set `CurrentLocation` to the location ID (e.g., `1` for Rome)
   - Set that location's `CurrentHouse` to the NewHouse index in HousesData
   - Ensure the save entry at that index has `IsCompleted: false`,
     `BlockIndexes: []`. If there are fewer save entries than needed, the game
     creates them on first launch.
   - File: `~/.config/unity3d/mishmashroom/House Art_ Building Puzzle/gameData.json`

7. **Start Play mode** and wait ~10 minutes. Monitor console:
   - Look for `Start: LevelType: Rome, LevelNumber: XX, LevelName: Rome_YY`
     to confirm the correct house loaded
   - Look for `PosContainer` GameObjects to confirm house is fully set up
   - Check for errors (ignore known assembly warnings and
     "Target platform misconfiguration")

### Export parameters (house_data.json)

| Field     | Default | Description |
|-----------|---------|-------------|
| `spacing` | 12.0    | Gap (Unity units) before this house in the scrollable list. Set to 0 only for the first house in a location. Existing houses use 8–13. |
| `placement.position` | 0 | Insertion index hint. The importer always appends at the end regardless. |
| `placement.location` | "Rome" | Target location name (must match LocationsData). |
| `placement.houseName` | "NewHouse" | Asset name. Determines re-import detection. |

### Common pitfalls
- **Spacing=0 makes houses overlap**: The `Spacing` field in HouseData controls
  the gap between houses in the scrollable list. If 0, the house appears on top
  of the previous one. Always export with spacing=12 unless it's the first house.
  (Fixed 2026-03-25: spacing is now an independent parameter, default 12.)
- **Game overwrites gameData.json**: ALWAYS stop Play mode before editing the
  save file. Editing during play is silently reverted.
- **Null entries in HousesData**: Removing a house asset without removing its
  entry from HousesData leaves `{fileID: 0}` → NullReferenceException in
  AtlasLoadingStage.cs:50.
- **Never reorder HousesData**: The gameData save array maps indices 1:1 to
  HousesData. Changing order (inserting/removing in the middle, restoring from
  git) breaks all save data for that location.
- **Save index vs HousesData index**: If the save has MORE entries than
  HousesData (from previous states), the extra entries are harmless but stale.
