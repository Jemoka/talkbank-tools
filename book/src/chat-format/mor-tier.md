# The %mor Tier: Morphological Analysis

**Status:** Reference
**Last updated:** 2026-05-11 20:35 EDT

The `%mor` (morphological) dependent tier provides word-by-word morphosyntactic annotation aligned with the main tier. Each main-tier word receives a morphological code specifying part of speech, lemma, and grammatical features.

## Format Overview

```text
*CHI:	I want cookies .
%mor:	pron|I-Prs-Nom-S1 verb|want-Fin-Ind-Pres-S1 noun|cookie-Plur .
```

Each `%mor` item has the structure **`POS|lemma[-Feature]*`**, where:

- **POS** — part-of-speech category (`noun`, `verb`, `pron`, `det`, `aux`, etc.)
- **`|`** — pipe separator (always present)
- **Lemma** — base form of the word (`cookie`, `be`, `I`). May contain language-specific compound or derivational boundary markers (see [Compound Lemma Boundaries](#compound-lemma-boundaries) below)
- **Features** — zero or more morphological features, each preceded by `-` (`-Plur`, `-Fin-Ind-Pres-S3`)

Items are space-separated and terminate with a punctuation marker (`.`, `?`, `!`, etc.).

## The UD MOR Format

TalkBank's `%mor` tier uses a format inspired by [Universal Dependencies (UD)](https://universaldependencies.org/) but adapted to CHAT conventions. We call this the **UD MOR format** to distinguish it from the older CLAN-era MOR format.

The UD MOR format was introduced via batchalign's Stanza-based morphosyntax pipeline. Stanza produces standard UD analysis (UPOS, lemma, morphological features, dependency relations), and the Rust mapping layer converts this to CHAT `%mor` and `%gra` tiers. The new format has been adopted for all new corpus annotation.

### Structure: Flat POS|lemma\[-Feature]*

Every morphological word is flat — a single POS tag, a single lemma, and a linear chain of features:

```text
POS|lemma[-Feature1][-Feature2][-Feature3]...
```

There are no compounds, prefixes, subcategories, or nested structures in the UD MOR format. The entire morphological analysis of a word is captured by the POS+lemma+features triple.

**Examples:**

| Word | %mor code | POS | Lemma | Features |
|------|-----------|-----|-------|----------|
| dog | `noun\|dog` | noun | dog | (none) |
| dogs | `noun\|dog-Plur` | noun | dog | Plur |
| running | `verb\|run-Part-Pres-S` | verb | run | Part, Pres, S |
| is | `aux\|be-Fin-Ind-Pres-S3` | aux | be | Fin, Ind, Pres, S3 |
| I | `pron\|I-Prs-Nom-S1` | pron | I | Prs, Nom, S1 |
| the | `det\|the-Def-Art` | det | the | Def, Art |

### Multi-Word Tokens (Clitics)

English contractions and similar multi-word tokens (MWTs) are represented using the **tilde (`~`) separator** for post-clitics:

```text
*CHI:	it's red .
%mor:	pron|it~aux|be-Fin-Ind-Pres-S3 adj|red .
```

Here `it's` is a single main-tier word that expands to two morphological words: `pron|it` (main) and `aux|be-Fin-Ind-Pres-S3` (post-clitic). The `~` indicates the two MOR words are fused into one orthographic token.

Each clitic counts as its own **chunk** for `%gra` alignment — `pron|it~aux|be-Fin-Ind-Pres-S3` produces 2 chunks, each needing its own grammatical relation.

### Terminator

The `%mor` tier ends with a terminator that matches the main tier's utterance terminator:

```text
*CHI:	what is that ?
%mor:	pron|what aux|be-Fin-Ind-Pres-S3 det|that ?
```

The terminator (`.`, `?`, `!`, `+...`, etc.) counts as one chunk for `%gra` alignment.

## How It Diverges from UD

The UD MOR format is **UD-inspired but not UD-compliant**. Several deliberate adaptations make it fit CHAT conventions while preserving most UD information. This section catalogs every divergence.

### 1. POS Tags Are Lowercased UPOS

UD uses uppercase UPOS tags (`NOUN`, `VERB`, `PRON`). CHAT uses lowercase (`noun`, `verb`, `pron`). This is a lossless, trivially reversible surface change.

| UD UPOS | CHAT POS |
|---------|----------|
| NOUN | `noun` |
| VERB | `verb` |
| AUX | `aux` |
| PRON | `pron` |
| DET | `det` |
| ADJ | `adj` |
| ADV | `adv` |
| ADP | `adp` |
| PROPN | `propn` |
| INTJ | `intj` |
| CCONJ | `cconj` |
| SCONJ | `sconj` |
| NUM | `num` |
| PART | `part` |
| X | `x` |

### 2. Feature Values Are Flat, Not Key=Value (Currently)

UD represents morphological features as key=value pairs: `Number=Plur`, `Tense=Past`, `Person=3`. The current CHAT convention drops the keys and uses only the values: `-Plur`, `-Past`, `-S3`.

This is the most significant divergence from UD, because:

- **Information loss**: `Plur` could in principle be `Number=Plur` or `Degree=Plur` (though in practice the UD feature value set has no real ambiguities).
- **Collapsed person/number**: UD `Person=3|Number=Sing` becomes `-S3` — a combined code that cannot be mechanically decomposed back to its UD components.
- **Feature ordering**: Features appear in a conventional order determined by the generation pipeline, not in UD's alphabetical order.

**The data model now supports key=value features.** The `MorFeature` type has an optional `key` field — when present, the feature serializes as `Key=Value` (e.g., `-Number=Plur`); when absent, it serializes as just the value (e.g., `-Plur`). This is forward-compatible: existing flat features parse and serialize identically, and if batchalign's mapper begins emitting `Key=Value` features, they flow through the parser and model without any format changes.

### 3. Multi-Value Features: Commas Preserved

UD encodes multi-value features with commas: `PronType=Int,Rel` (the word is *both* interrogative and relative). In CHAT `%mor`, the comma is **preserved within the feature value**:

```text
-Int,Rel
```

This is treated as a single feature value `"Int,Rel"`. The grammar accepts commas within feature values, and the model stores them as-is. No decomposition occurs — the model faithfully records the string that appears in the `%mor` tier.

> **Historical note**: Earlier documentation described a "comma-stripping" convention where `PronType=Int,Rel` became `-IntRel` (concatenated without separator). The current grammar and parser preserve the comma. Existing corpus data using the concatenated form (`-IntRel`) also parses correctly — it's simply treated as the flat value `"IntRel"`.

### 4. Dependency Relations Are Uppercase with Dash Subtypes

The `%gra` tier (not `%mor`, but closely related) uses uppercase relation names with dashes for subtypes, where UD uses lowercase with colons:

| UD | CHAT %gra |
|----|-----------|
| `nsubj` | `NSUBJ` |
| `acl:relcl` | `ACL-RELCL` |
| `obl:tmod` | `OBL-TMOD` |

This is lossless — case and separator are trivially reversible.

### 5. ROOT Head Convention

In UD, the root word has `head=0`. In `%gra`, two conventions coexist:

- **UD convention**: `head=0` (e.g., `3|0|ROOT`) — the standard we now emit
- **Legacy TalkBank convention**: `head=self` (e.g., `3|3|ROOT`) — found in older corpus data

The parser and validator accept both forms. New output uses `head=0`.

### 6. No XPOS, No DEPREL Subtypes in %mor

UD provides both UPOS (universal POS) and XPOS (language-specific POS). CHAT `%mor` uses only UPOS-equivalent tags — there is no XPOS field. Language-specific POS distinctions are not represented.

Similarly, UD's fine-grained dependency relation subtypes (e.g., `nsubj:pass`) appear in `%gra` as `NSUBJ-PASS`, but the `%mor` tier itself contains no dependency information.

### 7. No Morpheme Segmentation

Traditional CHAT MOR formats (CLAN-era) supported morpheme-level segmentation with compound markers (`+`), prefix markers (`#`), and suffix chains (`-SUFFIX&type`). The UD MOR format does not use any of these — each word is analyzed as a flat POS+lemma+features triple.

The grammar still *accepts* some of these legacy markers for backward compatibility with older corpus data, but the canonical UD MOR format does not produce them.

## Compound Lemma Boundaries

Several UD treebanks use special characters *inside* lemmas to mark morphological boundaries. These are meaningful linguistic annotations preserved in the CHAT `%mor` lemma field when possible.

### Known Markers Across Languages

| Language | Marker | Meaning | Example Lemma | In %mor |
|----------|--------|---------|---------------|---------|
| **Estonian** | `=` | Compound boundary | `maja=uks` (house-door) | `noun\|maja=uks` — **preserved** |
| **Basque** | `!` | Derivational boundary | `partxi!se` (share + derivation) | `noun\|partxi!se-Ine` — **preserved** |
| **Finnish** | `#` | Compound boundary | `jää#kaappi` (ice-cabinet) | `noun\|jää_kaappi` — **mangled** (`#` → `_`) |

`=` and `!` pass through the cleaning pipeline because they are not reserved CHAT `%mor` syntax characters. `#` is reserved in traditional CHAT MOR for prefix markers (e.g., `v|#un#do`), so the sanitizer replaces it with `_`.

> **Gotcha: `=` ambiguity with legacy CLAN translation glosses.**
> Legacy CLAN `%mor` tiers use `=` for translation glosses (e.g., `n|perro=dog`), a convention
> predating UD adoption. The parser treats `=` identically in both cases — it is preserved as
> part of the lemma string. This means legacy `n|perro=dog` parses successfully but the
> translation semantics are lost: the model stores `perro=dog` as a single lemma, indistinguishable
> from an Estonian compound like `maja=uks`. Since we cannot reliably disambiguate the two uses
> without language-specific context, legacy translation glosses are silently absorbed into the
> lemma. Files with legacy `=translation` syntax still parse and round-trip correctly, but the
> translation information is not semantically accessible. This affects corpora that predate our
> UD MOR adoption and lack Stanza coverage for their language.

### Multi-Word Expression Lemmas (Stanza `_` Convention)

Stanza uses underscores in lemmas to represent multi-word expressions across many languages: `New_York`, `parce_que` (French), `pick_up` (English), `a_causa_di` (Italian). The current cleaning pipeline strips underscores entirely (`New_York` → `NewYork`), which is a known data quality issue and should be treated as an open data-quality limitation of the current mapper.

### Multi-Value Features (Commas in Feature Values)

UD encodes multi-value features with commas: `PronType=Int,Rel` means a word is *both* interrogative and relative. These commas appear in the CHAT `%mor` feature suffix and are **preserved as-is**:

```text
pron|wat-Int,Rel
```

This is sometimes mistaken for a compound lemma marker, but commas in UD always appear in the **feature column** (CONLLU column 6), never in the **lemma column** (CONLLU column 3). In CHAT `%mor`, they appear after the `-` feature separator, not inside the lemma. The grammar, both parsers, and the data model all accept commas in feature values. See [Section 3: Multi-Value Features](#3-multi-value-features-commas-preserved) above.

### Future Direction

The current handling of compound lemma boundaries is inconsistent across languages. A possible future improvement is a unified Unicode separator character that would normalize all compound/derivational boundary markers (`=`, `!`, `#`, and potentially `_`) into a single convention. This has not been implemented as of 2026-03-02 and requires a design decision on which character to use and whether to preserve the original markers in a structured field.

## Data Model

The Rust data model in `talkbank-model` represents `%mor` tiers with these types:

### MorTier

The top-level tier container:

```rust,ignore
pub struct MorTier {
    pub tier_type: MorTierType,    // MorTierType::Mor
    pub(crate) items: MorItems,    // Vec<Mor> wrapper; accessed via accessor methods
    pub terminator: Terminator,    // typed terminator (.`, `?`, `!`, `+...`, etc.)
    pub span: Span,                // source location
}
```

### Mor (Item)

One item aligned with one main-tier word:

```rust,ignore
pub struct Mor {
    pub main: MorWord,                        // required main word
    pub post_clitics: SmallVec<[MorWord; 2]>, // optional ~clitics
}
```

### MorWord

A single morphological word (POS + lemma + features):

```rust,ignore
pub struct MorWord {
    pub pos: PosCategory,                    // e.g., "noun"
    pub lemma: MorStem,                      // e.g., "dog"
    pub features: SmallVec<[MorFeature; 4]>, // e.g., [Plur]
}
```

### MorFeature

A morphological feature with optional key:

```rust,ignore
pub struct MorFeature {
    key: Option<Arc<str>>,  // e.g., Some("Number") or None
    value: Arc<str>,        // e.g., "Plur"
}
```

Construction examples:

```rust,ignore
// Flat feature (current convention)
MorFeature::new("Plur")         // key=None, value="Plur"
MorFeature::new("S3")           // key=None, value="S3"
MorFeature::new("Int,Rel")      // key=None, value="Int,Rel"

// Keyed feature (UD-standard, forward-compatible)
MorFeature::new("Number=Plur")  // key=Some("Number"), value="Plur"
MorFeature::new("Tense=Past")   // key=Some("Tense"), value="Past"

// Explicit constructors
MorFeature::flat("Plur")
MorFeature::with_key_value("Number", "Plur")
```

**Lossless roundtrip guarantee**: `MorFeature::new` auto-detects the `=` delimiter. Features without `=` are flat; features with `=` split into key+value. Serialization reproduces the original format exactly — flat features stay flat, keyed features keep their key.

### PosCategory and MorStem

Both are interned `Arc<str>` newtypes for memory efficiency:

```rust,ignore
pub struct PosCategory(pub Arc<str>);  // interned via pos_interner()
pub struct MorStem(pub Arc<str>);      // interned via stem_interner()
```

Common values (`noun`, `verb`, `the`, `a`, `be`, etc.) are pre-populated in the interner. Cloning is O(1) — atomic reference count increment.

### Memory Layout

The model uses `SmallVec` for inline storage of common cases:

- `Mor.post_clitics: SmallVec<[MorWord; 2]>` — most words have 0-1 clitics
- `MorWord.features: SmallVec<[MorFeature; 4]>` — most words have 0-4 features
- `MorFeature` key and value are `Arc<str>` — interned for deduplication

For a typical 30-word utterance with `%mor`, the model allocates approximately 30 `Mor` items, each with 1 `MorWord` and 0-4 `MorFeature` values. The interning system ensures that repeated POS tags, stems, and feature values share a single allocation across the entire file.

## Grammar

The tree-sitter grammar for `%mor` is defined in `grammar.js`. The relevant rules:

```text
mor_content → mor_word (mor_post_clitic)*
mor_post_clitic → tilde mor_word
mor_word → mor_pos pipe mor_lemma (mor_feature)*
mor_feature → hyphen mor_feature_value
mor_feature_value → /[^\.\?\|\+~\-\s\r\n]+/
```

Key design decisions:

- **`mor_feature_value` accepts `=` and `!`**: The regex `[^\.\?\|\+~\-\s\r\n]+` matches any characters except the MOR structural delimiters. This means `Number=Plur` parses as a single `mor_feature_value` node. The split on `=` happens in the model layer, not the grammar — following the "parse, don't validate" principle.
- **`mor_feature_value` accepts `,`**: Multi-value features like `Int,Rel` parse as a single node.
- **No compound/prefix rules**: The grammar has no rules for `+` (compounds) or `#` (prefixes) in the UD MOR format. These are legacy CHAT MOR features not used in UD-style output.

## Parser

The tree-sitter parser produces `MorTier` from CHAT text. It is GLR-based and error-recovering, producing a CST that the Rust `talkbank-parser` crate walks to construct `MorTier`. Used by the CLI, LSP, and batchalign. High-frequency values (`PosCategory`, `MorStem`) are interned via `Arc<str>` during construction.

The `resources/corpus/reference/` set is the correctness gate for `%mor` parsing —
every file must parse and round-trip cleanly. The file count grows as
new constructs are added; run `find resources/corpus/reference -name '*.cha' | wc -l`
to get the live total.

## Validation

The `%mor` tier undergoes several validation checks:

### Content Validation (E711)

Every `MorWord` is checked for:
- **Empty POS**: `|lemma` with no POS before the pipe
- **Empty lemma**: `pos|` with no lemma after the pipe
- **Empty feature**: bare `-` separator with no feature text

### Main-tier Alignment (E705 / E706)

The `%mor` tier must align 1-to-1 with the main tier's alignable
words (excluding pauses, events, and other non-word content). The
number of `Mor` items must equal the number of alignable main-tier
words. The validator emits **E705 `MorCountMismatchTooFew`** when
`%mor` has fewer items than the main tier and **E706
`MorCountMismatchTooMany`** when it has more. Terminator-mismatch
errors are emitted separately as E707 (presence) and E716 (value).

### GRA Alignment (E720)

When both `%mor` and `%gra` tiers are present, the number of `%gra`
relations must equal the number of `%mor` **chunks** (including
clitics and the terminator). A mismatch emits **E720
`MorGraCountMismatch`**. This is computed via `MorTier::count_chunks()`.

(`%gra`'s own internal validators — E708 malformed relation, E709
invalid index, E712 word-index out of range, E713 head-index out of
range, E721 non-sequential index, E722 no ROOT, E723 multiple
ROOTs, E724 circular dependency — are documented in
[Dependent Tiers § %gra](dependent-tiers.md#gra--grammatical-relations).)

## JSON Serialization

The `MorTier` serializes to JSON using serde. `MorFeature` serializes as a plain string (`"Plur"` or `"Number=Plur"`), so the JSON schema is simply `"type": "string"`. Example:

```json
{
  "tier_type": "Mor",
  "items": [
    {
      "main": {
        "pos": "pron",
        "lemma": "I",
        "features": ["Prs", "Nom", "S1"]
      }
    },
    {
      "main": {
        "pos": "verb",
        "lemma": "want",
        "features": ["Fin", "Ind", "Pres", "S1"]
      }
    },
    {
      "main": {
        "pos": "noun",
        "lemma": "cookie",
        "features": ["Plur"]
      }
    }
  ],
  "terminator": "."
}
```

When key=value features are present, they serialize with the key included:

```json
"features": ["Number=Plur", "Tense=Past"]
```

The JSON schema for `MorFeature` is `"type": "string"` regardless of whether keys are present.

## Migration from Traditional CHAT MOR

### What Changed

The traditional CHAT MOR format (CLAN-era) used a complex, hierarchically structured notation:

```text
%mor:	pro:sub|I v|want n|cookie-PL .
```

Key differences from the UD MOR format:

| Aspect | Traditional CHAT MOR | UD MOR |
|--------|---------------------|--------|
| **POS tags** | CLAN categories (`pro:sub`, `v`, `n`, `adj`, `adv`) | Lowercased UPOS (`pron`, `verb`, `noun`, `adj`, `adv`) |
| **POS subtypes** | Colon-separated (`pro:sub`, `det:art`, `v:aux`) | Flat (subtypes dropped or encoded differently) |
| **Features** | CLAN suffix system (`-PL`, `-PAST`, `-3S`, `-PRES`) | UD feature values (`-Plur`, `-Past`, `-S3`, `-Pres`) |
| **Compounds** | `+` separator (`n|+n\|black+n\|bird`) | Not used (lemma contains the full form) |
| **Prefixes** | `#` separator (`v|#un#do`) | Not used |
| **Morpheme segmentation** | Full segmentation (`v\|eat&PAST`) | Not used (features are abstract, not morphemic) |
| **Translations** | `=` separator (`n\|perro=dog`) | Not present in base format (separate mechanism) |

### What the Model Removed

The UD MOR redesign (2026) removed the following types from the data model:

- `MorSuffix` — suffix with type discriminant (`fusional`, `derivational`, etc.)
- `MorCompound` — compound word with `+` separator
- `MorPrefix` — prefix with `#` separator
- `MorSubcategory` — POS subcategory after colon
- `AnnotatedChunk` — chunk with optional translation
- `Chunk` — enum of word/compound/terminator

These were replaced by the flat `MorWord { pos, lemma, features }` structure. The model went from ~12 types to 4 (`MorTier`, `Mor`, `MorWord`, `MorFeature`).

### Backward Compatibility

The grammar still accepts many traditional CHAT MOR constructs (colons in POS tags, etc.) because the reference corpus contains files in both formats. The parser produces the same flat `MorWord` regardless — legacy constructs are mapped to the simplified structure during parsing.

### What Stays the Same

Despite the format changes, fundamental CHAT conventions remain:

- Pipe (`|`) separates POS from lemma
- Hyphen (`-`) introduces features
- Tilde (`~`) marks post-clitics
- Space separates items
- Terminator ends the tier
- 1-to-1 alignment with main tier words

## Toward Full UD Compatibility

The current format is UD-*inspired* but not UD-*compliant*. Here is a roadmap of what would be needed for full lossless UD round-tripping:

### Already Supported

- POS tags (UPOS equivalents)
- Lemmas
- Feature values (flat and key=value)
- MWT expansions (clitics)
- Dependency relations (via `%gra`)

### Gaps Remaining

1. **Feature keys**: The model supports `Key=Value` features, but batchalign's mapper currently emits flat values only. When the mapper switches to emitting `Number=Plur` instead of just `Plur`, the parser, model, and serializer handle it automatically with no code changes.

2. **Person+Number composites**: UD has separate `Person=3` and `Number=Sing` features. CHAT combines them into `-S3` (3rd person singular). Decomposing `S3` back to `Person=3|Number=Sing` would require a lookup table or a convention change.

3. **Multi-value feature delimiter**: UD uses commas (`PronType=Int,Rel`). CHAT preserves these commas in the feature value, but the semantic structure (two separate values) is not explicitly modeled. The model treats `Int,Rel` as an opaque string.

4. **XPOS**: UD provides language-specific POS tags (XPOS) alongside universal tags (UPOS). CHAT `%mor` has no XPOS field. This information is simply not represented.

5. **Morpheme-level analysis**: UD's `MISC` field can encode morpheme boundaries and glosses. CHAT's UD MOR format does not attempt morpheme segmentation — features are abstract grammatical categories, not morphemic decompositions.

### The Path Forward

The model is designed so that moving toward UD compliance requires **no breaking changes**:

- `MorFeature` already supports `Key=Value` — just needs the mapper to emit keys
- `PosCategory` is an opaque string — could hold XPOS in a separate field if needed
- JSON schema uses `"type": "string"` for features — adding keys doesn't break consumers
- The grammar already accepts `=` in feature values — no grammar changes needed

The migration can happen incrementally: the mapper starts emitting key=value features, existing flat data continues to parse identically, and corpus files can be upgraded at their own pace.
