// -----------------------------------------------------------------------
// Data injected from the extension (via INJECT_DATA script block)
// Expects: DATA, CMD_NAME, FILE_NAME
// -----------------------------------------------------------------------

const vscode = acquireVsCodeApi();

// Human-readable labels and descriptions for each CLAN analysis command.
const COMMAND_INFO = {
    freq:    { label: 'Frequency',             desc: 'Word and morpheme frequency distribution' },
    mlu:     { label: 'Mean Length Utterance', desc: 'MLU-w and MLU-m computed per speaker' },
    mlt:     { label: 'Mean Length Turn',      desc: 'Average turn length across speakers' },
    wdlen:   { label: 'Word Length',           desc: 'Distribution of word lengths in characters' },
    wdsize:  { label: 'Word Size',             desc: 'Character length histogram from %mor stems' },
    maxwd:   { label: 'Maximum Words',         desc: 'Longest utterances ranked by speaker' },
    freqpos: { label: 'Frequency by Position', desc: 'Word frequency per utterance position' },
    timedur: { label: 'Time Duration',         desc: 'Utterance timing and duration statistics' },
    kwal:    { label: 'KWAL Search',           desc: 'Keyword-in-context concordance search' },
    combo:   { label: 'Combination Search',    desc: 'Multi-word pattern co-occurrence' },
    gemlist: { label: 'Gem List',              desc: 'Utterances within gem segments' },
    cooccur: { label: 'Co-occurrence',         desc: 'Word and morpheme co-occurrence matrix' },
    dist:    { label: 'Distribution',          desc: 'Frequency distribution across speakers' },
    chip:    { label: 'CHIP Analysis',         desc: 'Contingent imitation and prompt analysis' },
    phonfreq: { label: 'Phonological Frequency', desc: 'Phonological segment frequency from %pho tier' },
    modrep:   { label: 'Model & Replica',        desc: 'Model utterance and child replica analysis' },
    vocd:     { label: 'Vocabulary Diversity',    desc: 'D statistic for lexical diversity' },
    codes:    { label: 'Code Frequency',          desc: 'Frequency counts of coding tier codes' },
    complexity: { label: 'Syntactic Complexity',   desc: 'Complexity ratio from %gra dependency relations' },
    corelex:  { label: 'Core Vocabulary',          desc: 'Words above frequency threshold' },
    chains:   { label: 'Code Chains',             desc: 'Sequential chains of codes on %cod tier' },
    dss:      { label: 'Dev. Sentence Score',     desc: 'Developmental Sentence Scoring per speaker' },
    eval:     { label: 'Language Evaluation',     desc: 'Combined evaluation: MLU, TTR, DSS, IPSyn' },
    flucalc:  { label: 'Fluency Calculation',     desc: 'Disfluency measures and fluency ratios' },
    ipsyn:    { label: 'Index of Prod. Syntax',   desc: 'IPSyn score from morphosyntactic analysis' },
    keymap:   { label: 'Keyword Contingency',     desc: 'Contingency table of keyword codes' },
    kideval:  { label: 'Child Evaluation',        desc: 'DSS + IPSyn + MLU combined child measures' },
    mortable: { label: 'Morpheme Table',          desc: 'Morpheme frequency by POS script categories' },
    rely:     { label: 'Inter-rater Reliability', desc: 'Agreement and kappa between two coded files' },
    script:   { label: 'Script Comparison',       desc: 'Transcript vs template word count comparison' },
    sugar:    { label: 'SUGAR Analysis',          desc: 'Grammatical analysis of sampled utterances' },
    trnfix:   { label: 'Tier Comparison',         desc: 'Mismatches between two dependent tiers' },
    uniq:     { label: 'Unique Utterances',       desc: 'Repeated and unique utterance detection' },
};

// Six accent colours drawn from VS Code's syntax-highlight palette.
const ACCENT_COLORS = [
    '#4ec9b0',  // teal
    '#ce9178',  // rust / salmon
    '#569cd6',  // cornflower blue
    '#dcdcaa',  // khaki / gold
    '#c586c0',  // lavender
    '#9cdcfe',  // sky blue
];

const MAX_ROWS = 200;

// -- Header ---------------------------------------------------------------
const info = COMMAND_INFO[CMD_NAME] || { label: CMD_NAME.toUpperCase(), desc: '' };
document.getElementById('cmd-eyebrow').textContent = 'CLAN Analysis \u00b7 ' + CMD_NAME;
document.getElementById('cmd-display').textContent = info.label;
document.getElementById('cmd-description').textContent = info.desc;
document.getElementById('file-name').textContent = FILE_NAME;

// -- Helpers --------------------------------------------------------------

/**
 * Return the speaker code embedded in a heading (e.g. "CHI" from "CHI" or
 * "Speaker: CHI") if the heading looks like it IS just the code itself,
 * otherwise null.  Matches 2-6 consecutive uppercase letters.
 */
function parseSpeakerCode(heading) {
    const trimmed = heading.trim();
    return /^[A-Z]{2,6}$/.test(trimmed) ? trimmed : null;
}

