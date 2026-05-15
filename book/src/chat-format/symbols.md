# Symbols

**Status:** Reference — partially inaccurate, see warning on the CA section
**Last updated:** 2026-05-11 23:19 EDT

> **WARNING:** The "CA (Conversation Analysis) Delimiters" section below
> erases a parser-level distinction. The symbol registry
> (`resources/spec/symbols/symbol_registry.json`) splits these characters into
> three grammatical categories with different parser treatment:
> `ca_delimiter_symbols`, `ca_element_symbols`, and
> `word_segment_forbidden_*`. `↑` and `↓` *attach to a word*
> (`book↑` is one token); `→`, `↗`, `↘` are *separators* that the
> parser splits as their own nodes. Lumping them together as "CA
> delimiters" with one semantic is misleading. Until this page is
> rewritten to mirror the registry, trust the registry. See
> `<workspace>/docs/architecture-review-2026-05-01.md` §6 stale-doc
> entry for this page.

CHAT uses a rich set of symbols for transcription conventions. This page documents the symbol categories and the symbol registry that drives both the grammar and the Rust crates.

## Symbol Registry

The authoritative symbol definitions live in `resources/spec/symbols/symbol_registry.json`. This JSON file is the single source of truth — it generates:

- Character sets for the tree-sitter grammar (`grammar.js`)
- Rust constants for the model and validation crates
- Validation rules for the spec tool

After any change to the symbol registry, run:

```bash
node resources/spec/symbols/validate_symbol_registry.js && node scripts/generate-symbol-sets.js && node resources/spec/symbols/generate_rust_symbol_sets.js
```

## Symbol Categories

### Terminators

Punctuation that ends an utterance:

| Symbol | Name | Usage |
|--------|------|-------|
| `.` | Period | Declarative |
| `?` | Question | Interrogative |
| `!` | Exclamation | Exclamatory |
| `+...` | Trailing off | Incomplete utterance |
| `+..?` | Trailing-off question | Question trails off |
| `+/.` | Interruption | Speaker interrupted by another |
| `+//.` | Self-interruption | Speaker interrupts self |
| `+/?` | Interrupted question | Question interrupted |
| `+!?` | Broken question | Exclamation-question |
| `+"/.` | Quoted new line | Quotation continues on next line |

### CA (Conversation Analysis) Delimiters

Symbols used in conversation analysis notation:

| Symbol | Meaning |
|--------|---------|
| `↑` | Rising pitch |
| `↓` | Falling pitch |
| `→` | Level pitch |
| `↗` | Rise-fall |
| `↘` | Fall-rise |
| `≋` | Creaky voice |
| `∙` | Micropause |

### CA Elements

Symbols that appear within conversation analysis annotations (prosodic marking, stress, etc.).

### Word Segment Characters

Characters that are forbidden at the start of words, forbidden in the rest of words, or forbidden throughout. These define the lexical boundaries of what constitutes a "word" in CHAT.

The grammar uses these sets to construct the word-matching regex patterns. Characters like `[`, `]`, `<`, `>`, `(`, `)` are structural delimiters and cannot appear inside words.

### Event Segment Characters

Characters forbidden in event descriptions (`&=event` content). Events have slightly different lexical rules than words.

## Language Codes

CHAT uses ISO 639-3 three-letter language codes in `@Languages` headers and `@s:` word markers:

```chat
@Languages:	eng, fra
*CHI:	I want a croissant@s:fra .
```

Common codes: `eng` (English), `fra` (French), `deu` (German), `spa` (Spanish), `zho` (Mandarin), `jpn` (Japanese).

## Special Markers

### @ Markers (Word-Level)

| Marker | Meaning |
|--------|---------|
| `@s:LANG` | Second language word |
| `@l` | Letter |
| `@c` | Child-invented form |
| `@f` | Family-specific word |
| `@n` | Neologism |
| `@o` | Onomatopoeia |
| `@b` | Babbling |
| `@wp` | Word play |
| `@si` | Signed word |

### & Markers (Events and Fillers)

| Prefix | Meaning |
|--------|---------|
| `&=` | Paralinguistic event (e.g., `&=laughs`) |
| `&-` | Filler (e.g., `&-um`) |
| `&+` | Phonological fragment (e.g., `&+sh`) |
| `&~` | Nonword (e.g., `&~mama`) |
| `&*` | Other speaker's speech event (e.g., `&*MOT:word` — speech attributed to another speaker) |

### Scope Markers

| Marker | Meaning |
|--------|---------|
| `[/]` | Partial retrace — speaker repeats the same words |
| `[//]` | Full retrace — speaker restarts with different words |
| `[///]` | Multiple retracing — multiple false starts |
| `[/-]` | Reformulation — speaker rephrases with different structure |
| `[*]` | Error |
| `[?]` | Best guess |
| `[>]` | Overlap follows |
| `[<]` | Overlap precedes |
| `[= text]` | Explanation |
| `[: text]` | Replacement |
