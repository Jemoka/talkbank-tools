# Language Handling in CHAT: Complete Data Model

**Status:** Current
**Last updated:** 2026-05-06 20:33 EDT

---

## Overview

CHAT supports multi-level language specification for multilingual corpora and code-switching analysis:

1. **File level**: `@Languages` header declares all languages used in the file
2. **Tier level**: `[- lang]` directive switches utterance language
3. **Word level**: `@s` and `@s:lang` markers for code-switched individual words

This document explains how these layers work together and how our data model represents them.

---

## 1. File-Level Languages: @Languages Header

### Purpose

Declares the **primary**, **secondary**, and optionally **tertiary** languages used throughout the file.

### Syntax

```text
@Languages:   eng, spa, fra
              ^^^  ^^^  ^^^
              1°   2°   3°
```

- **Primary language** (1st position): Default for all tiers and words
- **Secondary language** (2nd position): Referenced by bare `@s` shortcut
- **Tertiary+ languages** (3rd+ positions): Must use explicit `@s:lang` markers

### Data Model

**Header enum**:
```rust,ignore
pub enum Header {
    Languages { codes: LanguageCodes },
    // ...
}

pub struct LanguageCodes(pub Vec<LanguageCode>);
```

**Extraction** (from ChatFile):
```rust,ignore
let declared_languages: Vec<LanguageCode> = chat_file
    .headers()
    .find_map(|h| {
        if let Header::Languages { codes } = h {
            Some(codes.0.clone())  // LanguageCodes tuple struct
        } else {
            None
        }
    })
    .unwrap_or_else(|| vec![primary_lang.clone()]);
```

### Example

```chat
@Languages:   eng, spa
@Participants: CHI Child, MOT Mother
@ID: eng|corpus|CHI|...
@ID: eng|corpus|MOT|...

*CHI: I want biberon@s please .
      ^^^^^^^^^^^^^^^^^^^^^^^^^^^
      Primary language (eng)
              ^^^^^^^
              Secondary language (spa), marked with @s
```

---

## 2. Tier-Level Language: `[- lang]` Directive

### Purpose

**Switches the language for an entire utterance**. Used when a speaker produces a full utterance in a different language.

### Syntax

```text
*MOT: hola cómo estás [- spa] ?
      ^^^^^^^^^^^^^^^
      Entire utterance is Spanish
```

The directive applies to **the entire tier** (all words in the utterance).

### Data Model

**MainTierContent struct**:
```rust,ignore
pub struct MainTierContent {
    pub content: Vec<UtteranceContent>,
    pub terminator: Option<Terminator>,
    pub language_code: Option<LanguageCode>,  // ← [- lang] directive
    // ...
}
```

- `language_code: None` → Use primary language from @Languages
- `language_code: Some("spa")` → This utterance is Spanish

### Morphosyntax Processing

When Rust builds batch payloads for Stanza, it includes the tier language:

```rust,ignore
let utterance_lang = utt.main.content.language_code
    .clone()
    .unwrap_or_else(|| primary_lang.clone());

// Include in batch payload
MorphosyntaxBatchItem {
    words: vec!["hola", "cómo", "estás"],
    lang: utterance_lang,  // "spa"
    // ...
}
```

Python then routes the entire utterance to the Spanish Stanza model.

### Skipping vs. Processing

**`skipmultilang` flag**:
- `true`: Skip utterances with `[- lang]` directive (only process primary language)
- `false` (default): Process utterances in their declared language

```rust,ignore
let skip = skipmultilang
    && utt.main.content.language_code.is_some()
    && utt.main.content.language_code.as_ref() != Some(&primary_lang);

if !skip {
    // Process with Stanza model for utterance_lang
}
```

---

## 3. Word-Level Language: @s Markers

### Purpose

Marks **individual code-switched words** within an utterance.

### Syntax

| Marker | Meaning | Example |
|--------|---------|---------|
| `@s` | Shortcut: secondary language from @Languages | `biberon@s` → Spanish |
| `@s:spa` | Explicit single language | `biberon@s:spa` → Spanish |
| `@s:eng+fra` | Multiple languages (word legal in both) | `cafe@s:eng+fra` |
| `@s:eng&spa` | Ambiguous (could be either language) | `no@s:eng&spa` |

### Data Model: CHAT Syntax Layer

