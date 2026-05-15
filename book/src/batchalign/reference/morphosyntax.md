# Morphosyntax Pipeline

**Status:** Current
**Last updated:** 2026-05-07 18:30 EDT

## 1. Overview

The batchalign morphosyntax pipeline (`morphotag` command) adds %mor and %gra tiers to
CHAT transcripts.  Rust owns CHAT parsing, word extraction, UD-to-CHAT mapping, AST
injection, and serialization.  Python's only role is ML inference — calling Stanza for
POS/lemma/dependency analysis.

### Per-file scoping via `@Options`

`morphotag` reads two CHAT-level directives, both with narrow,
command-specific semantics:

- **`@Options: CA`** — "the morphotag command is not to be run on
  this file." CA transcripts are pass-through: existing %mor /
  %gra is preserved unchanged, no Stanza inference, no provenance
  comment.
- **`@Options: NoAlign`** — scoped to the `align` command, NOT
  morphotag. NoAlign files run morphotag normally. (Pre-2026-05-07
  the pipeline incorrectly conflated NoAlign with a global skip,
  leaving NoAlign files with stale morphotag and no rerun path;
  fixed.)

Full semantics + worked examples: [`@Options` and Per-File Command
Scoping](./chat-options.md).

## 2. Architecture

```mermaid
flowchart TD
    chat["CHAT files"]
    parse["parse_lenient()\n(talkbank-transform::parse)"]
    clear["clear_morphosyntax()\nstrip existing %mor/%gra"]
    collect["collect_payloads()\nper-utterance word lists"]
    batch["execute_v2 morphosyntax\n→ Python Stanza worker<br/>(no-op cache: always infer)"]
    mode{"TokenizationMode?"}
    preserve["map_ud_sentence()\nmerge MWT → clitics\n1 MOR per CHAT word"]
    retok["map_ud_sentence_expanded()\n1 MOR per component word\nfilter Range parents"]
    inject_p["inject_morphosyntax()\nadd %mor/%gra tiers"]
    inject_r["retokenize_utterance()\nrewrite main tier + inject"]
    l2{"L2 @s words\ndeferred?"}
    l2disp["dispatch_secondary_l2()\nplan + dispatch secondary Stanza"]
    splice["splice_l2_into_chat()\nreplace L2|xxx"]
    out["Serialize → CHAT"]

    chat --> parse --> clear --> collect --> batch
    batch --> mode
    mode -->|Preserve| preserve --> inject_p
    mode -->|StanzaRetokenize| retok --> inject_r
    inject_p --> l2
    inject_r --> l2
    l2 -->|yes| l2disp --> splice --> out
    l2 -->|no| out
```

The diagram shows the two injection paths that diverge based on
`TokenizationMode`. The L2 secondary dispatch runs after primary
injection by default; pass `--no-l2-morphotag` to skip it.

**Cache note:** Morphosyntax (a text NLP task) uses a **no-op cache** —
all utterances skip cache lookup and are always sent to Stanza inference.
This is faster than SQLite lookups, as Stanza workers stay warm between
utterances in the cross-file batch. Audio tasks (transcribe, align)
use real caching; text tasks (morphosyntax, utseg, translate) do not.

### Data Flow

Rust: `crates/batchalign/src/morphosyntax/batch.rs` — `run_morphosyntax_batch_impl()`
  │
  ├── Parse CHAT (Rust AST via tree-sitter, parsed once per file)
  │
  ├── clear_morphosyntax()  — strip existing %mor/%gra tiers
  │
  ├── collect_payloads()  — extract utterance word lists globally
  │
  ├── Batch infer (all utterances pool → one Stanza call per language)
  │     ├── Group by language, dispatch concurrently
  │     ├── Python worker (python/batchalign/inference/morphosyntax.py)
  │     │     • Replace special forms with "xbxxx"
  │     │     • nlp(combined_text) → Stanza UD analysis
  │     │     • Return raw UD results as JSON
  │     └── Repartition responses back by file
  │
  ├── map_ud_sentence() or map_ud_sentence_expanded()
  │     → %mor/%gra (UD→CHAT mapping, Rust)
  │
  ├── inject_results()  — AST injection + validation
  │
   ├── dispatch_secondary_l2() (if `@s` words and not `--no-l2-morphotag`)
   │     → transform-layer plan, secondary dispatch, merge, splice
  │
  ├── apply_pos_hints() (if --respect-pos-hints, default on)
  │     → transcriber `$POS` annotations override POS categories
  │
  ├── remove_empty_morphosyntax_placeholders()
  │     → sweep serialize-time empty %mor/%gra slots
  │
  └── Serialize → CHAT (now with %mor/%gra, L2 morphology, POS hints)