/**
 * Animate a stat-card element from 0 to its target value using a cubic
 * ease-out over ~600 ms.  Falls back to setting textContent directly when
 * the target is non-numeric.
 */
function animateCount(el, targetStr) {
    const num = parseFloat(targetStr);
    if (isNaN(num) || !isFinite(num)) { el.textContent = targetStr; return; }
    const decPlaces = targetStr.includes('.') ? (targetStr.split('.')[1] || '').length : 0;
    const duration = 600;
    const start = performance.now();
    function tick(now) {
        const t = Math.min((now - start) / duration, 1);
        const eased = 1 - Math.pow(1 - t, 3);
        const cur = num * eased;
        el.textContent = decPlaces > 0
            ? cur.toFixed(decPlaces)
            : Math.round(cur).toLocaleString();
        if (t < 1) { requestAnimationFrame(tick); }
    }
    requestAnimationFrame(tick);
}

/** Build a single stat card (key label + large value). */
function makeStatCard(key, value, accentColor) {
    const card = document.createElement('div');
    card.className = 'stat-card';

    const keyEl = document.createElement('div');
    keyEl.className = 'stat-key';
    keyEl.textContent = key;

    const valStr = String(value);
    const numVal = parseFloat(valStr);
    const isNumeric = !isNaN(numVal) && isFinite(numVal);

    const valEl = document.createElement('div');
    valEl.className = isNumeric ? 'stat-value' : 'stat-value plain';
    valEl.style.color = isNumeric ? accentColor : '';
    if (isNumeric) {
        animateCount(valEl, valStr);
    } else {
        valEl.textContent = valStr;
    }

    card.appendChild(keyEl);
    card.appendChild(valEl);
    return card;
}

/**
 * Find the maximum value of a given column across all rows, used to scale
 * the proportional bar-chart overlay.
 */
function findColumnMax(rows, colKey) {
    let max = 0;
    for (const row of rows) {
        const v = parseFloat(String(row[colKey] ?? ''));
        if (!isNaN(v) && v > max) { max = v; }
    }
    return max;
}

/**
 * Render an array of row objects as an HTML table.
 * Applies a proportional bar-chart overlay to the last numeric column,
 * and truncates to MAX_ROWS with a notice when there are more rows.
 */
