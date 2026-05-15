// -----------------------------------------------------------------------
// Data injected from the extension (via INJECT_DATA script block)
// Expects: dotSource
// -----------------------------------------------------------------------

// -- State ----------------------------------------------------------------
let currentZoom  = 100;   // percent
let panX         = 20;    // px offset from content origin
let panY         = 20;
let svgElement   = null;
let graphvizInst = null;
let isDragging   = false;
let dragStartX   = 0;
let dragStartY   = 0;
let panAtDragX   = 0;
let panAtDragY   = 0;

const graphContainer = document.getElementById('graphContainer');
const content        = document.getElementById('content');
const zoomLevelEl    = document.getElementById('zoomLevel');
const zoomSlider     = document.getElementById('zoomSlider');

// -- Transform helper -----------------------------------------------------
function applyTransform() {
    const scale = currentZoom / 100;
    content.style.transform =
        'translate(' + panX + 'px,' + panY + 'px) scale(' + scale + ')';
    zoomLevelEl.textContent = currentZoom + '%';
    zoomSlider.value = currentZoom;
}

// -- SVG theming ----------------------------------------------------------
// After Graphviz renders, override inline fill/stroke/font attributes
// so the graph respects the current VS Code colour theme.
function themeGraphSvg(svg) {
    // Resolve CSS variables to actual values by reading computed styles
    // on a temporary element.
    const probe = document.createElement('div');
    probe.style.display = 'none';
    document.body.appendChild(probe);

    function resolveVar(cssVar, fallback) {
        probe.style.color = 'var(' + cssVar + ',' + fallback + ')';
        const resolved = getComputedStyle(probe).color;
        return resolved || fallback;
    }

    const bg     = resolveVar('--vscode-editor-background',    '#1e1e1e');
    const fg     = resolveVar('--vscode-editor-foreground',    '#d4d4d4');
    const border = resolveVar('--vscode-panel-border',         '#454545');
    const accent = resolveVar('--vscode-textLink-foreground',  '#4ec9b0');
    const dim    = resolveVar('--vscode-descriptionForeground','#858585');
    document.body.removeChild(probe);

    // Node shapes (polygon = rectangle/diamond, ellipse)
    svg.querySelectorAll('polygon, ellipse').forEach(el => {
        if (el.getAttribute('fill') !== 'none') {
            el.setAttribute('fill', bg);
        }
        if (el.getAttribute('stroke') && el.getAttribute('stroke') !== 'none') {
            el.setAttribute('stroke', border);
        }
    });

    // Edge paths
    svg.querySelectorAll('.edge path, .edge line').forEach(el => {
        el.setAttribute('stroke', accent);
        el.setAttribute('stroke-opacity', '0.7');
    });

    // Arrow heads on edges
    svg.querySelectorAll('.edge polygon').forEach(el => {
        el.setAttribute('fill', accent);
        el.setAttribute('fill-opacity', '0.7');
        el.setAttribute('stroke', accent);
    });

    // Text labels — nodes bold, edges dim
    svg.querySelectorAll('.node text').forEach(el => {
        el.setAttribute('fill', fg);
        el.style.fontFamily = "'JetBrains Mono', monospace";
        el.style.fontSize   = '12px';
    });
    svg.querySelectorAll('.edge text').forEach(el => {
        el.setAttribute('fill', dim);
        el.style.fontFamily = "'JetBrains Mono', monospace";
        el.style.fontSize   = '11px';
    });
    svg.querySelectorAll('.graph text').forEach(el => {
        el.setAttribute('fill', fg);
        el.style.fontFamily = "'JetBrains Mono', monospace";
    });

    // Background rect (if any)
    svg.querySelectorAll('polygon[fill="#ffffff"], polygon[fill="white"]').forEach(el => {
        el.setAttribute('fill', bg);
        el.setAttribute('stroke', 'none');
    });
}

// -- Graphviz loading and rendering ----------------------------------------
async function initGraphviz() {
    try {
        const hpccWasm = await import(GRAPHVIZ_URI);
        graphvizInst = await hpccWasm.Graphviz.load();
        return true;
    } catch (err) {
        content.innerHTML =
            '<div class="error">Failed to load Graphviz renderer.\\n\\n' +
            String(err) + '</div>';
        return false;
    }
}

async function renderGraph() {
    try {
        if (!graphvizInst && !(await initGraphviz())) { return; }

        const svgString = graphvizInst.layout(dotSource, 'svg', 'dot');
        content.innerHTML = svgString;
        svgElement = content.querySelector('svg');

        if (!svgElement) { throw new Error('Graphviz produced no SVG output'); }

        svgElement.removeAttribute('width');
        svgElement.removeAttribute('height');
        svgElement.style.maxWidth = 'none';
        svgElement.style.overflow = 'visible';

        themeGraphSvg(svgElement);
        enableButtons(true);

        // Auto-fit on first render.
        fitToWindow();

    } catch (err) {
        enableButtons(false);
        content.innerHTML =
            '<div class="error">Render error: ' + String(err) + '</div>';
    }
}

function enableButtons(on) {
    ['zoomInBtn','zoomOutBtn','fitBtn','resetPanBtn','downloadSvgBtn','downloadPngBtn'].forEach(id => {
        document.getElementById(id).disabled = !on;
    });
    zoomSlider.disabled = !on;
}

