// -----------------------------------------------------------------------
// KidEval panel webview script
//
// Communicates with kidevalPanel.ts via PostMessage:
//   → discoverDatabases, runAnalysis
//   ← databases, results, error, fileInfo
// -----------------------------------------------------------------------

// @ts-check
/* global acquireVsCodeApi, MODE_CONFIG */

const vscode = acquireVsCodeApi();

const ACCENT_COLORS = [
    '#4ec9b0',  // teal
    '#ce9178',  // rust / salmon
    '#569cd6',  // cornflower blue
    '#dcdcaa',  // khaki / gold
    '#c586c0',  // lavender
    '#9cdcfe',  // sky blue
];

// -- DOM refs -------------------------------------------------------------
const langSelect = document.getElementById('lang-select');
const activitySelect = document.getElementById('activity-select');
const genderSelect = document.getElementById('gender-select');
const ageFrom = document.getElementById('age-from');
const ageTo = document.getElementById('age-to');
const btnRun = document.getElementById('btn-run');
const btnRunNoDb = document.getElementById('btn-run-no-db');
const statusEl = document.getElementById('status');
const resultsEl = document.getElementById('results');
const fileNameEl = document.getElementById('file-name');
const dbInfoPanel = document.getElementById('db-info-panel');
const dbInfoName = document.getElementById('db-info-name');
const dbInfoDetails = document.getElementById('db-info-details');
const dbGrid = document.getElementById('db-grid');

let databases = [];
let selectedDbPath = '';
let fileLanguage = '';
let fileActivity = '';

// -- Language / activity name maps ----------------------------------------
const LANGUAGE_NAMES = {
    eng: 'English (NA)', engu: 'English (UK)', fra: 'French',
    jpn: 'Japanese', spa: 'Spanish', zho: 'Chinese',
    nld: 'Dutch', yue: 'Cantonese', hrv: 'Croatian',
    deu: 'German', ell: 'Greek', eus: 'Basque',
    tha: 'Thai', por: 'Portuguese', ind: 'Indonesian',
};

const ACTIVITY_NAMES = {
    narrative: 'Narrative', toyplay: 'Toyplay',
    eval: 'Evaluation', 'eval-d': 'Dementia Eval',
    general: 'General',
};

// -- Apply mode config to header ------------------------------------------
document.getElementById('cmd-eyebrow').textContent =
    'CLAN Analysis \u00b7 ' + MODE_CONFIG.analyzeCommand;
document.getElementById('cmd-display').textContent = MODE_CONFIG.title;
document.getElementById('cmd-description').textContent = MODE_CONFIG.description;

// -- Initialize -----------------------------------------------------------
vscode.postMessage({ command: 'discoverDatabases', libDir: MODE_CONFIG.defaultLibDir });
setStatus('Discovering databases\u2026', false);

// -- Event listeners ------------------------------------------------------

langSelect.addEventListener('change', onLanguageChange);
activitySelect.addEventListener('change', onActivityChange);

btnRun.addEventListener('click', () => {
    runAnalysis(true);
});

btnRunNoDb.addEventListener('click', () => {
    runAnalysis(false);
});

// -- PostMessage handler --------------------------------------------------

window.addEventListener('message', (event) => {
    const msg = event.data;
    switch (msg.command) {
        case 'databases':
            handleDatabases(msg.databases);
            break;
        case 'results':
            handleResults(msg.data);
            break;
        case 'error':
            setStatus(msg.message, true);
            break;
        case 'fileInfo':
            fileNameEl.textContent = msg.fileName;
            // Auto-detect language and activity from file metadata
            if (msg.language) { fileLanguage = msg.language; }
            if (msg.activity) { fileActivity = msg.activity; }
            // Re-apply auto-detection if databases are already loaded
            if (databases.length > 0) { applyAutoDetection(); }
            break;
    }
});

// -- Database discovery ---------------------------------------------------

