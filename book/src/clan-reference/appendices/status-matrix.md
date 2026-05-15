# Command Status Matrix

**Status:** Current
**Last updated:** 2026-05-12 04:26 EDT

Status of all CLAN commands in the Rust reimplementation.

**Coverage: 70/70 CLAN binaries** (100%). Every CLAN binary in `OSX-CLAN/src/unix/bin/` has a corresponding module. The 6 NLP commands (MOR, POST, MEGRASP, etc.) are deliberately not implemented but produce clear error messages. Additionally, 7 implemented commands are outside that local binary inventory: COMPLEXITY, CORELEX, WDSIZE, LAB2CHAT, RTF2CHAT, ROLES, and TRIM.

| Category | Count | Golden Tests |
|----------|-------|-------------|
| Validation | 1 | — |
| Analysis | 34 | 53 |
| Transforms | 23 | 30 |
| Converters | 15 | 15 |
| Not implemented | 6 | — |
| **Total** | **79** | **98** |

## Validation Commands (1)

| Command | Status | Notes |
|---------|--------|-------|
| CHECK | Implemented | Full flag support (+cN, +eN, +gN, +u), 161 error numbers mapped |

## Analysis Commands (34)

| Command | Status | Golden Tests | Parity |
|---------|--------|-------------|--------|
| CHAINS | Implemented | Yes | Verified |
| COMPLEXITY | Implemented | Yes | New (auto-detects UD/legacy) |
| CORELEX | Implemented | Yes | New command |
| CHIP | Implemented | Yes | 100% |
| CODES | Implemented | Yes | Verified |
| COMBO | Implemented | Yes | Verified |
| COOCCUR | Implemented | Yes | Verified |
| DIST | Implemented | Yes | 100% |
| DSS | Implemented | Yes | Verified |
| EVAL | Implemented | Yes | Verified |
| EVAL-D | Implemented | — | Variant of EVAL (DementiaBank norms) |
| FLUCALC | Implemented | Yes | Verified |
| FREQ | Implemented | Yes | Verified |
| FREQPOS | Implemented | Yes | Verified |
| GEMLIST | Implemented | Yes | Verified |
| IPSYN | Implemented | Yes | Verified |
| KEYMAP | Implemented | Yes | Verified |
| KIDEVAL | Implemented | Yes | Verified |
| KWAL | Implemented | Yes | Verified |
| MAXWD | Implemented | Yes | 100% |
| MODREP | Implemented | Yes | Verified |
| MLT | Implemented | Yes | 100% |
| MLU | Implemented | Yes | 100% |
| MORTABLE | Implemented | Yes | Verified |
| PHONFREQ | Implemented | Yes | Verified |
| RELY | Implemented | Yes | Verified |
| SCRIPT | Implemented | Yes | Verified |
| SUGAR | Implemented | Yes | Verified |
| TIMEDUR | Implemented | Yes | 100% |
| TRNFIX | Implemented | Yes | Verified |
| UNIQ | Implemented | Yes | 99% (1 accepted) |
| VOCD | Implemented | Yes | 100% |
| WDLEN | Implemented | Yes | 100% |
| WDSIZE | Implemented | Yes | New (mor stem lengths) |

## Transform Commands (23)

| Command | Status | Golden Tests |
|---------|--------|-------------|
| CHSTRING | Implemented | Yes |
| COMBTIER | Implemented | Yes |
| COMPOUND | Implemented | Yes |
| DATACLEAN | Implemented | Yes |
| DATES | Implemented | Yes |
| DELIM | Implemented | Yes (4 accepted divergences) |
| FIXIT | Implemented | Yes |
| FIXBULLETS | Implemented | Yes |
| FLO | Implemented | Yes |
| GEM | Implemented | Yes (2 tests: all gems + filtered) |
| INDENT | Implemented | Yes (CLAN binary has infinite-loop bug, Rust-only) |
| LINES | Implemented | Yes |
| LONGTIER | Implemented | Yes |
| LOWCASE | Implemented | Yes |
| MAKEMOD | Implemented | Yes |
| ORT | Implemented | Yes |
| POSTMORTEM | Implemented | Yes |
| QUOTES | Implemented | Yes |
| REPEAT | Implemented | Yes |
| RETRACE | Implemented | Yes |
| ROLES | Implemented | Yes |
| TIERORDER | Implemented | Yes |
| TRIM | Implemented | Yes (2 tests: exclude-mor + exclude-all) |

## Format Converters (15)

| Command | Status | Notes |
|---------|--------|-------|
| CHAT2TEXT | Implemented | Plain text export |
| CHAT2ELAN | Implemented | Reverse of ELAN2CHAT |
| CHAT2PRAAT | Implemented | Praat TextGrid export (bidirectional in praat2chat module) |
| CHAT2SRT | Implemented | SRT subtitle export |
| CHAT2VTT | Implemented | WebVTT subtitle export (Rust-side extension of the SRT converter; not in CLAN) |
| ELAN2CHAT | Implemented | ELAN XML import |
| LAB2CHAT | Implemented | LAB format import |
| LENA2CHAT | Implemented | LENA ITS import |
| LIPP2CHAT | Implemented | LIPP format import |
| PLAY2CHAT | Implemented | PLAY format import |
| PRAAT2CHAT | Implemented | Praat TextGrid import |
| RTF2CHAT | Implemented | Rich Text import |
| SALT2CHAT | Implemented | SALT format import |
| SRT2CHAT | Implemented | Subtitle import |
| TEXT2CHAT | Implemented | Plain text import |

## Deliberately Not Implemented (6)

These commands depend on the legacy CLAN MOR data model (trie-based lexicons, HMM/Brill taggers, MaxEnt parsers) which is incompatible with the UD-style morphological representation used in the current CHAT grammar. Use batchalign's neural pipeline instead.

| Command | Purpose | Rationale |
|---------|---------|-----------|
| MOR / MOR_P | Morphological analysis | ~11K lines C, trie lexicon + 5 rule engines, legacy format |
| POST | POS disambiguation | Requires ^-separated ambiguity format not in grammar |
| MEGRASP | Dependency parsing | Requires trained MaxEnt model weights |
| POSTLIST | POST database listing | Operates on proprietary binary format |
| POSTMODRULES | POST rule modification | Operates on proprietary binary format |
| POSTTRAIN | POST model training | Operates on proprietary binary format |

## Compatibility-Alias Subcommands

These CLAN binaries have their own `clap` subcommand variant — exposed
under the same name as the legacy CLAN binary — but are categorized
as compatibility aliases in
`crates/chatter-cli/src/cli/args/clan_commands.rs` (search for
`CompatibilityAlias`). The dedicated subcommand exists so legacy
CLAN scripts that invoke the binary by name continue to work.

Five subcommand names are bucketed as `CompatibilityAlias`: `check`,
`fixit`, `indent`, `longtier`, and `gemfreq`. Only `gemfreq` is
documented in detail here because its dispatch target differs from
its name; the other four (`check`, `fixit`, `indent`, `longtier`)
are listed in the main category tables above and dispatch to their
namesakes through the standard pipeline.

| Subcommand | Dispatches to | Notes |
|---|---|---|
| `gemfreq` | `freq` (with `--gem` required) | See [GEMFREQ](../commands/gemfreq.md). Implemented via `Gemfreq` enum variant + clap `ArgGroup::required(true)` on `--gem`; routes to `AnalysisCommandName::Freq`. |
