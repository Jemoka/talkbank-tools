// -----------------------------------------------------------------------
// @ID Editor webview script
//
// Communicates with idEditorPanel.ts via PostMessage:
//   → save (with edited entries)
//   ← entries, saved, error
// -----------------------------------------------------------------------

// @ts-check
/* global acquireVsCodeApi */

const vscode = acquireVsCodeApi();

const ACCENT_COLORS = [
    '#4ec9b0',  // teal
    '#ce9178',  // rust / salmon
    '#569cd6',  // cornflower blue
    '#dcdcaa',  // khaki / gold
    '#c586c0',  // lavender
    '#9cdcfe',  // sky blue
];

const COMMON_ROLES = [
    'Target_Child', 'Child', 'Mother', 'Father', 'Sibling',
    'Grandmother', 'Grandfather', 'Investigator', 'Observer',
    'Teacher', 'Therapist', 'Clinician', 'Participant',
    'Partner', 'Nurse', 'Doctor', 'Student', 'Adult',
    'Brother', 'Sister', 'Aunt', 'Uncle', 'Cousin',
    'Babysitter', 'Playmate', 'Visitor', 'Environment',
];

// -- DOM refs -------------------------------------------------------------
const entriesEl = document.getElementById('entries');
const btnSave = document.getElementById('btn-save');
const statusEl = document.getElementById('status');
const fileNameEl = document.getElementById('file-name');

let currentEntries = [];

// -- Event listeners ------------------------------------------------------

btnSave.addEventListener('click', () => {
    const updated = readFormEntries();
    vscode.postMessage({ command: 'save', entries: updated });
    setStatus('Saving…', '');
});

// -- PostMessage handler --------------------------------------------------

window.addEventListener('message', (event) => {
    const msg = event.data;
    switch (msg.command) {
        case 'entries':
            currentEntries = msg.entries || [];
            fileNameEl.textContent = msg.fileName || '';
            renderEntries(currentEntries);
            setStatus('', '');
            break;
        case 'saved':
            setStatus('Changes saved.', 'success');
            break;
        case 'error':
            setStatus(msg.message, 'error');
            break;
    }
});

// -- Render entries -------------------------------------------------------

function renderEntries(entries) {
    entriesEl.innerHTML = '';

    if (entries.length === 0) {
        const p = document.createElement('p');
        p.className = 'empty';
        p.textContent = 'No @ID headers found in this file.';
        entriesEl.appendChild(p);
        return;
    }

    entries.forEach((entry, idx) => {
        const card = document.createElement('div');
        card.className = 'participant';
        const color = ACCENT_COLORS[idx % ACCENT_COLORS.length];
        card.style.setProperty('--accent-color', color);
        card.dataset.index = idx;
        card.dataset.line = entry.line;

        // Header
        const header = document.createElement('div');
        header.className = 'participant-header';

        const codeEl = document.createElement('span');
        codeEl.className = 'participant-code';
        codeEl.textContent = entry.fields.speaker || '???';
        codeEl.style.color = color;
        header.appendChild(codeEl);

        const roleEl = document.createElement('span');
        roleEl.className = 'participant-role';
        roleEl.textContent = entry.fields.role || '';
        header.appendChild(roleEl);

        card.appendChild(header);

        // Field grid
        const grid = document.createElement('div');
        grid.className = 'field-grid';

        grid.appendChild(makeTextField('Language', 'language', entry.fields.language, true));
        grid.appendChild(makeTextField('Corpus', 'corpus', entry.fields.corpus, false));
        grid.appendChild(makeTextField('Speaker', 'speaker', entry.fields.speaker, true));
        grid.appendChild(makeTextField('Age', 'age', entry.fields.age, false, 'e.g. 3;06.15'));
        grid.appendChild(makeSexField(entry.fields.sex));
        grid.appendChild(makeTextField('Group', 'group', entry.fields.group, false));
        grid.appendChild(makeTextField('SES', 'ses', entry.fields.ses, false));
        grid.appendChild(makeRoleField(entry.fields.role));
        grid.appendChild(makeTextField('Education', 'education', entry.fields.education, false));
        grid.appendChild(makeTextField('Custom', 'custom', entry.fields.custom, false));

        card.appendChild(grid);
        entriesEl.appendChild(card);
    });
}

function makeTextField(label, name, value, required, placeholder) {
    const field = document.createElement('div');
    field.className = 'field';

    const labelEl = document.createElement('label');
    labelEl.className = 'field-label';
    labelEl.textContent = label + (required ? ' *' : '');

    const input = document.createElement('input');
    input.type = 'text';
    input.name = name;
    input.value = value || '';
    if (placeholder) input.placeholder = placeholder;
    if (required) input.className = 'required';

    field.appendChild(labelEl);
    field.appendChild(input);
    return field;
}

function makeSexField(value) {
    const field = document.createElement('div');
    field.className = 'field';

    const labelEl = document.createElement('label');
    labelEl.className = 'field-label';
    labelEl.textContent = 'Sex';

    const select = document.createElement('select');
    select.name = 'sex';

    const options = ['', 'male', 'female'];
    for (const opt of options) {
        const option = document.createElement('option');
        option.value = opt;
        option.textContent = opt || '(not specified)';
        if (opt === (value || '').toLowerCase()) option.selected = true;
        select.appendChild(option);
    }

    // If the value doesn't match known options, add it as-is
    const normalized = (value || '').toLowerCase();
    if (value && normalized !== 'male' && normalized !== 'female') {
        const option = document.createElement('option');
        option.value = value;
        option.textContent = value;
        option.selected = true;
        select.appendChild(option);
    }

    field.appendChild(labelEl);
    field.appendChild(select);
    return field;
}

function makeRoleField(value) {
    const field = document.createElement('div');
    field.className = 'field';

    const labelEl = document.createElement('label');
    labelEl.className = 'field-label';
    labelEl.textContent = 'Role *';

    // Use a datalist for autocomplete with free-form input
    const input = document.createElement('input');
    input.type = 'text';
    input.name = 'role';
    input.value = value || '';
    input.className = 'required';
    input.setAttribute('list', 'role-options');

    field.appendChild(labelEl);
    field.appendChild(input);
    return field;
}

// -- Read form back -------------------------------------------------------

function readFormEntries() {
    const cards = entriesEl.querySelectorAll('.participant');
    const entries = [];
    cards.forEach((card) => {
        const line = parseInt(card.dataset.line, 10);
        const fields = {};
        const fieldNames = [
            'language', 'corpus', 'speaker', 'age', 'sex',
            'group', 'ses', 'role', 'education', 'custom'
        ];
        for (const name of fieldNames) {
            const el = card.querySelector(`[name="${name}"]`);
            fields[name] = el ? el.value : '';
        }
        entries.push({ line, fields });
    });
    return entries;
}

// -- Status ---------------------------------------------------------------

function setStatus(text, type) {
    statusEl.textContent = text;
    statusEl.className = type ? 'status ' + type : 'status';
}

// -- Role datalist (shared across all participants) -----------------------
const datalist = document.createElement('datalist');
datalist.id = 'role-options';
for (const role of COMMON_ROLES) {
    const opt = document.createElement('option');
    opt.value = role;
    datalist.appendChild(opt);
}
document.body.appendChild(datalist);