function handleDatabases(dbs) {
    databases = dbs || [];

    if (databases.length === 0) {
        langSelect.innerHTML = '<option value="">No databases found</option>';
        activitySelect.innerHTML = '<option value="">—</option>';
        dbGrid.innerHTML = '<tr><td class="empty">No normative databases found.</td></tr>';
        btnRun.disabled = true;
        setStatus('No normative databases found. You can still run without a database.', false);
        return;
    }

    // Populate language dropdown from unique languages
    const langs = [...new Set(databases.map(db => db.language).filter(Boolean))].sort();
    langSelect.innerHTML = '<option value="">Select language…</option>';
    for (const lang of langs) {
        const opt = document.createElement('option');
        opt.value = lang;
        opt.textContent = LANGUAGE_NAMES[lang] || lang;
        langSelect.appendChild(opt);
    }

    // Render availability grid
    renderAvailabilityGrid(databases);

    // Auto-detect from file metadata
    applyAutoDetection();

    setStatus('Select language and activity, then compare with database.', false);
}

function applyAutoDetection() {
    if (fileLanguage && langSelect.querySelector(`option[value="${fileLanguage}"]`)) {
        langSelect.value = fileLanguage;
        onLanguageChange();
    }
    if (fileActivity && activitySelect.querySelector(`option[value="${fileActivity}"]`)) {
        activitySelect.value = fileActivity;
        onActivityChange();
    }
}

function onLanguageChange() {
    const lang = langSelect.value;

    // Filter databases for selected language
    const matching = databases.filter(db => db.language === lang);
    const activities = [...new Set(matching.map(db => db.corpus_type).filter(Boolean))].sort();

    activitySelect.innerHTML = '';
    if (activities.length === 0) {
        activitySelect.innerHTML = '<option value="">—</option>';
    } else if (activities.length === 1) {
        // Auto-select the only activity
        const opt = document.createElement('option');
        opt.value = activities[0];
        opt.textContent = ACTIVITY_NAMES[activities[0]] || activities[0];
        activitySelect.appendChild(opt);
    } else {
        activitySelect.innerHTML = '<option value="">Select activity…</option>';
        for (const act of activities) {
            const opt = document.createElement('option');
            opt.value = act;
            opt.textContent = ACTIVITY_NAMES[act] || act;
            activitySelect.appendChild(opt);
        }
    }

    onActivityChange();
}

function onActivityChange() {
    const lang = langSelect.value;
    const activity = activitySelect.value;

    // Find matching database
    const match = databases.find(
        db => db.language === lang && db.corpus_type === activity
    );

    if (match) {
        dbInfoPanel.style.display = '';
        const fileName = match.name || (match.path ? match.path.split('/').pop() : '');
        dbInfoName.textContent = fileName;
        const langName = LANGUAGE_NAMES[match.language] || match.language || '';
        const actName = ACTIVITY_NAMES[match.corpus_type] || match.corpus_type || '';
        const count = match.entry_count != null ? `${match.entry_count} samples` : '';
        dbInfoDetails.textContent = [count, langName, actName].filter(Boolean).join(' \u00b7 ');
        selectedDbPath = match.path;
        btnRun.disabled = false;
    } else {
        dbInfoPanel.style.display = 'none';
        selectedDbPath = '';
        btnRun.disabled = true;
    }

    highlightGridCell(lang, activity);
}

function renderAvailabilityGrid(dbs) {
    const langs = [...new Set(dbs.map(db => db.language).filter(Boolean))].sort();
    const activities = [...new Set(dbs.map(db => db.corpus_type).filter(Boolean))].sort();

    // Build lookup: {lang: {activity: db}}
    const lookup = {};
    for (const db of dbs) {
        if (!db.language || !db.corpus_type) continue;
        if (!lookup[db.language]) lookup[db.language] = {};
        lookup[db.language][db.corpus_type] = db;
    }

    let html = '<thead><tr><th></th>';
    for (const act of activities) {
        html += `<th>${ACTIVITY_NAMES[act] || act}</th>`;
    }
    html += '</tr></thead><tbody>';

    for (const lang of langs) {
        html += `<tr><td class="grid-lang">${LANGUAGE_NAMES[lang] || lang}</td>`;
        for (const act of activities) {
            const db = lookup[lang] && lookup[lang][act];
            if (db) {
                const count = db.entry_count != null ? db.entry_count : '';
                html += `<td class="grid-cell grid-available" `
                    + `data-lang="${lang}" data-activity="${act}" `
                    + `title="${count} samples">`
                    + `\u25cf ${count}</td>`;
            } else {
                html += '<td class="grid-cell grid-empty">\u2014</td>';
            }
        }
        html += '</tr>';
    }
    html += '</tbody>';

    dbGrid.innerHTML = html;

    // Click handler on available cells
    dbGrid.querySelectorAll('.grid-available').forEach(cell => {
        cell.addEventListener('click', () => {
            langSelect.value = cell.dataset.lang;
            onLanguageChange();
            activitySelect.value = cell.dataset.activity;
            onActivityChange();
        });
    });
}