**Word struct**:
```rust,ignore
pub struct Word {
    // ...
    pub lang: Option<WordLanguageMarker>,
}

pub enum WordLanguageMarker {
    /// Bare @s (shortcut to secondary language)
    Shortcut,
    /// Single explicit language @s:spa
    Explicit(WordLanguage),
    /// Multiple languages @s:eng+fra (legal in all)
    Multiple(Vec<WordLanguage>),
    /// Ambiguous languages @s:eng&spa (unclear which)
    Ambiguous(Vec<WordLanguage>),
}

pub struct WordLanguage {
    pub code: Option<LanguageCode>,  // ISO 639-3 code
    pub variant: Option<String>,     // Optional variant like :spa%mex
}
```

This is the **CHAT syntax representation** - it preserves the exact marker as written in the file.

### Data Model: Semantic Resolution Layer

**LanguageResolution enum** (from `resolve_word_language()`):
```rust,ignore
pub enum LanguageResolution {
    /// Single definite language (after resolving @s shortcut)
    Single(LanguageCode),
    /// Multiple languages (code-mixing): @s:eng+fra
    Multiple(Vec<LanguageCode>),
    /// Ambiguous between languages: @s:eng&spa
    Ambiguous(Vec<LanguageCode>),
    /// No language could be resolved (error)
    Unresolved,
}
```

This is the **semantic representation** - it resolves shortcuts and produces actual language codes.

### Resolution Algorithm

**`resolve_word_language()` function**:

```rust,ignore
pub fn resolve_word_language(
    word: &Word,
    tier_language: Option<&LanguageCode>,
    declared_languages: &[LanguageCode],
) -> (LanguageResolution, Vec<ParseError>)
```

**Resolution rules**:

1. **@s shortcut** → Secondary language from @Languages
   - Example: `@Languages: eng, spa` → `@s` resolves to `spa`
   - Error if used in tertiary language tier (E244)

2. **@s:spa** → Single(spa)

3. **@s:eng+fra** → `Multiple([eng, fra])`

4. **@s:eng&spa** → `Ambiguous([eng, spa])`

5. **No marker** → Use tier language (or primary if no tier directive)

### Example Resolution

**Input CHAT**:
```text
@Languages:   eng, spa, fra

*CHI: I want biberon@s and croissant@s:fra please .
```

**Resolution**:
- `"I"` → No marker → Single(eng) (tier default)
- `"want"` → No marker → Single(eng)
- `"biberon@s"` → Shortcut → Single(spa) (secondary language)
- `"and"` → No marker → Single(eng)
- `"croissant@s:fra"` → Explicit → Single(fra)
- `"please"` → No marker → Single(eng)

---

## 4. Batch Payload: Rust → Python

### Current Implementation (Semantic Resolution)

When building morphosyntax batch payloads, Rust:
1. Resolves each word's language using `resolve_word_language()`
2. Sends **semantic resolution**, not CHAT syntax

**Batch payload struct**:
```rust,ignore
#[derive(serde::Serialize, serde::Deserialize)]
struct MorphosyntaxBatchItem {
    words: Vec<String>,                          // Word texts
    terminator: String,                          // ".", "?", "!"
    special_forms: Vec<(
        Option<FormType>,                        // @c, @s, @b markers
        Option<LanguageResolution>,              // Resolved language
    )>,
    lang: LanguageCode,                          // Tier-level language
}
```

**Example JSON payload**:
```json
{
  "words": ["I", "want", "biberon", "please"],
  "special_forms": [
    [null, null],                              // "I" - no marker, primary lang
    [null, null],                              // "want"
    ["S", {"Single": "spa"}],                  // "biberon@s" → resolved to Spanish
    [null, null]                               // "please"
  ],
  "lang": "eng"                                // Utterance language
}
```

**What Python receives** (semantics, not syntax):
- `"spa"` - resolved language code, not `Shortcut` enum variant
- `{"Multiple": ["eng", "fra"]}` - multiple languages, not `+` syntax
- `{"Ambiguous": ["eng", "spa"]}` - ambiguous, not `&` syntax

### Why Semantic Resolution?

**Problem with sending syntax**:
Python would receive `Shortcut` and need to:
1. Know the @Languages header order
2. Know the current tier language
3. Implement the same resolution logic as Rust

**Solution with semantic resolution**:
Rust handles all resolution complexity once; Python receives clean language codes ready for routing.

---

## 5. Current Morphosyntax Behavior

### Per-Utterance Language Routing ✅ IMPLEMENTED

**Mechanism**: `[- lang]` directive
**Routing**: Group utterances by `lang` field, send entire batches to appropriate Stanza model

```python
# Python batch callback groups by language
by_lang = defaultdict(list)
for item in batch:
    by_lang[item["lang"]].append(item)

# Route to language-specific models
for lang_code, items in by_lang.items():
    if lang_code == "spa":
        results = stanza_es.process(items)
    elif lang_code == "fra":
        results = stanza_fr.process(items)
    # ...
```

