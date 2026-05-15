/**
 * mediaResolver.ts
 *
 * Resolves a media filename from a CHAT @Media: header to an absolute path on
 * disk by trying several candidate locations and extensions.
 *
 * Search order:
 *   1. docDir/<mediaName>          (exact name as written in @Media:)
 *   2. docDir/<stem><ext>          (for each ext in MEDIA_EXTENSIONS)
 *   3. docDir/media/<stem><ext>    (CLAN conventionally puts media in a subdir)
 */

import * as fs from 'fs';
import * as path from 'path';

/**
 * File extensions tried when the @Media: header does not include one (or has
 * an extension that doesn't exist on disk). Ordered by commonality.
 */
const MEDIA_EXTENSIONS = [
    '.mov', '.mp4', '.mp3', '.wav', '.m4v',
    '.aif', '.avi', '.wmv', '.mpg', '.aiff',
];

/**
 * Finds the absolute path to a media file referenced in a CHAT @Media: header.
 *
 * @param docDir    - Directory that contains the .cha file.
 * @param mediaName - Filename (possibly with or without extension) from @Media:.
 * @returns Absolute path to the first matching file, or undefined if not found.
 */
export function resolveMediaPath(docDir: string, mediaName: string): string | undefined {
    // Step 1: try the name verbatim (it may already carry the right extension).
    const verbatim = path.join(docDir, mediaName);
    if (fs.existsSync(verbatim)) {
        return verbatim;
    }

    // Compute stem (strip any existing extension so we can append our own).
    const existingExt = path.extname(mediaName);
    const stem = existingExt ? mediaName.slice(0, -existingExt.length) : mediaName;

    // Step 2: try docDir/<stem><ext> for each known media extension.
    for (const ext of MEDIA_EXTENSIONS) {
        const candidate = path.join(docDir, stem + ext);
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }

    // Step 3: try docDir/media/<stem><ext> (CLAN's conventional media subdirectory).
    for (const ext of MEDIA_EXTENSIONS) {
        const candidate = path.join(docDir, 'media', stem + ext);
        if (fs.existsSync(candidate)) {
            return candidate;
        }
    }

    return undefined;
}
