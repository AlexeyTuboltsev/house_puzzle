// House Puzzle Editor — Frontend

let compositeImg = null;    // HTMLImageElement of composite
let brickImages = {};       // brick_id -> HTMLImageElement
let bricks = [];            // brick data from backend
let pieces = [];            // piece data from backend
let canvasW = 0, canvasH = 0;
let selectedBrickId = -1;
let hoveredBrickId = -1;
let selectedPieceId = -1;
let hoveredPieceId = -1;
let viewMode = 'pieces';

// Piece edit mode state
let editMode = false;
let shapeManualVerts = 0; // 0 = auto, >0 = manual override from slider
let editPieceId = -1;
let editBrickIds = [];        // working copy of brick_ids being edited
let originalBrickIds = [];    // snapshot to detect changes / revert

// Pre-rendered piece composites: pieceId -> { canvas, x, y, w, h }
let pieceComposites = {};

// View scale (fixed, fit to viewport)
let zoom = 1;

// Cached vectorized outline paths (in house coords) for overlay
// Array of { pieceId, points: [[x,y], ...] }
let cachedOutlinePaths = [];
let showOutlineOverlay = true;

// Canvas pan (for tall houses / scrolling)
let panY = 0;
let isPanning = false;
let panStartY = 0;
let panStartPanY = 0;

// Preset state
let currentPresetName = '';
let currentPresetValues = null;  // snapshot of params when preset was loaded
const PARAM_IDS = ['target_count', 'min_border', 'seed'];

// --- Wave system ---
let waves = [];           // [{ id, name, pieceIds: [] }, ...]
let nextWaveId = 1;
let selectedWaveId = -1;  // which wave is active for assignment
let assignMode = false;   // lasso assign mode
let hiddenWaveIds = new Set(); // waves whose pieces are hidden on canvas

// Lasso state
let isLassoing = false;
let lassoWasDrag = false; // true if mouse moved > threshold during lasso
let lassoStartX = 0;
let lassoStartY = 0;
let lassoEndX = 0;
let lassoEndY = 0;

// Highlight: pieces highlighted from wave panel or lasso selection
let highlightedPieceIds = new Set();  // multiple pieces can be highlighted

// Drag piece between waves
let dragPieceId = -1;
let dragSourceWaveId = null; // null = unassigned

const canvas = document.getElementById('houseCanvas');
const ctx = canvas.getContext('2d');
const canvasArea = document.getElementById('canvasArea');
const loading = document.getElementById('loadingOverlay');

// --- Persistence (localStorage) ---

const STORAGE_KEY = 'housePuzzle';

function saveState() {
    const state = {
        preset: currentPresetName,
        params: getCurrentParamValues(),
        tif: document.getElementById('tifSelect').value,
        view: viewMode,
        waves: waves,
        nextWaveId: nextWaveId,
    };
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(state)); } catch (e) {}
}

function loadSavedState() {
    try {
        const raw = localStorage.getItem(STORAGE_KEY);
        return raw ? JSON.parse(raw) : null;
    } catch (e) { return null; }
}

// --- Initialization ---

async function init() {
    fitCanvas();
    render();
    await loadTifList();
    await loadPresetList();

    // Restore saved state
    const saved = loadSavedState();
    if (saved) {
        // Restore TIF selection
        if (saved.tif) {
            const tifSelect = document.getElementById('tifSelect');
            if ([...tifSelect.options].some(o => o.value === saved.tif)) {
                tifSelect.value = saved.tif;
            }
        }
        // Restore preset or raw params
        if (saved.preset) {
            document.getElementById('presetSelect').value = saved.preset;
            await loadPreset(saved.preset);
        }
        // Apply any param overrides (dirty state)
        if (saved.params) {
            applyParamValues(saved.params);
            checkPresetDirty();
        }
        // Restore view mode
        if (saved.view) {
            viewMode = saved.view;
            document.querySelectorAll('.view-toggles button').forEach(b => b.classList.remove('active'));
            const btnId = 'view' + saved.view.charAt(0).toUpperCase() + saved.view.slice(1);
            const btn = document.getElementById(btnId);
            if (btn) btn.classList.add('active');
        }
        // Restore waves
        if (saved.waves) {
            waves = saved.waves;
            nextWaveId = saved.nextWaveId || (waves.length ? Math.max(...waves.map(w => w.id)) + 1 : 1);
        }
    } else {
        // First visit: load Default preset
        const presetSelect = document.getElementById('presetSelect');
        if ([...presetSelect.options].some(o => o.value === 'Default')) {
            presetSelect.value = 'Default';
            await loadPreset('Default');
        }
    }
    renderWavesPanel();
}

async function loadTifList() {
    const resp = await fetch('/api/list_tifs');
    const data = await resp.json();
    const select = document.getElementById('tifSelect');
    select.innerHTML = '<option value="">-- Select TIF --</option>';
    data.tifs.forEach(t => {
        const opt = document.createElement('option');
        opt.value = t.path;
        opt.textContent = `${t.name} (${t.size_mb} MB)`;
        select.appendChild(opt);
    });
}

// --- TIF Loading ---

let _loading = false; // double-click guard

async function loadTif() {
    const path = document.getElementById('tifSelect').value;
    if (!path || _loading) return;
    _loading = true;

    showLoading('Parsing TIF & extracting layers...');

    try {
        const resp = await fetch('/api/load_tif', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ path }),
        });
        const data = await resp.json();

        if (data.error) {
            alert(data.error);
            return;
        }

        bricks = data.bricks;
        pieces = [];
        pieceComposites = {};
        canvasW = data.canvas.width;
        canvasH = data.canvas.height;
        selectedBrickId = -1;
        hoveredBrickId = -1;
        selectedPieceId = -1;
        hoveredPieceId = -1;
        highlightedPieceIds.clear();
        panY = 0;
        brickImages = {};
        // Clear brick composite cache
        for (const key of Object.keys(getBrickComp)) {
            if (key.startsWith('_brickComp_')) delete getBrickComp[key];
        }

        document.getElementById('stat_canvas').textContent = `${canvasW}×${canvasH}`;
        document.getElementById('stat_bricks').textContent = data.num_bricks;
        document.getElementById('stat_pieces').textContent = '-';
        document.getElementById('stat_selected').textContent = '-';

        document.getElementById('target_count').max = data.num_bricks;

        compositeImg = new Image();
        compositeImg.onload = () => {
            resetView();
            render();
            loadBrickImages();
        };
        compositeImg.src = '/api/composite.png?' + Date.now();

        document.getElementById('mergeBtn').disabled = false;
        document.getElementById('exportBtn').disabled = true;
        document.getElementById('canvasInfo').textContent =
            `${canvasW}×${canvasH} | ${data.num_bricks} bricks | Adjust settings and click Generate Puzzle`;
        saveState();

    } catch (err) {
        alert('Failed to load TIF: ' + err.message);
    } finally {
        _loading = false;
        hideLoading();
    }
}

function loadBrickImages() {
    let loaded = 0;
    const total = bricks.length;

    for (const brick of bricks) {
        const img = new Image();
        img.onload = () => {
            brickImages[brick.id] = img;
            loaded++;
            if (loaded === total) {
                document.getElementById('canvasInfo').textContent =
                    `${canvasW}×${canvasH} | ${total} bricks loaded | Adjust settings and click Generate Puzzle`;
                render();
            } else if (loaded % 20 === 0) {
                render();
            }
        };
        img.onerror = () => { loaded++; };
        img.src = `/api/brick/${brick.id}.png`;
    }
}

// --- Piece composite pre-rendering ---

function buildPieceComposites() {
    pieceComposites = {};
    for (const piece of pieces) {
        const px = piece.x;
        const py = piece.y;
        const pw = piece.width;
        const ph = piece.height;

        const off = document.createElement('canvas');
        off.width = pw;
        off.height = ph;
        const offCtx = off.getContext('2d');

        for (const brick of piece.bricks) {
            const img = brickImages[brick.id];
            if (!img) continue;
            offCtx.drawImage(img, brick.x - px, brick.y - py, brick.width, brick.height);
        }

        pieceComposites[piece.id] = { canvas: off, x: px, y: py, w: pw, h: ph };
    }
}

// --- Merge ---

async function doMerge() {
    if (_loading) return;
    _loading = true;
    showLoading('Generating puzzle...');

    try {
        const resp = await fetch('/api/merge', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                target_count: parseInt(document.getElementById('target_count').value),
                seed: parseInt(document.getElementById('seed').value),
                min_border: parseInt(document.getElementById('min_border').value),
            }),
        });
        const data = await resp.json();

        if (data.error) {
            alert(data.error);
            return;
        }

        pieces = data.pieces;
        selectedPieceId = -1;
        hoveredPieceId = -1;
        highlightedPieceIds.clear();
        document.getElementById('stat_pieces').textContent = data.num_pieces;
        document.getElementById('stat_selected').textContent = '-';
        document.getElementById('exportBtn').disabled = false;

        buildPieceComposites();

        // Clear waves on regenerate — piece IDs change completely
        waves = [];
        nextWaveId = 1;
        selectedWaveId = -1;
        if (assignMode) toggleAssignMode();
        hiddenWaveIds.clear();

        // Cache vectorized outlines for overlay
        cachedOutlinePaths = buildOutlinePaths();

        document.getElementById('canvasInfo').textContent =
            `${canvasW}×${canvasH} | ${data.num_pieces} pieces | Hover/click to inspect`;
        // Re-render current view (don't switch away from blueprint)
        if (viewMode === 'blueprint') {
            const svg = document.getElementById('blueprintSvg');
            svg.innerHTML = '';
        }
        render();
        renderWavesPanel();

    } catch (err) {
        alert('Generate failed: ' + err.message);
    } finally {
        _loading = false;
        hideLoading();
    }
}

