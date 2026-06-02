# chatter-lsp

**Status:** Current
**Last updated:** 2026-06-01 00:52 PDT

[Language Server Protocol](https://microsoft.github.io/language-server-protocol/) implementation for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

`chatter-lsp` is both a library (reusable IDE server implementation) and a
standalone stdio binary (`chatter-lsp`) for CHAT transcription files via the
Language Server Protocol. It uses tree-sitter for incremental parsing and the
`talkbank-model` validation pipeline for real-time diagnostics.

## Features

- **Diagnostics** — real-time validation errors and warnings as you type
- **Hover** — alignment timing, speaker info, and error explanations
- **Completion** — speaker codes, header keywords, and coding symbols
- **Code actions** — quick fixes for auto-fixable validation errors
- **Semantic highlighting** — syntax-aware token coloring via `talkbank-highlight`
- **Document formatting** — canonical CHAT normalization
- **Go to definition / references** — navigate speaker and tier relationships

## Editor Integration

### VS Code

Install the [TalkBank VS Code extension](../../apps/vscode-extension/), which bundles and
launches the `chatter-lsp` binary automatically.

### Other Editors

Any editor with LSP support can use the server. Start it with:

```bash
chatter-lsp
```

The server communicates over stdio using the standard LSP JSON-RPC protocol.

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