```text

### Module Inventory

**Rust** — `talkbank-transform` crate (`crates/talkbank-transform/src/`)

The core morphosyntax pipeline logic lives in `talkbank-transform`. Most files
handle CHAT-side extraction, UD→CHAT mapping, and injection. The Rust server
and PyO3 bridge both depend on this crate for morphosyntax orchestration.

| File | Purpose |
|------|---------|
| `pipeline/parse.rs` | CHAT parsing entry point |
| `extract.rs` | Word extraction from AST for morphosyntax input |
| `inject.rs` | Primary + retokenize injection. Chooses `map_ud_sentence` (Preserve) vs `map_ud_sentence_expanded` (Retokenize) |
| `morphosyntax/l2/` | L2 code-switching: planning, extract, merge, splice @s words via secondary Stanza models |
| `morphosyntax/sentence_mapping.rs` | `map_ud_sentence()`, `map_ud_sentence_expanded()`, shared `build_gra_and_validate()` |
| `morphosyntax/mapping_helpers.rs` | `assemble_mors()` (clitic merge), `is_clitic()`, `map_relation()` |
| `morphosyntax/stanza_raw.rs` | Parse raw Stanza JSON output, supply defaults for Range token annotation fields |
| `morphosyntax/lang_en.rs` | English-specific rules (200+ irregular verbs) |
| `morphosyntax/lang_fr.rs` | French-specific rules (pronoun case, APM) |
| `morphosyntax/lang_ja.rs` | Japanese-specific rules (verb form, 140+ rules) |
| `morphosyntax/lang_it.rs` | Italian-specific rules |

**Rust** — `batchalign` crate (`crates/batchalign/src/chat_ops/nlp/`)

Mapping-related utilities and test harnesses that depend on `talkbank-transform`:

| File | Purpose |
|------|---------|
| `mapping/mod.rs` | Mapping orchestration and UD→CHAT dispatch |
| `mapping/helpers.rs` | Mapping utilities |
| `types.rs` | `UdSentence`, `UdWord`, `UdId` (Single/Range/Decimal), `UdResponse`, `UniversalPos` |

**Python** (stateless ML inference only)

| File | Purpose |
|------|---------|
| `inference/morphosyntax.py` | Calls Stanza `nlp()`, returns raw `to_dict()` output |

Python does no orchestration, caching, or UD→CHAT mapping — all handled by Rust.

## 3. What Batchalign Needs from %mor

Batchalign treats %mor tiers as mostly opaque.  No pipeline decomposes POS, lemma, or
features into structured data for analysis.  The consumers and what they actually access:

| Consumer | What it accesses | Decomposes POS/lemma/features? |
|----------|-----------------|-------------------------------|
| **Cache** (`engine.py`) | Final %mor/%gra strings (BLAKE3 key) | No — stores/retrieves whole strings |
| **Coreference** (`coref`) | Token boundaries in %mor tier | No — counts tokens only |
| **WER evaluation** (`benchmark`) | Token count from %mor for word-level accuracy | No — counts only |
| **Pre-serialization validation** (`validation.py`) | Chunk count alignment (%mor chunks vs %gra relations) | No — calls `count_chunks()` in Rust |
| **CLAN commands** (clan-core crate in talkbank-tools) | Full %mor structure (POS, lemma, suffixes for FREQ/MLU/MLT) | Yes — but via talkbank-model's Mor type |
| **Forced alignment** | No %mor access | N/A |
| **ASR / diarization** | No %mor access | N/A |

**Key finding:** Within batchalign itself, %mor is a cached final string.  The pipeline
generates it (via Stanza + Rust mapping), stores it, and injects it into the AST — but
never reads it back to extract linguistic information.  Downstream consumers that do
decompose %mor (CLAN commands) do so through `talkbank-model`'s typed `Mor` structure, not
through batchalign code.

**Implication for the format:** The flat `POS|lemma[-Feature]*` structure that Stanza
produces is sufficient for batchalign's needs.  Richer UD `key=value` features flow through
the pipeline without code changes — they'd be encoded as CHAT suffixes by `mapping.rs` and
round-tripped by the parser — but no batchalign consumer currently needs them.

## 4. Two MOR Traditions

%mor tiers in CHAT come from two fundamentally different sources, and understanding which
one batchalign produces is key to assessing "information loss."

### CLAN MOR Grammars (Legacy)

Hand-coded per-language grammars, maintained since the 1990s.  They produce rich
morphological structure:

- **Subcategorized POS:** `pro:sub|I`, `n:prop|John`, `v:cop|be`
- **Compounds:** `adj|+adj|big+n|bird` (structured multi-stem words)
- **Prefixes:** `trans#n|port`
- **Morpheme segmentation:** `go&PAST` (fusional) vs `cat-PL` (agglutinative)
- **Language-specific affix inventories** hand-coded per grammar

