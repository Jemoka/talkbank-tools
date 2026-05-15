# MWT (Multi-Word Token) Handling

**Status:** Current
**Last updated:** 2026-05-02 11:15 EDT

---

## What Is MWT?

Multi-Word Tokens (MWT) are contractions that represent multiple
syntactic words in a single orthographic token.  English examples:

| Contraction | Expanded     | %mor output                        |
|-------------|-------------|-------------------------------------|
| don't       | do + n't    | `aux\|do-Fin-Ind-Pres~part\|not`   |
| I'm         | I + 'm      | `pron\|I-Prs-Nom-S1~aux\|be-Fin-Ind-Pres-S1` |
| that's      | that + 's   | `pron\|that-Dem~aux\|be-Fin-Ind-Pres-S3` |
| can't       | can + n't   | `aux\|can-Fin-S~part\|not`          |
| where's     | where + 's  | `adv\|where~aux\|be-Fin-Ind-Pres-S3` |

In CHAT %mor notation, expanded MWT components are joined with `~`
(post-clitics) or `$` (pre-clitics).

---

## Current Stanza Tokenization Policy

Current batchalign3 follows the Python-master-style Stanza configuration
(`tokenize_no_ssplit=True`) rather than the older Rust
`tokenize_pretokenized=True` approach.

### Why

Without MWT expansion, Stanza analyzes contractions as single words,
producing linguistically incorrect POS tags:

| Word    | With MWT (correct)       | Without MWT (incorrect) |
|---------|------------------------|------------------------|
| don't   | AUX "do" + PART "not"  | ADV "don't"            |
| I'm     | PRON "I" + AUX "be"    | ADV "im"               |
| that's  | PRON "that" + AUX "be" | AUX "that"             |
| can't   | AUX "can" + PART "not" | INTJ "cant"            |

On a Brown/Eve transcript (010600a.cha), Python master produces 644
`~` joins.  Without MWT, Rust produced 1.  Of the differing %mor lines,
99.2% differed solely because of MWT.

### The Two Stanza Tokenizer Modes

**`tokenize_pretokenized=True`** (original Rust approach):
Stanza's tokenizer is completely bypassed.  Each whitespace-separated
token becomes a single Stanza `Token` with a single `Word`.  The MWT
processor still runs, but because "don't" was never split by the
tokenizer, MWT sees it as atomic and does not expand it.

**`tokenize_no_ssplit=True`** (Python master, now our approach):
Stanza's neural tokenizer runs — it splits text into tokens, including
splitting contractions ("don't" -> "do" + "n't") — but does not insert
sentence boundaries.  The MWT processor then annotates these splits
with range IDs (`id: [2, 3]`).

### Why `tokenize_pretokenized` Was Originally Chosen

The Rust implementation originally chose `tokenize_pretokenized=True`
for a specific reason: **guaranteed 1:1 token mapping**.  With the
tokenizer bypassed, the number of input tokens exactly equals the
number of Stanza tokens, making it trivially safe to zip Stanza's
output back onto CHAT AST words.

The problem: this guarantee comes at the cost of losing MWT entirely,
which means all contractions in English (and French, Italian, etc.) get
wrong POS tags.

### What We Changed

- **MWT-capable languages** (English, French, Italian, etc.): Switch
  to `tokenize_no_ssplit=True` + a `tokenize_postprocessor` callback
  that merges spurious tokenizer splits back to original CHAT words.
  English uses the GUM MWT package (`package={"mwt": "gum"}`).
- **Non-MWT languages** (Japanese, Chinese, Korean, etc.): Keep
  `tokenize_pretokenized=True` for safety.  The neural tokenizer would
  re-segment already-tokenized CJK text unpredictably.

See `MWT_LANGS` in `worker/_stanza_loading.py` for the live allowlist.
The English-specific pipeline branch loads MWT GUM via this allowlist.

---

## The Core Problem: Stanza Creates "Words" That Don't Exist in CHAT

When Stanza's neural tokenizer runs, it can:

1. **Split contractions** (intended): "don't" -> "do", "n't"
2. **Split compounds** (unintended): "ice-cream" -> "ice", "-", "cream"
3. **Normalize text** (unintended): "cafe" for "café" (accent stripping)
4. **Re-segment** (unintended): "l'homme" -> "l'", "homme" (French)