function highlightGridCell(lang, activity) {
    dbGrid.querySelectorAll('.grid-cell').forEach(cell => {
        cell.classList.remove('grid-selected');
    });
    if (lang && activity) {
        const cell = dbGrid.querySelector(
            `.grid-available[data-lang="${lang}"][data-activity="${activity}"]`
        );
        if (cell) cell.classList.add('grid-selected');
    }
}

// -- Run analysis ---------------------------------------------------------

function runAnalysis(withDatabase) {
    setStatus('<span class="spinner"></span>Running KidEval analysis…', false);
    resultsEl.innerHTML = '';
    btnRun.disabled = true;
    btnRunNoDb.disabled = true;

    const msg = { command: 'runAnalysis' };

    if (withDatabase && selectedDbPath) {
        msg.databasePath = selectedDbPath;
        msg.databaseFilter = buildFilter();
    }

    vscode.postMessage(msg);
}

function buildFilter() {
    const filter = {};

    const gender = genderSelect.value;
    if (gender) { filter.gender = gender; }

    const from = parseInt(ageFrom.value, 10);
    if (!isNaN(from) && from > 0) { filter.age_from_months = from; }

    const to = parseInt(ageTo.value, 10);
    if (!isNaN(to) && to > 0) { filter.age_to_months = to; }

    return Object.keys(filter).length > 0 ? filter : undefined;
}

// -- Render results -------------------------------------------------------

function handleResults(data) {
    btnRun.disabled = !selectedDbPath;
    btnRunNoDb.disabled = false;
    resultsEl.innerHTML = '';

    if (!data || typeof data !== 'object') {
        setStatus('No results returned.', true);
        return;
    }

    setStatus('', false);

    let colorIdx = 0;
    for (const [key, val] of Object.entries(data)) {
        resultsEl.appendChild(renderSection(key, val, colorIdx++));
    }

    btnExportCsv.disabled = false;
}

// -- Section renderer (mirrors analysisPanel.js but with comparison awareness)

function renderSection(title, value, colorIndex) {
    const accentColor = ACCENT_COLORS[colorIndex % ACCENT_COLORS.length];
    const section = document.createElement('div');
    section.className = 'section';
    section.style.setProperty('--section-color', accentColor);

    // Section header
    const header = document.createElement('div');
    header.className = 'section-header';

    const titleEl = document.createElement('div');
    titleEl.className = 'section-title';
    titleEl.textContent = title;
    header.appendChild(titleEl);

    if (/^[A-Z]{2,6}$/.test(title.trim())) {
        const badge = document.createElement('span');
        badge.className = 'speaker-badge';
        badge.textContent = title.trim();
        header.appendChild(badge);
    }
    section.appendChild(header);

    if (typeof value !== 'object' || value === null) {
        // Scalar
        const grid = document.createElement('div');
        grid.className = 'stat-grid';
        grid.appendChild(makeStatCard(title, value, accentColor));
        section.appendChild(grid);
        return section;
    }

    if (Array.isArray(value) && value.length > 0 && typeof value[0] === 'object') {
        // Array of objects → check if it looks like comparison data
        if (isComparisonTable(value)) {
            section.appendChild(renderComparisonTable(value, accentColor));
        } else {
            section.appendChild(renderGenericTable(value, accentColor));
        }
        return section;
    }

    // Object with mixed scalar/array fields
    const primitives = Object.entries(value).filter(
        ([, v]) => !Array.isArray(v) && (typeof v !== 'object' || v === null)
    );
    const arrays = Object.entries(value).filter(([, v]) => Array.isArray(v));

    if (primitives.length > 0) {
        const grid = document.createElement('div');
        grid.className = 'stat-grid';
        for (const [k, v] of primitives) {
            grid.appendChild(makeStatCard(k, v, accentColor));
        }
        section.appendChild(grid);
    }

    for (const [k, v] of arrays) {
        if (v.length > 0 && typeof v[0] === 'object') {
            const label = document.createElement('div');
            label.className = 'sub-section-label';
            label.textContent = k;
            section.appendChild(label);
            if (isComparisonTable(v)) {
                section.appendChild(renderComparisonTable(v, accentColor));
            } else {
                section.appendChild(renderGenericTable(v, accentColor));
            }
        }
    }

    return section;
}

