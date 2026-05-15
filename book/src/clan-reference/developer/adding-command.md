# Adding a Command

**Status:** Current
**Last updated:** 2026-05-12 17:30 EDT

## Steps

1. **Create the module** — `crates/clan-core/src/commands/<name>.rs` with
   four types:
   - `<Name>Config` — command configuration (from CLI flags)
   - `<Name>State` — mutable accumulator (implements `Default`)
   - `<Name>Result` — typed output (implements `Serialize`, `Debug`,
     `CommandOutput`); this is what the trait's associated `Output` type
     resolves to
   - `<Name>Command` — unit struct implementing `AnalysisCommand`

2. **Register** — add `pub mod <name>;` to `crates/clan-core/src/commands/mod.rs`

3. **Wire CLI** — add a subcommand variant to `ClanCommands` in
   `crates/chatter-cli/src/cli/args/clan_commands.rs`

4. **Wire dispatch** — add the match arm in the appropriate family module
   (`analysis.rs`, `transforms.rs`, `converters.rs`, `compatibility.rs`,
   or `helpers.rs`) under `crates/chatter-cli/src/commands/clan/`; keep
   `run_clan()` in `crates/chatter-cli/src/commands/clan/mod.rs` as the
   thin top-level dispatcher

5. **Add golden test** — add a test case in the relevant file under
   `crates/clan-core/tests/clan_golden/`

## Skeleton

The trait is defined in `crates/clan-core/src/framework/command.rs`.
It has three associated types (`Config`, `State`, `Output`) and three
methods (`process_utterance`, `end_file` with a default no-op impl, and
`finalize`).

```rust,ignore
//! # NAME — Brief description
//!
//! What the command does and when to use it.

use serde::Serialize;
use talkbank_model::Utterance;

use crate::framework::{AnalysisCommand, CommandOutput, FileContext};

/// Command configuration.
pub struct NameConfig {
    // fields from CLI flags
}

/// Per-run accumulator (carried across all files).
#[derive(Default)]
pub struct NameState {
    // mutable state built up during processing
}

/// Typed analysis result.
#[derive(Debug, Serialize)]
pub struct NameResult {
    // typed output fields
}

impl CommandOutput for NameResult {
    fn render_text(&self) -> String { todo!() }
    fn render_clan(&self) -> String { todo!() }
}

/// The command.
pub struct NameCommand {
    pub config: NameConfig,
}

impl AnalysisCommand for NameCommand {
    type Config = NameConfig;
    type State = NameState;
    type Output = NameResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        // accumulate per-utterance data into `state`
    }

    // end_file has a default no-op impl; override only if you need
    // per-file finalization.

    fn finalize(&self, state: Self::State) -> Self::Output {
        // compute final result from the accumulated state
        todo!()
    }
}
```

## Conventions

- Use `countable_words()` for word iteration — don't roll your own filter
- Use `NormalizedWord` for frequency maps
- Handle missing `%mor` gracefully (skip morpheme counting, don't panic)
- Keep the module under 400 lines; split into submodules well before it becomes another 800+ line file
