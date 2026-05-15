import { describe, expect, it } from 'vitest';

import { parseCodesFile } from '../coderModel';

describe('coderModel', () => {
    it('parses tab-indented .cut trees into nested code nodes', () => {
        const tree = parseCodesFile([
            '$PRA',
            '\t$PRA:request',
            '\t$PRA:demand',
            '$ACT',
            '\t$ACT:play',
        ].join('\n'));

        expect(tree).toHaveLength(2);
        expect(tree[0]).toMatchObject({
            code: '$PRA',
            depth: 0,
        });
        expect(tree[0].children).toEqual([
            { code: '$PRA:request', depth: 1, children: [] },
            { code: '$PRA:demand', depth: 1, children: [] },
        ]);
        expect(tree[1]).toMatchObject({
            code: '$ACT',
            depth: 0,
        });
        expect(tree[1].children).toEqual([
            { code: '$ACT:play', depth: 1, children: [] },
        ]);
    });

    it('ignores blank lines and CHAT-style header/comment lines', () => {
        const tree = parseCodesFile([
            '@Begin',
            '% comment',
            '',
            '$TOP',
            '\t$CHILD',
        ].join('\n'));

        expect(tree).toEqual([
            {
                code: '$TOP',
                depth: 0,
                children: [
                    {
                        code: '$CHILD',
                        depth: 1,
                        children: [],
                    },
                ],
            },
        ]);
    });
});