// --- Export ---

let _exportDirHandle = null;

async function doExport() {
    if (!pieces.length) return;
    showLoading('Exporting...');
    try {
        const resp = await fetch('/api/export', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({}),
        });
        const blob = await resp.blob();

        // Try File System Access API (remembers last save directory)
        if (window.showSaveFilePicker) {
            try {
                const handle = await window.showSaveFilePicker({
                    suggestedName: 'house_puzzle_export.zip',
                    startIn: _exportDirHandle || 'downloads',
                    types: [{ description: 'ZIP Archive', accept: { 'application/zip': ['.zip'] } }],
                });
                _exportDirHandle = handle;
                const writable = await handle.createWritable();
                await writable.write(blob);
                await writable.close();
                return;
            } catch (e) {
                if (e.name === 'AbortError') return;
            }
        }

        // Fallback: classic download
        const url = URL.createObjectURL(blob);
        const a = document.createElement('a');
        a.href = url;
        a.download = 'house_puzzle_export.zip';
        a.click();
        URL.revokeObjectURL(url);
    } catch (err) {
        alert('Export failed: ' + err.message);
    } finally {
        hideLoading();
    }
}

// --- View switching ---

function setView(mode) {
    if (editMode) cancelEditPiece();
    viewMode = mode;
    document.querySelectorAll('.view-toggles button').forEach(b => b.classList.remove('active'));
    const btnId = 'view' + mode.charAt(0).toUpperCase() + mode.slice(1);
    const btn = document.getElementById(btnId);
    if (btn) btn.classList.add('active');
    document.getElementById('editBtnRow').style.display = 'none';
    // Hide SVG overlay when not in blueprint mode
    if (mode !== 'blueprint') {
        const svg = document.getElementById('blueprintSvg');
        svg.style.display = 'none';
        svg.innerHTML = '';
    }
    saveState();
    render();
}

function toggleOutlines(checked) {
    showOutlineOverlay = checked;
    render();
}

// --- Canvas ---

function fitCanvas() {
    if (!canvasW) {
        const rect = canvasArea.getBoundingClientRect();
        canvas.width = rect.width;
        canvas.height = rect.height;
        return;
    }
    const rect = canvasArea.getBoundingClientRect();
    const infoBar = document.getElementById('canvasInfo');
    const infoH = infoBar ? infoBar.offsetHeight : 0;
    const pad = 16;
    const zoomW = (rect.width - pad * 2) / canvasW;
    const zoomH = (rect.height - pad * 2 - infoH) / canvasH;
    zoom = Math.min(zoomW, zoomH);
    canvas.width = rect.width;
    canvas.height = rect.height;
    // Allow vertical scrolling for tall houses
    const scaledH = canvasH * zoom;
    const availH = rect.height - infoH;
    if (scaledH > availH) {
        canvasArea.style.overflow = 'hidden';
    } else {
        canvasArea.style.overflow = 'hidden';
    }
    // Clamp panY
    clampPan();
}

function clampPan() {
    const rect = canvasArea.getBoundingClientRect();
    const scaledH = canvasH * zoom;
    const maxPan = Math.max(0, scaledH - rect.height + 40);
    panY = Math.max(0, Math.min(panY, maxPan));
}

function resetView() {
    panY = 0;
    fitCanvas();
}

function render() {
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    if (!canvasW) return;

    ctx.save();
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY_ = (canvas.height - canvasH * zoom) / 2;
    const effectivePadY = padY_ - panY;
    ctx.translate(padX, effectivePadY);
    ctx.scale(zoom, zoom);

    // Draw blueprint background behind everything (visible through hidden-wave holes)
    if (viewMode !== 'blueprint' && pieces.length && hiddenWaveIds.size > 0) {
        renderBlueprintBackground();
    }

    // Draw composite background (only when no pieces yet; not for blueprint)
    if (compositeImg && compositeImg.complete && viewMode !== 'blueprint') {
        if (!pieces.length) {
            ctx.drawImage(compositeImg, 0, 0, canvasW, canvasH);
        }
    }

    if (editMode) {
        renderEditMode();
    } else if (viewMode === 'pieces') {
        if (pieces.length) renderPieces();
    } else if (viewMode === 'blueprint') {
        renderBlueprint();
    }

    // Draw outline overlay on pieces view (not blueprint — it has its own strokes)
    if (showOutlineOverlay && viewMode !== 'blueprint' && pieces.length) {
        renderOutlineOverlay();
    }

    // Draw lasso rectangle if active
    if (isLassoing && assignMode) {
        ctx.save();
        ctx.strokeStyle = '#e0a050';
        ctx.lineWidth = 2 / zoom;
        ctx.setLineDash([6 / zoom, 4 / zoom]);
        ctx.fillStyle = 'rgba(224, 160, 80, 0.1)';
        const lx = Math.min(lassoStartX, lassoEndX);
        const ly = Math.min(lassoStartY, lassoEndY);
        const lw = Math.abs(lassoEndX - lassoStartX);
        const lh = Math.abs(lassoEndY - lassoStartY);
        ctx.fillRect(lx, ly, lw, lh);
        ctx.strokeRect(lx, ly, lw, lh);
        ctx.restore();
    }

    ctx.restore();
}

function getBrickComp(brick) {
    const img = brickImages[brick.id];
    if (!img) return null;
    const key = '_brickComp_' + brick.id;
    if (!getBrickComp[key]) {
        const off = document.createElement('canvas');
        off.width = brick.width;
        off.height = brick.height;
        off.getContext('2d').drawImage(img, 0, 0, brick.width, brick.height);
        getBrickComp[key] = { canvas: off, x: brick.x, y: brick.y, w: brick.width, h: brick.height };
    }
    return getBrickComp[key];
}

function renderBricks() {
    for (const brick of bricks) {
        const img = brickImages[brick.id];
        if (!img) continue;

        const isSelected = brick.id === selectedBrickId;
        const isHovered = brick.id === hoveredBrickId;

        ctx.globalAlpha = (isSelected || isHovered) ? 1.0 : 0.85;
        ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
        ctx.globalAlpha = 1.0;
    }

    if (hoveredBrickId >= 0 && hoveredBrickId !== selectedBrickId) {
        const brick = bricks.find(b => b.id === hoveredBrickId);
        if (brick) {
            const img = brickImages[brick.id];
            if (img) ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
            const comp = getBrickComp(brick);
            if (comp) drawPieceSilhouetteOutline(comp, 'rgba(60, 200, 255, 0.8)', 3);
        }
    }

    if (selectedBrickId >= 0) {
        const brick = bricks.find(b => b.id === selectedBrickId);
        if (brick) {
            const img = brickImages[brick.id];
            const comp = getBrickComp(brick);

            if (img) ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
            if (comp) drawPieceSilhouetteOutline(comp, '#ff6030', 6);

            ctx.fillStyle = 'rgba(255, 96, 48, 0.9)';
            ctx.font = `bold ${Math.round(14 / zoom)}px sans-serif`;
            ctx.textAlign = 'center';
            ctx.fillText(
                `#${brick.id} (${brick.width}×${brick.height}) [${brick.type}]`,
                brick.x + brick.width / 2,
                brick.y - 8 / zoom,
            );
        }
    }
}

function renderPieces() {
    const hiddenPids = getHiddenPieceIds();

    // Draw all pieces except hovered/selected/highlighted/hidden (those go on top or are invisible)
    for (const piece of pieces) {
        if (hiddenPids.has(piece.id)) continue;
        if (piece.id === hoveredPieceId || piece.id === selectedPieceId || highlightedPieceIds.has(piece.id)) continue;
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const hue = (piece.id * 137.508) % 360;
        ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);

        const tint = makeTintedCanvas(comp.canvas, hue, 0.12);
        ctx.drawImage(tint, comp.x, comp.y, comp.w, comp.h);

        if (zoom > 0.12) {
            ctx.fillStyle = `hsla(${hue}, 80%, 85%, 0.85)`;
            ctx.font = `bold ${Math.round(13 / zoom)}px sans-serif`;
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(`#${piece.id}`, comp.x + comp.w / 2, comp.y + comp.h / 2);
        }
    }

    // Highlighted pieces (from wave panel or lasso selection)
    for (const hpId of highlightedPieceIds) {
        if (hiddenPids.has(hpId)) continue;
        if (hpId === selectedPieceId || hpId === hoveredPieceId) continue;
        const piece = pieces.find(p => p.id === hpId);
        if (piece) {
            const comp = pieceComposites[piece.id];
            if (comp) {
                ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);
                drawPieceSilhouetteOutline(comp, '#e0a050', 5);
            }
        }
    }

    // Draw hover outline on top of all pieces
    if (hoveredPieceId >= 0 && hoveredPieceId !== selectedPieceId && !hiddenPids.has(hoveredPieceId)) {
        const piece = pieces.find(p => p.id === hoveredPieceId);
        if (piece) {
            const comp = pieceComposites[piece.id];
            if (comp) {
                ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);
                drawPieceSilhouetteOutline(comp, 'rgba(60, 200, 255, 0.9)', 4);
            }
        }
    }

    // Draw selected piece on top of everything
    if (selectedPieceId >= 0 && !hiddenPids.has(selectedPieceId)) {
        const piece = pieces.find(p => p.id === selectedPieceId);
        if (piece) {
            const comp = pieceComposites[piece.id];
            if (comp) {
                ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);
                drawPieceSilhouetteOutline(comp, '#ff6030', 6);

                ctx.fillStyle = 'rgba(255, 96, 48, 0.95)';
                ctx.font = `bold ${Math.round(14 / zoom)}px sans-serif`;
                ctx.textAlign = 'center';
                ctx.fillText(
                    `Piece #${piece.id} (${piece.num_bricks} bricks, ${piece.width}×${piece.height})`,
                    comp.x + comp.w / 2,
                    comp.y - 8 / zoom,
                );
            }
        }
    }
}

