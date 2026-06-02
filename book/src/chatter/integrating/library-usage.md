# Library Usage

**Status:** Current
**Last updated:** 2026-05-11 08:29 EDT

The TalkBank Rust crates can be used as dependencies in your own Rust
projects for parsing, validating, and manipulating CHAT files. This page
shows the most common entry points; the API reference on docs.rs (once
published) is the authoritative source. Until then, treat the rustdoc
comments inside each crate's `src/lib.rs` as the source of truth.

> **Examples on this page are mirrored as a real Cargo test at
> `crates/core/talkbank-transform/tests/book_library_usage_examples.rs`.**
> The book renders them as `rust,ignore` so mdbook doesn't try to link
> against the workspace's many compiled crate variants; the parallel
> test runs the same code under `cargo test` and is what catches API
> drift between this page and the libraries. If you edit either,
> update both.

**Important:** some legacy tree-sitter fragment helpers are synthetic
rather than semantically honest. They can inject fragment input into
boilerplate CHAT text and parse the resulting synthetic file. Prefer
full-file parsing for real tree-sitter use, and do not treat legacy
fragment helpers as the long-term fragment API. For direct-parser
fragment semantics, use direct-parser-native tests instead of treating
synthetic wrappers as the oracle.

## Adding Dependencies

The TalkBank library crates are currently a **source-first preview**
surface. There is not yet a dedicated crates.io publication workflow for
them, so use git or path dependencies today:

```toml
[dependencies]
talkbank-model = { path = "../talkbank-tools/crates/core/talkbank-model" }
talkbank-transform = { path = "../talkbank-tools/crates/core/talkbank-transform" }
talkbank-parser = { path = "../talkbank-tools/crates/core/talkbank-parser" }
```

The published-crate workflow is tracked separately; once it lands these
paths can become `version = "X.Y"` deps.

## Parsing and Validating a CHAT File

The simplest entry point is `parse_and_validate` from
`talkbank-transform`. It takes the source text and a
`ParseValidateOptions`, returns a fully constructed `ChatFile`, or a
`PipelineError` if parsing or validation failed.

```rust,ignore
# extern crate talkbank_model;
# extern crate talkbank_transform;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let source = std::fs::read_to_string("file.cha")?;
let options = ParseValidateOptions::default().with_validation();
let chat_file = parse_and_validate(&source, options)?;

for utt in chat_file.utterances() {
    println!("Speaker: {}", utt.main.speaker);
}
# Ok(())
# }
```

`ChatFile` is generic over a `ValidationState` parameter; the
`parse_and_validate` return defaults to the validated state.
`chat_file.utterances()` returns an iterator over `&Utterance` derived
from the file's `lines` (utterances are interleaved with headers and
comments in source order).

For batch workflows where parser construction overhead matters, reuse a
single `TreeSitterParser` and call `parse_and_validate_with_parser`:

```rust,ignore
# extern crate talkbank_model;
# extern crate talkbank_parser;
# extern crate talkbank_transform;
use talkbank_model::ParseValidateOptions;
use talkbank_parser::TreeSitterParser;
use talkbank_transform::parse_and_validate_with_parser;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
# let chat_files: Vec<std::path::PathBuf> = Vec::new();
let parser = TreeSitterParser::new()?;
let options = ParseValidateOptions::default().with_validation();

for path in &chat_files {
    let source = std::fs::read_to_string(path)?;
    let chat_file = parse_and_validate_with_parser(&parser, &source, options.clone())?;
    let _ = chat_file;
}
# Ok(())
# }
```

`ParseValidateOptions` also exposes `with_alignment()` (implies
`with_validation()`, additionally validates cross-tier alignment for
`%mor`, `%gra`, `%pho`, `%wor`) and `with_strict_linkers()` (enables
E351–E355 self-completion/other-completion linker checks).

## Working with the Model

`ChatFile` stores participants and language metadata as top-level fields
populated from `@Participants` / `@ID` / `@Languages` headers during
parsing. Utterances live in `lines` and are iterated via
`chat_file.utterances()`.

```rust,ignore
# extern crate talkbank_model;
# extern crate talkbank_transform;
use talkbank_model::DependentTier;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let source = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child
@ID:\teng|test|CHI|||||Target_Child|||
*CHI:\thello world .
%mor:\tco|hello n|world .
@End
";
let chat_file = parse_and_validate(source, ParseValidateOptions::default().with_validation())?;

// Participant metadata is top-level on the ChatFile.
let _participants = &chat_file.participants;

// Iterate utterances and their dependent tiers.
for utt in chat_file.utterances() {
    for tier in &utt.dependent_tiers {
        if let DependentTier::Mor(mor_tier) = tier {
            for item in mor_tier.items() {
                println!("POS: {}, Lemma: {}", item.main.pos, item.main.lemma);
            }
        }
    }
}
# Ok(())
# }
```

`DependentTier` is a closed-set enum (`Mor`, `Gra`, `Pho`, `Mod`, `Sin`,
`Act`, `Add`, `Com`, `Err`, `Exp`, `Gpx`, `Int`, `Lan`, …); match on the
variants you care about and ignore the rest. `MorTier::items()` returns
`&[Mor]`; each `Mor` has a main `MorWord` plus optional post-clitics.