These grammars are incomplete (not all languages covered), inconsistent across languages,
and require manual maintenance.  They encode a specific morphological theory baked into
each grammar file.

### Stanza UD (What Batchalign Produces)

Automatically trained models producing Universal Dependencies analysis for 70+ languages:

- **Flat UPOS:** `pron|I`, `propn|John`, `aux|be`
- **Lemma + feature list:** `verb|go-Past`, `noun|cat-Plur`
- **MWT clitics:** `pron|I~aux|will`
- **No compounds, no prefixes, no morpheme segmentation**
- **Consistent cross-linguistic feature inventory** (UD standard)
- **Richer dependency structures** (%gra from UD is genuinely better than what CLAN produced)

### What the Model Looks Like

The shared `talkbank-model` `Mor` type:

```rust
struct Mor {
    main: MorWord,
    post_clitics: SmallVec<[MorWord; 2]>,
}

struct MorWord {
    pos: PosCategory,                      // "noun", "verb", "pron", ...
    lemma: MorStem,                        // cleaned stem text
    features: SmallVec<[MorFeature; 4]>,   // flat ordered list
}
```

Three fields per word: POS (string), lemma (string), features (ordered vector of
strings).  This maps cleanly to what Stanza produces — `UPOS` to `pos`, `lemma` to
`lemma`, UD feature values to `features`, MWT components to `post_clitics`.

### What's "Lost"

| Structure | Legacy MOR grammar | Stanza UD | Model representation |
|-----------|-------------------|-----------|---------------------|
| POS subcategories | `pro:sub\|I` | `pron\|I` | POS string — subcategories preserved if present (parser accepts `pro:sub`) |
| Compounds | `adj\|+adj\|big+n\|bird` | Not produced | Parsed by grammar if encountered; no typed compound field |
| Prefixes | `trans#n\|port` | Not produced | Parsed by grammar if encountered; stored in stem |
| Morpheme segmentation | `go&PAST` vs `go-PAST` | Not produced | Both parsed; suffix carries separator character |
| UD feature keys | N/A | `Number=Plur` | `MorFeature` has optional key field — preserved if present |
| xpos (language-specific POS) | N/A | Available in Stanza | Discarded — only UPOS used |

The structures that the model doesn't have typed fields for — compounds, prefixes,
morpheme boundaries — are structures that **Stanza never produces**.  The model was shaped
to match the producer.

### Freedom from CLAN MOR Constraints

This is mostly a good thing:

- **Cross-linguistic consistency.** CLAN MOR grammars varied wildly per language.  UD gives
  the same feature inventory everywhere.
- **No manual grammar maintenance.** Stanza models are trained automatically.  Adding a
  language means training a model, not writing a grammar by hand.
- **Better dependency analysis.** UD %gra is demonstrably more accurate than what CLAN
  produced — the Rust mapper's O(N) cycle detection catches issues that Python's CLAN-era
  code missed in 87.5% of utterances on test data.
- **Feature transparency.** UD features like `Number=Plur` are semantically meaningful
  and machine-readable.  CLAN suffixes like `-PL` required per-grammar documentation.

### The CLAN Caveat

CLAN commands (`FREQ`, `MLU`, `MLT`) access %mor through `talkbank-model`'s `Mor` type.
For counting (MLU) and frequency (FREQ), the flat `pos + lemma + features` structure is
sufficient.  For fine-grained morphological queries on legacy corpus data — "find all
compound nouns", "count prefixed verbs" — you'd currently have to pattern-match on the POS
string (e.g., `n:prop` contains `:`) or lemma, which works but isn't ideal.

Should the model grow structured compound/prefix/subcategory fields?  Not urgently.
The flat model serves all current use cases, and enriching it would be additive (no
breakage).  Now that batchalign shares `talkbank-model` via path dependencies (no more
vendored copy), any such enrichment is a shared decision visible in talkbank-tools's review
process — the right place for it to happen.

## 5. UD-to-CHAT Mapping

Absorbed from the former `mor-gra-generation.md`.  The mapping lives in
`crates/talkbank-transform/src/morphosyntax/`, with the main entry point being
`sentence_mapping.rs::map_ud_sentence`.

### Pipeline