function drawPieceSilhouetteOutline(comp, color, thickness) {
    ctx.save();

    const t = Math.max(1, Math.round(thickness / zoom));
    const pad = t + 2;
    const outlineCanvas = document.createElement('canvas');
    outlineCanvas.width = comp.w + pad * 2;
    outlineCanvas.height = comp.h + pad * 2;
    const oCtx = outlineCanvas.getContext('2d');

    const silCanvas = document.createElement('canvas');
    silCanvas.width = comp.canvas.width;
    silCanvas.height = comp.canvas.height;
    const sCtx = silCanvas.getContext('2d');
    sCtx.drawImage(comp.canvas, 0, 0);
    sCtx.globalCompositeOperation = 'source-in';
    sCtx.fillStyle = color;
    sCtx.fillRect(0, 0, silCanvas.width, silCanvas.height);

    for (let dx = -t; dx <= t; dx++) {
        for (let dy = -t; dy <= t; dy++) {
            if (dx * dx + dy * dy > t * t) continue;
            oCtx.drawImage(silCanvas, pad + dx, pad + dy);
        }
    }

    oCtx.globalCompositeOperation = 'destination-out';
    oCtx.drawImage(comp.canvas, pad, pad);

    ctx.drawImage(outlineCanvas, comp.x - pad, comp.y - pad);
    ctx.restore();
}

function makeTintedCanvas(srcCanvas, hue, alpha) {
    const tint = document.createElement('canvas');
    tint.width = srcCanvas.width;
    tint.height = srcCanvas.height;
    const tCtx = tint.getContext('2d');

    tCtx.drawImage(srcCanvas, 0, 0);
    tCtx.globalCompositeOperation = 'source-in';
    tCtx.fillStyle = `hsla(${hue}, 60%, 50%, ${alpha})`;
    tCtx.fillRect(0, 0, tint.width, tint.height);

    return tint;
}

function renderBlueprint() {
    if (!pieces.length) return;

    const svg = document.getElementById('blueprintSvg');
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY_ = (canvas.height - canvasH * zoom) / 2 - panY;
    svg.setAttribute('width', canvas.width);
    svg.setAttribute('height', canvas.height);
    svg.style.display = 'block';

    const strokeW = 4;
    let svgContent = '';

    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const outline = coarseTraceSnap(comp);
        if (outline.length < 3) continue;
        const simplified = autoSimplify(outline, 1);
        const refined = refineCorners(simplified, outline, 20, 1);

        let d = '';
        for (let i = 0; i < refined.length; i++) {
            const sx = (comp.x + refined[i][0]) * zoom + padX;
            const sy = (comp.y + refined[i][1]) * zoom + padY_;
            d += (i === 0 ? 'M' : 'L') + sx.toFixed(1) + ',' + sy.toFixed(1);
        }
        d += 'Z';
        svgContent += `<path d="${d}" fill="#2a5da8" stroke="white" stroke-width="${strokeW.toFixed(1)}" stroke-linejoin="round" stroke-linecap="round" paint-order="fill stroke"/>`;
    }

    svg.innerHTML = svgContent;
}

function getHiddenPieceIds() {
    const hidden = new Set();
    for (const waveId of hiddenWaveIds) {
        const wave = waves.find(w => w.id === waveId);
        if (wave) {
            for (const pid of wave.pieceIds) hidden.add(pid);
        }
    }
    return hidden;
}

function renderBlueprintBackground() {
    // Draw blueprint-style shapes for hidden pieces so they show through
    if (!cachedOutlinePaths.length) return;
    const hiddenPids = getHiddenPieceIds();
    if (hiddenPids.size === 0) return;

    ctx.save();
    for (const path of cachedOutlinePaths) {
        if (!hiddenPids.has(path.pieceId)) continue;
        if (path.points.length < 3) continue;
        ctx.beginPath();
        ctx.moveTo(path.points[0][0], path.points[0][1]);
        for (let i = 1; i < path.points.length; i++) {
            ctx.lineTo(path.points[i][0], path.points[i][1]);
        }
        ctx.closePath();
        ctx.fillStyle = '#2a5da8';
        ctx.fill();
        ctx.strokeStyle = 'white';
        ctx.lineWidth = 4 / zoom;
        ctx.lineJoin = 'round';
        ctx.stroke();
    }
    ctx.restore();
}

function buildOutlinePaths() {
    // Pre-compute vectorized outlines for all pieces (house coords)
    const paths = [];
    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;
        const outline = coarseTraceSnap(comp);
        if (outline.length < 3) continue;
        const simplified = autoSimplify(outline, 1);
        const refined = refineCorners(simplified, outline, 20, 1);
        // Store as absolute house coords
        const absPoints = refined.map(([x, y]) => [comp.x + x, comp.y + y]);
        paths.push({ pieceId: piece.id, points: absPoints });
    }
    return paths;
}

function renderOutlineOverlay() {
    // Draw thin outline paths on the canvas (already in house-coord transform)
    if (!cachedOutlinePaths.length) return;
    ctx.save();
    ctx.strokeStyle = 'rgba(120, 120, 120, 0.7)';
    ctx.lineWidth = 1 / zoom;
    ctx.lineJoin = 'round';
    ctx.lineCap = 'round';
    for (const path of cachedOutlinePaths) {
        if (path.points.length < 3) continue;
        ctx.beginPath();
        ctx.moveTo(path.points[0][0], path.points[0][1]);
        for (let i = 1; i < path.points.length; i++) {
            ctx.lineTo(path.points[i][0], path.points[i][1]);
        }
        ctx.closePath();
        ctx.stroke();
    }
    ctx.restore();
}

function tracePieceContours(comp, epsilon) {
    const W = comp.w, H = comp.h;
    const mCanvas = document.createElement('canvas');
    mCanvas.width = W;
    mCanvas.height = H;
    const mCtx = mCanvas.getContext('2d');
    mCtx.drawImage(comp.canvas, 0, 0);
    const data = mCtx.getImageData(0, 0, W, H).data;

    const mask = new Uint8Array(W * H);
    for (let i = 0; i < W * H; i++) {
        mask[i] = data[i * 4 + 3] > 30 ? 1 : 0;
    }

    function cell(x, y) {
        return (x >= 0 && x < W && y >= 0 && y < H) ? mask[y * W + x] : 0;
    }

    const edgeMap = new Map();

    function addEdge(fx, fy, tx, ty) {
        const k = fx + ',' + fy;
        if (!edgeMap.has(k)) edgeMap.set(k, []);
        edgeMap.get(k).push({tx, ty, used: false});
    }

    for (let y = 0; y <= H; y++) {
        for (let x = 0; x < W; x++) {
            const above = cell(x, y - 1), below = cell(x, y);
            if (above && !below) addEdge(x, y, x + 1, y);
            else if (!above && below) addEdge(x + 1, y, x, y);
        }
    }

    for (let x = 0; x <= W; x++) {
        for (let y = 0; y < H; y++) {
            const left = cell(x - 1, y), right = cell(x, y);
            if (right && !left) addEdge(x, y + 1, x, y);
            else if (!right && left) addEdge(x, y, x, y + 1);
        }
    }

    const contours = [];

    for (const [startK, startEdges] of edgeMap) {
        for (const se of startEdges) {
            if (se.used) continue;
            se.used = true;

            const [sx, sy] = startK.split(',').map(Number);
            const loop = [[sx, sy]];
            let cx = se.tx, cy = se.ty;
            let dx = se.tx - sx, dy = se.ty - sy;

            for (let step = 0; step < (W + 1) * (H + 1) * 2; step++) {
                if (cx === sx && cy === sy) break;
                loop.push([cx, cy]);

                const outs = edgeMap.get(cx + ',' + cy);
                if (!outs) break;

                const turns = [
                    [-dy, dx],
                    [dx, dy],
                    [dy, -dx],
                    [-dx, -dy],
                ];

                let picked = null;
                for (const [tdx, tdy] of turns) {
                    for (const e of outs) {
                        if (!e.used && (e.tx - cx) === tdx && (e.ty - cy) === tdy) {
                            picked = e;
                            break;
                        }
                    }
                    if (picked) break;
                }

                if (!picked) break;
                picked.used = true;
                dx = picked.tx - cx;
                dy = picked.ty - cy;
                cx = picked.tx;
                cy = picked.ty;
            }

            if (loop.length >= 3) contours.push(loop);
        }
    }

    return contours.map(c => douglasPeuckerClosed(c, epsilon));
}

function douglasPeuckerClosed(points, epsilon) {
    if (points.length <= 4 || epsilon <= 0) return points;

    let maxDist = 0, idxA = 0, idxB = 1;
    for (let i = 0; i < points.length; i++) {
        for (let j = i + 1; j < points.length; j++) {
            const d = (points[i][0] - points[j][0]) ** 2 + (points[i][1] - points[j][1]) ** 2;
            if (d > maxDist) {
                maxDist = d;
                idxA = i;
                idxB = j;
            }
        }
    }

    const half1 = points.slice(idxA, idxB + 1);
    const half2 = points.slice(idxB).concat(points.slice(0, idxA + 1));

    const s1 = douglasPeucker(half1, epsilon);
    const s2 = douglasPeucker(half2, epsilon);

    return s1.slice(0, -1).concat(s2.slice(0, -1));
}

