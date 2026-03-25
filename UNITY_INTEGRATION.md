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
