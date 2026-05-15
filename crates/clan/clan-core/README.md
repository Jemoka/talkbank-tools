# clan-core

**Status:** Current
**Last updated:** 2026-04-29 07:53 EDT

Rust reimplementation of [CLAN](https://talkbank.org/0info/manuals/CLAN.html)
(Computerized Language Analysis) commands for
[CHAT transcripts](https://talkbank.org/0info/manuals/CHAT.html).

## Overview

CLAN is a toolkit by Brian MacWhinney containing ~116 analysis commands
originally implemented in ~215K lines of C/C++. This crate faithfully
reimplements the self-contained analysis commands, leveraging the TalkBank
Rust parsing and model infrastructure for correctness and performance.

The crate provides three categories of commands:

- **Analysis commands** (30) — read-only analyses that compute statistics
  and produce tabular output (FREQ, MLU, DSS, VOCD, etc.)
- **Transform commands** (17) — modify CHAT files in place by adding,
  removing, or rewriting tiers and content (FLO, CHSTRING, DATES, etc.)
- **Format converters** (11) — convert between CHAT and other transcription
  formats (SRT, ELAN, Praat TextGrid, SALT, etc.)

All commands are invoked via `chatter clan <command>`. Legacy CLAN `+flag` syntax
(e.g., `+t*CHI`, `+s"word"`) is supported and automatically translated to modern flags.

## Analysis Commands

| Command | Description | CLAN Manual |
|---------|-------------|-------------|
| CHAINS | Clause chain analysis via code markers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409147) |
| CHIP | Child/parent interaction profile (imitation, repetition, overlap) | |
| CODES | Frequency table of codes from %cod tier | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409098) |
| COMBO | Boolean keyword search (AND/OR logic) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409095) |
| COOCCUR | Word co-occurrence counting (adjacent bigrams) | |
| DIST | Word distribution analysis (first/last turn per word) | |
| DSS | Developmental Sentence Scoring (grammatical complexity) | [ref](https://talkbank.org/0info/manuals/CLAN.html#DSS_Command) |
| EVAL | Language sample evaluation (lexical diversity, error rates, MLU) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc87376473) |
| FLUCALC | Fluency calculation (disfluency metrics) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409273) |
| FREQ | Word frequency analysis (tokens, types, TTR) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409093) |
| FREQPOS | Word frequency by position in utterance (initial/final/other) | |
| GEMFREQ | Word frequency within gem segments (alias for `freq --gem`) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409096) |
| GEMLIST | List gem segments with utterance counts and speakers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409206) |
| IPSYN | Index of Productive Syntax (syntactic structure scoring) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409276) |
| KEYMAP | Contingency tables for coded data (code-follows-code frequencies) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409207) |
| KIDEVAL | Child language evaluation (combined DSS, VOCD, IPSYN, EVAL) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409281) |
| KWAL | Keyword And Line (keyword-in-context search) | |
| MAXWD | Longest words per speaker | |
| MLT | Mean Length of Turn (in utterances and words/morphemes) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409101) |
| MLU | Mean Length of Utterance (in morphemes from %mor, fallback to words) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409094) |
| MODREP | Model/replica comparison from %mod and %pho tiers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409226) |
| MORTABLE | Cross-tabulation of morphological categories | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409286) |
| PHONFREQ | Phonological frequency analysis from %pho tier | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409227) |
| RELY | Inter-rater agreement (Cohen's kappa) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409232) |
| SCRIPT | Compare utterances to a template script (accuracy metrics) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409234) |
| SUGAR | Morphosyntactic structure scoring from %mor and %gra tiers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409287) |
| TIMEDUR | Time duration from media timestamp bullets | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409240) |
| TRNFIX | Compare two dependent tiers and flag misalignments | |
| UNIQ | Report repeated utterances with frequency counts | |
| VOCD | Vocabulary diversity (D statistic via bootstrap TTR sampling) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409241) |
| WDLEN | Word length distribution (in characters) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409247) |

## Transform Commands

