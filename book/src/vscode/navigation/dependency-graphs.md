# Dependency Graphs

**Status:** Current
**Last updated:** 2026-04-16 22:25 EDT

The extension can render the grammatical dependency structure of any utterance as an interactive, color-coded graph. This turns the compact `%gra` tier notation (`1|2|SUBJ 2|0|ROOT 3|2|OBJ`) into a visual diagram where you can see at a glance how words relate syntactically.

## Opening a Graph

1. Place your cursor on an utterance that has both `%mor` and `%gra` tiers.
2. Invoke the command using any of these methods:
   - **Keyboard:** `Cmd+Shift+G` (macOS) / `Ctrl+Shift+G` (Windows/Linux)
   - **Context menu:** right-click, then select **Show Dependency Graph**
   - **Command Palette:** `Cmd+Shift+P` and type **TalkBank: Show Dependency Graph**
3. A side panel opens with the rendered graph.

If the utterance lacks a `%gra` tier, the command reports a message and does nothing.

## Graph Layout

Words are displayed as labeled boxes, arranged left-to-right in utterance order (matching the main tier). Colored arcs connect each word to its syntactic head, with the grammatical relation label on each arc. The ROOT relation connects to an invisible root node above the word row.

### Relation Color Coding

| Color | Relations |
|-------|-----------|
| Blue | SUBJ |
| Red | OBJ, OBJ2 |
| Green | ROOT |
| Orange | JCT (adjunct) |
| Purple | MOD (modifier) |
| Light blue | DET (determiner) |
| Teal | QUANT (quantifier) |
| Gray | All other relations |

> **(SCREENSHOT: Dependency graph for a complex utterance with colored arcs)**
> *Capture this: open a dependency graph for an utterance like `*CHI: I want the big cookie .` that has SUBJ, OBJ, DET, and MOD relations. The graph should show multiple colored arcs.*

## Toolbar Controls

The toolbar at the top of the graph panel provides zoom and export controls:

| Button | Action |
|--------|--------|
| **Zoom In** | Increase zoom level by 10% |
| **Zoom Out** | Decrease zoom level by 10% |
| **Slider** | Drag to set zoom (10%--300%) |
| **Fit** | Auto-fit the graph to the panel size |
| **SVG** | Download the graph as an SVG vector file |
| **PNG** | Download the graph as a high-resolution PNG (2x pixel density) |

## Singleton Panel

The graph panel reuses a single tab. Invoking the command on a different utterance updates the existing panel rather than opening a new one. This keeps the editor layout clean when you are stepping through multiple utterances.

## Stale-baseline indicator

If you request a graph while the current document has parse errors
that block a fresh reparse, the graph renders from the last
successful parse rather than failing. In that state the DOT output
gains a muted top-left label:

```text
stale baseline
```

rendered in Courier 10pt, color `#888888`. The label is subordinate
to the graph content (placement + font choice keep it out of the
visual center) and disappears the next time you request a graph
after a successful parse.

This is one of two surfaces that expose `ParseState::StaleBaseline`
to users — the other is alignment-consuming hover cards (see
[Cross-Tier Alignment → Stale-baseline indicator](alignment.md#stale-baseline-indicator)).
Go to Definition, highlights, and inlay hints stay silent in the
same state because their single-position outputs are typically still
correct, and a warning there would be noise.

## How It Works

The rendering pipeline is entirely offline -- no internet connection required.

1. **Server side** (`chatter-lsp`, in `graph/`): The LSP server receives the request via the `talkbank/showDependencyGraph` custom command. It extracts word labels from the `%mor` tier (`graph/labels.rs`), styles each `%gra` relation with a color (`graph/edges.rs`), and assembles a Graphviz DOT digraph (`graph/builder.rs`) with invisible ordering edges to preserve left-to-right word order.

2. **Client side** (`graphPanel.ts`): The extension receives the DOT string and passes it to the bundled `@hpcc-js/wasm` Graphviz renderer, which runs Graphviz entirely in WebAssembly. The rendered SVG is injected into a webview panel. The toolbar provides zoom, fit, and export controls over the SVG DOM.

Because the Graphviz engine is bundled as WASM (`@hpcc-js/wasm`), the entire pipeline works offline on any platform -- no external Graphviz installation is needed.

## Related Chapters

- [Cross-Tier Alignment](alignment.md) -- hover and highlighting for `%mor`/`%gra` alignment
- [Go to Definition](go-to-definition.md) -- jump from `%gra` items to the main tier word
- [CLAN Analysis Commands](../analysis/command-reference.md) -- further syntactic analysis via CLAN commands
- [LSP Protocol](../developer/lsp-protocol.md) -- details of the `talkbank/showDependencyGraph` custom command