```text
Main tier words
    ↓
Stanza NLP (Python worker, `worker/_infer_hosts.py` → `inference/morphosyntax.py`)
    ↓  produces UdSentence { words: Vec<UdWord> }
    ↓  each UdWord has: id, text, lemma, upos, feats, head, deprel
    ↓
map_ud_sentence() (Rust, mapping.rs)
    ↓  produces (Vec<Mor>, Vec<GrammaticalRelation>)
    ↓
Post-construction validation
    ↓  rejects if chunk count != gra count
    ↓
inject_morphosyntax_from_cache() or add_morphosyntax_batched()
    ↓  writes %mor and %gra tiers into the AST
    ↓
CHAT serialization
```

### MOR Generation: Two Mapping Variants

The mapping layer provides two functions that differ only in how MWT Range
tokens are handled. Both share identical GRA/validation logic via the
internal `build_gra_and_validate()` helper.

```mermaid
flowchart LR
    ud["UdSentence\n(from Stanza)"]
    mode{"Mapping\nvariant?"}
    merged["map_ud_sentence()\nassemble_mors() merges\nRange → 1 clitic MOR"]
    expanded["map_ud_sentence_expanded()\nmap_ud_word_to_mor() per component\nRange → N individual MORs"]
    gra["build_gra_and_validate()\nchunk indexing, GRA relations,\nroot check, terminator, validation"]
    out["(Vec&lt;Mor&gt;, Vec&lt;GrammaticalRelation&gt;)"]

    ud --> mode
    mode -->|"Preserve\n(L2 splice)"| merged --> gra
    mode -->|"StanzaRetokenize\n(main tier rewrite)"| expanded --> gra
    gra --> out
```

**`map_ud_sentence()`** — Preserve mode and L2 splice. Produces one `Mor`
item per CHAT word. MWT Range tokens are merged into a single clitic MOR
via `assemble_mors()`:

```text
"I'll" → Range(1,2): ["I", "'ll"]
  is_clitic("I", en) → false → main_idx = 0
  Post-clitics: ["'ll"]
  Result: pron|I~aux|will (1 MOR, 2 chunks)
```

**`map_ud_sentence_expanded()`** — Retokenize mode. Produces one `Mor`
per component word. Range parent tokens are skipped; each component gets
its own MOR via `map_ud_word_to_mor()`:

```text
"gonna" → Range(1,2): ["gon", "na"]
  gon → verb|go-Part-Pres-S (1 MOR)
  na  → part|to             (1 MOR)
  Result: 2 separate MORs (matched to 2 tokens on rewritten main tier)
```

The expanded variant exists because the retokenize path rewrites the
main tier with Stanza's tokens — each token needs its own MOR item.
The Preserve path keeps the original main tier, so Range components
must be merged into one clitic MOR to match the single CHAT word.

### GRA Generation

**Critical: %gra indices are per-chunk, not per-word.**

Each %mor chunk (including clitics) needs its own %gra relation.  The GRA builder:

1. **Builds a chunk-based index mapping** (`ud_to_chunk_idx`).  Each UD word ID maps
   to a sequential chunk index.  For MWT ranges, each component gets its own index:

   ```text
   Range(1,2) "I'll": ID 1 → chunk 1, ID 2 → chunk 2
   Single(3) "give":  ID 3 → chunk 3
   Single(4) "you":   ID 4 → chunk 4
   ```

2. **Emits one GRA relation per component** (not one per MWT).  Each component's
   UD head and deprel are used directly:

   ```text
   I:    head=3 (give), deprel=nsubj → 1|3|NSUBJ
   'll:  head=3 (give), deprel=aux   → 2|3|AUX
   give: head=0 (root)               → 3|0|ROOT
   you:  head=3 (give), deprel=iobj  → 4|3|IOBJ
   ```

3. **Adds terminator** PUNCT relation pointing to ROOT.

### TalkBank Conventions

The mapper applies four CHAT-specific transformations, all lossless:

| UD Convention | TalkBank Convention | Example |
|--------------|--------------------|---------|
| `head=0` for root | `head=0` (same — UD standard) | `2\|0\|ROOT` |
| Subtypes with colon | Subtypes with dash | `acl:relcl` → `ACL-RELCL` |
| Lowercase relations | Uppercase relations | `nsubj` → `NSUBJ` |
| Multi-value features with comma | Commas preserved | `PronType=Int,Rel` → `-Int,Rel` |

The first three are trivially reversible surface-syntax changes.  The comma convention
preserves the UD multi-value separator as-is.  This differs from older CLAN-produced
corpus data which used concatenation (`IntRel`, `AccNom`).  The tree-sitter grammar and
%mor parser both accept commas in suffix values.

