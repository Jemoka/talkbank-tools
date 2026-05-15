/**
 * Pure coder-mode data structures and parsing helpers.
 *
 * These helpers stay free of VS Code and Effect dependencies so the command
 * runtime can test the `.cut` parsing rules without spinning up editor mocks.
 */

/**
 * One node in the hierarchical `.cut` coder tree.
 */
export interface CodeNode {
    /**
     * Raw code label to insert into `%cod`.
     */
    readonly code: string;
    /**
     * Child nodes nested under this code.
     */
    readonly children: readonly CodeNode[];
    /**
     * Tab-indentation depth from the original `.cut` file.
     */
    readonly depth: number;
}

/**
 * Parse a `.cut` coder file into a tree of hierarchical code nodes.
 */
export function parseCodesFile(text: string): CodeNode[] {
    const roots: CodeNode[] = [];
    const stack: Array<{ node: CodeNode; depth: number }> = [];

    for (const line of text.split('\n')) {
        if (line.trim() === '' || line.startsWith('%') || line.startsWith('@')) {
            continue;
        }

        let depth = 0;
        while (depth < line.length && line[depth] === '\t') {
            depth++;
        }

        const code = line.trim();
        if (code === '') {
            continue;
        }

        const node: CodeNode = { code, children: [], depth };

        while (stack.length > 0 && stack[stack.length - 1].depth >= depth) {
            stack.pop();
        }

        if (stack.length === 0) {
            roots.push(node);
        } else {
            (stack[stack.length - 1].node.children as CodeNode[]).push(node);
        }
        stack.push({ node, depth });
    }

    return roots;
}