function douglasPeucker(points, epsilon) {
    if (points.length <= 2) return points;

    const [fx, fy] = points[0];
    const [lx, ly] = points[points.length - 1];
    const dx = lx - fx, dy = ly - fy;
    const lenSq = dx * dx + dy * dy;

    let maxDist = 0, maxIdx = 0;
    for (let i = 1; i < points.length - 1; i++) {
        const [px, py] = points[i];
        const dist = lenSq === 0
            ? Math.sqrt((px - fx) ** 2 + (py - fy) ** 2)
            : Math.abs(dx * (fy - py) - dy * (fx - px)) / Math.sqrt(lenSq);
        if (dist > maxDist) {
            maxDist = dist;
            maxIdx = i;
        }
    }

    if (maxDist > epsilon) {
        const left = douglasPeucker(points.slice(0, maxIdx + 1), epsilon);
        const right = douglasPeucker(points.slice(maxIdx), epsilon);
        return left.slice(0, -1).concat(right);
    }
    return [points[0], points[points.length - 1]];
}

// --- Shape view: single piece vectorization ---

function coarseTraceSnap(comp) {
    const W = comp.w, H = comp.h;
    const CELL = 5;
    const PAD = 3;
    const GW = Math.ceil(W / CELL) + PAD * 2;
    const GH = Math.ceil(H / CELL) + PAD * 2;

    const c = document.createElement('canvas');
    c.width = W; c.height = H;
    const cCtx = c.getContext('2d');
    cCtx.drawImage(comp.canvas, 0, 0);
    const data = cCtx.getImageData(0, 0, W, H).data;

    const boundaryPts = [];
    for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
            if (data[(y * W + x) * 4 + 3] <= 30) continue;
            let isBnd = false;
            for (const [dx, dy] of [[-1,0],[1,0],[0,-1],[0,1]]) {
                const nx = x + dx, ny = y + dy;
                if (nx < 0 || nx >= W || ny < 0 || ny >= H ||
                    data[(ny * W + nx) * 4 + 3] <= 30) {
                    isBnd = true; break;
                }
            }
            if (isBnd) boundaryPts.push([x, y]);
        }
    }

    const grid = new Uint8Array(GW * GH);
    for (let y = 0; y < H; y++) {
        for (let x = 0; x < W; x++) {
            if (data[(y * W + x) * 4 + 3] > 30) {
                const gx = Math.floor(x / CELL) + PAD;
                const gy = Math.floor(y / CELL) + PAD;
                grid[gy * GW + gx] = 1;
            }
        }
    }

    const dilGrid = new Uint8Array(GW * GH);
    for (let gy = 0; gy < GH; gy++) {
        for (let gx = 0; gx < GW; gx++) {
            if (grid[gy * GW + gx]) {
                for (let dy = -1; dy <= 1; dy++) {
                    for (let dx = -1; dx <= 1; dx++) {
                        const nx = gx + dx, ny = gy + dy;
                        if (nx >= 0 && nx < GW && ny >= 0 && ny < GH) {
                            dilGrid[ny * GW + nx] = 1;
                        }
                    }
                }
            }
        }
    }

    const exterior = new Uint8Array(GW * GH);
    const q = [];
    for (let x = 0; x < GW; x++) {
        if (!dilGrid[x]) { exterior[x] = 1; q.push(x); }
        const bi = (GH - 1) * GW + x;
        if (!dilGrid[bi]) { exterior[bi] = 1; q.push(bi); }
    }
    for (let y = 0; y < GH; y++) {
        const li = y * GW;
        if (!dilGrid[li]) { exterior[li] = 1; q.push(li); }
        const ri = y * GW + GW - 1;
        if (!dilGrid[ri]) { exterior[ri] = 1; q.push(ri); }
    }
    let qi = 0;
    while (qi < q.length) {
        const idx = q[qi++];
        const gx = idx % GW, gy = (idx - gx) / GW;
        for (const [dx, dy] of [[-1,0],[1,0],[0,-1],[0,1]]) {
            const nx = gx + dx, ny = gy + dy;
            if (nx < 0 || nx >= GW || ny < 0 || ny >= GH) continue;
            const ni = ny * GW + nx;
            if (!exterior[ni] && !dilGrid[ni]) { exterior[ni] = 1; q.push(ni); }
        }
    }

    const solid = new Uint8Array(GW * GH);
    for (let i = 0; i < GW * GH; i++) solid[i] = exterior[i] ? 0 : 1;

    const mooreX = [1, 1, 0, -1, -1, -1, 0, 1];
    const mooreY = [0, 1, 1, 1, 0, -1, -1, -1];

    let startX = -1, startY = -1;
    outer: for (let gy = 0; gy < GH; gy++) {
        for (let gx = 0; gx < GW; gx++) {
            if (solid[gy * GW + gx]) {
                startX = gx; startY = gy;
                break outer;
            }
        }
    }
    if (startX < 0) return [];

    const traced = [];
    const visited = new Set();
    let curX = startX, curY = startY;
    let backDir = 4;

    do {
        const key = curY * GW + curX;
        if (!visited.has(key)) {
            traced.push([curX, curY]);
            visited.add(key);
        }

        let found = false;
        for (let i = 1; i <= 8; i++) {
            const dir = (backDir + i) % 8;
            const nx = curX + mooreX[dir];
            const ny = curY + mooreY[dir];
            if (nx >= 0 && nx < GW && ny >= 0 && ny < GH && solid[ny * GW + nx]) {
                backDir = (dir + 4) % 8;
                curX = nx;
                curY = ny;
                found = true;
                break;
            }
        }
        if (!found) break;
    } while (curX !== startX || curY !== startY);

    if (traced.length < 3) return [];

    const boundaryTraced = traced.filter(([gx, gy]) => {
        for (const [dx, dy] of [[-1,0],[1,0],[0,-1],[0,1],[-1,-1],[1,-1],[-1,1],[1,1]]) {
            const nx = gx + dx, ny = gy + dy;
            if (nx < 0 || nx >= GW || ny < 0 || ny >= GH || exterior[ny * GW + nx]) {
                return true;
            }
        }
        return false;
    });

    if (boundaryTraced.length < 3) return [];

    const snapped = boundaryTraced.map(([gx, gy]) => {
        const px = (gx - PAD + 0.5) * CELL, py = (gy - PAD + 0.5) * CELL;
        let bestDist = Infinity, bestX = px, bestY = py;
        for (const [bx, by] of boundaryPts) {
            const d = (bx - px) * (bx - px) + (by - py) * (by - py);
            if (d < bestDist) {
                bestDist = d; bestX = bx; bestY = by;
            }
        }
        return [bestX, bestY];
    });

    return snapped;
}

function hausdorffToPoly(pts, poly) {
    let maxDist = 0;
    for (const [px, py] of pts) {
        let minD = Infinity;
        for (let i = 0; i < poly.length; i++) {
            const j = (i + 1) % poly.length;
            const [ax, ay] = poly[i], [bx, by] = poly[j];
            const dx = bx - ax, dy = by - ay;
            const lenSq = dx * dx + dy * dy;
            let t = lenSq > 0 ? ((px - ax) * dx + (py - ay) * dy) / lenSq : 0;
            t = Math.max(0, Math.min(1, t));
            const cx = ax + t * dx, cy = ay + t * dy;
            const d = (px - cx) * (px - cx) + (py - cy) * (py - cy);
            if (d < minD) minD = d;
        }
        if (minD > maxDist) maxDist = minD;
    }
    return Math.sqrt(maxDist);
}

function refineCorners(simplified, outline, threshold, maxDeviation) {
    if (simplified.length <= 4) return simplified;
    let pts = simplified.map(p => [...p]);
    let changed = true;
    while (changed) {
        changed = false;
        for (let i = 0; i < pts.length && pts.length > 4; i++) {
            const j = (i + 1) % pts.length;
            const dx = pts[i][0] - pts[j][0];
            const dy = pts[i][1] - pts[j][1];
            if (Math.sqrt(dx * dx + dy * dy) > threshold) continue;

            const prevIdx = (i - 1 + pts.length) % pts.length;
            const nextIdx = (j + 1) % pts.length;
            const prev = pts[prevIdx];
            const next = pts[nextIdx];

            const mid = [(pts[i][0] + pts[j][0]) / 2, (pts[i][1] + pts[j][1]) / 2];
            const searchR2 = (threshold * 3) * (threshold * 3);

            const lx = next[0] - prev[0], ly = next[1] - prev[1];
            const lineLen = Math.sqrt(lx * lx + ly * ly);

            let maxDist = -1;
            let bestPt = mid;
            for (const op of outline) {
                const dm2 = (op[0] - mid[0]) ** 2 + (op[1] - mid[1]) ** 2;
                if (dm2 > searchR2) continue;
                if (lineLen > 0) {
                    const d = Math.abs(lx * (prev[1] - op[1]) - ly * (prev[0] - op[0])) / lineLen;
                    if (d > maxDist) {
                        maxDist = d;
                        bestPt = op;
                    }
                }
            }

            const candidate = [...pts];
            candidate[i] = [...bestPt];
            if (j > i) {
                candidate.splice(j, 1);
            } else {
                candidate.splice(0, 1);
            }
            const newDist = hausdorffToPoly(outline, candidate);
            if (newDist > maxDeviation) continue;

            pts = candidate;
            changed = true;
            break;
        }
    }
    return pts;
}

function autoSimplify(outline, tolerance) {
    if (outline.length <= 3) return outline;

    let lo = 3, hi = outline.length;
    while (lo < hi) {
        const mid = (lo + hi) >> 1;
        const simplified = visvalingamWhyatt(outline, mid);
        const dist = hausdorffToPoly(outline, simplified);
        if (dist <= tolerance) {
            hi = mid;
        } else {
            lo = mid + 1;
        }
    }
    return visvalingamWhyatt(outline, lo);
}