| Command | Description | CLAN Manual |
|---------|-------------|-------------|
| CHSTRING | String replacement using a changes file | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409309) |
| COMBTIER | Combine multiple dependent tiers into one | |
| COMPOUND | Normalize compound word formatting (dash to plus notation) | |
| DATACLEAN | Fix common CHAT formatting errors (spacing, brackets, tabs) | |
| DATES | Age computation from @Birth and @Date headers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409311) |
| DELIM | Add missing terminators to main tiers | |
| FIXBULLETS | Fix parsed bullet timing consistency and offsets | |
| FLO | Simplified fluent output (add %flo tier with countable words only) | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409312) |
| LINES | Add or remove line numbers on tiers | |
| LOWCASE | Lowercase words on main tiers | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409329) |
| MAKEMOD | Generate %mod tier with pronunciation model (CMU dictionary) | |
| ORT | Map between alternate orthographies via code tables | |
| POSTMORTEM | Pattern-matching rules for %mor post-processing | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409261) |
| QUOTES | Extract quoted text to separate utterance | |
| REPEAT | Mark utterances containing repetitions/revisions with [+ rep] | |
| RETRACE | Add %ret tier with verbatim copy of main tier content | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409318) |
| TIERORDER | Reorder dependent tiers to canonical order | |

## Format Converters

| Command | Description | CLAN Manual |
|---------|-------------|-------------|
| CHAT2SRT | CHAT to SRT/WebVTT subtitle conversion | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409296) |
| ELAN2CHAT | ELAN XML (.eaf) to CHAT conversion | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409298) |
| LAB2CHAT | LAB timing labels to CHAT conversion | |
| LENA2CHAT | LENA device XML to CHAT conversion | |
| LIPP2CHAT | LIPP phonetic profile to CHAT conversion | |
| PLAY2CHAT | PLAY annotation to CHAT conversion | |
| PRAAT2CHAT | Praat TextGrid to/from CHAT conversion | [ref](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409302) |
| RTF2CHAT | RTF (Rich Text Format) to CHAT conversion | |
| SALT2CHAT | SALT transcript to CHAT conversion | |
| SRT2CHAT | SRT subtitles to CHAT conversion | |
| TEXT2CHAT | Plain text to CHAT conversion | |

## Framework

The shared infrastructure in `src/framework/` replaces CLAN's CUTT framework:

- **`AnalysisCommand` trait** — Interface for analysis commands: `process_utterance()` accumulates state, `finish()` produces typed results.
- **`TransformCommand` trait** — Interface for transform commands that modify CHAT files in place.
- **`FilterConfig`** — Speaker, tier, word, and gem filtering (equivalent to CLAN's `+t`, `-t`, `+s`, `-s`, `+g`, `-g` flags).
- **`AnalysisRunner`** — File loading, filter application, and command dispatch.
- **`CommandOutput` trait** — Typed output formatting (text, JSON, CSV).

## Usage

Commands are invoked through the `chatter` CLI (`crates/chatter-cli/`).

### Word frequency analysis

```bash
chatter clan freq transcript.cha
chatter clan freq '+t*CHI' transcript.cha       # CLAN syntax
```

### MLU for a specific speaker

```bash
chatter clan mlu transcript.cha --speaker CHI
chatter clan mlu '+t*CHI' transcript.cha         # CLAN syntax
```

### Generate simplified fluent output

```bash
chatter clan flo transcript.cha -o output.cha
```

### Convert CHAT to SRT subtitles

```bash
chatter clan chat2srt transcript.cha -o transcript.srt
```

### CLAN flag equivalents

Both styles work interchangeably — CLAN flags are automatically rewritten:

| CLAN flag | Modern equivalent | Meaning |
|-----------|-------------------|---------|
| `+t*CHI` | `--speaker CHI` | Include speaker |
| `-t*CHI` | `--exclude-speaker CHI` | Exclude speaker |
| `+t%mor` | `--tier mor` | Include dependent tier |
| `-t%gra` | `--exclude-tier gra` | Exclude dependent tier |
| `+t@ID="..."` | `--id-filter "..."` | Filter by @ID fields |
| `+s<word>` | `--include-word <word>` | Include word |
| `-s<word>` | `--exclude-word <word>` | Exclude word |
| `+g<label>` | `--gem <label>` | Include gem |
| `-g<label>` | `--exclude-gem <label>` | Exclude gem |
| `+z25-125` | `--range 25-125` | Utterance range |
| `+r6` | `--include-retracings` | Count retraced material |
| `+dN` | `--display-mode N` | Display mode |
| `+k` | `--case-sensitive` | Case-sensitive matching |

## Credits

- **Brian MacWhinney** — Original CLAN design and C/C++ implementation
- **Leonid Spektor** — Original CLAN C code development
- **Franklin Chen** — Rust reimplementation

## License

BSD-3-Clause. See [LICENSE](LICENSE) for details.