**Status**: Fully working since 2026-02-15.

### Per-Word Language Routing ✅ IMPLEMENTED WITH FALLBACK

**Current behavior**:

1. The primary morphosyntax pass still emits `L2|xxx` as the safe intermediate
   placeholder for language-marked words.
2. `talkbank-transform` then extracts deferred `@s` positions, plans
   contiguous spans plus host-side attachment, and asks `batchalign` only for
   the secondary Stanza worker dispatch.
3. Successful secondary results are merged back into `%mor`/`%gra`; only
   unresolved, ambiguous, unsupported, or explicitly opted-out cases remain
   `L2|xxx`.

**Why keep `L2|xxx` as an intermediate / fallback?**
- the primary model still cannot be trusted for foreign-word morphology
- `L2|xxx` is the honest fallback when no single trustworthy secondary route exists
- splice/lowering needs a safe placeholder before secondary dispatch completes

**What's still limited**:
- `@s:eng+spa` / `@s:eng&spa` do not dispatch because there is no single target
- unsupported secondary languages remain `L2|xxx` (see "Unsupported
  non-primary languages" below for the handling contract)

**Unsupported non-primary languages**:

`morphotag` only requires the **primary** `@Languages` code to be
Stanza-supported; files whose primary is unsupported are skipped with a
typed diagnostic before the pipeline runs. When the primary IS
supported, unsupported non-primary content is processed cleanly with an
`L2|xxx` fallback rather than crashing the worker:

- `[- UNSUPPORTEDLANG]` whole-utterance precodes — the utterance is
  grouped under `UNSUPPORTEDLANG`, the worker partitions that group out
  of the dispatch list (`partition_groups_by_stanza_support`), and
  every word receives `L2|xxx`.
- `@s:UNSUPPORTEDLANG` per-word markers — the secondary L2 dispatch
  span is short-circuited the same way; the host primary analysis is
  preserved and the marker's slot stays `L2|xxx`.

Other utterances and spans in the same file that target supported
languages continue to receive real morphology.

**Validation / repair policy around that behavior**:
- explicit `@s:LANG` still resolves and dispatches even when `LANG` is absent
  from `@Languages`, but validation emits warn-only E254 so the header drift is
  visible
- whole-utterance same-language all-`@s` runs now raise E255 and must be
  normalized to `[- lang]` rather than treated as acceptable shorthand
- `chatter debug fix-s` is the repair path for both cases: it rewrites
  the qualifying whole-utterance pattern, clears bare `@s` shortcuts on
  fillers and nonwords as well as on regular words (so that the new
  `[- LANG]` precode does not flip filler resolution), and appends
  missing explicit languages to `@Languages`. The predicate only fires
  when every word-bearing item — including fillers, nonwords, and
  retraced material — carries an explicit language attribution
  resolving to the same target.

Current boundary:
- per-word routing for resolvable `@s` words is implemented
- conservative fallback remains for the unresolved / unsupported cases

---

## 6. Special Cases and Edge Cases

### Tertiary Languages Need Explicit Markers

**Valid**:
```text
@Languages:   eng, spa, fra

*CHI: I want croissant@s:fra .
              ^^^^^^^^^^^^^^^
              Explicit @s:fra marker required
```

**Invalid** (error E244):
```text
@Languages:   eng, spa, fra
*CHI: [- fra] je veux croissant@s .
                          ^^^^^^^
                          @s shortcut not allowed in tertiary tier
```

**Why**: `@s` shortcut only resolves to **secondary** language (2nd position). Tertiary languages must use explicit `@s:lang`.

### Multiple/Ambiguous Languages

**Multiple** (`+`): Word is valid in ALL listed languages
```text
*CHI: I want cafe@s:eng+fra .
             ^^^^
             English "café" or French "café" - both valid
```

**Ambiguous** (`&`): Unclear which language the word belongs to
```text
*CHI: no@s:eng&spa quiero .
      ^^
      English "no" or Spanish "no"? Ambiguous.
```

**Validation**: Both forms require the word to be valid in ALL listed languages.

### No @Languages Header

If `@Languages` is missing:
- Inferred from `@ID` headers (first 3-letter language code)
- Falls back to primary language passed to processing function

---

## 7. Validation and Errors

### Language Resolution Errors

| Error | Trigger | Example |
|-------|---------|---------|
| **E244** | @s shortcut in tertiary language tier | `[- fra] word@s` when fra is 3rd+ language |
| **E254** | Explicit `@s:LANG` language missing from `@Languages` | `@Languages: eng` with `hola@s:spa` |
| **E255** | Whole-utterance same-language all-`@s` pattern where `[- lang]` should be used | `hola@s como@s estas@s .` |
| **E361** | Invalid language code | `word@s:xyz` (xyz not in ISO 639-3) |
| **Unresolved** | No language context available | Word with @s but no @Languages header |

### Validation Context

```text
let (resolved, errors) = resolve_word_language(
    &word,
    tier_language,
    &declared_languages,
);
```

Errors are collected during resolution and reported via the validation system.

---

## 8. Summary Tables

### Language Scope Levels

| Level | Syntax | Scope | Data Model Field |
|-------|--------|-------|------------------|
| **File** | `@Languages: eng, spa` | All utterances | `Header::Languages { codes }` |
| **Tier** | `[- spa]` | Single utterance | `MainTierContent.language_code` |
| **Word** | `@s:spa` | Single word | `Word.lang` |

### Word Language Markers

| Marker | Meaning | Syntax Enum | Resolved Enum | Example |
|--------|---------|-------------|---------------|---------|
| (none) | Tier default | `None` | `Single(tier_lang)` | `hello` |
| `@s` | Secondary lang | `Shortcut` | `Single(spa)` | `biberon@s` |
| `@s:spa` | Explicit | `Explicit(spa)` | `Single(spa)` | `biberon@s:spa` |
| `@s:eng+fra` | Multiple | `Multiple([eng,fra])` | `Multiple([eng,fra])` | `cafe@s:eng+fra` |
| `@s:eng&spa` | Ambiguous | `Ambiguous([eng,spa])` | `Ambiguous([eng,spa])` | `no@s:eng&spa` |

### Morphosyntax Processing Status

| Feature | Scope | Status | Implementation |
|---------|-------|--------|----------------|
| **Per-utterance routing** | `[- lang]` | ✅ Implemented | Rust batches by tier language, Python routes to Stanza |
| **Per-word routing** | `@s:lang` | ✅ Implemented with fallback | Transform-layer L2 planning + secondary dispatch; unresolved/unsupported cases remain L2\|xxx |

---

## 9. Code References

### Rust (talkbank-model)

- **Language syntax markers**: `crates/talkbank-model/src/model/content/word/language.rs` — `WordLanguageMarker` enum (Shortcut, Explicit, Multiple, Ambiguous)
- **Resolution logic**: `crates/talkbank-model/src/validation/word/language/resolve.rs` — `resolve_word_language()` function
- **Header language list**: `crates/talkbank-model/src/model/header/header_enum/header.rs` — `Header::Languages { codes }`
- **Tier language directive**: `crates/talkbank-model/src/model/content/main_tier.rs` — `MainTierContent.language_code` field

### Rust (talkbank-transform - payload & injection)

- **Batch payload definition**: `crates/talkbank-transform/src/morphosyntax/payload.rs:29-55` — `MorphosyntaxBatchItem` struct (words, terminator, special_forms, lang)
- **L2|xxx placeholder for unresolved words**: `crates/talkbank-transform/src/morphosyntax/injection.rs` — replaces pos with `PosCategory::new("L2")` when word has language marker but isn't routed
- **L2 splice logic**: `crates/talkbank-transform/src/morphosyntax/l2/splice.rs` — replaces L2|xxx with real morphology after secondary dispatch

### Rust (batchalign - orchestration)

- **Batch collection & language grouping**: `crates/batchalign/src/morphosyntax/batch.rs:40-250` — `run_morphosyntax_batch_impl()` groups utterances by language, dispatches per-language workers

### Python (stateless inference only)

- **Inference function**: `python/batchalign/inference/morphosyntax.py:batch_infer_morphosyntax()` — calls Stanza `nlp()` per language, returns raw UD output
- **Per-utterance language routing**: Python worker receives `lang` field, groups batch items by language, routes to appropriate Stanza model

---

## 10. Future Work (Low Priority)

The remaining low-priority work is no longer "per-word routing exists or not";
it is policy/refinement work on top of the implemented L2 dispatch path:

1. warning/reporting surfaces for unresolved `@s:eng+spa` / `@s:eng&spa`
2. policy-sensitive normalization around all-`@s` utterances and headers
3. broader secondary-language quality work for weak or unsupported models

---

## References

- CHAT Manual: https://talkbank.org/0info/manuals/CHAT.html
- ISO 639-3 Language Codes: https://iso639-3.sil.org/
- Related docs: `per-word-language-divergence.md`, `per-word-language-switching-ideal-approach.md`
