Функционал который нужен:
Слеивание пазлин из кирпичей:
1. Склеивать рандомно кирпичи в пазлины. Манипулировать количеством пазлин в доме ( крупность деталек)
2. Генерить чертёж - слой белыми линиями 4px толщиной по центру границы детали (то есть линия не внутри границы и не снаружи)
3. Размер пазлин:  минимально 50х150px или 100×100px, максимально 400×300px (как то манипулировать количеством максимальных и минимальных - или сразу установить их колич?)
4. Форма Пазлины не должны повторяться, а если повторились то стоять максимально удалённо ( в разных волнах). Названия слоя одинаковых пазлин должны быть одинаковыми: same_1 или same_2 и тд.
5. некоторые дома мы попробуем небоскребами, так что будет скролл дома вниз.
6. возможность корректировки пазлин вручную - добавление или отделение кирпичей от пазлины
7. Иметь Два варианта соединения деталек 1) окна и двери всегда отдельно или 2) склеивать кирпичи с окнами


Вторая часть функций - левел дизайн (самая важная, так как первую мы можем делать в Иллюстр вручную):
1. Разделение на волны, обводя зону деталек курсором на доме
2. Нижнее скролл меню, где по волнам расставляются пазлины кодом. И возможность переставлять их вручную в нижнем меню
3. Возможность вручную корректировать конфигурацию волны курсором на доме
4. Место детали на доме должно подсвечиваться, если нажать на деталь в ниж меню и наоборот
5. В идеале чтобы код автоматом подсвечивал красным место и деталь, которая будет падать (такое бывает, когда деталь стоит не в правильном порядке - то есть её опорная деталь оказывается в следующей волне)
6. Видеть общее количество пазлин
7. Опять напишу - скролл небоскрёбов вверх-вниз ( под нижнее меню)

Техдолг:
1. Сделать полностью программный export API (без браузера) — чтобы /api/export покрывал весь функционал включая генерацию outline paths серверно, без необходимости открывать фронтенд

## Wave integration status
Frontend wave UI exists and wave data is already sent with export:
- `editor.js:387` sends `waves: [{wave: 1, pieceIds: [...]}, ...]`
- `unity_export.py` maps these to `steps[].blockIndices` in house_data.json
- Importer reads steps and populates `HouseData.SchemeData.Steps[]`
- **Fallback**: if no waves defined by user, `_auto_waves_by_y()` splits into 3 waves by Y position (bottom first)
- **TODO**: verify in-game that manually-assigned waves from the UI produce correct step progression

## Visual polish

### Match outline/scheme stroke weight to existing houses
The scheme.png and light.png stroke widths don't quite match the weight of
existing houses. Needs experimenting with `stroke_width` in `_rasterize_outlines()`
(currently 4) and `MaxFilter` kernel size in `_rasterize_outline_boundary()`
(currently 9) until they look consistent with the originals.

## UI/UX polish

### Puzzle generation is very slow
Generation (merge + compositing) takes noticeably long. Needs profiling — likely
the Python merge algorithm or the JS canvas compositing of many bricks.

### Piece regen after edit is very slow
After manually editing a piece (adding/removing bricks), the recomposite step is
slow. Should only recomposite the affected piece, not the full set.

### Selection is very slow
Selecting pieces (lasso or click) is noticeably laggy. Needs profiling and
optimization — likely re-rendering or hit-testing on every mouse move.

### Wave selection highlight — house only, no tray highlight
When selecting pieces via the select tool that belong to a wave, show the brown
selection highlight on the house canvas as usual, but do NOT show the brown
selection border/rectangle on the piece thumbnail in the wave panel.

## Bugs

### Dragged piece renders under already-placed pieces
When the user drags a piece from the tray toward the house, it floats underneath
pieces that are already placed. The dragged piece should be on the topmost sorting
layer so it's always visible above the house.

## Unity export improvements

### Bottom piece collider vectorization
Instead of computing `groundOffset` to shift the whole house down, modify the collider
generation for bottom-level pieces: when a piece touches the canvas bottom edge, replace
the traced bottom contour segment with a straight line at y=0 (pixel space = canvas height).
This makes the collider flush with the ground by construction, no offset needed.

**Why**: groundOffset is a global shift that only helps the single lowest piece. If multiple
bottom pieces have different gaps, some still won't be perfectly flush. Per-piece bottom
clamping would make every bottom piece individually correct.

### ScalingFactor: principled formula instead of heuristic
Replace the empirical `round(220 / avg_sprite_width)` in the exporter with the formula
derived from camera/canvas geometry:

```
ScalingFactor = round(refHeight / (PPU × 2 × orthoSize))
```

**Known values** (from Game.unity scene):
- `refHeight = 2868` (CanvasScaler reference resolution Y)
- `PPU = 50` (sprite pixels per unit)
- `orthoSize = 14.33` (orthographic camera size)
- Result: `2868 / (50 × 28.66) = 2.001 ≈ 2`

**Why it works**: a tray piece at `spriteWidth × SF` canvas units must appear the same
screen size as the world-space sprite at `spriteWidth / PPU` world units. At the reference
resolution the canvas scale factor is 1.0, so the formula reduces to the ratio above.
Holds for any sprite size; holds for devices with the same aspect ratio as the reference
(~0.46, standard portrait phones). Drifts slightly on very different aspect ratios due
to `matchWidthOrHeight = 0.5`.

**Expose as settings**: add `orthoSize` and `refResolution` as optional export parameters
(alongside `ppu`) so the formula adapts if camera/canvas settings change in Unity.
Currently these are hardcoded in the scene, but exposing them avoids silent breakage.
