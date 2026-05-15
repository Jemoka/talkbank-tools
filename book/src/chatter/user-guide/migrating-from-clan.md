# Migrating from CLAN to `talkbank-tools`

**Status:** Current
**Last updated:** 2026-05-11 16:38 EDT

This page is for two audiences:

- users moving day-to-day workflows from legacy CLAN binaries to `chatter`
- developers moving implementation work from older CLAN- or Java/Python-based tooling to the Rust rewrite

It is not a promise of character-for-character equivalence with every historical CLAN binary. The goal of the rewrite is a stable, documented Rust toolchain for CHAT parsing, validation, normalization, JSON interchange, and CLAN-style analyses.

## User Workflow Crosswalk

| Legacy workflow | `talkbank-tools` workflow | Notes |
| --- | --- | --- |
| Run `CHECK` on one file | `chatter validate file.cha` | Validation uses stable Rust error codes and richer diagnostics |
| Run `CHECK` on a corpus | `chatter validate corpus/` | Recursive directory validation with cache support |
| Normalize with `fixit` / `indent` / `longtier` | `chatter normalize file.cha` | Normalization writes to stdout unless `-o/--output` is used |
| Convert to an interchange format | `chatter to-json file.cha` | JSON output is defined by the published schema |
| Convert back from interchange format | `chatter from-json file.json` | Uses the same AST/serializer as the Rust toolchain |
| Run CLAN-style analysis commands | `chatter clan ...` | One entry point, with text/json/csv/clan output modes where supported |

## Legacy Flag Translation

`chatter` accepts both legacy CLAN syntax (`+t*CHI`, `-t*MOT`, `+s"word"`, `+z25-125`) **and** the modern explicit syntax (`--speaker CHI`, `--exclude-speaker MOT`, etc.). Both work identically — you do not have to retrain muscle memory. Under the hood, a pre-clap pass in `clan_core::clan_args::rewrite_clan_args` rewrites any `+flag`/`-flag` tokens that follow the `clan` subcommand into their `--flag` equivalents before argument parsing proper begins. Anything the rewriter does not recognise passes through unchanged, so mistyped legacy flags surface as ordinary clap errors rather than silent misinterpretation.

The table below is the complete list of legacy flags the rewriter currently handles. If a flag does not appear here, it is not translated; use the modern `--flag` spelling.

| Legacy CLAN | Modern chatter | Purpose |
| --- | --- | --- |
| `+t*CHI` | `--speaker CHI` | Include speaker |
| `-t*MOT` | `--exclude-speaker MOT` | Exclude speaker |
| `+t%mor` | `--tier mor` | Include dependent tier |
| `-t%gra` | `--exclude-tier gra` | Exclude dependent tier |
| `+t@ID="eng\|*\|CHI\|*"` | `--id-filter eng\|*\|CHI\|*` | Filter by `@ID` header pattern |
| `+s"word"` or `+sword` | `--include-word word` | Include word in search / frequency |
| `-s"word"` or `-sword` | `--exclude-word word` | Exclude word |
| `+glabel` | `--gem label` | Restrict to a named gem |
| `-glabel` | `--exclude-gem label` | Skip a named gem |
| `+z25-125` | `--range 25-125` | Utterance index range |
| `+r6` | `--include-retracings` | Include retraced material (MLU, FREQ) |
| ~~`+dN`~~ | ~~`--display-mode N`~~ | Numeric display mode — **currently non-functional**: the rewriter at `crates/clan-core/src/clan_args.rs:101` produces `--display-mode`, but no `clap` field consumes it. Per-command N tables are CLAN-historical and need source-grounded specs before the flag can be honoured; tracked in `docs/superpowers/plans/2026-05-11-clan-rewriter-honor-three-flags.md` Phase 3. |
| ~~`+k`~~ | ~~`--case-sensitive`~~ | Case-sensitive matching — **currently non-functional**: the rewriter at `crates/clan-core/src/clan_args.rs:104` produces `--case-sensitive`, but no `clap` field consumes it, so the flag fails parsing. Word matching is case-insensitive today. |
| ~~`+fEXT`~~ | ~~`--output-ext EXT`~~ | Output file extension — **currently non-functional**: the rewriter at `crates/clan-core/src/clan_args.rs:107` produces `--output-ext`, but no `clap` field consumes it. Phase 2 of the rewriter-honor plan adds the clap field with `conflicts_with -o`. |
| `+wN` | `--context-after N` | KWAL trailing-context lines |
| `-wN` | `--context-before N` | KWAL leading-context lines |
| `+u` | (no-op) | Merge speakers — default behaviour, dropped silently |

CHECK (`chatter clan check …`) has its own context-sensitive overloads. Inside the `check` subcommand the rewriter recognises these additional forms:

| Legacy CHECK flag | Modern chatter | Purpose |
| --- | --- | --- |
| `+cN` | `--bullets N` | Bullet-check level |
| `+e` | `--list-errors` | List all known error codes |
| `+eN` | `--error N` | Report only a specific error |
| `-eN` | `--exclude-error N` | Suppress a specific error |
| `+g1` | (no-op) | Prosodic-delimiter checks are always on |
| `+g2` | `--check-target` | Require `CHI` to have `Target_Child` role |
| `+g3` | (no-op) | Word-level checks run via the parser |
| `+g4` | `--check-id true` | Warn on missing `@ID` tiers |
| `+g5` | `--check-unused` | Report unused speakers |
| `+u` (in `check`) | `--check-ud` | Validate Universal Dependencies features |