function isComparisonTable(rows) {
    if (rows.length === 0) return false;
    const first = rows[0];
    return 'z_score' in first || 'db_mean' in first || 'db_sd' in first;
}

function renderComparisonTable(rows, accentColor) {
    const table = document.createElement('table');

    const thead = document.createElement('thead');
    const hrow = document.createElement('tr');
    const cols = [
        { key: 'label', label: 'Measure', numeric: false },
        { key: 'score', label: 'Score', numeric: true },
        { key: 'db_mean', label: 'DB Mean', numeric: true },
        { key: 'db_sd', label: 'DB SD', numeric: true },
        { key: 'z_score', label: 'Z-Score', numeric: true },
        { key: 'db_n', label: 'N', numeric: true },
    ];

    for (const col of cols) {
        const th = document.createElement('th');
        th.textContent = col.label;
        if (col.numeric) th.className = 'num';
        hrow.appendChild(th);
    }
    thead.appendChild(hrow);
    table.appendChild(thead);

    const tbody = document.createElement('tbody');
    for (const row of rows) {
        const tr = document.createElement('tr');
        for (const col of cols) {
            const td = document.createElement('td');
            const val = row[col.key];

            if (col.key === 'z_score') {
                if (val === null || val === undefined) {
                    td.textContent = '—';
                    td.className = 'num';
                } else {
                    const z = parseFloat(val);
                    td.textContent = z.toFixed(2);
                    td.className = 'num';
                    if (z >= 0) td.classList.add('z-pos');
                    else td.classList.add('z-neg');
                    if (Math.abs(z) >= 1.5) td.classList.add('z-extreme');
                }
            } else if (col.numeric) {
                td.className = 'num';
                td.textContent = val === null || val === undefined ? '—'
                    : typeof val === 'number' ? formatNum(val) : String(val);
            } else {
                td.textContent = val === null || val === undefined ? '' : String(val);
            }

            tr.appendChild(td);
        }
        tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    return table;
}

function renderGenericTable(rows, accentColor) {
    if (!rows || rows.length === 0) {
        const p = document.createElement('p');
        p.className = 'empty';
        p.textContent = '(no data)';
        return p;
    }

    const headers = [];
    const headerSet = new Set();
    for (const row of rows) {
        for (const key of Object.keys(row)) {
            if (!headerSet.has(key)) { headers.push(key); headerSet.add(key); }
        }
    }

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
    for (const row of rows) {
        const tr = document.createElement('tr');
        for (const h of headers) {
            const td = document.createElement('td');
            const val = row[h];
            td.textContent = val === null || val === undefined ? '' : String(val);
            tr.appendChild(td);
        }
        tbody.appendChild(tr);
    }
    table.appendChild(tbody);
    return table;
}

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
    valEl.className = 'stat-value';
    if (isNumeric) {
        valEl.style.color = accentColor;
        valEl.textContent = formatNum(numVal);
    } else {
        valEl.textContent = valStr;
    }

    card.appendChild(keyEl);
    card.appendChild(valEl);
    return card;
}

function formatNum(n) {
    if (Number.isInteger(n)) return n.toLocaleString();
    return n.toFixed(2);
}

function setStatus(html, isError) {
    statusEl.innerHTML = html;
    statusEl.className = isError ? 'status error' : 'status';
}

// -- CSV export -----------------------------------------------------------

const btnExportCsv = document.getElementById('btn-export-csv');

function csvEscape(value) {
    const str = (value || '').trim();
    if (str.includes(',') || str.includes('"') || str.includes('\n')) {
        return '"' + str.replace(/"/g, '""') + '"';
    }
    return str;
}

function collectCsvFromTables() {
    const tables = document.querySelectorAll('#results table');
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

btnExportCsv.addEventListener('click', () => {
    const csv = collectCsvFromTables();
    if (csv) {
        vscode.postMessage({ command: 'exportCsv', csv });
    }
});
