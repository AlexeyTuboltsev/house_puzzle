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
let viewMode = 'bricks';

// Piece edit mode state
let editMode = false;
let shapeManualVerts = 0; // 0 = auto, >0 = manual override from slider
let editPieceId = -1;
let editBrickIds = [];        // working copy of brick_ids being edited
let originalBrickIds = [];    // snapshot to detect changes / revert

// Pre-rendered piece composites: pieceId -> { canvas, x, y }
let pieceComposites = {};

// View scale (fixed, fit to viewport)
let zoom = 1;

const canvas = document.getElementById('houseCanvas');
const ctx = canvas.getContext('2d');
const canvasArea = document.getElementById('canvasArea');
const loading = document.getElementById('loadingOverlay');

// --- Initialization ---

async function init() {
    fitCanvas();
    render();
    await loadTifList();
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

async function loadTif() {
    const path = document.getElementById('tifSelect').value;
    if (!path) return;

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
        document.getElementById('canvasInfo').textContent =
            `${canvasW}×${canvasH} | ${data.num_bricks} bricks | Click to select`;

    } catch (err) {
        alert('Failed to load TIF: ' + err.message);
    } finally {
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
                    `${canvasW}×${canvasH} | ${total} bricks loaded | Click to select`;
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
    showLoading('Merging bricks...');

    try {
        const resp = await fetch('/api/merge', {
            method: 'POST',
            headers: { 'Content-Type': 'application/json' },
            body: JSON.stringify({
                target_count: parseInt(document.getElementById('target_count').value),
                seed: parseInt(document.getElementById('seed').value),
                windows_separate: document.getElementById('windows_separate').checked,
                max_width: parseInt(document.getElementById('max_width').value),
                max_height: parseInt(document.getElementById('max_height').value),
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
        document.getElementById('stat_pieces').textContent = data.num_pieces;
        document.getElementById('stat_selected').textContent = '-';
        document.getElementById('exportBtn').disabled = false;
        document.getElementById('blueprintBtn').disabled = false;

        buildPieceComposites();
        setView('pieces');
        render();

    } catch (err) {
        alert('Merge failed: ' + err.message);
    } finally {
        hideLoading();
    }
}

// --- Export ---

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
    document.getElementById(btnId).classList.add('active');
    document.getElementById('editBtnRow').style.display = 'none';
    document.getElementById('blueprintParams').style.display = mode === 'blueprint' ? 'block' : 'none';
    document.getElementById('shapeParams').style.display = mode === 'shape' ? 'block' : 'none';
    // Hide SVG overlay when not in blueprint/shape mode
    if (mode !== 'blueprint' && mode !== 'shape') {
        const svg = document.getElementById('blueprintSvg');
        svg.style.display = 'none';
        svg.innerHTML = '';
    }
    render();
}

function showBlueprint() { setView('blueprint'); }

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
    // Fit to viewport: use whichever dimension is more constraining
    const zoomW = (rect.width - pad * 2) / canvasW;
    const zoomH = (rect.height - pad * 2 - infoH) / canvasH;
    zoom = Math.min(zoomW, zoomH);
    canvas.width = rect.width;
    canvas.height = rect.height;
    canvasArea.style.overflow = 'hidden';
}

function resetView() {
    fitCanvas();
}

function render() {
    const w = canvas.width;
    const h = canvas.height;
    ctx.clearRect(0, 0, w, h);
    if (!canvasW) return;

    ctx.save();
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY = (canvas.height - canvasH * zoom) / 2;
    ctx.translate(padX, padY);
    ctx.scale(zoom, zoom);

    // Draw composite background (not for blueprint)
    if (compositeImg && compositeImg.complete && viewMode !== 'blueprint') {
        ctx.globalAlpha = (viewMode === 'bricks') ? 0.4 : 0.7;
        ctx.drawImage(compositeImg, 0, 0, canvasW, canvasH);
        ctx.globalAlpha = 1.0;
    }

    if (editMode) {
        renderEditMode();
    } else if (viewMode === 'bricks') {
        renderBricks();
    } else if (viewMode === 'pieces') {
        renderPieces();
    } else if (viewMode === 'blueprint') {
        renderBlueprint();
    } else if (viewMode === 'shape') {
        renderShape();
    }

    ctx.restore();
}

function getBrickComp(brick) {
    // Wrap a brick image as a composite object compatible with drawPieceSilhouetteOutline
    const img = brickImages[brick.id];
    if (!img) return null;
    // Use a cached offscreen canvas per brick
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

    // Draw hover outline on top of all bricks
    if (hoveredBrickId >= 0 && hoveredBrickId !== selectedBrickId) {
        const brick = bricks.find(b => b.id === hoveredBrickId);
        if (brick) {
            const img = brickImages[brick.id];
            if (img) ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
            const comp = getBrickComp(brick);
            if (comp) drawPieceSilhouetteOutline(comp, 'rgba(60, 200, 255, 0.8)', 3);
        }
    }

    // Draw selected brick outline on top of everything
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
    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const isHovered = piece.id === hoveredPieceId;
        const isSelected = piece.id === selectedPieceId;
        const hue = (piece.id * 137.508) % 360;

        // Draw the piece composite image
        ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);

        // Color tint overlay (using the composite as shape mask)
        if (!isSelected) {
            const tintAlpha = isHovered ? 0.3 : 0.12;
            const tint = makeTintedCanvas(comp.canvas, hue, tintAlpha);
            ctx.drawImage(tint, comp.x, comp.y, comp.w, comp.h);
        }

        // Label
        if (zoom > 0.12) {
            ctx.fillStyle = isSelected
                ? 'rgba(255, 96, 48, 0.95)'
                : `hsla(${hue}, 80%, 85%, 0.85)`;
            ctx.font = `bold ${Math.round(13 / zoom)}px sans-serif`;
            ctx.textAlign = 'center';
            ctx.textBaseline = 'middle';
            ctx.fillText(`#${piece.id}`, comp.x + comp.w / 2, comp.y + comp.h / 2);
        }
    }

    // Draw hover outline on top of all pieces
    if (hoveredPieceId >= 0 && hoveredPieceId !== selectedPieceId) {
        const piece = pieces.find(p => p.id === hoveredPieceId);
        if (piece) {
            const comp = pieceComposites[piece.id];
            if (comp) {
                ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);
                drawPieceSilhouetteOutline(comp, 'rgba(0, 0, 0, 0.9)', 1);
            }
        }
    }

    // Draw selected piece on top for prominence
    if (selectedPieceId >= 0) {
        const piece = pieces.find(p => p.id === selectedPieceId);
        if (piece) {
            const comp = pieceComposites[piece.id];
            if (comp) {
                ctx.drawImage(comp.canvas, comp.x, comp.y, comp.w, comp.h);
                drawPieceSilhouetteOutline(comp, 'rgba(0, 0, 0, 0.9)', 1);

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
    // Use shadow trick: draw the piece composite with a colored shadow,
    // offset to create outline on all sides, then mask out the interior.
    //
    // Technique: draw the silhouette 4 times with shadow offsets in each direction.
    // This creates a glow/outline that follows the actual alpha shape.

    ctx.save();

    // Create a clipping region that EXCLUDES the piece interior
    // so only the shadow (outline) is visible outside the shape
    const outlineCanvas = document.createElement('canvas');
    const pad = thickness * 2;
    outlineCanvas.width = comp.w + pad * 2;
    outlineCanvas.height = comp.h + pad * 2;
    const oCtx = outlineCanvas.getContext('2d');

    // Draw shadow-expanded silhouette
    oCtx.shadowColor = color;
    oCtx.shadowBlur = thickness / zoom;

    // Draw from multiple offsets for uniform outline
    const offsets = [
        [pad, pad - 1], [pad, pad + 1],
        [pad - 1, pad], [pad + 1, pad],
    ];
    for (const [ox, oy] of offsets) {
        oCtx.drawImage(comp.canvas, ox, oy);
    }

    // Remove the interior by drawing the piece shape with destination-out
    oCtx.shadowColor = 'transparent';
    oCtx.shadowBlur = 0;
    oCtx.globalCompositeOperation = 'destination-out';
    oCtx.drawImage(comp.canvas, pad, pad);

    // Draw the outline canvas onto the main canvas
    ctx.drawImage(outlineCanvas, comp.x - pad, comp.y - pad);

    ctx.restore();
}

function makeTintedCanvas(srcCanvas, hue, alpha) {
    // Create a tinted version of the source canvas (preserving alpha shape)
    const tint = document.createElement('canvas');
    tint.width = srcCanvas.width;
    tint.height = srcCanvas.height;
    const tCtx = tint.getContext('2d');

    // Draw source to get the alpha mask
    tCtx.drawImage(srcCanvas, 0, 0);

    // Apply color using source-in compositing
    tCtx.globalCompositeOperation = 'source-in';
    tCtx.fillStyle = `hsla(${hue}, 60%, 50%, ${alpha})`;
    tCtx.fillRect(0, 0, tint.width, tint.height);

    return tint;
}

function renderBlueprint() {
    if (!pieces.length) return;

    const blueprintBlue = '#2a5da8';
    const epsilon = parseFloat(document.getElementById('smoothing').value);

    // 1. Fill all piece areas with solid blue on canvas
    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;
        const solid = makeSolidCanvas(comp.canvas, blueprintBlue);
        ctx.drawImage(solid, comp.x, comp.y, comp.w, comp.h);
    }

    // 2. Trace each piece outline as a closed SVG polygon
    const svg = document.getElementById('blueprintSvg');
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY = (canvas.height - canvasH * zoom) / 2;
    svg.setAttribute('width', canvas.width);
    svg.setAttribute('height', canvas.height);
    svg.style.display = 'block';

    const strokeW = Math.max(3, 4 * zoom);
    let svgContent = '';

    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const contours = tracePieceContours(comp, epsilon);

        for (const pts of contours) {
            if (pts.length < 3) continue;
            let d = '';
            for (let i = 0; i < pts.length; i++) {
                const sx = (comp.x + pts[i][0]) * zoom + padX;
                const sy = (comp.y + pts[i][1]) * zoom + padY;
                d += (i === 0 ? 'M' : 'L') + sx.toFixed(1) + ',' + sy.toFixed(1);
            }
            d += 'Z';
            svgContent += `<path d="${d}" fill="none" stroke="white" stroke-width="${strokeW.toFixed(1)}" stroke-linejoin="round" stroke-linecap="round"/>`;
        }
    }

    svg.innerHTML = svgContent;
}

function tracePieceContours(comp, epsilon) {
    // Create thresholded binary mask
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

    // Build directed boundary edges between grid vertices (0..W, 0..H).
    // Convention: opaque cell is on the RIGHT side of travel direction.
    // This ensures following edges traces a CW contour around opaque regions.
    const edgeMap = new Map(); // "x,y" -> [{tx, ty, used}]

    function addEdge(fx, fy, tx, ty) {
        const k = fx + ',' + fy;
        if (!edgeMap.has(k)) edgeMap.set(k, []);
        edgeMap.get(k).push({tx, ty, used: false});
    }

    // Horizontal edges: between vertex (x,y) and (x+1,y)
    for (let y = 0; y <= H; y++) {
        for (let x = 0; x < W; x++) {
            const above = cell(x, y - 1), below = cell(x, y);
            if (above && !below) addEdge(x, y, x + 1, y);
            else if (!above && below) addEdge(x + 1, y, x, y);
        }
    }

    // Vertical edges: between vertex (x,y) and (x,y+1)
    for (let x = 0; x <= W; x++) {
        for (let y = 0; y < H; y++) {
            const left = cell(x - 1, y), right = cell(x, y);
            if (right && !left) addEdge(x, y + 1, x, y);
            else if (!right && left) addEdge(x, y, x, y + 1);
        }
    }

    // Chain directed edges into closed loops.
    // At junctions (saddle points with 2+ outgoing edges), prefer right turns.
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

                // Pick next edge: prefer right turn, then straight, then left
                const turns = [
                    [-dy, dx],   // right (CW 90)
                    [dx, dy],    // straight
                    [dy, -dx],   // left (CCW 90)
                    [-dx, -dy],  // u-turn
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

    // Simplify each contour
    return contours.map(c => douglasPeuckerClosed(c, epsilon));
}

function douglasPeuckerClosed(points, epsilon) {
    // For closed polygons: find the two farthest points, split into two
    // open polylines, simplify each, and recombine.
    if (points.length <= 4 || epsilon <= 0) return points;

    // Find the two points with maximum distance
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

    // Split into two halves
    const half1 = points.slice(idxA, idxB + 1);
    const half2 = points.slice(idxB).concat(points.slice(0, idxA + 1));

    const s1 = douglasPeucker(half1, epsilon);
    const s2 = douglasPeucker(half2, epsilon);

    // Combine (remove duplicate junction points)
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
    // 1. Build binary mask from piece composite
    // 2. Downscale to coarse grid — any cell with opaque pixels → filled
    // 3. Trace outer contour of coarse grid
    // 4. Scale back up and snap to nearest original boundary pixel
    const W = comp.w, H = comp.h;
    const CELL = 5; // cell size — gaps smaller than this are bridged
    const PAD = 3;   // padding cells around the grid for exterior flood fill
    const GW = Math.ceil(W / CELL) + PAD * 2;
    const GH = Math.ceil(H / CELL) + PAD * 2;

    const c = document.createElement('canvas');
    c.width = W; c.height = H;
    const cCtx = c.getContext('2d');
    cCtx.drawImage(comp.canvas, 0, 0);
    const data = cCtx.getImageData(0, 0, W, H).data;

    // Collect original boundary pixels (for snapping later)
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

    // Build coarse grid (with 1-cell padding so contour tracing has clear exterior)
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

    // Dilate coarse grid by 1 cell to bridge small gaps between bricks
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

    // Flood fill exterior from border
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

    // Solid = everything not exterior
    const solid = new Uint8Array(GW * GH);
    for (let i = 0; i < GW * GH; i++) solid[i] = exterior[i] ? 0 : 1;

    // Moore neighborhood boundary tracing on the solid grid
    // This produces a properly ordered, non-self-intersecting contour
    // 8-connected neighbors clockwise: right, down-right, down, down-left, left, up-left, up, up-right
    const mooreX = [1, 1, 0, -1, -1, -1, 0, 1];
    const mooreY = [0, 1, 1, 1, 0, -1, -1, -1];

    // Find topmost-leftmost solid cell as start
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

    // Trace boundary using Moore neighborhood
    const traced = [];
    const visited = new Set();
    let curX = startX, curY = startY;
    // We entered from the left (since it's the leftmost in its row), so backtrack direction is 4 (left)
    let backDir = 4; // direction index pointing to the cell we came from

    do {
        const key = curY * GW + curX;
        if (!visited.has(key)) {
            traced.push([curX, curY]);
            visited.add(key);
        }

        // Scan clockwise starting from (backDir + 1) % 8
        let found = false;
        for (let i = 1; i <= 8; i++) {
            const dir = (backDir + i) % 8;
            const nx = curX + mooreX[dir];
            const ny = curY + mooreY[dir];
            if (nx >= 0 && nx < GW && ny >= 0 && ny < GH && solid[ny * GW + nx]) {
                // backtrack direction for next cell = opposite of dir
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

    // Filter to only boundary cells (adjacent to exterior) to remove interior path segments
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

    // Convert to pixel coords and snap to nearest original boundary pixel
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

// Compute max distance from any point in `pts` to the nearest edge of polygon `poly`
function hausdorffToPoly(pts, poly) {
    let maxDist = 0;
    for (const [px, py] of pts) {
        let minD = Infinity;
        for (let i = 0; i < poly.length; i++) {
            const j = (i + 1) % poly.length;
            const [ax, ay] = poly[i], [bx, by] = poly[j];
            // Point-to-segment distance squared
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

// Collapse close vertex pairs at concave corners into a single corner apex vertex
// When two consecutive vertices are close, they likely straddle a concave corner.
// Replace them with the outline point deepest into the corner (furthest from the
// line connecting their neighbors).
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

            // Close pair found — find the concave corner apex
            const prevIdx = (i - 1 + pts.length) % pts.length;
            const nextIdx = (j + 1) % pts.length;
            const prev = pts[prevIdx];
            const next = pts[nextIdx];

            // Search outline points near the midpoint of the pair
            const mid = [(pts[i][0] + pts[j][0]) / 2, (pts[i][1] + pts[j][1]) / 2];
            const searchR2 = (threshold * 3) * (threshold * 3);

            // Line prev→next direction
            const lx = next[0] - prev[0], ly = next[1] - prev[1];
            const lineLen = Math.sqrt(lx * lx + ly * ly);

            let maxDist = -1;
            let bestPt = mid;
            for (const op of outline) {
                const dm2 = (op[0] - mid[0]) ** 2 + (op[1] - mid[1]) ** 2;
                if (dm2 > searchR2) continue;
                if (lineLen > 0) {
                    // Perpendicular distance from line(prev, next)
                    const d = Math.abs(lx * (prev[1] - op[1]) - ly * (prev[0] - op[0])) / lineLen;
                    if (d > maxDist) {
                        maxDist = d;
                        bestPt = op;
                    }
                }
            }

            // Try the collapse and check if it worsens the fit
            const candidate = [...pts];
            candidate[i] = [...bestPt];
            if (j > i) {
                candidate.splice(j, 1);
            } else {
                candidate.splice(0, 1);
            }
            const newDist = hausdorffToPoly(outline, candidate);
            if (newDist > maxDeviation) continue; // skip — would worsen fit too much

            // Accept the collapse
            pts = candidate;
            changed = true;
            break; // restart scan after modification
        }
    }
    return pts;
}

// Find minimum vertices where max deviation from full outline stays within tolerance
function autoSimplify(outline, tolerance) {
    if (outline.length <= 3) return outline;

    // Binary search: find smallest n where hausdorff <= tolerance
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

function renderShape() {
    if (!pieces.length) return;

    // Find the piece to show — use selected piece, or first piece
    const pieceId = selectedPieceId >= 0 ? selectedPieceId : pieces[0].id;
    const piece = pieces.find(p => p.id === pieceId);
    if (!piece) return;
    const comp = pieceComposites[piece.id];
    if (!comp) return;

    // Draw the piece composite as black silhouette
    const silCanvas = document.createElement('canvas');
    silCanvas.width = comp.w; silCanvas.height = comp.h;
    const silCtx = silCanvas.getContext('2d');
    silCtx.drawImage(comp.canvas, 0, 0);
    silCtx.globalCompositeOperation = 'source-in';
    silCtx.fillStyle = 'black';
    silCtx.fillRect(0, 0, comp.w, comp.h);
    ctx.drawImage(silCanvas, comp.x, comp.y);

    // Draw the precise raster outline (white) as reference
    drawPieceSilhouetteOutline(comp, 'white', 2);

    // Coarse grid trace + snap to boundary
    const outline = coarseTraceSnap(comp);
    if (outline.length < 3) return;

    const slider = document.getElementById('targetVerts');
    let simplified;
    if (shapeManualVerts > 0) {
        // Manual slider override
        simplified = visvalingamWhyatt(outline, shapeManualVerts);
    } else {
        // Auto-optimize: find min vertices within tolerance
        simplified = autoSimplify(outline, 1);
    }
    // Collapse close vertex pairs at concave corners (threshold = 4 * CELL, max deviation = 1px)
    simplified = refineCorners(simplified, outline, 20, 1);
    slider.value = simplified.length;
    document.getElementById('val_targetVerts').textContent = simplified.length;

    // Draw the simplified shape as SVG overlay (if checkbox enabled)
    const svg = document.getElementById('blueprintSvg');
    const showVector = document.getElementById('showVector').checked;
    if (!showVector) {
        svg.style.display = 'none';
        return;
    }
    const padX = (canvas.width - canvasW * zoom) / 2;
    const padY = (canvas.height - canvasH * zoom) / 2;
    svg.setAttribute('width', canvas.width);
    svg.setAttribute('height', canvas.height);
    svg.style.display = 'block';

    const strokeW = Math.max(2, 3 * zoom);

    // Build path
    let d = '';
    for (let i = 0; i < simplified.length; i++) {
        const sx = (comp.x + simplified[i][0]) * zoom + padX;
        const sy = (comp.y + simplified[i][1]) * zoom + padY;
        d += (i === 0 ? 'M' : 'L') + sx.toFixed(1) + ',' + sy.toFixed(1);
    }
    d += 'Z';

    // Draw filled shape + outline + vertex dots
    let svgContent = `<path d="${d}" fill="rgba(42,93,168,0.3)" stroke="#ff6030" stroke-width="${strokeW.toFixed(1)}" stroke-linejoin="round"/>`;

    // Draw vertex markers
    const dotR = Math.max(3, 4 * zoom);
    for (let i = 0; i < simplified.length; i++) {
        const sx = (comp.x + simplified[i][0]) * zoom + padX;
        const sy = (comp.y + simplified[i][1]) * zoom + padY;
        const fillColor = i === 0 ? '#00ff80' : '#ff6030';
        svgContent += `<circle cx="${sx.toFixed(1)}" cy="${sy.toFixed(1)}" r="${dotR.toFixed(1)}" fill="${fillColor}" stroke="white" stroke-width="1"/>`;
        const fs = Math.max(9, 11 * zoom);
        svgContent += `<text x="${(sx + dotR + 2).toFixed(1)}" y="${(sy - dotR - 1).toFixed(1)}" fill="yellow" font-size="${fs}" font-weight="bold">${i}</text>`;
    }

    // Label with vertex count
    const labelX = (comp.x + comp.w / 2) * zoom + padX;
    const labelY = (comp.y - 10) * zoom + padY;
    svgContent += `<text x="${labelX.toFixed(1)}" y="${labelY.toFixed(1)}" fill="#ff6030" font-size="${Math.max(14, 16 * zoom)}" text-anchor="middle" font-weight="bold">Piece #${piece.id} — ${simplified.length} vertices</text>`;

    svg.innerHTML = svgContent;
}

function visvalingamWhyatt(points, targetCount) {
    if (points.length <= targetCount) return points;

    // Work with indices so we can efficiently remove points
    // For closed polygon: each point has neighbors wrapping around
    let pts = points.map((p, i) => ({ x: p[0], y: p[1], idx: i, removed: false }));
    let n = pts.length;

    function triangleArea(a, b, c) {
        return Math.abs((b.x - a.x) * (c.y - a.y) - (c.x - a.x) * (b.y - a.y)) / 2;
    }

    // Build linked list for efficient neighbor access
    for (let i = 0; i < pts.length; i++) {
        pts[i].prev = (i - 1 + pts.length) % pts.length;
        pts[i].next = (i + 1) % pts.length;
    }

    function getArea(i) {
        return triangleArea(pts[pts[i].prev], pts[i], pts[pts[i].next]);
    }

    while (n > targetCount) {
        // Find point with smallest triangle area
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

        // Remove it
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
    // Draw source to get the alpha mask
    cCtx.drawImage(srcCanvas, 0, 0);
    // Fill with solid color, keeping only the alpha shape
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
    // Start with original shape
    rCtx.drawImage(srcCanvas, 0, 0);
    // Intersect with shifted copies — each shift erodes one edge
    rCtx.globalCompositeOperation = 'destination-in';
    rCtx.drawImage(srcCanvas, amount, 0);   // erode left edge
    rCtx.drawImage(srcCanvas, -amount, 0);  // erode right edge
    rCtx.drawImage(srcCanvas, 0, amount);   // erode top edge
    rCtx.drawImage(srcCanvas, 0, -amount);  // erode bottom edge
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

    // Find bricks removed from the edited piece — they need new solo pieces
    const newSet = new Set(editBrickIds);
    const removedBrickIds = originalBrickIds.filter(bid => !newSet.has(bid));

    // Remove newly-added bricks from their old pieces (stolen from other pieces)
    for (const other of pieces) {
        if (other.id === editPieceId) continue;
        other.brick_ids = other.brick_ids.filter(bid => !newSet.has(bid));
        other.bricks = other.bricks.filter(b => !newSet.has(b.id));
        other.num_bricks = other.brick_ids.length;
    }

    // Update the edited piece
    piece.brick_ids = [...editBrickIds];
    piece.bricks = editBrickIds.map(bid => {
        const b = bricks.find(br => br.id === bid);
        return b ? { id: b.id, x: b.x, y: b.y, width: b.width, height: b.height, type: b.type } : null;
    }).filter(Boolean);
    piece.num_bricks = piece.brick_ids.length;

    // Create new solo pieces for removed bricks
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

    // Recompute bounding boxes for modified pieces
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

    // Remove empty pieces
    pieces = pieces.filter(p => p.brick_ids.length > 0);

    // Re-index
    pieces.forEach((p, i) => p.id = i);

    // Find the new id of our edited piece
    const newPiece = pieces.find(p =>
        editBrickIds.length > 0 && p.brick_ids.includes(editBrickIds[0])
    );
    selectedPieceId = newPiece ? newPiece.id : -1;
    editPieceId = selectedPieceId;

    // Rebuild composites
    buildPieceComposites();

    document.getElementById('stat_pieces').textContent = pieces.length;
    if (selectedPieceId >= 0) {
        const sp = pieces.find(p => p.id === selectedPieceId);
        document.getElementById('stat_selected').textContent =
            `Piece #${sp.id} (${sp.num_bricks} bricks, ${sp.width}×${sp.height})`;
    }

    // Exit edit mode
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

function toggleBrickInEdit(brickId) {
    if (!editMode) return;
    const idx = editBrickIds.indexOf(brickId);
    if (idx >= 0) {
        // Don't allow removing the last brick
        if (editBrickIds.length <= 1) return;
        editBrickIds.splice(idx, 1);
    } else {
        editBrickIds.push(brickId);
    }

    // Check if changed from original
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
    // Draw all bricks dimmed, highlight ones in the edited piece
    const editSet = new Set(editBrickIds);

    for (const brick of bricks) {
        const img = brickImages[brick.id];
        if (!img) continue;

        const inPiece = editSet.has(brick.id);
        ctx.globalAlpha = inPiece ? 1.0 : 0.3;
        ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
        ctx.globalAlpha = 1.0;
    }

    // Draw outlines for bricks in the piece
    for (const bid of editBrickIds) {
        const brick = bricks.find(b => b.id === bid);
        if (!brick) continue;
        const comp = getBrickComp(brick);
        if (comp) drawPieceSilhouetteOutline(comp, 'rgba(80, 255, 120, 0.8)', 3);
    }

    // Draw hover outline on top
    if (hoveredBrickId >= 0) {
        const brick = bricks.find(b => b.id === hoveredBrickId);
        if (brick) {
            const img = brickImages[brick.id];
            if (img) ctx.drawImage(img, brick.x, brick.y, brick.width, brick.height);
            const inPiece = editSet.has(brick.id);
            const comp = getBrickComp(brick);
            if (comp) {
                // Red outline if would be removed, green if would be added
                const color = inPiece ? 'rgba(255, 80, 80, 0.9)' : 'rgba(80, 255, 120, 0.9)';
                drawPieceSilhouetteOutline(comp, color, 4);
            }
        }
    }

    // Label
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
    const padY = (canvas.height - canvasH * zoom) / 2;
    return [(sx - padX) / zoom, (sy - padY) / zoom];
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
    // Use piece composites for pixel-accurate hit testing
    for (const piece of pieces) {
        const comp = pieceComposites[piece.id];
        if (!comp) continue;

        const lx = hx - comp.x;
        const ly = hy - comp.y;
        if (lx < 0 || ly < 0 || lx >= comp.w || ly >= comp.h) continue;

        // Check alpha in composite canvas
        try {
            const cCtx = comp.canvas.getContext('2d');
            const pixel = cCtx.getImageData(Math.round(lx), Math.round(ly), 1, 1).data;
            if (pixel[3] > 30) return piece.id;
        } catch { continue; }
    }
    return -1;
}

canvas.addEventListener('mousemove', (e) => {
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
    } else if (viewMode === 'bricks') {
        const newHovered = findBrickAt(hx, hy);
        if (newHovered !== hoveredBrickId) {
            hoveredBrickId = newHovered;
            if (hoveredBrickId >= 0) {
                const b = bricks.find(br => br.id === hoveredBrickId);
                document.getElementById('stat_hovered').textContent =
                    `#${b.id} (${b.width}×${b.height}) [${b.type}]`;
            } else {
                document.getElementById('stat_hovered').textContent = '-';
            }
            render();
        }
    } else if (viewMode === 'pieces' || viewMode === 'shape') {
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

canvas.addEventListener('click', (e) => {
    const [hx, hy] = screenToHouse(e.clientX, e.clientY);

    if (editMode) {
        const clickedId = findBrickAt(hx, hy);
        if (clickedId >= 0) {
            toggleBrickInEdit(clickedId);
        }
        return;
    }

    if (viewMode === 'bricks') {
        const clickedId = findBrickAt(hx, hy);
        if (clickedId === selectedBrickId) {
            selectedBrickId = -1;
            document.getElementById('stat_selected').textContent = '-';
        } else {
            selectedBrickId = clickedId;
            if (clickedId >= 0) {
                const b = bricks.find(br => br.id === clickedId);
                document.getElementById('stat_selected').textContent =
                    `#${b.id} (${b.width}×${b.height}) [${b.type}]`;
            } else {
                document.getElementById('stat_selected').textContent = '-';
            }
        }
    } else if (viewMode === 'pieces' || viewMode === 'shape') {
        const clickedId = findPieceAt(hx, hy);
        if (clickedId === selectedPieceId) {
            selectedPieceId = -1;
            document.getElementById('stat_selected').textContent = '-';
            document.getElementById('editBtnRow').style.display = 'none';
        } else {
            selectedPieceId = clickedId;
            shapeManualVerts = 0; // reset to auto on new piece selection
            if (clickedId >= 0) {
                const p = pieces.find(pc => pc.id === clickedId);
                document.getElementById('stat_selected').textContent =
                    `Piece #${p.id} (${p.num_bricks} bricks, ${p.width}×${p.height})`;
                if (viewMode === 'pieces') {
                    document.getElementById('editBtnRow').style.display = 'flex';
                }
            } else {
                document.getElementById('stat_selected').textContent = '-';
                document.getElementById('editBtnRow').style.display = 'none';
            }
        }
    }
    render();
});

canvas.addEventListener('contextmenu', (e) => e.preventDefault());

// --- Slider updates ---

document.getElementById('target_count').addEventListener('input', (e) => {
    document.getElementById('val_target_count').textContent = e.target.value;
});
document.getElementById('seed').addEventListener('input', (e) => {
    document.getElementById('val_seed').textContent = e.target.value;
});
document.getElementById('max_width').addEventListener('input', (e) => {
    document.getElementById('val_max_width').textContent = e.target.value;
});
document.getElementById('max_height').addEventListener('input', (e) => {
    document.getElementById('val_max_height').textContent = e.target.value;
});
document.getElementById('smoothing').addEventListener('input', (e) => {
    document.getElementById('val_smoothing').textContent = e.target.value;
    if (viewMode === 'blueprint') render();
});
document.getElementById('targetVerts').addEventListener('input', (e) => {
    shapeManualVerts = parseInt(e.target.value);
    document.getElementById('val_targetVerts').textContent = e.target.value;
    if (viewMode === 'shape') render();
});
document.getElementById('showVector').addEventListener('change', () => {
    if (viewMode === 'shape') render();
});

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

// --- Start ---

init();
