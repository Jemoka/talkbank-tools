/**
 * specialChars.ts
 *
 * CHAT/CA special character input via compose-key mode.
 *
 * Ported from OSX-CLAN's CharToSpChar() function. Two prefix keys:
 * - F1 (CA mode): Conversation Analysis symbols (intonation, overlap, etc.)
 * - F2 (non-CA mode): IPA, diacritics, and other CHAT markers
 *
 * Usage: press the compose key, then press the character key to insert the
 * corresponding Unicode symbol at the cursor position.
 */

import * as vscode from 'vscode';

/** Map from trigger key to Unicode codepoint for CA (F1) mode. */
const CA_CHARS: Record<string, string> = {
    '1': '\u21D7', // rise to high ⇗
    '2': '\u2197', // rise to mid ↗
    '3': '\u2192', // level →
    '4': '\u2198', // fall to mid ↘
    '5': '\u21D8', // fall to low ⇘
    '6': '\u221E', // unmarked ending ∞
    '+': '\u224B', // continuation ≋
    '.': '\u2219', // inhalation ∙
    '=': '\u2248', // latching ≈
    'u': '\u2261', // uptake ≡
    'U': '\u2261',
    '[': '\u2308', // raised [ ⌈
    ']': '\u2309', // raised ] ⌉
    '{': '\u230A', // lowered [ ⌊
    '}': '\u230B', // lowered ] ⌋
    '*': '\u204E', // creaky ⁎
    '/': '\u2047', // unsure ⁇
    '0': '\u00B0', // softer °
    ')': '\u25C9', // louder ◉
    'd': '\u2581', // low pitch ▁
    'h': '\u2594', // high pitch ▔
    'l': '\u263A', // smile voice ☺
    'L': '\u263A',
    'b': '\u264B', // breathy voice ♋
    'w': '\u222C', // whisper ∬
    'W': '\u222C',
    'y': '\u03AB', // yawn Ϋ
    'Y': '\u03AB',
    's': '\u222E', // singing ∮
    'S': '\u222E',
    'p': '\u00A7', // precise §
    'P': '\u00A7',
    'n': '\u223E', // constriction ∾
    'N': '\u223E',
    'r': '\u21BB', // pitch reset ↻
    'R': '\u21BB',
    'c': '\u1F29', // laugh in word Ἡ
    'C': '\u1F29',
    'q': '\u2907', // hurried start ⤇
    'Q': '\u2907',
    'x': '\u2906', // sudden stop ⤆
    'X': '\u2906',
    't': '\u2051', // hardening ⁑
    'T': '\u2051',
};

/** Map from trigger key to Unicode codepoint for non-CA (F2) mode. */
const NONCA_CHARS: Record<string, string> = {
    'H': '\u02B0', // raised h ʰ
    ',': '\u0323', // dot diacritic ̣
    '<': '\u2039', // group start ‹
    '>': '\u203A', // group end ›
    't': '\u201E', // tag/sentence particle „
    'T': '\u201E',
    'v': '\u2021', // vocative ‡
    'V': '\u2021',
    '-': '\u0304', // stress  ̄
    'q': '\u0294', // glottal stop ʔ
    'Q': '\u0295', // Hebrew glottal ʕ
    ';': '\u030C', // caron  ̌
    '1': '\u02C8', // raised stroke ˈ
    '2': '\u02CC', // lowered stroke ˌ
    '{': '\u3014', // sign group start 〔
    '}': '\u3015', // sign group end 〕
    'm': '\u2026', // %pho missing word …
    '_': '\u0332', // underline  ̲
    '\'': '\u201C', // open quote "
    '"': '\u201D', // close quote "
    '=': '\u2260', // crossed equal ≠
    '/': '\u21AB', // left arrow with circle ↫
    ':': '\u02D0', // long vowel ː
};

/** Active compose mode: null = inactive, 'ca' = F1 prefix, 'nonca' = F2 prefix */
let composeMode: 'ca' | 'nonca' | null = null;

/** Status bar item showing compose mode. */
let statusItem: vscode.StatusBarItem | undefined;

/** The type handler disposable, registered only while compose mode is active. */
let typeDisposable: vscode.Disposable | undefined;

function showStatus(mode: 'ca' | 'nonca'): void {
    if (!statusItem) {
        statusItem = vscode.window.createStatusBarItem(vscode.StatusBarAlignment.Right, 200);
    }
    statusItem.text = mode === 'ca' ? '$(symbol-key) CA Char…' : '$(symbol-key) CHAT Char…';
    statusItem.backgroundColor = new vscode.ThemeColor('statusBarItem.warningBackground');
    statusItem.show();
}

function hideStatus(): void {
    statusItem?.hide();
}

function deactivateCompose(): void {
    composeMode = null;
    hideStatus();
    typeDisposable?.dispose();
    typeDisposable = undefined;
}

/**
 * Enter compose mode. The next printable keystroke will be intercepted
 * and replaced with the corresponding special character.
 */
export function enterComposeMode(mode: 'ca' | 'nonca'): void {
    // If already active, cancel first.
    if (typeDisposable) {
        deactivateCompose();
    }

    composeMode = mode;
    showStatus(mode);

    // Register a 'type' command override to intercept the next keystroke.
    typeDisposable = vscode.commands.registerCommand('type', (args: { text: string }) => {
        const charMap = composeMode === 'ca' ? CA_CHARS : NONCA_CHARS;
        const replacement = charMap[args.text];

        // Always deactivate after one keystroke.
        deactivateCompose();

        const editor = vscode.window.activeTextEditor;
        if (!editor) { return; }

        if (replacement) {
            editor.edit(editBuilder => {
                for (const selection of editor.selections) {
                    editBuilder.replace(selection, replacement);
                }
            });
        } else {
            // No mapping found — insert the original character and show a hint.
            vscode.commands.executeCommand('default:type', args);
            vscode.window.setStatusBarMessage(
                `No special character for '${args.text}' in ${composeMode === 'ca' ? 'CA' : 'CHAT'} mode`,
                3000,
            );
        }
    });
}

/** Cancel compose mode without inserting anything. */
export function cancelCompose(): void {
    if (composeMode) {
        deactivateCompose();
        vscode.window.setStatusBarMessage('Compose cancelled', 2000);
    }
}

/** Dispose all resources (called from extension deactivate). */
export function disposeSpecialChars(): void {
    deactivateCompose();
    statusItem?.dispose();
}
