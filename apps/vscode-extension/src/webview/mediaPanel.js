// -----------------------------------------------------------------------
// Data injected from the extension (via INJECT_DATA script block)
// Expects: SEGMENTS, MEDIA_URI, START_IDX, IS_VIDEO, DEFAULT_SPEED, LOOP_COUNT, PAUSE_MS, WALK_LENGTH
// -----------------------------------------------------------------------

// -----------------------------------------------------------------------
// VS Code API (message passing back to extension)
// -----------------------------------------------------------------------
const vscode = acquireVsCodeApi();

// -----------------------------------------------------------------------
// DOM references
// -----------------------------------------------------------------------
const media       = document.getElementById('media');
const segBarFill  = document.getElementById('segBarFill');
const timeCurrent = document.getElementById('timeCurrent');
const timeEnd     = document.getElementById('timeEnd');
const segCount    = document.getElementById('segCount');
const statusEl    = document.getElementById('status');
const prevBtn     = document.getElementById('prevBtn');
const stopBtn     = document.getElementById('stopBtn');
const nextBtn     = document.getElementById('nextBtn');
const loopBtn     = document.getElementById('loopBtn');
const segList     = document.getElementById('segList');
const speedSlider = document.getElementById('speedSlider');
const speedLabel  = document.getElementById('speedLabel');

// -----------------------------------------------------------------------
// Playback state
// -----------------------------------------------------------------------
let currentIndex = START_IDX;
let pollTimer    = null;
let looping      = false;
/** How many times the current segment has looped (for LOOP_COUNT setting). */
let loopIterations = 0;
/** How many segments have been played since continuous play started (for WALK_LENGTH). */
let walkSteps = 0;
/** Timer for inter-segment pause. */
let pauseTimer = null;

// -----------------------------------------------------------------------
// Initialise media element
// -----------------------------------------------------------------------
media.src = MEDIA_URI;
media.preload = 'auto';

// Apply default playback speed from settings.
if (typeof DEFAULT_SPEED === 'number' && DEFAULT_SPEED !== 100) {
    var rate = DEFAULT_SPEED / 100;
    media.playbackRate = rate;
    speedSlider.value = DEFAULT_SPEED;
    speedLabel.textContent = rate === 1 ? '1x' : rate.toFixed(2).replace(/0$/, '') + 'x';
}

// -----------------------------------------------------------------------
// Time formatting — "m:ss.t" (one decimal place, zero-padded seconds)
// -----------------------------------------------------------------------
function fmtMs(ms) {
    const totalSec = ms / 1000;
    const m = Math.floor(totalSec / 60);
    const s = (totalSec % 60).toFixed(1).padStart(4, '0');
    return m + ':' + s;
}

function fmtSec(sec) {
    return fmtMs(sec * 1000);
}

// -----------------------------------------------------------------------
// Segment list — built once at startup
// -----------------------------------------------------------------------
function buildSegmentList() {
    if (SEGMENTS.length === 0) {
        const notice = document.createElement('div');
        notice.className = 'empty-notice';
        notice.textContent = 'No timed segments found.';
        segList.appendChild(notice);
        return;
    }

    SEGMENTS.forEach(function(seg, idx) {
        const row = document.createElement('div');
        row.className = 'seg-row';
        row.dataset.index = idx;

        const indicator = document.createElement('span');
        indicator.className = 'seg-row-indicator';
        indicator.textContent = '';  // filled in setCurrentRow

        const range = document.createElement('span');
        range.className = 'seg-row-range';
        range.textContent = fmtMs(seg.beg) + ' \u2192 ' + fmtMs(seg.end);

        const lineLabel = document.createElement('span');
        lineLabel.className = 'seg-row-line';
        lineLabel.textContent = 'L' + (seg.line + 1);

        row.appendChild(indicator);
        row.appendChild(range);
        row.appendChild(lineLabel);

        row.addEventListener('click', function() {
            jumpTo(idx);
        });

        segList.appendChild(row);
    });
}

/** Highlight the row for the given index and scroll it into view. */
function setCurrentRow(index) {
    segList.querySelectorAll('.seg-row').forEach(function(row, idx) {
        const isCurrent = idx === index;
        row.classList.toggle('current', isCurrent);
        row.querySelector('.seg-row-indicator').textContent = isCurrent ? '\u25b8' : '';
    });

    const currentRow = segList.querySelector('.seg-row.current');
    if (currentRow) {
        currentRow.scrollIntoView({ block: 'nearest', behavior: 'smooth' });
    }
}

/** Update all display elements for the given segment index. */
function updateDisplay(index) {
    const seg = SEGMENTS[index];
    if (!seg) {
        segCount.textContent = '\u2014';
        timeEnd.textContent = '\u2014';
        return;
    }
    segCount.textContent = (index + 1) + ' / ' + SEGMENTS.length;
    timeEnd.textContent = fmtMs(seg.end);
    timeCurrent.textContent = fmtMs(seg.beg);
    setCurrentRow(index);
}

/** Update the thin segment progress bar (0–1 fraction). */
function setProgress(fraction) {
    segBarFill.style.width = (Math.min(1, Math.max(0, fraction)) * 100).toFixed(1) + '%';
}

function clearPoll() {
    if (pollTimer !== null) {
        clearInterval(pollTimer);
        pollTimer = null;
    }
    if (pauseTimer !== null) {
        clearTimeout(pauseTimer);
        pauseTimer = null;
    }
}

function onPlaybackEnded() {
    clearPoll();
    media.pause();
    setProgress(1);
    statusEl.textContent = 'Stopped.';
    stopBtn.disabled = true;
    prevBtn.disabled = false;
    looping = false;
    loopBtn.classList.remove('active');
    walkSteps = 0;
    vscode.postMessage({ command: 'stopped' });
}