### POS Mapping

POS categories use lowercased UPOS tags:

| UPOS | CHAT POS | Suffix features |
|------|----------|-----------------|
| NOUN | `noun\|` | Gender, Number, Case, Ger |
| VERB/AUX | `verb\|`/`aux\|` | VerbForm, Tense, Person, -irr |
| PRON | `pron\|` | PronType, Case, Reflex, Number, Person |
| DET | `det\|` | Gender, Definite, PronType |
| ADJ | `adj\|` | Degree, Case |
| ADP | `adp\|` | (none) |
| PROPN | `propn\|` | (none) |
| INTJ | `intj\|` | (none) |
| CCONJ | `cconj\|` | (none) |
| SCONJ | `sconj\|` | (none) |

Language-specific rules exist for English (200+ irregular verbs in `lang_en.rs`), French
(pronoun case, APM in `lang_fr.rs`), and Japanese (verb form overrides, 140+ rules in
`lang_ja.rs`).

## 6. Post-Construction Validation

`map_ud_sentence` validates generated output before returning:

### Structural GRA Validation

`validate_generated_gra` enforces four rules:

- **Single root** — exactly one self-referential or head=0 relation (excluding terminator)
- **No circular dependencies** — no word is its own ancestor in the head chain
- **Valid heads** — all head references point to existing word indices or 0
- **Sequential indices** — guaranteed by construction

Cycle detection uses an **O(N) White-Gray-Black DFS with memoization**: each word follows
its head chain to the root, marking nodes IN_PROGRESS (gray) on the way down and NO_CYCLE
(black) on the way back.  Encountering a gray node means a cycle.

On failure, `validate_generated_gra` returns `Err(MappingError)` with a detailed error
message including the full invalid structure.  The caller (morphosyntax orchestrator) logs the error and
skips the utterance — no corrupted %gra is written to disk.

The Rust mapper uses `HashMap<usize, usize>` to translate UD word IDs to CHAT chunk
indices.  Missing keys fall through to `unwrap_or(&0)` (caught by the valid-heads check),
rather than silently wrapping around like Python master's `actual_indicies[elem[1]-1]`
array indexing — which was the root cause of Python's circular dependency bug.

### Chunk Count Alignment

The critical guard added after the MWT/GRA bug:

```rust,ignore
let mor_chunk_count = mors.iter().map(|m| m.count_chunks()).sum::<usize>() + 1;
if gras.len() != mor_chunk_count {
    return Err(MappingError::ChunkCountMismatch { ... });
}
```

This catches any mismatch at generation time, preventing corrupted data from ever being
written to CHAT files.

## 7. Module Details

### `lib.rs` — PyO3 Entry Points

Key morphosyntax functions exported to Python via `#[pymodule]`:

| Function | Purpose |
|----------|---------|
| `extract_morphosyntax_payloads(handle)` | Extract all utterance payloads as JSON array |
| `inject_morphosyntax_from_cache(handle, ...)` | Inject cached %mor/%gra strings into AST |
| `add_morphosyntax_batched(handle, lang, callback, ...)` | Batched pipeline: extract → callback → inject |
| `extract_morphosyntax_strings(handle)` | Extract final %mor/%gra strings for cache storage |
| `extract_nlp_words(chat_text, domain)` | Extract alignable words as JSON |
| `py_dp_align(payload, reference, case_insensitive)` | Hirschberg alignment |

### `extract.rs` — Word Extraction

Walks the CHAT AST using `walk_words()` (from `talkbank-model`) and collects words appropriate for a given tier domain. The walker centralizes traversal of all 24 `UtteranceContent` variants and 22 `BracketedItem` variants; `extract.rs` provides only the word-handling closures for `counts_for_tier()` filtering and `ReplacedWord` branch logic.

```rust
pub struct ExtractedWord {
    pub text: String,              // cleaned text (for NLP)
    pub raw_text: String,          // original text (with markers)
    pub special_form: Option<String>,  // @c → "c", @s → "s", etc.
}
```

**Domain-aware traversal** via `TierDomain`:

| Domain | Retraces | Replacements | Untranscribed (xxx/yyy/www) |
|--------|----------|-------------|---------------------|
| Mor | Skipped | Use replacement words | Skipped (case-insensitive) |
| Wor | Included | Use original words | Included |