function visvalingamWhyatt(points, targetCount) {
    if (points.length <= targetCount) return points;

    let pts = points.map((p, i) => ({ x: p[0], y: p[1], idx: i, removed: false }));
    let n = pts.length;

    function triangleArea(a, b, c) {
        return Math.abs((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) / 2;
    }

    for (let i = 0; i < pts.length; i++) {
        pts[i].prev = (i - 1 + pts.length) % pts.length;
        pts[i].next = (i + 1) % pts.length;
    }

    function getArea(i) {
        return triangleArea(pts[pts[i].prev], pts[i], pts[pts[i].next]);
    }

    while (n > targetCount) {
        let minArea = Infinity;
        let minIdx = -1;
        for (let i = 0; i < pts.length; i++) {
            if (pts[i].removed) continue;
            const area = getArea(i);
            if (area < minArea) {
                minArea = area;
                minIdx = i;
            }
        }
        if (minIdx < 0) break;

        pts[minIdx].removed = true;
        const prevI = pts[minIdx].prev;
        const nextI = pts[minIdx].next;
        pts[prevI].next = nextI;
        pts[nextI].prev = prevI;
        n--;
    }

    return pts.filter(p => !p.removed).map(p => [p.x, p.y]);
}

function makeSolidCanvas(srcCanvas, color) {
    const c = document.createElement('canvas');
    c.width = srcCanvas.width;
    c.height = srcCanvas.height;
    const cCtx = c.getContext('2d');
    cCtx.drawImage(srcCanvas, 0, 0);
    cCtx.globalCompositeOperation = 'source-in';
    cCtx.fillStyle = color;
    cCtx.fillRect(0, 0, c.width, c.height);
    return c;
}

function erodeCanvas(srcCanvas, amount) {
    const w = srcCanvas.width;
    const h = srcCanvas.height;
    const result = document.createElement('canvas');
    result.width = w;
    result.height = h;
    const rCtx = result.getContext('2d');
    rCtx.drawImage(srcCanvas, 0, 0);
    rCtx.globalCompositeOperation = 'destination-in';
    rCtx.drawImage(srcCanvas, amount, 0);
    rCtx.drawImage(srcCanvas, -amount, 0);
    rCtx.drawImage(srcCanvas, 0, amount);
    rCtx.drawImage(srcCanvas, 0, -amount);
    return result;
}

// --- Piece edit mode ---

function startEditPiece() {
    if (selectedPieceId < 0 || viewMode !== 'pieces') return;
    const piece = pieces.find(p => p.id === selectedPieceId);
    if (!piece) return;

    editMode = true;
    editPieceId = selectedPieceId;
    editBrickIds = [...piece.brick_ids];
    originalBrickIds = [...piece.brick_ids];

    document.getElementById('editBtnRow').style.display = 'none';
    document.getElementById('editActionRow').style.display = 'flex';
    document.getElementById('editHint').style.display = 'block';
    document.getElementById('saveEditBtn').disabled = true;
    render();
}

function cancelEditPiece() {
    editMode = false;
    editPieceId = -1;
    editBrickIds = [];
    originalBrickIds = [];

    document.getElementById('editActionRow').style.display = 'none';
    document.getElementById('editHint').style.display = 'none';
    if (selectedPieceId >= 0) {
        document.getElementById('editBtnRow').style.display = 'flex';
    }
    render();
}

function saveEditPiece() {
    if (!editMode || editPieceId < 0) return;

    const piece = pieces.find(p => p.id === editPieceId);
    if (!piece) { cancelEditPiece(); return; }

    const newSet = new Set(editBrickIds);
    const removedBrickIds = originalBrickIds.filter(bid => !newSet.has(bid));

    for (const other of pieces) {
        if (other.id === editPieceId) continue;
        other.brick_ids = other.brick_ids.filter(bid => !newSet.has(bid));
        other.bricks = other.bricks.filter(b => !newSet.has(b.id));
        other.num_bricks = other.brick_ids.length;
    }

    piece.brick_ids = [...editBrickIds];
    piece.bricks = editBrickIds.map(bid => {
        const b = bricks.find(br => br.id === bid);
        return b ? { id: b.id, x: b.x, y: b.y, width: b.width, height: b.height, type: b.type } : null;
    }).filter(Boolean);
    piece.num_bricks = piece.brick_ids.length;

    for (const bid of removedBrickIds) {
        const b = bricks.find(br => br.id === bid);
        if (!b) continue;
        pieces.push({
            id: pieces.length,
            brick_ids: [bid],
            bricks: [{ id: b.id, x: b.x, y: b.y, width: b.width, height: b.height, type: b.type }],
            num_bricks: 1,
            x: b.x, y: b.y, width: b.width, height: b.height,
        });
    }

    for (const p of pieces) {
        if (p.brick_ids.length === 0) continue;
        const pBricks = p.brick_ids.map(bid => bricks.find(br => br.id === bid)).filter(Boolean);
        p.x = Math.min(...pBricks.map(b => b.x));
        p.y = Math.min(...pBricks.map(b => b.y));
        const maxR = Math.max(...pBricks.map(b => b.x + b.width));
        const maxB = Math.max(...pBricks.map(b => b.y + b.height));
        p.width = maxR - p.x;
        p.height = maxB - p.y;
    }

    pieces = pieces.filter(p => p.brick_ids.length > 0);
    pieces.forEach((p, i) => p.id = i);

    // Update wave references after re-indexing
    // Build old-to-new mapping: not straightforward since ids changed
    // Instead, rebuild wave pieceIds based on brick membership
    rebuildWaveIdsAfterReindex();

    const newPiece = pieces.find(p =>
        editBrickIds.length > 0 && p.brick_ids.includes(editBrickIds[0])
    );
    selectedPieceId = newPiece ? newPiece.id : -1;
    editPieceId = selectedPieceId;

    buildPieceComposites();

    document.getElementById('stat_pieces').textContent = pieces.length;
    if (selectedPieceId >= 0) {
        const sp = pieces.find(p => p.id === selectedPieceId);
        document.getElementById('stat_selected').textContent =
            `Piece #${sp.id} (${sp.num_bricks} bricks, ${sp.width}×${sp.height})`;
    }

    editMode = false;
    editPieceId = -1;
    editBrickIds = [];
    originalBrickIds = [];

    document.getElementById('editActionRow').style.display = 'none';
    document.getElementById('editHint').style.display = 'none';
    if (selectedPieceId >= 0) {
        document.getElementById('editBtnRow').style.display = 'flex';
    }
    render();
    renderWavesPanel();
}

function rebuildWaveIdsAfterReindex() {
    // After pieces are re-indexed, wave pieceIds become stale.
    // We can't easily map old to new, so we clear stale ones.
    const validIds = new Set(pieces.map(p => p.id));
    for (const wave of waves) {
        wave.pieceIds = wave.pieceIds.filter(id => validIds.has(id));
    }
}

function toggleBrickInEdit(brickId) {
    if (!editMode) return;
    const idx = editBrickIds.indexOf(brickId);
    if (idx >= 0) {
        if (editBrickIds.length <= 1) return;
        editBrickIds.splice(idx, 1);
    } else {
        editBrickIds.push(brickId);
    }

    const changed = !arraysEqual(editBrickIds, originalBrickIds);
    document.getElementById('saveEditBtn').disabled = !changed;
    render();
}

function arraysEqual(a, b) {
    if (a.length !== b.length) return false;
    const sa = [...a].sort();
    const sb = [...b].sort();
    return sa.every((v, i) => v === sb[i]);
}

function renderEditMode() {
    const editSet = new Set(editBrickIds);

    for (const brick of bricks) {
        const img = brickImages[brick.id];
        if (!img) continue;

        const inPiece = editSet.has(brick.id);
        ctx.globalAlpha = inPiece ? 1.0 : 0.3;
        ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
        ctx.globalAlpha = 1.0;
    }

    for (const bid of editBrickIds) {
        const brick = bricks.find(b => b.id === bid);
        if (!brick) continue;
        const comp = getBrickComp(brick);
        if (comp) drawPieceSilhouetteOutline(comp, 'rgba(80, 255, 120, 0.8)', 3);
    }

    if (hoveredBrickId >= 0) {
        const brick = bricks.find(b => b.id === hoveredBrickId);
        if (brick) {
            const img = brickImages[brick.id];
            if (img) ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
            const inPiece = editSet.has(brick.id);
            const comp = getBrickComp(brick);
            if (comp) {
                const color = inPiece ? 'rgba(255, 80, 80, 0.9)' : 'rgba(80, 255, 120, 0.9)';
                drawPieceSilhouetteOutline(comp, color, 4);
            }
        }
    }

    const piece = pieces.find(p => p.id === editPieceId);
    if (piece) {
        ctx.fillStyle = 'rgba(80, 255, 120, 0.95)';
        ctx.font = `bold ${Math.round(14 / zoom)}px sans-serif`;
        ctx.textAlign = 'center';
        const cx = (piece.x + piece.width / 2);
        ctx.fillText(
            `Editing Piece #${piece.id} (${editBrickIds.length} bricks)`,
            cx, piece.y - 8 / zoom,
        );
    }
}

// --- Mouse interaction ---

function screenToHouse(clientX, clientY) {
    const rect = canvas.getBoundingClientRect();
    const sx = clientX - rect.left;
    const sy = clientY - rect.top;
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY_ = (canvas.height - canvasH * zoom) / 2 - panY;
    return [(sx - padX) / zoom, (sy - padY_) / zoom];
}

function findBrickAt(hx, hy) {
    for (let i = bricks.length - 1; i >= 0; i--) {
        const b = bricks[i];
        if (hx >= b.x && hx <= b.x + b.width && hy >= b.y && hy <= b.y + b.height) {
            const img = brickImages[b.id];
            if (img && isPixelOpaque(img, Math.round(hx - b.x), Math.round(hy - b.y))) {
                return b.id;
            }
        }
    }
    return -1;
}

const hitTestCache = {};

function isPixelOpaque(img, x, y) {
    const key = img.src;
    if (!hitTestCache[key]) {
        const c = document.createElement('canvas');
        c.width = img.naturalWidth;
        c.height = img.naturalHeight;
        const cCtx = c.getContext('2d');
        cCtx.drawImage(img, 0, 0);
        hitTestCache[key] = cCtx;
    }
    try {
        const pixel = hitTestCache[key].getImageData(x, y, 1, 1).data;
        return pixel[3] > 30;
    } catch { return true; }
}

function findPieceAt(hx, hy) {
    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const lx = hx - comp.x;
        const ly = hy - comp.y;
        if (lx < 0 || ly < 0 || lx >= comp.w || ly >= comp.h) continue;

        try {
            const cCtx = comp.canvas.getContext('2d');
            const pixel = cCtx.getImageData(Math.round(lx), Math.round(ly), 1, 1).data;
            if (pixel[3] > 30) return piece.id;
        } catch { continue; }
    }
    return -1;
}