function renderTable(rows, accentColor) {
    if (!rows || rows.length === 0) {
        const p = document.createElement('p');
        p.className = 'empty';
        p.textContent = '(no data)';
        return p;
    }

    // Collect column headers in first-occurrence order.
    const headers = [];
    const headerSet = new Set();
    for (const row of rows) {
        for (const key of Object.keys(row)) {
            if (!headerSet.has(key)) { headers.push(key); headerSet.add(key); }
        }
    }

    // Determine whether the last column qualifies for bar-chart overlay.
    const lastCol = headers[headers.length - 1];
    const colMax = findColumnMax(rows, lastCol);
    const hasBarChart = colMax > 0 &&
        rows.slice(0, 5).some(r => {
            const v = parseFloat(String(r[lastCol] ?? ''));
            return !isNaN(v) && isFinite(v);
        });

    const displayRows = rows.length > MAX_ROWS ? rows.slice(0, MAX_ROWS) : rows;
    const wasTruncated = rows.length > MAX_ROWS;

    const frag = document.createDocumentFragment();
    const table = document.createElement('table');

    const thead = document.createElement('thead');
    const hrow = document.createElement('tr');
    for (const h of headers) {
        const th = document.createElement('th');
        th.textContent = h;
        hrow.appendChild(th);
    }
    thead.appendChild(hrow);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    for (const row of displayRows) {
        const tr = document.createElement('tr');
        headers.forEach((h, idx) => {
            const td = document.createElement('td');
            const val = row[h];
            const str = val === null || val === undefined ? '' : String(val);
            td.textContent = str;

            // Proportional bar on the last numeric column.
            if (hasBarChart && idx === headers.length - 1) {
                const numV = parseFloat(str);
                if (!isNaN(numV) && numV > 0) {
                    const pct = ((numV / colMax) * 100).toFixed(1);
                    td.className = 'td-bar';
                    // CSS custom property gradient — avoids inline style repetition.
                    td.style.setProperty(
                        '--bar-gradient',
                        `linear-gradient(to right, color-mix(in srgb, ${accentColor} 20%, transparent) ${pct}%, transparent ${pct}%)`
                    );
                }
            }
            tr.appendChild(td);
        });
        tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    frag.appendChild(table);

    if (wasTruncated) {
        const notice = document.createElement('p');
        notice.className = 'truncation-notice';
        notice.textContent = 'Showing first ' + MAX_ROWS + ' of ' + rows.length + ' rows.';
        frag.appendChild(notice);
    }

    return frag;
}

/**
 * Render one top-level JSON section: a coloured header, stat cards for
 * scalar values, and tables for array-of-object values.
 */
function renderSection(title, value, colorIndex) {
    const accentColor = ACCENT_COLORS[colorIndex % ACCENT_COLORS.length];
    const section = document.createElement('div');
    section.className = 'section';
    section.style.setProperty('--section-color', accentColor);

    // Section header with optional speaker badge.
    const header = document.createElement('div');
    header.className = 'section-header';

    const titleEl = document.createElement('div');
    titleEl.className = 'section-title';
    titleEl.textContent = title;
    header.appendChild(titleEl);

    const speakerCode = parseSpeakerCode(title);
    if (speakerCode) {
        const badge = document.createElement('span');
        badge.className = 'speaker-badge';
        badge.textContent = speakerCode;
        header.appendChild(badge);
    }
    section.appendChild(header);

    // Render value according to its type.
    if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'object' && value[0] !== null) {
        // Array of objects -> table.
        section.appendChild(renderTable(value, accentColor));

    } else if (Array.isArray(value)) {
        // Array of primitives -> monospace pre block.
        const pre = document.createElement('pre');
        pre.textContent = value.join(', ');
        section.appendChild(pre);

    } else if (typeof value === 'object' && value !== null) {
        // Object -> stat cards for scalars + sub-tables for nested arrays.
        const primitives = Object.entries(value).filter(
            ([, v]) => !Array.isArray(v) && (typeof v !== 'object' || v === null)
        );
        const arrays = Object.entries(value).filter(
            ([, v]) => Array.isArray(v)
        );
        const objects = Object.entries(value).filter(
            ([, v]) => typeof v === 'object' && v !== null && !Array.isArray(v)
        );

        if (primitives.length > 0) {
            const grid = document.createElement('div');
            grid.className = 'stat-grid';
            for (const [k, v] of primitives) {
                grid.appendChild(makeStatCard(k, v === null ? 'null' : v, accentColor));
            }
            section.appendChild(grid);
        }

        for (const [k, v] of arrays) {
            if (v.length > 0 && typeof v[0] === 'object') {
                const label = document.createElement('div');
                label.className = 'sub-section-label';
                label.textContent = k;
                section.appendChild(label);
                section.appendChild(renderTable(v, accentColor));
            } else if (v.length > 0) {
                const pre = document.createElement('pre');
                pre.textContent = v.join(', ');
                section.appendChild(pre);
            }
        }

        for (const [, v] of objects) {
            // Flatten one level of nested objects into additional stat cards.
            const subPrimitives = Object.entries(v).filter(
                ([, sv]) => typeof sv !== 'object' || sv === null
            );
            if (subPrimitives.length > 0) {
                const grid = document.createElement('div');
                grid.className = 'stat-grid';
                for (const [k, sv] of subPrimitives) {
                    grid.appendChild(makeStatCard(k, sv === null ? 'null' : sv, accentColor));
                }
                section.appendChild(grid);
            }
        }

    } else {
        // Scalar at top level -> single stat card.
        const grid = document.createElement('div');
        grid.className = 'stat-grid';
        grid.appendChild(makeStatCard(title, value, accentColor));
        section.appendChild(grid);
    }

    return section;
}

// -- Main render ----------------------------------------------------------
const container = document.getElementById('content');

if (Array.isArray(DATA) && DATA.length > 0 && typeof DATA[0] === 'object') {
    // Top-level array of objects -> single full-width table.
    container.appendChild(renderTable(DATA, ACCENT_COLORS[0]));
} else if (typeof DATA === 'object' && DATA !== null && !Array.isArray(DATA)) {
    // Top-level object -> one section per key.
    let colorIdx = 0;
    for (const [key, val] of Object.entries(DATA)) {
        container.appendChild(renderSection(key, val, colorIdx++));
    }
} else {
    // Fallback: pretty-printed JSON.
    const pre = document.createElement('pre');
    pre.textContent = JSON.stringify(DATA, null, 2);
    container.appendChild(pre);
}

// -- CSV export -----------------------------------------------------------

function csvEscape(value) {
    const str = (value || '').trim();
    if (str.includes(',') || str.includes('"') || str.includes('\n')) {
        return '"' + str.replace(/"/g, '""') + '"';
    }
    return str;
}

function collectCsvFromTables() {
    const tables = document.querySelectorAll('#content table');
    if (tables.length === 0) return null;
    const lines = [];
    for (const table of tables) {
        // Find section title (preceding .section-title element)
        const section = table.closest('.section');
        if (section) {
            const title = section.querySelector('.section-title');
            if (title) lines.push('"' + title.textContent.replace(/"/g, '""') + '"');
        }
        // Headers
        const ths = table.querySelectorAll('thead th');
        if (ths.length > 0) {
            lines.push(Array.from(ths).map(th => csvEscape(th.textContent)).join(','));
        }
        // Rows
        const trs = table.querySelectorAll('tbody tr');
        for (const tr of trs) {
            const tds = tr.querySelectorAll('td');
            lines.push(Array.from(tds).map(td => csvEscape(td.textContent)).join(','));
        }
        lines.push(''); // blank line between tables
    }
    return lines.join('\n');
}

document.getElementById('btn-export-csv').addEventListener('click', () => {
    const csv = collectCsvFromTables();
    if (csv) {
        vscode.postMessage({ command: 'exportCsv', csv });
    }
});
