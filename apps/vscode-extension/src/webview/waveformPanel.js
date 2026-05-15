// -----------------------------------------------------------------------
// Data injected from the extension (via INJECT_DATA script block)
// Expects: SEGMENTS, MEDIA_URI
// -----------------------------------------------------------------------

const vscode     = acquireVsCodeApi();
const statusBar  = document.getElementById('statusBar');
const container  = document.getElementById('canvasContainer');
const canvas     = document.getElementById('waveCanvas');
const ctx        = canvas.getContext('2d');

// Zoom controls
const zoomSlider  = document.getElementById('zoomSlider');
const zoomLevelEl = document.getElementById('zoomLevel');
const zoomInBtn   = document.getElementById('zoomInBtn');
const zoomOutBtn  = document.getElementById('zoomOutBtn');
const fitBtn      = document.getElementById('fitBtn');

// Current highlighted segment index (-1 = none).
let highlightedIndex = -1;
// Full decoded channel data (kept for re-computing peaks on zoom).
let channelData = null;
// Total duration in milliseconds (from decoded audio buffer).
let totalDurationMs = 0;
// Zoom level: 100 = fit-to-window, higher = zoomed in.
let zoomPercent = 100;
// Scroll offset in pixels.
let scrollOffset = 0;

// -----------------------------------------------------------------------
// Resize canvas to match physical pixels (device pixel ratio aware).
// -----------------------------------------------------------------------
function resizeCanvas() {
    const dpr = window.devicePixelRatio || 1;
    const rect = container.getBoundingClientRect();
    const logicalWidth = Math.round(rect.width * (zoomPercent / 100));
    canvas.width  = Math.round(logicalWidth * dpr);
    canvas.height = Math.round(rect.height * dpr);
    canvas.style.width = logicalWidth + 'px';
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
}

// -----------------------------------------------------------------------
// Compute peak amplitude per canvas column from PCM samples.
// -----------------------------------------------------------------------
function buildPeakData(data, canvasLogicalWidth) {
    const sampleCount = data.length;
    const peaks = new Float32Array(canvasLogicalWidth);
    for (let col = 0; col < canvasLogicalWidth; col++) {
        const startSample = Math.floor((col / canvasLogicalWidth) * sampleCount);
        const endSample   = Math.floor(((col + 1) / canvasLogicalWidth) * sampleCount);
        let peak = 0;
        for (let s = startSample; s < endSample; s++) {
            const abs = Math.abs(data[s]);
            if (abs > peak) { peak = abs; }
        }
        peaks[col] = peak;
    }
    return peaks;
}

// -----------------------------------------------------------------------
// Draw the full waveform + overlays onto the canvas.
// -----------------------------------------------------------------------
function redraw() {
    if (!channelData) { return; }

    resizeCanvas();

    const dpr = window.devicePixelRatio || 1;
    const W = canvas.width  / dpr;
    const H = canvas.height / dpr;

    const peakData = buildPeakData(channelData, Math.round(W));

    // Clear.
    ctx.clearRect(0, 0, W, H);

    // Background.
    ctx.fillStyle = getComputedStyle(document.body)
        .getPropertyValue('--vscode-panel-background') || '#1e1e1e';
    ctx.fillRect(0, 0, W, H);

    if (totalDurationMs === 0) { return; }

    // -----------------------------------------------------------------------
    // Draw segment overlay rectangles.
    // -----------------------------------------------------------------------
    for (let i = 0; i < SEGMENTS.length; i++) {
        const seg = SEGMENTS[i];
        const xStart = (seg.beg / totalDurationMs) * W;
        const xEnd   = (seg.end / totalDurationMs) * W;
        ctx.fillStyle = 'rgba(86, 156, 214, 0.25)';
        ctx.fillRect(xStart, 0, Math.max(1, xEnd - xStart), H);
    }

    // -----------------------------------------------------------------------
    // Draw waveform.
    // -----------------------------------------------------------------------
    ctx.fillStyle = getComputedStyle(document.body)
        .getPropertyValue('--vscode-editor-foreground') || '#d4d4d4';
    const midY = H / 2;
    for (let col = 0; col < peakData.length; col++) {
        const peak = peakData[col];
        const barH = peak * H * 0.9;
        ctx.fillRect(col, midY - barH / 2, 1, barH);
    }

    // -----------------------------------------------------------------------
    // Draw highlighted (current) segment.
    // -----------------------------------------------------------------------
    if (highlightedIndex >= 0 && highlightedIndex < SEGMENTS.length) {
        const seg = SEGMENTS[highlightedIndex];
        const xStart = (seg.beg / totalDurationMs) * W;
        const xEnd   = (seg.end / totalDurationMs) * W;
        // Bright overlay for current segment.
        ctx.fillStyle = 'rgba(255, 215, 0, 0.3)';
        ctx.fillRect(xStart, 0, Math.max(2, xEnd - xStart), H);
        // Vertical playhead line.
        ctx.strokeStyle = '#ffd700';
        ctx.lineWidth = 2;
        ctx.beginPath();
        ctx.moveTo(xStart, 0);
        ctx.lineTo(xStart, H);
        ctx.stroke();
    }
}