/** Advance to the next segment, respecting PAUSE_MS and WALK_LENGTH. */
function advanceToNext(nextIndex) {
    walkSteps++;
    // WALK_LENGTH: 0 = unlimited, >0 = stop after N segments
    if (typeof WALK_LENGTH === 'number' && WALK_LENGTH > 0 && walkSteps >= WALK_LENGTH) {
        walkSteps = 0;
        onPlaybackEnded();
        return;
    }
    // PAUSE_MS: insert a delay before the next segment
    if (typeof PAUSE_MS === 'number' && PAUSE_MS > 0) {
        media.pause();
        statusEl.textContent = 'Pausing\u2026';
        pauseTimer = setTimeout(function() {
            pauseTimer = null;
            playSegment(nextIndex);
        }, PAUSE_MS);
    } else {
        playSegment(nextIndex);
    }
}

// -----------------------------------------------------------------------
// Core playback
// -----------------------------------------------------------------------
function playSegment(index) {
    if (index >= SEGMENTS.length) { onPlaybackEnded(); return; }

    if (index !== currentIndex) { loopIterations = 0; }
    currentIndex = index;
    const seg = SEGMENTS[index];

    vscode.postMessage({ command: 'segmentChanged', index: index });

    updateDisplay(index);
    statusEl.textContent = looping ? 'Looping\u2026' : 'Playing\u2026';
    stopBtn.disabled = false;
    prevBtn.disabled = index === 0;
    nextBtn.disabled = index >= SEGMENTS.length - 1;

    media.currentTime = seg.beg / 1000;
    media.play().catch(function(err) {
        statusEl.textContent = 'Playback error: ' + err.message;
    });

    const segDuration = (seg.end - seg.beg);  // ms

    clearPoll();
    pollTimer = setInterval(function() {
        const elapsed = (media.currentTime * 1000) - seg.beg;
        timeCurrent.textContent = fmtSec(media.currentTime);
        setProgress(elapsed / segDuration);

        if (media.currentTime >= seg.end / 1000) {
            clearPoll();
            if (looping) {
                loopIterations++;
                // LOOP_COUNT: 0 = infinite, >0 = stop after N loops
                if (typeof LOOP_COUNT === 'number' && LOOP_COUNT > 0 && loopIterations >= LOOP_COUNT) {
                    looping = false;
                    loopBtn.classList.remove('active');
                    loopIterations = 0;
                    onPlaybackEnded();
                } else {
                    playSegment(currentIndex);
                }
            } else if (index + 1 < SEGMENTS.length) {
                advanceToNext(index + 1);
            } else {
                onPlaybackEnded();
            }
        }
    }, 100);
}

/** Jump to a segment directly (e.g. from clicking the list). */
function jumpTo(index) {
    clearPoll();
    looping = false;
    loopBtn.classList.remove('active');
    walkSteps = 0;
    playSegment(index);
}

// -----------------------------------------------------------------------
// Time display outside of an active poll
// -----------------------------------------------------------------------
media.addEventListener('timeupdate', function() {
    if (pollTimer === null) {
        timeCurrent.textContent = fmtSec(media.currentTime);
    }
});

// -----------------------------------------------------------------------
// Control buttons
// -----------------------------------------------------------------------
stopBtn.addEventListener('click', function() { onPlaybackEnded(); });

prevBtn.addEventListener('click', function() {
    if (currentIndex > 0) { jumpTo(currentIndex - 1); }
});

nextBtn.addEventListener('click', function() {
    if (currentIndex + 1 < SEGMENTS.length) { jumpTo(currentIndex + 1); }
});

loopBtn.addEventListener('click', function() {
    looping = !looping;
    loopBtn.classList.toggle('active', looping);
    statusEl.textContent = looping ? 'Looping\u2026' : 'Playing\u2026';
});

speedSlider.addEventListener('input', function() {
    var rate = parseInt(speedSlider.value) / 100;
    media.playbackRate = rate;
    speedLabel.textContent = rate === 1 ? '1x' : rate.toFixed(2).replace(/0$/, '') + 'x';
});

// -----------------------------------------------------------------------
// Inbound messages from the extension
// -----------------------------------------------------------------------
window.addEventListener('message', function(event) {
    const msg = event.data;
    if (!msg || !msg.command) { return; }

    switch (msg.command) {
        case 'rewind': {
            const seconds = typeof msg.seconds === 'number' ? msg.seconds : 2;
            media.currentTime = Math.max(0, media.currentTime - seconds);
            break;
        }
        case 'setLoop': {
            looping = !looping;
            loopBtn.classList.toggle('active', looping);
            statusEl.textContent = looping ? 'Looping\u2026' : 'Playing\u2026';
            break;
        }
        case 'requestTimestamp': {
            const ms = Math.round(media.currentTime * 1000);
            vscode.postMessage({ command: 'timestamp', ms: ms });
            break;
        }
        case 'seekTo': {
            if (typeof msg.ms === 'number') {
                media.currentTime = msg.ms / 1000;
            }
            break;
        }
    }
});

// -----------------------------------------------------------------------
// Initialise on load
// -----------------------------------------------------------------------
buildSegmentList();
updateDisplay(START_IDX);

media.addEventListener('canplay', function onCanPlay() {
    media.removeEventListener('canplay', onCanPlay);
    statusEl.textContent = 'Ready.';
    playSegment(START_IDX);
});

media.addEventListener('error', function() {
    statusEl.textContent = 'Error loading media file.';
});
