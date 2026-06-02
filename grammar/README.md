# tree-sitter-talkbank

**Status:** Current
**Last updated:** 2026-06-01 00:52 PDT

[![Grammar CI](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-grammar.yml/badge.svg)](https://github.com/TalkBank/talkbank-tools/actions/workflows/bazel-grammar.yml)
[![crates.io](https://img.shields.io/crates/v/tree-sitter-talkbank)](https://crates.io/crates/tree-sitter-talkbank)

A [tree-sitter](https://tree-sitter.github.io/tree-sitter/) grammar for the [CHAT transcription format](https://talkbank.org/0info/manuals/CHAT.html) used by [TalkBank](https://talkbank.org).

CHAT (Codes for the Human Analysis of Transcripts) is a standardized format for transcribing and annotating conversational interactions, widely used in language acquisition research, aphasia studies, and other areas of linguistic analysis.

### The CHAT Manual

The official CHAT manual is the authoritative reference for the transcription format. The Word document is the source of truth; the HTML and PDF are generated from it.

- **Word** (authoritative): [talkbank.org/0info/manuals/CHAT.docx](https://talkbank.org/0info/manuals/CHAT.docx)
- **HTML** (generated): [talkbank.org/0info/manuals/CHAT.html](https://talkbank.org/0info/manuals/CHAT.html)
- **PDF** (generated): [talkbank.org/0info/manuals/CHAT.pdf](https://talkbank.org/0info/manuals/CHAT.pdf)

### What it parses

```
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child Child, MOT Mother
@ID:	eng|Brown|CHI|2;6.|female|||Target_Child|||
@Date:	12-SEP-1962
*CHI:	want more cookie .          <- main tier (speaker + utterance + terminator)
%mor:	v|want qn|more n|cookie     <- morphology tier
%gra:	1|0|ROOT 2|3|QUANT 3|1|OBJ  <- grammatical relations tier
*MOT:	here you go .
@End
```

The grammar produces a full concrete syntax tree (CST) covering headers, main tiers, dependent tiers (morphology, phonology, grammar relations), annotations, overlap markers, CA transcription features, and more. See [GRAMMAR.md](GRAMMAR.md) for the grammar architecture guide.

### Parse tree walkthrough

For a minimal valid CHAT file:

```
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child, MOT Mother
@ID:	eng|corpus|CHI|3;00.|female|typical||Child|||
@ID:	eng|corpus|MOT|||||Mother|||
*CHI:	want more cookie .
@End
```

The grammar produces this CST (simplified):

```
(document
  (utf8_header)
  (begin_header)
  (languages_header)
  (participants_header)
  (id_header)
  (id_header)
  (line
    (utterance
      (main_tier
        (star) speaker: (speaker)        ;; *CHI
        (colon) (tab)
        (tier_body
          content: (contents
            (content_item               ;; "want"
              (word_with_optional_annotations
                word: (standalone_word)))
            (whitespaces)
            (content_item               ;; "more"
              (word_with_optional_annotations
                word: (standalone_word)))
            (whitespaces)
            (content_item               ;; "cookie"
              (word_with_optional_annotations
                word: (standalone_word)))
            (whitespaces))
          ending: (utterance_end
            (period)                    ;; terminator
            (newline))))))
  (end_header))
```

Every byte is accounted for — whitespace, structural punctuation, and content are all explicit nodes. The Rust parser in [`talkbank-parser`](../crates/talkbank-parser/) walks this CST to build a semantic AST.

## Bindings

The actively supported downstream binding is Rust (used by `talkbank-parser`).

Other language bindings in `bindings/` are currently scaffold-level and not the primary maintenance target for v0.1.

For Rust-specific installation and usage details, see `bindings/rust/README.md`.

## Editor support

The `queries/` directory provides tree-sitter queries for:

- **Syntax highlighting** (`highlights.scm`)
- **Injections** (`injections.scm`)
- **Local scope symbols** (`locals.scm`)
- **Symbol tagging** (`tags.scm`)

These are the minimal boilerplate query files used by the default language bindings.

Additional editor query families (for example folds/indents/textobjects) are post-v1 TODOs.

## Documentation

- [GRAMMAR.md](GRAMMAR.md) — grammar architecture guide: design patterns, precedence strategy, node type taxonomy, how to add new constructs
- [CONTRIBUTING.md](CONTRIBUTING.md) — development guide, spec-driven test generation, release steps
- [CHAT Manual](https://talkbank.org/0info/manuals/CHAT.docx) — official CHAT format specification ([HTML](https://talkbank.org/0info/manuals/CHAT.html), [PDF](https://talkbank.org/0info/manuals/CHAT.pdf))

## Upstream sources of truth

This repository is intentionally minimal: grammar implementation plus generated artifacts.

- CHAT format specification: [talkbank.org/0info/manuals/CHAT.docx](https://talkbank.org/0info/manuals/CHAT.docx)
- Construct, error, and symbol specs: [`resources/spec/`](../resources/spec/)

## Development

```bash
npm install                    # Install dependencies
npx tree-sitter generate      # Generate parser from grammar.js
npx tree-sitter test          # Run the test suite
npx tree-sitter parse file.cha # Parse a CHAT file
npm start                      # Launch the web playground
```

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full development guide, including spec-driven test generation and release steps.

## Generated artifacts

Several files in this repository are generated and should not be edited by hand:

- **`src/parser.c`**, **`src/grammar.json`**, **`src/node-types.json`** — generated by `tree-sitter generate` from `grammar.js`
- **`src/generated_symbol_sets.js`** — symbol constants imported by `grammar.js`, generated from the [CHAT spec symbol registry](../resources/spec/symbols/)
- **`test/corpus/`** — test cases generated from CHAT format [specifications](../resources/spec/) via `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` (from repo root)

Contributors working on the grammar itself only need `tree-sitter generate` and `tree-sitter test`. Regenerating symbol sets or test corpus requires running `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs` from the repo root.

## License

BSD-3-Clause. See [LICENSE](LICENSE).