canvas.addEventListener('mousemove', (e) => {
    // Handle lasso drag
    if (isLassoing && assignMode) {
        const [hx, hy] = screenToHouse(e.clientX, e.clientY);
        lassoEndX = hx;
        lassoEndY = hy;
        const dx = Math.abs(lassoEndX - lassoStartX);
        const dy = Math.abs(lassoEndY - lassoStartY);
        if (dx > 5 || dy > 5) lassoWasDrag = true;
        render();
        return;
    }

    // Handle middle-button panning
    if (isPanning) {
        const dy = e.clientY - panStartY;
        panY = panStartPanY - dy;
        clampPan();
        render();
        return;
    }

    const [hx, hy] = screenToHouse(e.clientX, e.clientY);

    if (editMode) {
        const newHovered = findBrickAt(hx, hy);
        if (newHovered !== hoveredBrickId) {
            hoveredBrickId = newHovered;
            if (hoveredBrickId >= 0) {
                const b = bricks.find(br => br.id === hoveredBrickId);
                const inPiece = editBrickIds.includes(hoveredBrickId);
                document.getElementById('stat_hovered').textContent =
                    `#${b.id} (${b.width}×${b.height}) [${inPiece ? 'in piece' : 'not in piece'}]`;
            } else {
                document.getElementById('stat_hovered').textContent = '-';
            }
            render();
        }
    } else if (viewMode === 'pieces' && pieces.length) {
        const newHovered = findPieceAt(hx, hy);
        if (newHovered !== hoveredPieceId) {
            hoveredPieceId = newHovered;
            if (hoveredPieceId >= 0) {
                const p = pieces.find(pc => pc.id === hoveredPieceId);
                document.getElementById('stat_hovered').textContent =
                    `Piece #${p.id} (${p.num_bricks} bricks, ${p.width}×${p.height})`;
            } else {
                document.getElementById('stat_hovered').textContent = '-';
            }
            render();
        }
    }
});

canvas.addEventListener('mouseleave', () => {
    hoveredBrickId = -1;
    hoveredPieceId = -1;
    document.getElementById('stat_hovered').textContent = '-';
    render();
});

canvas.addEventListener('mousedown', (e) => {
    // Middle mouse button: pan
    if (e.button === 1) {
        e.preventDefault();
        isPanning = true;
        panStartY = e.clientY;
        panStartPanY = panY;
        canvas.style.cursor = 'grabbing';
        return;
    }

    // Left click in assign mode: start lasso
    if (e.button === 0 && assignMode && selectedWaveId >= 0 && !editMode) {
        const [hx, hy] = screenToHouse(e.clientX, e.clientY);
        isLassoing = true;
        lassoWasDrag = false;
        lassoStartX = hx;
        lassoStartY = hy;
        lassoEndX = hx;
        lassoEndY = hy;
        return;
    }
});

canvas.addEventListener('mouseup', (e) => {
    // End panning
    if (e.button === 1 && isPanning) {
        isPanning = false;
        canvas.style.cursor = 'crosshair';
        return;
    }

    // End lasso
    if (e.button === 0 && isLassoing) {
        isLassoing = false;
        finishLasso();
        render();
        return;
    }
});

canvas.addEventListener('click', (e) => {
    // If lasso drag just ended, don't process as click
    if (assignMode && lassoWasDrag) return;

    const [hx, hy] = screenToHouse(e.clientX, e.clientY);

    if (editMode) {
        const clickedId = findBrickAt(hx, hy);
        if (clickedId >= 0) {
            toggleBrickInEdit(clickedId);
        }
        return;
    }

    // Single click in assign/select mode: toggle piece in selected wave
    if (assignMode && selectedWaveId >= 0 && viewMode === 'pieces' && pieces.length) {
        const clickedId = findPieceAt(hx, hy);
        if (clickedId >= 0) {
            togglePieceInWave(clickedId, selectedWaveId);
        }
        return;
    }

    if (viewMode === 'pieces' && pieces.length) {
        const clickedId = findPieceAt(hx, hy);
        if (clickedId === selectedPieceId) {
            selectedPieceId = -1;
            highlightedPieceIds.clear();
            document.getElementById('stat_selected').textContent = '-';
            document.getElementById('editBtnRow').style.display = 'none';
        } else {
            selectedPieceId = clickedId;
            highlightedPieceIds.clear();
            shapeManualVerts = 0;
            if (clickedId >= 0) {
                const p = pieces.find(pc => pc.id === clickedId);
                document.getElementById('stat_selected').textContent =
                    `Piece #${p.id} (${p.num_bricks} bricks, ${p.width}×${p.height})`;
                if (viewMode === 'pieces') {
                    document.getElementById('editBtnRow').style.display = 'flex';
                }
                highlightPieceInPanel(clickedId);
            } else {
                document.getElementById('stat_selected').textContent = '-';
                document.getElementById('editBtnRow').style.display = 'none';
            }
        }
    }
    render();
});

canvas.addEventListener('contextmenu', (e) => e.preventDefault());

// Mouse wheel for vertical pan
canvas.addEventListener('wheel', (e) => {
    e.preventDefault();
    panY += e.deltaY;
    clampPan();
    render();
}, { passive: false });

// --- Wave system ---

function clearPieceSelection() {
    selectedPieceId = -1;
    highlightedPieceIds.clear();
    document.getElementById('stat_selected').textContent = '-';
    document.getElementById('editBtnRow').style.display = 'none';
    render();
}

function addWave() {
    clearPieceSelection();
    const wave = {
        id: nextWaveId++,
        name: `Wave ${waves.length + 1}`,
        pieceIds: [],
    };
    waves.push(wave);
    selectedWaveId = wave.id;
    saveState();
    renderWavesPanel();
}

function removeWave(waveId) {
    waves = waves.filter(w => w.id !== waveId);
    if (selectedWaveId === waveId) {
        selectedWaveId = waves.length > 0 ? waves[waves.length - 1].id : -1;
    }
    // Rename waves sequentially
    waves.forEach((w, i) => w.name = `Wave ${i + 1}`);
    saveState();
    renderWavesPanel();
}

function selectWave(waveId) {
    selectedWaveId = waveId;
    renderWavesPanel();
}

function moveWave(waveId, direction) {
    const idx = waves.findIndex(w => w.id === waveId);
    if (idx < 0) return;
    const newIdx = idx + direction;
    if (newIdx < 0 || newIdx >= waves.length) return;
    [waves[idx], waves[newIdx]] = [waves[newIdx], waves[idx]];
    waves.forEach((w, i) => w.name = `Wave ${i + 1}`);
    saveState();
    renderWavesPanel();
}

function toggleWaveVisibility(waveId) {
    if (hiddenWaveIds.has(waveId)) {
        hiddenWaveIds.delete(waveId);
    } else {
        hiddenWaveIds.add(waveId);
    }
    renderWavesPanel();
    requestAnimationFrame(drawThumbCanvases);
    render();
}

function updateSelectButtonState() {
    const btn = document.getElementById('assignModeBtn');
    btn.disabled = waves.length === 0;
    if (waves.length === 0 && assignMode) {
        assignMode = false;
        btn.classList.remove('active');
        document.getElementById('assignHint').style.display = 'none';
    }
}

function toggleAssignMode() {
    assignMode = !assignMode;
    clearPieceSelection();
    const btn = document.getElementById('assignModeBtn');
    const hint = document.getElementById('assignHint');
    if (assignMode) {
        btn.classList.add('active');
        hint.style.display = 'block';
        canvas.style.cursor = 'crosshair';
        if (selectedWaveId < 0 && waves.length > 0) {
            selectedWaveId = waves[0].id;
            renderWavesPanel();
        }
    } else {
        btn.classList.remove('active');
        hint.style.display = 'none';
        canvas.style.cursor = 'crosshair';
    }
}

function finishLasso() {
    if (selectedWaveId < 0) return;

    const wave = waves.find(w => w.id === selectedWaveId);
    if (!wave) return;

    const lx = Math.min(lassoStartX, lassoEndX);
    const ly = Math.min(lassoStartY, lassoEndY);
    const lw = Math.abs(lassoEndX - lassoStartX);
    const lh = Math.abs(lassoEndY - lassoStartY);

    // Too small = accidental click, ignore
    if (lw < 5 && lh < 5) return;

    // Find pieces whose center falls within the lasso rectangle
    const assignedInOtherWaves = new Set();
    for (const w of waves) {
        for (const pid of w.pieceIds) assignedInOtherWaves.add(pid);
    }

    let changed = false;
    highlightedPieceIds.clear();
    for (const piece of pieces) {
        const cx = piece.x + piece.width / 2;
        const cy = piece.y + piece.height / 2;
        if (cx >= lx && cx <= lx + lw && cy >= ly && cy <= ly + lh) {
            // Remove from any other wave
            for (const w of waves) {
                if (w.id !== selectedWaveId) {
                    const idx = w.pieceIds.indexOf(piece.id);
                    if (idx >= 0) w.pieceIds.splice(idx, 1);
                }
            }
            // Add to selected wave if not already there
            if (!wave.pieceIds.includes(piece.id)) {
                wave.pieceIds.push(piece.id);
                changed = true;
            }
            // Highlight selected pieces on canvas
            highlightedPieceIds.add(piece.id);
        }
    }

    if (changed || highlightedPieceIds.size > 0) {
        saveState();
        renderWavesPanel();
        render();
    }
}

