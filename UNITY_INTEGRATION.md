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

---

## Open Problems

### 5. Blocks drift 0.118 units during placement and get rejected

**Status**: ROOT CAUSE FOUND, fix in progress (collider boundary resolution)

#### The drift mechanism
- CheckTruePositionSystem matches a block → snaps it to target position
- SetBlockSystem monitors drift: if block moves > 0.1 units from target → FallEvent (rejected)
- Block has Dynamic Rigidbody2D with gravityScale=10 during the 0.4s check period
- The block MUST have immediate physical support (ground or placed blocks) to not drift
- 0.118 units = ~3 frames of free-fall at gravityScale=10: `0.5 × 98.1 × 0.048² ≈ 0.113`
- ALL blocks drift the same 0.118 → NO block has any physical support

#### Comparison with working house (Amsterdam_1)
Tested Amsterdam_1 in-game with debug logging. Result: blocks go MATCH → SET with **zero drift**.
No DRIFT log at all. Working houses have colliders that provide immediate physical support.

**Amsterdam bottom blocks** (world space):
| Block | World Y (center) | Collider bottom | Gap to ground (Y=0) |
|-------|-------------------|-----------------|---------------------|
| #42   | 0.497             | -0.001          | touching            |
| #50   | 0.497             | -0.002          | touching            |
| #48   | 0.747             | +0.003          | 0.003               |

**Our bottom blocks** (world space, with 2px inset):
| Block | World Y (center) | Collider bottom | Gap to ground (Y=0) |
|-------|-------------------|-----------------|---------------------|
| #52   | 0.916             | +0.086          | 0.086               |
| #16   | 1.046             | +0.086          | 0.086               |
| #6    | 1.483             | +0.093          | 0.093               |

Amsterdam: gap ≈ 0 (instant collision). Ours: gap ≈ 0.086-0.093 (3 frames of fall needed,
but drift check rejects at frame 3).

#### Why our colliders have gaps

Two contributing factors:

1. **2px collider inset** in unity_export.py = 0.04 Unity units at PPU=50. The PSD pipeline
   uses FixOverlaps with 0.0001 unit steps (total inset ~0.001). Our inset was **40x larger**.

2. **Anti-aliased sprite edges**. Our piece sprites have NO clean alpha edges — 85% of pixels
   have alpha 252-254, edge pixels drop to 182-215. With threshold=191, the contour traces
   ~1-2px inside the visual edge, adding ~0.02-0.04 units to the gap.

   ```
   piece_052 alpha distribution:
     254: 2466 pixels (64%)    ← interior
     253:  344 pixels           ← near-edge
     252:  148 pixels           ← edge
     251:   31 pixels           ← edge
     182-215: ~200 pixels       ← outer edge (below threshold 191)
     No alpha=0 or alpha=255 pixels at all
   ```

#### Collider overlap/gap analysis (with 2px inset)

Measured all 60 piece colliders in world space:
- **150 overlapping pairs** — most adjacent blocks' colliders overlap even with 2px inset
- **17 pairs with gaps** — only 1 small gap (0.007), rest are 0.16-0.50 (not truly adjacent)
- Largest overlap: 0.229 area units (piece_002 ↔ piece_058)

**What overlapping colliders cause at runtime**: When a Dynamic block is placed and its
collider overlaps a Static (already placed) block, Unity physics **pushes the Dynamic block
out** of the overlap. Small overlap → tiny push → block stays near target. Large overlap →
big push → drift exceeds 0.1 → rejected.

#### The fix needed: midline boundary resolution

Neither pure inset=0 nor inset=2px works:
- **inset=0**: Ground contact works (gap ≈ 0.04), but inter-block overlaps cause push-out drift
- **inset=2px**: No overlaps, but ground contact broken (gap ≈ 0.086)

**Correct approach**: For each pair of adjacent/overlapping pieces, compute the exact **midline
boundary** between them and clip both colliders to it. Result: zero gap, zero overlap between
neighbors. Collider outer edges (facing ground/air) stay at full size for ground contact.

Two possible implementations:
1. **Postprocess** (chosen): Take existing traced contours, find overlapping pairs, compute
   midline, clip both to it
2. **Voronoi from scratch**: Skip contour tracing, compute boundaries from pixel ownership map

#### PSD pipeline collider process (for reference)
```
WriteInfoToData.WriteColliders():
  PixelCollider2D(alpha > 0.75)          →  pixel-perfect polygon per sprite
  PolygonColliderOptimizer (if >30 paths) →  simplified polygon

DebuggingHouse.FixOverlaps():
  For each overlapping pair:
    Iteratively inset by -0.0001 units/step (ClipperLib)
    Up to 10000 iterations until no overlap
  Result: ~0.001 unit total inset, preserves ground contact
```

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
For Amsterdam_1: 1070px/50ppu/2 - 16.2225 = 10.7 - 16.2225 = -5.5225.

### Collider pipeline (our code, current state)
```
unity_export.py:
  trace_alpha_contours(alpha_threshold=191)  →  contour pixels
  douglas_peucker_closed(epsilon=2.0)        →  simplified polygon
  [TODO: midline boundary resolution]        →  no overlap, no gap between neighbors
  contour_to_collider_path(ppu=50)           →  Unity local coords (center-pivot, Y-flip)
  → JSON: colliders[i].paths[j].points[k] = {x, y}

HousePuzzleImporter.cs:
  ConvertJsonColliders()  →  PolygonColliderData[]  →  houseData.Colliders
```

### Ground collider (runtime, verified via Unity MCP)
- Position: Y=-2.84, BoxCollider2D size.y=5.685 → **top edge at Y=0**
- Width scaled to cover all houses (scale.x=10.42)

### Key files
- `unity_export.py` — collider tracing, simplification, coordinate conversion
- `HousePuzzleImporter.cs` — Unity importer, reads JSON colliders
- `server.py` — `/api/export` endpoint
- `UNITY_INTEGRATION.md` — this file (read before making changes!)