**Case-insensitive untranscribed detection:** The `counts_for_tier()` gate
recognizes `xxx`, `yyy`, and `www` **case-insensitively** — uppercase variants
like `XXX` (illegal per E241 but common in legacy corpora) are also excluded
from extraction. Without this, uppercase untranscribed markers would be sent to
Stanza, which assigns them `UPOS=X`, producing a spurious `x|XXX` entry on
%mor that breaks alignment (E706). See `Word::compute_untranscribed()` in
`talkbank-model`.

### `dp_align.rs` — Hirschberg Alignment

Rust-only Hirschberg implementation (Python `utils/dp.py` no longer exists):
- **Cost model:** match=0, substitution=2, gap=1
- **Space:** O(min(n,m)) via Hirschberg's linear-space trick
- **Small cutoff:** Falls back to full DP table when n*m < 2048

### `mor_parser.rs` — %mor/%gra Parsing

Delegates to `talkbank_parser` for parsing %mor strings into typed
`Mor` and `GrammaticalRelation` structures.  `embed_gra_on_mors()` attaches GRA relations
to Mor items by 1-indexed chunk position.

### `inject.rs` — Morphosyntax Injection

1. Walks the AST using the same traversal order as `extract.rs`
2. Assigns Mor items to alignable Word nodes
3. Builds tier markers (`MorTierMarker`, `GraTierMarker`)
4. Adds to utterance dependent tiers, replacing existing tiers if present

**Key invariant:** traversal order in `inject.rs` must exactly match `extract.rs`.

### `retokenize/` — AST Retokenization

Split into five files: `mod.rs` (entry point), `rebuild.rs` (AST rebuilding),
`mapping.rs` (word index mapping), `parse_helpers.rs` (word parsing), `tests.rs`.

When `retokenize=true`, Stanza uses its own UD tokenizer, which can change word
boundaries (splits, merges, different text).  The algorithm:

1. **Filter Range parent tokens** from `ud_sentence.words` — only component
   words appear in the token vector. Range parents are the container entry
   (e.g., `id=[1,2] text="gonna"`) whose components follow immediately.
   Including both would overcount tokens and break MOR alignment.
2. Character-level DP alignment between original and Stanza token texts
3. Build mapping: original_word_idx → stanza_token_indices
4. Walk AST, rebuilding content vectors (1:1, 1:N splits, preserving non-word content)
5. New Words created via `DirectParser::parse_word()` (not `Word::new()`)
6. Inject MOR/GRA tiers (MOR items are per-component via `map_ud_sentence_expanded()`)

### `nlp/mapping/` — UD-to-CHAT Mapping

The core mapping logic lives in `mapping/mod.rs`. Two public entry points:

- **`map_ud_sentence()`** — merges MWT Range components into clitic MOR items (for Preserve mode and L2 splice)
- **`map_ud_sentence_expanded()`** — produces per-component MOR items (for Retokenize mode)