function togglePieceInWave(pieceId, waveId) {
    const wave = waves.find(w => w.id === waveId);
    if (!wave) return;

    const idx = wave.pieceIds.indexOf(pieceId);
    if (idx >= 0) {
        // Remove from wave (deselect)
        wave.pieceIds.splice(idx, 1);
        highlightedPieceIds.delete(pieceId);
    } else {
        // Remove from any other wave first
        for (const w of waves) {
            if (w.id !== waveId) {
                const i = w.pieceIds.indexOf(pieceId);
                if (i >= 0) w.pieceIds.splice(i, 1);
            }
        }
        // Add to this wave
        wave.pieceIds.push(pieceId);
        highlightedPieceIds.add(pieceId);
    }

    saveState();
    renderWavesPanel();
    requestAnimationFrame(drawThumbCanvases);
    render();
}

function removePieceFromWave(waveId, pieceId) {
    const wave = waves.find(w => w.id === waveId);
    if (!wave) return;
    wave.pieceIds = wave.pieceIds.filter(id => id !== pieceId);
    saveState();
    renderWavesPanel();
}

function getUnassignedPieces() {
    const assigned = new Set();
    for (const wave of waves) {
        for (const pid of wave.pieceIds) assigned.add(pid);
    }
    return pieces.filter(p => !assigned.has(p.id));
}

// --- Wave panel rendering ---

const THUMB_MAX_H = 52; // max thumbnail height in px

function computeThumbScale(piecesArr) {
    // Compute a uniform scale so the tallest piece fits THUMB_MAX_H,
    // preserving relative sizes among all pieces in the group.
    if (!piecesArr.length) return 1;
    const maxH = Math.max(...piecesArr.map(p => p.height));
    return maxH > 0 ? THUMB_MAX_H / maxH : 1;
}

function renderWavesPanel() {
    const body = document.getElementById('wavesBody');
    const unassigned = getUnassignedPieces();

    // Update piece count
    const countEl = document.getElementById('wavePieceCount');
    if (pieces.length) {
        const assignedCount = waves.reduce((sum, w) => sum + w.pieceIds.length, 0);
        countEl.textContent = `${assignedCount}/${pieces.length}`;
    } else {
        countEl.textContent = '';
    }

    let html = '';

    // Unassigned pieces row
    if (unassigned.length > 0) {
        const uScale = computeThumbScale(unassigned);
        html += `<div class="wave-row" data-wave-id="unassigned">`;
        html += `<div class="wave-row-header" onclick="selectWave(-1)">`;
        html += `<span class="wave-label unassigned-label">Unassigned</span>`;
        html += `<span class="wave-piece-count">${unassigned.length} pcs</span>`;
        html += `</div>`;
        html += `<div class="wave-pieces" data-wave-id="unassigned">`;
        for (const piece of unassigned) {
            html += renderPieceThumb(piece, null, uScale);
        }
        html += `</div></div>`;
    }

    // Wave rows
    for (const wave of waves) {
        const isSelected = wave.id === selectedWaveId;
        // Collect pieces for this wave to compute uniform scale
        const wavePieces = wave.pieceIds.map(pid => pieces.find(p => p.id === pid)).filter(Boolean);
        const wScale = computeThumbScale(wavePieces);
        const isHidden = hiddenWaveIds.has(wave.id);
        html += `<div class="wave-row${isSelected ? ' selected' : ''}" data-wave-id="${wave.id}">`;
        html += `<div class="wave-row-header" onclick="selectWave(${wave.id})">`;
        html += `<span class="wave-eye${isHidden ? ' hidden' : ''}" onclick="event.stopPropagation(); toggleWaveVisibility(${wave.id})" title="${isHidden ? 'Show' : 'Hide'} pieces">${isHidden ? '&#9673;' : '&#9678;'}</span>`;
        html += `<span class="wave-label">${wave.name}</span>`;
        html += `<span class="wave-piece-count">${wave.pieceIds.length} pcs</span>`;
        html += `<span class="wave-actions">`;
        const wIdx = waves.indexOf(wave);
        if (wIdx > 0) {
            html += `<button onclick="event.stopPropagation(); moveWave(${wave.id}, -1)" title="Move up">&#9650;</button>`;
        }
        if (wIdx < waves.length - 1) {
            html += `<button onclick="event.stopPropagation(); moveWave(${wave.id}, 1)" title="Move down">&#9660;</button>`;
        }
        html += `<button onclick="event.stopPropagation(); removeWave(${wave.id})" title="Delete wave">&#10005;</button>`;
        html += `</span>`;
        html += `</div>`;
        html += `<div class="wave-pieces" data-wave-id="${wave.id}">`;
        for (const piece of wavePieces) {
            html += renderPieceThumb(piece, wave.id, wScale);
        }
        if (wave.pieceIds.length === 0) {
            html += `<span style="color:#555; font-size:10px; padding:4px;">Drop pieces here</span>`;
        }
        html += `</div></div>`;
    }

    if (waves.length === 0 && unassigned.length === 0) {
        html += `<div style="padding:16px; color:#555; font-size:12px; text-align:center;">
            Generate a puzzle first, then create waves to organize pieces.
        </div>`;
    }

    body.innerHTML = html;

    // Set up drag and drop
    setupWaveDragDrop();
    updateSelectButtonState();
}

function renderPieceThumb(piece, waveId, thumbScale) {
    // thumbScale is pixels-per-house-pixel, uniform across a wave row
    const thumbW = Math.max(8, Math.round(piece.width * thumbScale));
    const thumbH = Math.max(8, Math.round(piece.height * thumbScale));

    const isHighlighted = highlightedPieceIds.has(piece.id);
    const isSelected = piece.id === selectedPieceId;

    let cls = 'piece-thumb';
    if (isSelected) cls += ' selected';
    if (isHighlighted) cls += ' highlighted';

    const waveAttr = waveId !== null ? `data-wave-id="${waveId}"` : 'data-wave-id="unassigned"';

    return `<div class="${cls}" data-piece-id="${piece.id}" ${waveAttr}
        draggable="true"
        onmouseenter="onThumbHover(${piece.id})"
        onmouseleave="onThumbLeave(${piece.id})"
        onclick="onThumbClick(${piece.id})"
        title="Piece #${piece.id} (${piece.num_bricks} bricks, ${piece.width}x${piece.height})">
        <canvas width="${thumbW}" height="${thumbH}" data-piece-id="${piece.id}"></canvas>
        <div class="piece-thumb-label">#${piece.id}</div>
    </div>`;
}

// Draw piece thumbnails after DOM insertion
function drawThumbCanvases() {
    const thumbs = document.querySelectorAll('.piece-thumb canvas[data-piece-id]');
    for (const thumbCanvas of thumbs) {
        const pid = parseInt(thumbCanvas.dataset.pieceId);
        const comp = pieceComposites[pid];
        if (!comp) continue;

        const tCtx = thumbCanvas.getContext('2d');
        const scale = thumbCanvas.height / Math.max(comp.h, 1);
        tCtx.clearRect(0, 0, thumbCanvas.width, thumbCanvas.height);
        tCtx.drawImage(comp.canvas, 0, 0, comp.w * scale, comp.h * scale);
    }
}

// Observer: draw canvases when they appear in DOM
const thumbObserver = new MutationObserver(() => {
    drawThumbCanvases();
});
thumbObserver.observe(document.getElementById('wavesBody'), { childList: true, subtree: true });

// Also draw on initial render
function renderWavesPanelAndThumbs() {
    renderWavesPanel();
    requestAnimationFrame(drawThumbCanvases);
}

// --- Piece thumb interactions ---

function onThumbHover(pieceId) {
    highlightedPieceIds.add(pieceId);
    render();
}

function onThumbLeave(pieceId) {
    highlightedPieceIds.delete(pieceId);
    render();
}

function onThumbClick(pieceId) {
    // Clear all multi-selection highlights, select just this one
    selectedPieceId = pieceId;
    highlightedPieceIds.clear();

    const p = pieces.find(pc => pc.id === pieceId);
    if (p) {
        document.getElementById('stat_selected').textContent =
            `Piece #${p.id} (${p.num_bricks} bricks, ${p.width}×${p.height})`;
        scrollCanvasToPiece(p);
    }
    render();
    // Re-render wave panel so all thumbs update their CSS classes
    renderWavesPanel();
    requestAnimationFrame(drawThumbCanvases);
}

function highlightPieceInPanel(pieceId) {
    // Scroll the wave panel to show the piece
    const thumb = document.querySelector(`.piece-thumb[data-piece-id="${pieceId}"]`);
    if (thumb) {
        thumb.scrollIntoView({ behavior: 'smooth', block: 'nearest', inline: 'nearest' });
        // Flash highlight
        document.querySelectorAll('.piece-thumb').forEach(el => {
            el.classList.toggle('selected', parseInt(el.dataset.pieceId) === pieceId);
        });
    }
}