// -- Fit to window --------------------------------------------------------
function fitToWindow() {
    if (!svgElement) { return; }
    const svgW = svgElement.viewBox.baseVal.width  || svgElement.getBBox().width;
    const svgH = svgElement.viewBox.baseVal.height || svgElement.getBBox().height;
    const cW   = graphContainer.clientWidth  - 40;
    const cH   = graphContainer.clientHeight - 40;
    if (svgW <= 0 || svgH <= 0) { return; }
    const scale = Math.min(cW / svgW, cH / svgH, 2);
    currentZoom = Math.max(10, Math.min(400, Math.round(scale * 100)));
    panX = 20;
    panY = 20;
    applyTransform();
}

// -- Drag-to-pan ----------------------------------------------------------
graphContainer.addEventListener('mousedown', e => {
    if (e.button !== 0) { return; }
    isDragging  = true;
    dragStartX  = e.clientX;
    dragStartY  = e.clientY;
    panAtDragX  = panX;
    panAtDragY  = panY;
    graphContainer.classList.add('dragging');
    e.preventDefault();
});

window.addEventListener('mousemove', e => {
    if (!isDragging) { return; }
    panX = panAtDragX + (e.clientX - dragStartX);
    panY = panAtDragY + (e.clientY - dragStartY);
    applyTransform();
});

window.addEventListener('mouseup', () => {
    isDragging = false;
    graphContainer.classList.remove('dragging');
});

// Scroll-wheel zoom centred on pointer.
graphContainer.addEventListener('wheel', e => {
    e.preventDefault();
    const delta     = e.deltaY > 0 ? -8 : 8;
    const newZoom   = Math.max(10, Math.min(400, currentZoom + delta));
    const scaleRatio = newZoom / currentZoom;

    // Adjust pan so the point under the pointer stays fixed.
    const rect  = graphContainer.getBoundingClientRect();
    const mouseX = e.clientX - rect.left;
    const mouseY = e.clientY - rect.top;
    panX = mouseX - scaleRatio * (mouseX - panX);
    panY = mouseY - scaleRatio * (mouseY - panY);

    currentZoom = newZoom;
    applyTransform();
}, { passive: false });

// -- Toolbar buttons ------------------------------------------------------
document.getElementById('zoomInBtn').addEventListener('click', () => {
    currentZoom = Math.min(currentZoom + 10, 400);
    applyTransform();
});

document.getElementById('zoomOutBtn').addEventListener('click', () => {
    currentZoom = Math.max(currentZoom - 10, 10);
    applyTransform();
});

zoomSlider.addEventListener('input', e => {
    currentZoom = parseInt(e.target.value);
    applyTransform();
});

document.getElementById('fitBtn').addEventListener('click', fitToWindow);

document.getElementById('resetPanBtn').addEventListener('click', () => {
    panX = 20; panY = 20;
    applyTransform();
});

// -- SVG download ---------------------------------------------------------
document.getElementById('downloadSvgBtn').addEventListener('click', () => {
    if (!svgElement) { return; }
    const clone = svgElement.cloneNode(true);
    const blob  = new Blob([new XMLSerializer().serializeToString(clone)], { type: 'image/svg+xml' });
    const url   = URL.createObjectURL(blob);
    const a     = Object.assign(document.createElement('a'), { href: url, download: 'dependency-graph.svg' });
    document.body.appendChild(a); a.click(); document.body.removeChild(a);
    URL.revokeObjectURL(url);
});

// -- PNG download (uses editor background for fill) -----------------------
document.getElementById('downloadPngBtn').addEventListener('click', async () => {
    if (!svgElement) { return; }
    try {
        const bbox    = svgElement.getBBox();
        const scale   = 2;
        const canvas  = document.createElement('canvas');
        canvas.width  = bbox.width  * scale;
        canvas.height = bbox.height * scale;
        const ctx     = canvas.getContext('2d');
        if (!ctx) { throw new Error('Canvas context unavailable'); }

        // Use the resolved editor background instead of hard-coded white.
        const probe = document.createElement('div');
        probe.style.cssText = 'display:none;background:var(--vscode-editor-background,#1e1e1e)';
        document.body.appendChild(probe);
        ctx.fillStyle = getComputedStyle(probe).backgroundColor || '#1e1e1e';
        document.body.removeChild(probe);

        ctx.fillRect(0, 0, canvas.width, canvas.height);
        ctx.scale(scale, scale);

        const clone   = svgElement.cloneNode(true);
        const blob    = new Blob([new XMLSerializer().serializeToString(clone)], { type: 'image/svg+xml;charset=utf-8' });
        const url     = URL.createObjectURL(blob);
        const img     = new Image();
        img.onload = () => {
            ctx.drawImage(img, 0, 0);
            URL.revokeObjectURL(url);
            canvas.toBlob(pngBlob => {
                if (!pngBlob) { return; }
                const pngUrl = URL.createObjectURL(pngBlob);
                const a = Object.assign(document.createElement('a'), { href: pngUrl, download: 'dependency-graph.png' });
                document.body.appendChild(a); a.click(); document.body.removeChild(a);
                URL.revokeObjectURL(pngUrl);
            }, 'image/png');
        };
        img.onerror = () => URL.revokeObjectURL(url);
        img.src = url;
    } catch (err) {
        console.error('PNG export error:', err);
    }
});

// -- Start ----------------------------------------------------------------
enableButtons(false);
applyTransform();
renderGraph();