Both delegate GRA generation, root validation, and chunk-count alignment to
the shared `build_gra_and_validate()` helper.
See [Section 5](#5-ud-to-chat-mapping) for details.

## 8. The Callback Pattern

### Batched Payload (Rust → Python)

The primary path is batched: Rust collects all utterance payloads in one pass and sends
them as a JSON array.  Each element:

```json
{
  "words": ["I", "eat", "cookies"],
  "terminator": ".",
  "special_forms": [null, null, null]
}
```

With special forms (e.g., `gumma@c`):
```json
{
  "words": ["gumma", "is", "yummy"],
  "terminator": ".",
  "special_forms": [["gumma", "c"], null, null]
}
```

### Response (Python → Rust)

```json
{
  "mor": "pro:sub|I v|eat n|cookie-PL .",
  "gra": "1|2|SUBJ 2|0|ROOT 3|2|OBJ 4|2|PUNCT",
  "tokens": ["I", "eat", "cookies"]
}
```

The `tokens` field is always present.  When `retokenize=false`, it's ignored.  When
`retokenize=true`, Rust compares tokens against original words and rebuilds the AST if
they differ.

### Worker-side batch inference (`worker/_infer_hosts.py` + `inference/morphosyntax.py`)

The worker-side morphosyntax host wraps Stanza to conform to this interface:

1. Parse JSON payload array
2. For each utterance: replace special-form words with `"xbxxx"` (Stanza placeholder)
3. Strip parentheses, join words as text
4. Set `tokenizer_context["sentence"]` for Stanza's postprocessor
5. Call `nlp(text)` under lock on GIL-enabled Python
6. `map_ud_sentence()` converts UD output to %mor/%gra
7. Extract Stanza token texts
8. Return `[{mor, gra, tokens}, ...]` JSON array

### Cache Orchestration

Caching is handled at the server orchestration level (`process_morphosyntax_batch()`), not in the
inference module.  The flow:

1. `extract_morphosyntax_payloads()` — get all utterance payloads
2. For each payload, compute BLAKE3 key from `text + lang + "|mwt"`
3. Cache hits: `inject_morphosyntax_from_cache()` — inject cached strings
4. Cache misses: send to batch callback, inject results, store in cache
5. `extract_morphosyntax_strings()` — extract final strings for cache storage

## 9. L2 Morphotag (Default)

By default, @s (code-switched) words are routed to secondary language
Stanza models. Pass `--no-l2-morphotag` to opt out and emit `L2|xxx`
stubs on the %mor tier instead.

### Dispatch Flow

```mermaid
sequenceDiagram
    participant R as Rust Server<br/>(batch.rs)
    participant P1 as Primary Stanza<br/>(e.g., German)
    participant P2 as Secondary Stanza<br/>(e.g., English)
    participant L2 as L2 Module<br/>(morphosyntax/l2/)

    R->>P1: morphotag all utterances<br/>(primary language)
    P1-->>R: UdResponse with L2|xxx<br/>for @s positions
    R->>L2: extract_l2_deferred_positions()
    L2-->>R: deferred positions + target lang
    R->>L2: plan_secondary_dispatch()
    L2-->>R: contiguous spans + host attachments
    R->>P2: infer_batch(retokenize=true)<br/>contiguous @s spans
    P2-->>R: UdResponse with<br/>Range tokens for contractions
    R->>L2: merge_planned_secondary_span()<br/>planned structural + lexical merge
    L2-->>R: merged Mor items
    R->>R: splice_l2_into_chat()<br/>replace L2|xxx with real MOR
```

### How It Works

1. **Primary pass** produces %mor/%gra for the entire utterance. @s words
   get `L2|xxx` placeholders via the special form handler in `inject.rs`.
2. **Extract deferred positions** identifies which words have `L2|xxx` and
   their target languages (from `@s:spa`, `@s:eng`, or bare `@s` resolved
   via `@Languages`).
3. **Plan dispatch spans** creates contiguous per-utterance spans of same-language
   @s words and computes the host attachment for each span root
   (e.g., `los@s:spa niños@s:spa` → one span of 2 words with an explicit
   external-anchor plan).
4. **Secondary dispatch** sends each planned span to a Stanza worker for the target
   language with `retokenize=true`. MWT contractions (`it's`, `don't`) are
   expanded via Range tokens — `map_ud_sentence()` merges them into clitics.
5. **Merge** combines secondary lexical output (lemma, features) with primary
   structural info (deprel, head) plus the planned host attachment using a
   6-level POS resolution priority.
6. **Splice** replaces `L2|xxx` with the merged MOR items and corrects GRA
   relations where the resolved POS contradicts the primary deprel.

### Validation and repair policy

- Whole-utterance same-language all-`@s` patterns are rejected during
  pre-validation (E255). The accepted CHAT form is utterance-level `[- lang]`.
- Explicit `@s:LANG` still routes to `LANG` even if `LANG` is absent from
  `@Languages`, but validation emits warn-only E254 to surface the header drift.
- `chatter debug fix-s` is the intended normalization tool for both cases: it
  rewrites the qualifying whole-utterance `@s` pattern, appends missing
  explicit languages to `@Languages`, and skips files that already need no
  change.

The fix-s rewrite predicate verifies that **every** word-bearing item
on the main tier (words, fillers `&~`/`&-`/`&+`, nonwords, retraced
material) resolves to the same target language. Fillers and nonwords
participate in the predicate AND have their `@s` shortcuts cleared
when the rewrite fires — otherwise a bare `@s` would flip its resolved
language under the new `[- LANG]` precode. See
[`chatter` CLI reference: `fix-s`](../../chatter/user-guide/cli-reference.md#debug)
for the full safety contract.

### Unsupported non-primary languages

`morphotag` skips files whose **primary** `@Languages` code is not
Stanza-supported with a typed diagnostic (no pipeline entry). When the
primary IS supported, non-primary content targeting an unsupported
language degrades gracefully:

- `[- UNSUPPORTEDLANG]` precodes — `infer_batch` partitions language
  groups via `partition_groups_by_stanza_support`; unsupported groups
  bypass Stanza dispatch and the words receive `L2|xxx` in `%mor`.
- `@s:UNSUPPORTEDLANG` per-word markers — the secondary dispatch path
  for that span is short-circuited the same way; the host primary
  analysis is preserved and the `@s` token's slot stays as `L2|xxx`.

The worker never crashes on an unsupported secondary, and other
utterances or spans in the same file targeting supported languages
continue to receive real morphology.

### Key Files

| File | Purpose |
|------|---------|
| `morphosyntax/l2/plan.rs` | Contiguous span planning and host-attachment planning |
| `morphosyntax/l2/extract.rs` | Extract primary structural info from UD responses |
| `morphosyntax/l2/spans.rs` | Group @s positions into contiguous dispatch spans |
| `morphosyntax/l2/merge.rs` | POS resolution priority, planned structural merge |
| `morphosyntax/l2/splice.rs` | Replace L2\|xxx in ChatFile with merged MOR |
| `morphosyntax/l2/deprel.rs` | UdDeprel newtype, deprel→POS constraint mapping |
| `morphosyntax/batch.rs` | `dispatch_secondary_l2()` — thin worker adapter over the transform-layer L2 seam |

### MWT Contraction Handling

L2 dispatch sends `retokenize=true` to the secondary worker, enabling
Stanza's MWT expander for the target language. For English @s words:

- `it's@s:eng` → `pron|it~aux|be` (clitic MOR, not `L2|xxx`)
- `don't@s:eng` → `aux|do~part|not` (clitic MOR)
- `working@s:eng` → `noun|work-Part-Pres-S` (no contraction, regular MOR)

The L2 path uses `map_ud_sentence()` (merged clitics), which is correct
because L2 does NOT rewrite the main tier — the @s word stays as-is,
and its %mor slot gets the clitic form.

## 10. Gotchas

### `cleaned_text` is Derived, Not Settable

The CHAT serializer uses `Word.content` (WordContents), not `raw_text` or `cleaned_text`.
Simply changing `word.cleaned_text = "new"` does not change serialized output.  To create
a word with different text, use `DirectParser::parse_word()`.

### `Word::new()` Bypasses Validation

`Word::new(raw_text, cleaned_text)` creates a minimal Word with a single `WordContent::Text`
element.  For retokenization where text comes from Stanza (may contain CHAT-significant
characters), prefer `parse_token_as_word()` which runs the full parser.

### Traversal Order Must Match Between extract/inject/retokenize

All three modules walk the AST using `walk_words()` / `walk_words_mut()` from
`talkbank-model`, ensuring identical traversal order. The walker handles group recursion
and domain-aware gating centrally. If leaf-handling closures apply different filtering
between extraction and injection, morphology is assigned to wrong words.

### Separator Word Counter Sync

`extract.rs` includes tag-marker separators (comma `,`, tag `„`, vocative `‡`) as NLP
words in the Mor domain.  Any code walking the AST with a `word_counter` must also
increment for separators.  `retokenize.rs` handles this explicitly.  Forgetting causes
counter desync.

### Manual JSON Parsing

`batchalign-core` uses manual JSON field extraction instead of serde_json at runtime to
avoid the dependency in the release binary.  The parsers handle escapes but are not
general-purpose.

### Special Forms and `xbxxx`

Words with `@c`, `@s`, `@b` markers are replaced with `"xbxxx"` before Stanza analysis.
When `retokenize=true`, `retokenize.rs` restores original text via `resolve_token_text()`.

### `skipmultilang` and Language Handling

When `skipmultilang=true`, utterances with `[- lang]` override where the language differs
from the file's primary language are skipped.  Language codes: file language is ISO 639-3
(`"eng"`, `"fra"`); callback adapter converts to ISO 639-1 (`"en"`, `"fr"`) for Stanza.
This flag is only about utterance-level `[- lang]` routing. Per-word `@s`
secondary dispatch is controlled separately by `--no-l2-morphotag`.

### `BracketedItems` is a Newtype

`BracketedContent.content` is `BracketedItems(Vec<BracketedItem>)`, a newtype that does
not implement `Default`.  Use `std::mem::replace(&mut field, BracketedItems(Vec::new()))`
instead of `std::mem::take()`.

### Uppercase Untranscribed Markers (XXX, YYY, WWW)

Legacy corpora frequently contain uppercase `XXX` instead of the required
lowercase `xxx`. These are flagged as E241 by the validator, but the morphotag
pipeline must still handle them correctly. The extraction layer's
`counts_for_tier()` gate uses `Word::compute_untranscribed()`, which matches
case-insensitively. This prevents uppercase variants from being sent to Stanza,
which would produce spurious `x|XXX` entries on the %mor tier and cause E706
alignment mismatches.

### Stanza `token.id` is Always a Tuple

`(word_id,)` for regular words, `(start, end)` for MWT.  Never assume it's an int.
