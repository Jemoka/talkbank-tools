# talkbank-model

**Status:** Current
**Last updated:** 2026-04-29 07:53 EDT

TalkBank data model and validation for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

This crate defines the complete abstract syntax tree (AST) for CHAT
(Codes for the Human Analysis of Transcripts), the standard transcription
format for language research used by TalkBank. It provides:

- **Data model** — Rust types for every CHAT construct: files, headers,
  participants, utterances, words, dependent tiers (%mor, %gra, %pho, etc.),
  annotations, and more.
- **Validation** — Multi-layer validation including structural checks,
  cross-tier alignment verification, and semantic consistency rules.
- **Serialization** — Full serde support for JSON round-tripping via the
  `talkbank-transform` crate.

The model is parser-independent: it represents the result of parsing but does
not depend on any particular parser. Both the tree-sitter and direct parsers
produce `ChatFile` values from this crate.

## Key Types

- `ChatFile` — Root AST node representing a complete `.cha` file
- `Utterance` — A single speaker turn with main tier and dependent tiers
- `Word` — Individual word with form, category, and language metadata
- `MorTier` / `GraTier` / `PhoTier` — Morphological, grammatical relation,
  and phonological dependent tiers
- `Header` — File-level metadata (participants, languages, options)

## Usage

```rust
use talkbank_model::{ChatFile, Provenance};

// ChatFile is typically produced by a parser, not constructed directly.
// See talkbank-transform for the parse-and-validate pipeline.
let file = ChatFile::<Provenance<true>>::default();
assert!(file.utterances().is_empty());
```

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
