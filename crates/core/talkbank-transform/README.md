# talkbank-transform

**Status:** Current
**Last updated:** 2026-04-29 07:53 EDT

Transformation pipelines for [CHAT format](https://talkbank.org/0info/manuals/CHAT.html) (CHAT to JSON, normalization, validation).

## Overview

This crate provides high-level pipeline functions that compose parsing,
validation, and transformation operations for CHAT files. It is the primary
entry point for applications that need to process `.cha` files end-to-end.

Key capabilities:

- **Parse + validate** — `parse_and_validate()` combines parsing with
  multi-layer validation (structural, alignment, semantic) in a single call.
- **CHAT to JSON** — `chat_to_json()` converts CHAT files to validated JSON
  conforming to the [CHAT JSON Schema](https://talkbank.org/schemas/v0.1/chat-file.json).
- **Normalization** — `normalize_chat()` produces canonical CHAT output.
- **Caching** — `UnifiedCache` provides SQLite-based caching for validation
  and round-trip results for large file collections.
- **Corpus operations** — `discover_corpora()` and `build_manifest()` for
  working with large file collections.
- **Parallel validation** — `validate_directory_streaming()` validates
  entire directories with concurrent file processing.

## Usage

```rust,no_run
use talkbank_transform::{parse_and_validate, PipelineError, ParseValidateOptions};

let content = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n\
    @ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
let options = ParseValidateOptions::default().with_validation();
let chat_file = parse_and_validate(content, options).unwrap();
assert_eq!(chat_file.utterances().count(), 1);
```

## License

BSD-3-Clause. See [LICENSE](../../LICENSE) for details.