## Serializing to CHAT

Bring the `WriteChat` trait into scope and call `to_chat_string()` for a
fully-rendered CHAT string, or `write_chat(&mut writer)` to stream into
any `std::fmt::Write`.

```rust,ignore
# extern crate talkbank_model;
# extern crate talkbank_transform;
use std::fmt::Write as _;

use talkbank_model::ParseValidateOptions;
use talkbank_model::WriteChat;
use talkbank_transform::parse_and_validate;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thello .\n@End\n";
let chat_file = parse_and_validate(source, ParseValidateOptions::default().with_validation())?;

// Convenience: render to a fresh String.
let chat_text = chat_file.to_chat_string();
assert!(chat_text.starts_with("@UTF8"));

// Streaming: write into any std::fmt::Write sink.
let mut output = String::new();
chat_file.write_chat(&mut output)?;
# Ok(())
# }
```

## Serializing to JSON

Prefer the schema-validated helpers in `talkbank_transform::json`:
`to_json_pretty_validated` checks the output against the JSON schema and
catches drift between the data model and the schema. The unvalidated
variants are a faster bypass when you've already validated upstream.

```rust,ignore
# extern crate talkbank_model;
# extern crate talkbank_transform;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::json::to_json_pretty_validated;
use talkbank_transform::parse_and_validate;

# fn main() -> Result<(), Box<dyn std::error::Error>> {
let source = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\thi .\n@End\n";
let chat_file = parse_and_validate(source, ParseValidateOptions::default().with_validation())?;

let json = to_json_pretty_validated(&chat_file)?;
assert!(json.contains("\"speaker\""));
# Ok(())
# }
```

The schema for `ChatFile` lives at `schemas/chat-file/chat-file.schema.json` and is
regenerated from the Rust types via `node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js`. For arbitrary
serde values (not just `ChatFile`), `to_json_unvalidated` /
`to_json_pretty_unvalidated` work the same way without the schema step.

## Custom Error Handling

Lower-level parser entry points stream diagnostics through the
`ErrorSink` trait. Implement it to collect, count, filter, or forward
errors as they arrive — useful when you need finer-grained control than
the `Result<ChatFile, PipelineError>` shape `parse_and_validate` returns.

```rust,ignore
# extern crate talkbank_model;
use talkbank_model::ErrorSink;
use talkbank_model::ParseError;

struct MyErrorHandler;

impl ErrorSink for MyErrorHandler {
    fn report(&self, error: ParseError) {
        // Custom handling: log, filter, count, etc.
        eprintln!("[{}] {}", error.code, error.message);
    }
}
```

`ErrorSink` is `Send + Sync`, and a blanket `&T: ErrorSink` impl means
borrowed references are sinks too — no `Arc` wrapper required. The
built-in `ErrorCollector` (gathers into a `Vec`), `ParseTracker` (counts
by severity), and `NullErrorSink` (discards) cover most common needs;
implement `ErrorSink` directly for everything else.

## Crate Selection Guide

| Need | Crate |
|------|-------|
| Data model types, error types, `WriteChat`, `ErrorSink` | `talkbank-model` |
| Tree-sitter CHAT parsing (low-level) | `talkbank-parser` |
| Full pipeline (parse + validate + JSON, schema validation) | `talkbank-transform` |
| CLAN analysis commands (FREQ, MLU, KIDEVAL, …) | `clan-core` |

`talkbank-model` is the foundation — every other crate depends on it. If
all you need are the AST types and validation, model alone is enough.
`talkbank-transform` brings parsing + JSON + caching but does not own
CLAN command logic.

## Batchalign3-Facing Surface

If you are building Batchalign3 or another external consumer, the stable
surface is usually:

| Batchalign3 need | Prefer |
|------------------|--------|
| Canonical full-file parsing | `talkbank-parser` |
| Parse/validate contracts and typed model access | `talkbank-model` |
| Alignment-aware downstream consumers (`align`, `compare`, `benchmark`) | `talkbank-model` alignment helpers plus the model AST |
| Whole-pipeline parse+validate+convert | `talkbank-transform` |

For batch workflows, keep parser instances reusable and keep alignment
logic separate from parse semantics.

For CLAN analysis integration, prefer the library-owned execution
boundary in `clan-core` instead of constructing command types
ad hoc in outer crates. In practice that means:

- use `clan_core::framework::UtteranceRange` and
  `clan_core::framework::DiscoveredChatFiles` for analysis input
  selection
- parse raw outer-layer command names into
  `clan_core::service::AnalysisCommandName` at the boundary
- use `clan_core::service::AnalysisOptions` and
  `clan_core::service::AnalysisRequestBuilder` when you need to
  translate raw outer-layer option bags into validated CLAN requests
  with library-owned defaults
- use `clan_core::service::AnalysisRequest` to describe which CLAN
  analysis to run
- use `clan_core::service::AnalysisService` when you need rendered
  or JSON analysis output from Rust code

That keeps CLI and editor integrations focused on adapting their own
request shapes while the CLAN crate owns command construction and shared
output behavior.