function scrollCanvasToPiece(piece) {
    // Calculate where the piece center is and adjust panY to center it
    const rect = canvasArea.getBoundingClientRect();
    const pieceCenterY = piece.y + piece.height / 2;
    const screenCenterY = rect.height / 2;
    const padY_ = (rect.height - canvasH * zoom) / 2;
    const targetPanY = (pieceCenterY * zoom + padY_) - screenCenterY;

    if (targetPanY > 0) {
        panY = targetPanY;
        clampPan();
        render();
    }
}

// --- Drag and drop for wave pieces ---

function clearDropMarkers() {
    document.querySelectorAll('.piece-thumb.drop-before').forEach(el => el.classList.remove('drop-before'));
    document.querySelectorAll('.piece-thumb.drop-after').forEach(el => el.classList.remove('drop-after'));
    document.querySelectorAll('.wave-pieces.drop-target-inside').forEach(el => el.classList.remove('drop-target-inside'));
    document.querySelectorAll('.wave-row.drop-target').forEach(el => el.classList.remove('drop-target'));
}

// Find the insertion index within a wave-pieces container based on cursor X
function getDropIndex(container, clientX) {
    const thumbs = container.querySelectorAll('.piece-thumb');
    if (thumbs.length === 0) return 0;

    for (let i = 0; i < thumbs.length; i++) {
        const rect = thumbs[i].getBoundingClientRect();
        const mid = rect.left + rect.width / 2;
        if (clientX < mid) return i;
    }
    return thumbs.length;
}

function showDropMarker(container, clientX) {
    clearDropMarkers();
    const thumbs = container.querySelectorAll('.piece-thumb');
    if (thumbs.length === 0) {
        container.classList.add('drop-target-inside');
        return;
    }

    const idx = getDropIndex(container, clientX);
    if (idx === 0) {
        thumbs[0].classList.add('drop-before');
    } else {
        thumbs[idx - 1].classList.add('drop-after');
    }
    container.closest('.wave-row').classList.add('drop-target');
}

function setupWaveDragDrop() {
    const thumbs = document.querySelectorAll('.piece-thumb[draggable="true"]');
    const wavePiecesContainers = document.querySelectorAll('.wave-pieces');

    for (const thumb of thumbs) {
        thumb.addEventListener('dragstart', (e) => {
            dragPieceId = parseInt(thumb.dataset.pieceId);
            dragSourceWaveId = thumb.dataset.waveId === 'unassigned' ? null : parseInt(thumb.dataset.waveId);
            thumb.classList.add('dragging');
            e.dataTransfer.effectAllowed = 'move';
            e.dataTransfer.setData('text/plain', String(dragPieceId));
        });

        thumb.addEventListener('dragend', () => {
            thumb.classList.remove('dragging');
            dragPieceId = -1;
            dragSourceWaveId = null;
            clearDropMarkers();
        });
    }

    for (const container of wavePiecesContainers) {
        container.addEventListener('dragover', (e) => {
            e.preventDefault();
            e.dataTransfer.dropEffect = 'move';
            showDropMarker(container, e.clientX);
        });

        container.addEventListener('dragleave', (e) => {
            // Only clear if we actually left the container
            if (!container.contains(e.relatedTarget)) {
                clearDropMarkers();
            }
        });

        container.addEventListener('drop', (e) => {
            e.preventDefault();
            const targetWaveId = container.dataset.waveId === 'unassigned' ? null : parseInt(container.dataset.waveId);
            const insertIdx = getDropIndex(container, e.clientX);
            clearDropMarkers();
            if (dragPieceId < 0) return;

            movePieceToWaveAt(dragPieceId, dragSourceWaveId, targetWaveId, insertIdx);
        });
    }
}

function movePieceToWaveAt(pieceId, fromWaveId, toWaveId, insertIdx) {
    // Remove from source wave
    if (fromWaveId !== null) {
        const srcWave = waves.find(w => w.id === fromWaveId);
        if (srcWave) {
            const srcIdx = srcWave.pieceIds.indexOf(pieceId);
            if (srcIdx >= 0) {
                srcWave.pieceIds.splice(srcIdx, 1);
                // If moving within the same wave and removing before insert point, adjust index
                if (fromWaveId === toWaveId && srcIdx < insertIdx) {
                    insertIdx--;
                }
            }
        }
    }
    // Also remove from any other wave
    for (const w of waves) {
        if (toWaveId !== null && w.id !== toWaveId) {
            w.pieceIds = w.pieceIds.filter(id => id !== pieceId);
        }
        if (toWaveId === null) {
            w.pieceIds = w.pieceIds.filter(id => id !== pieceId);
        }
    }

    // Insert at position in target wave
    if (toWaveId !== null) {
        const dstWave = waves.find(w => w.id === toWaveId);
        if (dstWave) {
            // Remove if somehow already there (shouldn't be after above cleanup)
            dstWave.pieceIds = dstWave.pieceIds.filter(id => id !== pieceId);
            // Clamp index
            const idx = Math.max(0, Math.min(insertIdx, dstWave.pieceIds.length));
            dstWave.pieceIds.splice(idx, 0, pieceId);
        }
    }

    saveState();
    renderWavesPanel();
    requestAnimationFrame(drawThumbCanvases);
}

// --- Slider updates ---

document.getElementById('target_count').addEventListener('input', (e) => {
    document.getElementById('val_target_count').textContent = e.target.value;
    checkPresetDirty();
});
document.getElementById('seed').addEventListener('input', (e) => {
    document.getElementById('val_seed').textContent = e.target.value;
    checkPresetDirty();
});
document.getElementById('min_border').addEventListener('input', (e) => {
    document.getElementById('val_min_border').textContent = e.target.value;
    checkPresetDirty();
});

// --- Presets ---

function getCurrentParamValues() {
    const vals = {};
    for (const id of PARAM_IDS) {
        const el = document.getElementById(id);
        vals[id] = el.type === 'number' ? parseInt(el.value) : parseInt(el.value);
    }
    return vals;
}

function applyParamValues(vals) {
    for (const [id, val] of Object.entries(vals)) {
        const el = document.getElementById(id);
        if (!el) continue;
        el.value = val;
        const label = document.getElementById('val_' + id);
        if (label) label.textContent = val;
    }
}

async function loadPresetList() {
    try {
        const resp = await fetch('/api/presets');
        const data = await resp.json();
        const select = document.getElementById('presetSelect');
        const current = currentPresetName;
        select.innerHTML = '<option value="">-- Preset --</option>';
        for (const name of data.presets) {
            const opt = document.createElement('option');
            opt.value = name;
            opt.textContent = name;
            if (name === current) opt.selected = true;
            select.appendChild(opt);
        }

    } catch (e) { /* ignore */ }
}

async function loadPreset(name) {
    if (!name) {
        currentPresetName = '';
        currentPresetValues = null;
        document.getElementById('presetReloadBtn').style.display = 'none';
        return;
    }
    try {
        const resp = await fetch('/api/presets/' + encodeURIComponent(name));
        const data = await resp.json();
        if (data.error) { alert(data.error); return; }
        applyParamValues(data);
        currentPresetName = name;
        currentPresetValues = getCurrentParamValues();
        document.getElementById('presetReloadBtn').style.display = 'none';
        saveState();
    } catch (e) {
        alert('Failed to load preset: ' + e.message);
    }
}

function reloadPreset() {
    if (currentPresetName) loadPreset(currentPresetName);
}

function checkPresetDirty() {
    if (!currentPresetValues) {
        document.getElementById('presetReloadBtn').style.display = 'none';
        saveState();
        return;
    }
    const current = getCurrentParamValues();
    let dirty = false;
    for (const id of PARAM_IDS) {
        if (current[id] !== currentPresetValues[id]) { dirty = true; break; }
    }
    document.getElementById('presetReloadBtn').style.display = dirty ? '' : 'none';
    saveState();
}

async function savePresetAs() {
    const name = prompt('Preset name:', currentPresetName || 'New Preset');
    if (!name) return;
    const params = getCurrentParamValues();
    try {
        const resp = await fetch('/api/presets', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({ name, ...params }),
        });
        const data = await resp.json();
        if (data.error) { alert(data.error); return; }
        currentPresetName = name;
        currentPresetValues = getCurrentParamValues();
        await loadPresetList();
        document.getElementById('presetSelect').value = name;
        document.getElementById('presetReloadBtn').style.display = 'none';
        saveState();
    } catch (e) {
        alert('Failed to save preset: ' + e.message);
    }
}


// --- Resize ---

window.addEventListener('resize', () => {
    fitCanvas();
    render();
});

// --- Helpers ---

function showLoading(msg) {
    loading.textContent = msg || 'Loading...';
    loading.classList.add('active');
}

function hideLoading() {
    loading.classList.remove('active');
}

// --- Waves panel resize ---

(function setupWavesResize() {
    const handle = document.getElementById('wavesResizeHandle');
    const panel = document.getElementById('wavesPanel');
    let startX = 0;
    let startW = 0;

    handle.addEventListener('mousedown', (e) => {
        e.preventDefault();
        startX = e.clientX;
        startW = panel.offsetWidth;
        handle.classList.add('active');
        document.addEventListener('mousemove', onMove);
        document.addEventListener('mouseup', onUp);
    });

    function onMove(e) {
        // Dragging left = making panel wider (panel is on the right)
        const delta = startX - e.clientX;
        const newW = Math.max(180, Math.min(600, startW + delta));
        panel.style.width = newW + 'px';
        fitCanvas();
        render();
    }

    function onUp() {
        handle.classList.remove('active');
        document.removeEventListener('mousemove', onMove);
        document.removeEventListener('mouseup', onUp);
        renderWavesPanel();
        requestAnimationFrame(drawThumbCanvases);
    }
})();

// --- Start ---

init();