These Stanza-created tokens are **NOT valid CHAT words**.  CHAT's word
grammar is strict (e.g., bare `-` is not a valid word).  If these tokens
leaked into the CHAT main tier, the file would become unparseable.

### Our Safety Guarantee: New Tokens Never Reach CHAT

The architecture enforces a hard type boundary between Stanza tokens
and CHAT words:

```text
CHAT main tier words (Word in Rust AST)
      │
      ├─ extract_nlp_words() ──> list of strings sent to Python
      │
      ├─ Python batch callback ──> Stanza processes strings ──> UdWord JSON
      │
      └─ map_ud_sentence() ──> Vec<Mor> ──> assigned to %mor dependent tier
```

At no point does a Stanza token become a CHAT `Word`.  The Rust types
are distinct:

- **`Word`** (CHAT AST node): Lives on the main tier.  Created only
  during CHAT parsing.  Immutable during morphosyntax processing.
- **`UdWord`** (Stanza output): Deserialized from Stanza's JSON.
  Consumed by `map_ud_word_to_mor()` to produce `Mor` nodes.  Never
  stored in the CHAT AST.
- **`Mor`** (morphology node): Lives on the %mor dependent tier.
  Contains POS category, lemma, and features.  One `Mor` per original
  CHAT word (with MWT components joined as clitics via `~`/`$`).

The Rust compiler enforces this separation — there is no function that
converts a `UdWord` into a `Word`.  Even if Stanza splits "ice-cream"
into three tokens, the result is a single `Mor` node assigned to the
original "ice-cream" `Word`.

### How Python Master Gets This Wrong

Python master has a `--retokenize` mode (ud.py:902-1004) that **does**
allow Stanza tokens to leak into the CHAT main tier:

```python
# Python master, retokenize mode:
ut, end = chat_parse_utterance(
    " ".join([i.text for i in sents[0].tokens]) + " " + ending,
    # ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^
    # This creates Word objects from STANZA TOKENS, not original CHAT words
    mor, gra, None, None
)
# ...
doc.content[indx] = Utterance(content=ut, ...)  # Overwrites main tier!
```

If Stanza normalizes "café" to "cafe" or splits "ice-cream" into three
tokens, those new forms end up in the CHAT file.  This is a data
integrity bug.

By default, the main tier is always the original parsed CHAT, untouched by
Stanza.  A `--retokenize` option exists (documented in `args.rs`) that
retokenizes the main tier to match UD tokenization; it bypasses the cache.

---

## How We Handle Spurious Tokenizer Splits

When Stanza splits a compound word ("ice-cream" -> "ice", "-", "cream"),
we merge those tokens back before they reach downstream processors.

### Mechanism: Character-Position Mapping

The `inference/_tokenizer_realign.py` module implements a `tokenize_postprocessor`
callback that Stanza calls after tokenization but before POS tagging.

The key insight: **Stanza's tokenizer only re-splits the same characters
— it never reorders, adds, or removes characters.**  This means:

```text
concat(stanza_tokens, no_spaces) == concat(original_words, no_spaces)
```

This invariant lets us use a simple O(n) character-position map instead
of an O(n*m) DP edit-distance alignment.

#### Algorithm

1. Build a character-to-word-index array from the original CHAT words:
   ```rust,ignore
   "ice-cream know" -> [0,0,0,0,0,0,0,0,0, 1,1,1,1]
                         i c e - c r e a m   k n o w
   ```

2. Build a character-to-token-index array from Stanza's tokens:
   ```rust,ignore
   "ice - cream know" -> [0,0,0, 1, 2,2,2,2,2, 3,3,3,3]
                           i c e  -  c r e a m   k n o w
   ```

3. For each original word, collect which Stanza tokens have characters
   in that word's range:
   ```rust,ignore
   word 0 ("ice-cream") -> tokens [0, 1, 2]  (need merging)
   word 1 ("know")       -> tokens [3]        (fine as-is)
   ```

4. Merge multi-token groups back into single tokens.

#### What Happens When Characters Don't Match

If Stanza normalizes text (e.g., accent stripping), the character
sequences won't match.  In that case, we **bail out immediately**:

```python
if ref_str != tok_str:
    L.debug("Character mismatch, skipping")
    return stanza_tokens  # Return unchanged — no merging
```

This is strictly safer than the old DP approach, which can
produce an alignment even when the character sequences differ (treating
mismatches as edit operations), potentially allowing normalized forms
to leak through.

