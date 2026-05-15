/**
 * Command handler for transcript-linked picture display.
 */

import * as path from 'path';
import { Effect } from 'effect';

import {
    ExtensionCommandRequirements,
    VSCodeWindowService,
} from '../effectCommandRuntime';
import { ExtensionContextService, tryAsync } from '../effectRuntime';
import { findPictures, PicturePanel } from '../picturePanel';

/**
 * Show the picture or picture picker associated with the active CHAT document.
 */
export function showPicture(): Effect.Effect<void, unknown, ExtensionCommandRequirements> {
    return Effect.gen(function*() {
        const context = yield* ExtensionContextService;
        const window = yield* VSCodeWindowService;
        const editor = window.activeTextEditor;
        if (!editor || editor.document.languageId !== 'chat') {
            return;
        }

        const docPath = editor.document.uri.fsPath;
        const pictures = findPictures(docPath, editor.document.getText());

        if (pictures.length === 0) {
            yield* Effect.asVoid(tryAsync('show missing picture message', () => Promise.resolve(
                window.showInformationMessage('No pictures found for this transcript.'),
            )));
            return;
        }

        if (pictures.length === 1) {
            yield* Effect.sync(() => {
                PicturePanel.createOrShow(context, pictures[0]);
            });
            return;
        }

        const items = pictures.map(picture => ({
            label: path.basename(picture),
            description: path.dirname(picture),
            path: picture,
        }));
        const picked = yield* tryAsync('show picture picker', () => Promise.resolve(
            window.showQuickPick(items, {
                placeHolder: 'Select a picture to display',
            }),
        ));
        if (picked) {
            yield* Effect.sync(() => {
                PicturePanel.createOrShow(context, picked.path);
            });
        }
    });
}