`+g1`–`+g5` are only rewritten as CHECK generics when the `check` subcommand is present in the argument list; in every other command, `+g…` remains gem filtering.

## Common CLAN Workflows in chatter

Side-by-side examples for the most frequent CLAN tasks. Either the legacy or the modern form works — pick whichever is easier to read.

**Frequency analysis for one speaker:**

```bash
# Legacy CLAN
freq +t*CHI file.cha

# chatter, modern syntax
chatter clan freq file.cha --speaker CHI

# chatter, legacy syntax (rewritten internally)
chatter clan freq file.cha +t*CHI
```

**MLU for a speaker over a utterance range:**

```bash
# Legacy
mlu +t*CHI +z25-125 file.cha

# chatter
chatter clan mlu file.cha --speaker CHI --range 25-125
```

**KWAL search with context window:**

```bash
# Legacy
kwal +t*CHI +s"cookie" -w2 +w3 file.cha

# chatter
chatter clan kwal file.cha \
    --speaker CHI --include-word cookie \
    --context-before 2 --context-after 3
```

**FREQ with multiple filters:**

```bash
# Legacy
freq +t*CHI +t%mor -s"the" file.cha

# chatter
chatter clan freq file.cha --speaker CHI --tier mor --exclude-word the
```

**CHECK restricted to a single error code:**

```bash
# Legacy
check +e6 file.cha

# chatter
chatter clan check file.cha --error 6
```

**Gem-scoped analysis:**

```bash
# Legacy
freq +gstory +t*CHI file.cha

# chatter
chatter clan freq file.cha --gem story --speaker CHI
```

## What's Different

Practical changes to expect when you swap a legacy CLAN tool for `chatter`:

- **Output formats.** `chatter` commands accept `--format text|json|csv|clan` where supported; legacy CLAN produced only its own text layout. Build pipelines on `--format json` rather than parsing text.
- **Error codes.** Diagnostics use the stable `E###` / `W###` system documented in `book/src/operations/errors/`, not CLAN's varied error numbering.
- **Cache.** `chatter validate` memoises clean results in the OS cache directory and reuses them on subsequent runs. Pass `--force` to bypass the cache.
- **Unicode.** Full UTF-8 throughout the pipeline; CLAN had several encoding quirks that `chatter` does not reproduce.
- **Determinism.** The same input always produces the same output. A handful of legacy CLAN commands had timing- or filesystem-order-dependent behaviour that `chatter` intentionally does not preserve.
- **JSON interchange.** `chatter to-json` / `chatter from-json` roundtrip through a published JSON schema. CLAN had no stable interchange format.
- **Platform.** One binary builds and runs on Windows, macOS, and Linux. No more platform-specific CLAN binaries or encoding quirks.
- **Unknown legacy flags fail loudly.** If the rewriter does not recognise a `+x` or `-y` token, it is passed through to clap unchanged, which then reports a normal argument error. There is no silent acceptance of unsupported flags.

## Important Behavioral Differences

- `chatter clan` is one command family, not a directory of separate CLAN executables.
- JSON conversion is first-class and schema-backed.
- Validation diagnostics use Rust error codes like `E301` or `W602`, not CLAN's historical numbering.
- Cache behavior is explicit. `validate` reuses cached clean results unless `--force` is passed.
- Normalization does not overwrite files unless you give an output path yourself.

## Recommended User Migration Steps

1. Start by replacing corpus-wide `CHECK` runs with `chatter validate`.
2. Switch any JSON/export glue to `chatter to-json` and `chatter from-json`.
3. Migrate CLAN analysis invocations to `chatter clan ...` and verify any output-format assumptions.
4. Rebaseline automation on Rust error codes and structured JSON output rather than parsing legacy text output.

## Developer Crosswalk

| Legacy implementation style | Rust rewrite replacement |
| --- | --- |
| String-oriented parsing and repair | Typed AST/model in `talkbank-model` |
| Tool-specific parser logic | Shared parser/transform crates |
| Ad-hoc validation checks | Stable validation rules and error codes |
| CLAN-only output assumptions | Shared `clan-core` command implementations plus JSON/CSV support |
| Loosely coupled scripts | Workspace crates plus integration tests and corpus gates |

## What to Change in Developer Workflows

- Build on the Rust crates instead of editing CLAN-era text manipulation logic.
- Treat the AST, serializer, and validator as the source of truth for current behavior.
- Add or update integration tests when changing CLI-visible behavior or corpus-wide semantics.
- Document behavior changes in this book when they affect users or downstream integrations.

## Scope of Compatibility

`talkbank-tools` aims for practical migration, not blind historical reproduction:

- legacy data should continue to parse where feasible
- CLAN-style analyses should document any deliberate divergence
- new documented public contracts are the Rust CLI, preview source-first Rust crates, JSON Schema, and documented diagnostics