### Why Not DP Alignment (Like Python Master)?

Older Python master (pre-Rust migration) used a Levenshtein edit-distance DP aligner
to match Stanza tokens back to original words at the character level.
Current batchalign3 does not use that approach for retokenization:

| Property | DP Alignment (old) | Character-Position Map (current) |
|----------|----------------------|------------------------------|
| Complexity | O(n*m) with Hirschberg optimization | O(n) linear scan |
| Ambiguity | Equal-cost alignments broken arbitrarily | Deterministic — each character has exactly one position |
| Normalization | Accepts mismatches as edit operations | Rejects immediately on any character difference |
| Failure mode | May return wrong alignment silently | Returns tokens unchanged (safe fallback) |

### How MWT Contractions Pass Through (Intentionally)

The postprocessor merges spurious splits but does **not** merge MWT
contractions.  When Stanza splits "don't" into ("do", "n't"), these
arrive as a **tuple** (Stanza's internal MWT marker), not plain strings.
The postprocessor treats tuples as MWT and returns them unchanged.

**Python-side hint preservation.** Rust handles MWT correctly in
`inject.rs`. On the Python side, if `_tokenizer_realign.py::_realign_sentence`
flattened the `(text, True)` tuples to plain strings via `_conform(tok)`
before the Rust char-DP aligner ran, MWT would silently skip Range-token
expansion for every contraction. `_realign_sentence` overlays Stanza's
original tuples onto aligner output where lengths
match and no merging happened, so the hint survives realignment and
Stanza's MWT processor continues to honor it. Applies to every language
listed in `MWT_LANGS` (`python/batchalign/worker/_stanza_loading.py`). See
[Stanza Limitations — Defect 2](stanza-limitations.md) for the full
trace and re-evaluation criteria.

Stanza's MWT processor then annotates these with Range markers
(`"id": [2, 3]`), which the Rust code in `sentence_mapping.rs` handles:

```rust,ignore
// sentence_mapping.rs: map_ud_sentence()
UdId::Range(start, end) => {
    // Group component words under one CHAT word index
    for j in 0..count {
        ud_to_chat_idx.insert(start + j, chat_idx);
    }
    chat_idx += 1;  // One CHAT word, multiple UD words
}
```

The component words are assembled into a single `Mor` with clitic
markers:

```rust,ignore
// mapping_helpers.rs: assemble_mors()
// "do" (AUX) + "n't" (PART) -> aux|do~part|not
if is_clitic(text) {
    post_clitics.push(mapped);
} else {
    head = Some(mapped);
}
```

### Thread Safety

The `TokenizerContext` is shared between the Stanza inference host (which sets
`original_words`) and the postprocessor (which reads them). Both execute under
the same `nlp_lock`:

```python
# python/batchalign/inference/morphosyntax.py
with nlp_lock:
    if tok_ctx is not None:
        tok_ctx.original_words = word_lists  # Set before nlp()
        doc = nlp(combined)                       # Postprocessor reads during this call
    if tok_ctx is not None:
        tok_ctx.original_words = []           # Clear after
```

---

## Comparison: What Each Approach Does With Edge Cases

### Compound Splitting: "ice-cream" -> `["ice", "-", "cream"]`

| Stage | Python Master | Our Approach |
|-------|---------------|--------------|
| Stanza output | 3 tokens | 3 tokens |
| Postprocessor | DP aligns chars, merges back to 1 token | Char-position map, merges back to 1 token |
| %mor result | 1 Mor node for "ice-cream" | 1 Mor node for "ice-cream" |
| Main tier | Retokenize mode: risk of 3 words | Always original (1 word) |

### Contraction: "don't" -> `["do", "n't"]`

| Stage | Python Master | Our Approach |
|-------|---------------|--------------|
| Stanza output | 2 tokens with MWT Range `[2,3]` | Same |
| Postprocessor | Kept as MWT tuple | Kept as MWT tuple |
| %mor result | `aux\|do~part\|not` | `aux\|do~part\|not` |
| Main tier | Original "don't" | Original "don't" |

### Possessive Apostrophe: "Claus'"

Stanza's English GUM MWT model treats possessive apostrophes as MWT
contractions.  For `Claus'`, it produces two MWT components
`[Claus (PROPN), ' (PUNCT)]`.  We follow Python master's behavior:

| Stage | Python Master | Our Approach |
|-------|---------------|--------------|
| Stanza output | `Claus'` MWT → `[Claus, ']` | Same |
| Postprocessor | `("Claus'", True)` — English+apostrophe → allow MWT | Same (`_is_contraction` returns True) |
| %mor result | `propn\|Claus~punct\|'` | `propn\|Claus~punct\|'` |
| CHAT validity | Valid | Valid |

**`clean_lemma` defensive fix**: When `'` is isolated as a PUNCT MWT component,
Stanza's lemma is also `'`.  The old `clean_lemma` stripped the apostrophe,
producing an empty string → `punct|` (empty stem) → E342 parse failure.
`mapping.rs` now falls back to the surface text when stripping produces empty:
`clean_lemma("'", "'")` returns `("'", false)`, producing `punct|'` (valid).
A `debug_assert!` at `MorStem` construction time catches any future regressions.

### Accent Normalization: "café" -> "cafe"

| Stage | Python Master | Our Approach |
|-------|---------------|--------------|
| Stanza output | "cafe" (accent stripped) | Same |
| Postprocessor | DP accepts mismatch as edit operation | **Bail out** — chars don't match |
| %mor result | Based on "cafe" (wrong lemma) | Based on "cafe" (Stanza's analysis, not merged) |
| Main tier | Retokenize: "cafe" leaks in | Always original "café" |

### Unicode Decomposition: "naïve" (NFC) vs "nai\u0308ve" (NFD)

| Stage | Python Master | Our Approach |
|-------|---------------|--------------|
| Stanza output | Possibly NFD-decomposed (6 chars vs 5) | Same |
| Postprocessor | DP: char count mismatch, Extra result | **Bail out** — char sequences differ |
| Main tier | Undefined behavior (breakpoint in dev) | Always original NFC form |

---

## Architecture: Two-Layer Design

Morphosyntax processing has two distinct layers, each handling a different
problem.  They use different languages because they interface with different
systems.

### Layer 1 — Python: Stanza Tokenizer Callback

**File**: `inference/_tokenizer_realign.py`
**Runs**: Inside `stanza.Pipeline.__call__()`, between the neural tokenizer and
the MWT/POS/depparse models.
**Language**: Python — Stanza's `tokenize_postprocessor` API requires a Python
callable.  This cannot be implemented in Rust because Stanza is a Python/PyTorch
library; it doesn't expose C FFI or any other non-Python hook.

**Responsibility**: Tell Stanza's MWT model whether a merged token should be
treated as a contraction (expand it) or as an accidental split (suppress expansion).

This layer has **no knowledge of CHAT, %mor, POS mapping, or language grammar**.
It only answers one question per merged token: "is this an MWT?"

The `_is_contraction()` function replicates Python master's rule exactly:

```python
# English tokens containing ' (except o' forms like o'clock) → True (allow MWT)
# Everything else → False (suppress MWT re-expansion)
def _is_contraction(text: str, alpha2: str) -> bool:
    if "'" not in text or alpha2 != "en":
        return False
    parts = text.split("'")
    if len(parts) >= 2 and parts[0].strip().lower() == "o":
        return False
    return True
```

The rule is tiny (4 lines) because the logic is simple — it's just a knob on
the neural MWT model, not a grammar.

### Layer 2 — Rust: UD → %mor/%gra Conversion

**Primary module**: `crates/talkbank-transform/src/morphosyntax/` — orchestrates the
full UD-to-CHAT mapping pipeline. Core components:
- `sentence_mapping.rs` — maps UD sentences to CHAT structure
- `injection.rs` — injects mapped results into transcripts
- `synthesis/` — synthesizes final `%mor` and `%gra` output
- `lang_en.rs`, `lang_fr.rs`, `lang_ja.rs` — language-specific mapping rules
- `mapping_helpers.rs` — common mapping utilities

**Runs**: After Stanza has produced POS tags, lemmas, and dependency relations.
**Language**: Rust — this layer has no Python dependency.  It reads Stanza's JSON
output (a `Vec<UdWord>`) and produces `%mor/%gra` strings.

**Responsibility**: All the substantive language-specific work:

- Map UD POS tags (VERB, NOUN, PRON...) to CHAT POS categories (verb|, noun|, pron|...)
- Apply POS-specific suffix rules (tense, number, case, degree...)
- Handle 200+ English irregular verbs (go → went, be → was/were/been...)
- Handle French pronominal clitics and APM markers
- Handle Japanese verb conjugation (140+ patterns)
- Assemble MWT components into clitic chains (`aux|do~part|not`)
- Build `%gra` dependency relations

### Why the Split

```text
CHAT words
    │
    │  extract_nlp_words() [Rust]
    ▼
"I don't know" (raw strings sent to Python)
    │
    │  stanza.Pipeline.__call__() [Python/PyTorch neural models]
    │      ├── neural tokenizer: splits "don't" → [do, n't]
    │      ├── tokenize_postprocessor [Python callback — Layer 1]
    │      │       merges splits, annotates contractions with True/False
    │      ├── MWT model: expands (don't, True) → do + n't with Range IDs
    │      ├── POS model: PRON, AUX, PART, VERB
    │      ├── lemma model: I, do, not, know
    │      └── depparse model: subj, aux, advmod, root
    ▼
UdWord JSON (Stanza's output)
    │
    │  map_ud_sentence() [Rust — Layer 2]
    ▼
%mor: pron|I-Prs-Nom-S1 aux|do-Fin-Ind-Pres-S2~part|not verb|know-Inf
%gra: 1|4|SUBJ 2|4|AUX 3|2|NEG 4|0|ROOT
```

The Python callback (Layer 1) sits inside the Stanza call because that is the
only point where we can influence tokenization.  Once Stanza has produced its
UD output, the Python layer is done — Rust takes over for all language-specific
morphosyntax generation.

**Rule of thumb**: If the decision affects *what tokens Stanza sees*, it belongs
in the Python callback (Layer 1).  If the decision affects *how Stanza's UD
output maps to CHAT %mor*, it belongs in Rust (Layer 2).

---

## Validation Results

Side-by-side on Brown/Eve 010600a.cha after implementation:

| Metric | Python master | Rust (before) | Rust (after) |
|--------|--------------|---------------|--------------|
| MWT `~` joins on %mor | 644 | 1 | 298* |

\* The count difference (644 vs 298) is because Rust counts unique
%mor lines with `~`, while Python master's count includes duplicates
from repeated contractions.  The actual MWT expansion coverage matches.

Example output:
```text
Input:   *CHI: I don't know .
%mor:    pron|I-Prs-Nom-S1 aux|do-Fin-Ind-Pres-S2~part|not verb|know-Inf .
```

---

## Code References

### Layer 1 — Python (Stanza Tokenizer Callback)

| Component | File | Description |
|-----------|------|-------------|
| MWT allowlist | `worker/_stanza_loading.py` | `MWT_LANGS` set — languages that use MWT tokenizer mode |
| Stanza config builder | `worker/_stanza_loading.py` | `load_stanza_models()` — chooses tokenizer mode, wires postprocessor |
| MWT contraction rule | `inference/_tokenizer_realign.py:_is_contraction()` | Replicates Python master ud.py:680–685: English+apostrophe → True |
| Tokenizer realignment | `inference/_tokenizer_realign.py:_realign_sentence()` | Character-position merging; merged tokens get `(text, bool)` tuples |
| Postprocessor factory | `inference/_tokenizer_realign.py:make_tokenizer_postprocessor()` | Creates the Stanza callback; captures `alpha2` in closure |
| Batch callback | `inference/morphosyntax.py:219-224` | `batch_infer_morphosyntax()` sets/clears `TokenizerContext.original_words` |

### Layer 2 — Rust (UD → %mor/%gra)

| Component | File | Description |
|-----------|------|-------------|
| MWT grouping | `sentence_mapping.rs:map_ud_sentence()` | `UdId::Range` groups MWT components under one CHAT word index |
| Clitic assembly | `mapping_helpers.rs:assemble_mors()` | Joins MWT components with `~` (post-clitic) or `$` (pre-clitic) |
| POS mapping | `mor_word.rs:map_ud_word_to_mor()` | UD UPOS → CHAT category; `clean_lemma` with empty-string fallback |
| English rules | `lang_en.rs` | Irregular verbs (200+), suffix patterns per POS |
| French rules | `lang_fr.rs` | Pronominal clitics, APM, case agreement |
| Japanese rules | `lang_ja.rs` | Verb conjugation (140+ patterns) |