// -----------------------------------------------------------------------
// Zoom helpers.
// -----------------------------------------------------------------------
function updateZoomUI() {
    zoomLevelEl.textContent = zoomPercent + '%';
    zoomSlider.value = zoomPercent;
}

function applyZoom(newZoom, centerFraction) {
    const oldZoom = zoomPercent;
    zoomPercent = Math.max(100, Math.min(2000, newZoom));
    updateZoomUI();

    // Adjust scroll to keep the center point stable.
    const containerWidth = container.clientWidth;
    const oldCanvasWidth = containerWidth * (oldZoom / 100);
    const newCanvasWidth = containerWidth * (zoomPercent / 100);
    const centerPx = scrollOffset + containerWidth * centerFraction;
    const newCenterPx = (centerPx / oldCanvasWidth) * newCanvasWidth;
    scrollOffset = Math.max(0, Math.min(
        newCanvasWidth - containerWidth,
        newCenterPx - containerWidth * centerFraction
    ));
    container.scrollLeft = scrollOffset;

    redraw();
}

function fitToWindow() {
    zoomPercent = 100;
    scrollOffset = 0;
    container.scrollLeft = 0;
    updateZoomUI();
    redraw();
}

// -----------------------------------------------------------------------
// Toolbar events.
// -----------------------------------------------------------------------
zoomInBtn.addEventListener('click', function() {
    applyZoom(zoomPercent + 50, 0.5);
});

zoomOutBtn.addEventListener('click', function() {
    applyZoom(zoomPercent - 50, 0.5);
});

zoomSlider.addEventListener('input', function(e) {
    applyZoom(parseInt(e.target.value), 0.5);
});

fitBtn.addEventListener('click', fitToWindow);

// Scroll-wheel zoom centered on pointer.
container.addEventListener('wheel', function(e) {
    if (!channelData) { return; }
    e.preventDefault();
    const rect = container.getBoundingClientRect();
    const centerFraction = (e.clientX - rect.left) / rect.width;
    const delta = e.deltaY > 0 ? -50 : 50;
    applyZoom(zoomPercent + delta, centerFraction);
}, { passive: false });

// Track scroll position for click calculations.
container.addEventListener('scroll', function() {
    scrollOffset = container.scrollLeft;
});

// -----------------------------------------------------------------------
// Load and decode audio.
// -----------------------------------------------------------------------
async function loadAudio() {
    try {
        statusBar.textContent = 'Fetching audio\u2026';
        const response = await fetch(MEDIA_URI);
        if (!response.ok) {
            throw new Error('HTTP ' + response.status);
        }
        const arrayBuffer = await response.arrayBuffer();

        statusBar.textContent = 'Decoding audio\u2026';
        const audioContext = new AudioContext();
        const audioBuffer  = await audioContext.decodeAudioData(arrayBuffer);

        totalDurationMs = audioBuffer.duration * 1000;

        // Use the first channel for the waveform.
        channelData = audioBuffer.getChannelData(0);

        statusBar.textContent = 'Rendering waveform\u2026';
        redraw();
        statusBar.textContent =
            'Duration: ' + (totalDurationMs / 1000).toFixed(2) + ' s  |  '
            + SEGMENTS.length + ' segments  |  Click to seek  |  Scroll to zoom';
    } catch (err) {
        statusBar.textContent = 'Error loading audio: ' + (err.message || String(err));
    }
}

// -----------------------------------------------------------------------
// Canvas click -> seek.
// -----------------------------------------------------------------------
canvas.addEventListener('click', function(e) {
    if (totalDurationMs === 0) { return; }
    const rect = canvas.getBoundingClientRect();
    const x = e.clientX - rect.left;
    const ms = Math.round((x / rect.width) * totalDurationMs);
    vscode.postMessage({ command: 'seek', ms: ms });
});

// -----------------------------------------------------------------------
// Inbound messages from the extension.
// -----------------------------------------------------------------------
window.addEventListener('message', function(event) {
    const msg = event.data;
    if (!msg) { return; }
    if (msg.command === 'highlightSegment' && typeof msg.index === 'number') {
        highlightedIndex = msg.index;
        redraw();

        // Auto-scroll to keep highlighted segment visible.
        if (highlightedIndex >= 0 && highlightedIndex < SEGMENTS.length && zoomPercent > 100) {
            const seg = SEGMENTS[highlightedIndex];
            const canvasWidth = container.clientWidth * (zoomPercent / 100);
            const segX = (seg.beg / totalDurationMs) * canvasWidth;
            const segEndX = (seg.end / totalDurationMs) * canvasWidth;
            const viewLeft = container.scrollLeft;
            const viewRight = viewLeft + container.clientWidth;
            if (segX < viewLeft || segEndX > viewRight) {
                container.scrollLeft = Math.max(0, segX - 40);
                scrollOffset = container.scrollLeft;
            }
        }
    }
});

// -----------------------------------------------------------------------
// Respond to container resize.
// -----------------------------------------------------------------------
const resizeObserver = new ResizeObserver(function() {
    if (channelData) { redraw(); }
});
resizeObserver.observe(container);

// Kick off audio loading.
loadAudio();
