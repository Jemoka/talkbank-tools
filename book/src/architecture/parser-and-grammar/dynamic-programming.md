# Dynamic Programming

**Status:** Current
**Last updated:** 2026-05-01 17:07 EDT

Where dynamic programming is used at runtime across the workspace,
which uses are intrinsically necessary, and which are
correctness-critical even when avoidable. A policy test enforces
that no new runtime DP appears outside the allowlisted call sites.

## Inventory

| Area | Call site | DP algorithm | Notes |
|---|---|---|---|
| Whisper ASR timestamp extraction | `python/batchalign/inference/audio.py` | DTW (`_dynamic_time_warping`) | Maps decoder tokens to audio frames from cross-attention matrices |
| Whisper FA token timing | `python/batchalign/inference/fa.py` | DTW | Token jump times extracted in Python, mapped in Rust |
| Wave2Vec forced alignment | `python/batchalign/inference/fa.py` | CTC forced alignment (Viterbi-style DP) | `torchaudio.functional.forced_align` on emission matrix vs transcript |
| FA word-level remapping | `crates/batchalign/src/chat_ops/fa/alignment.rs::apply_indexed_timings` | **None** | Indexed callback protocol maps timings 1:1 by index |
| FA token-level remapping | `crates/batchalign/src/chat_ops/fa/alignment.rs::align_token_timings` | **None** | Deterministic token→word stitching only; unmatched words remain untimed |
| UTR timing recovery | `crates/batchalign/src/chat_ops/fa/utr.rs::inject_utr_timing` | Hirschberg edit-distance DP | Global alignment of all document words against all ASR tokens |
| Morphosyntax retokenize mapping | `crates/talkbank-transform/src/retokenize/mapping.rs::build_word_token_mapping` | **None** | Deterministic span-join + length-aware monotonic fallback |
| WER evaluation | `crates/talkbank-transform/src/dp_align/` + `wer_conform.rs` | Hirschberg edit-distance DP | Canonical use of DP for transcript comparison |
| Compare command | `crates/batchalign/src/compare.rs` via `dp_align::align` | Hirschberg edit-distance DP | Aligns main vs gold transcript words to compute WER and inject `%xsrep` / `%xsmor` tiers |

## Classification

### Intrinsic — DP is the algorithm

- **WER evaluation / compare command.** Comparing two independent
  word sequences is exactly edit-distance territory.
- **CTC forced alignment** (`forced_align`). DP is intrinsic to the
  model family.
- **Whisper DTW path.** If cross-attention DTW is the chosen
  alignment method, DP is part of the method.

### Architecturally avoidable — removed from runtime

- **FA word/token remapping.** Removed; indexed or deterministic
  stitching only.
- **Retokenize char-level DP.** Removed; deterministic span-join
  with length-aware monotonic fallback.

### Correctness-critical — DP retained on purpose

- **UTR global ASR → transcript DP.** UTR has to align two
  independent full-document word sequences (transcript words and
  ASR tokens). A local/windowed matcher can starve later
  utterances of tokens that earlier utterances consumed — exactly
  what happened in the 407-style hand-edited transcript regression.
  UTR uses a single global Hirschberg alignment for this reason.
  Same category as WER/compare, not the avoidable-runtime-remap
  category.

```mermaid
flowchart TD
    input["Transcript words + ASR words"]
    cheap{"Unique exact subsequence?"}
    exact["Cheap monotonic mapping\n(no DP)"]
    global["Single global Hirschberg DP\n(correctness baseline)"]
    output["UTR timing injection"]

    input --> cheap
    cheap -->|yes| exact --> output
    cheap -->|no| global --> output
```

UTR is a monotonic aligner. Dense overlap and text/audio
reordering can still leave words unmatched in heavily reworked
hand-edited transcripts; that limitation is inherent to monotonic
DP.

## Hirschberg Optimizations

The `dp_align/` implementation in `talkbank-transform` includes
two optimizations beyond the textbook algorithm:

- **Prefix/suffix stripping.** Before entering the O(mn) DP core,
  `align()` strips matching prefixes and suffixes in O(n). For the
  primary use case (WER / transcript comparison, 80–95% accuracy),
  reduces effective DP problem size 10–100×. Only the differing
  middle portion enters Hirschberg recursion.
- **Generic `Alignable` trait.** Both `String` (word-level) and
  `char` (character-level) entry points share one generic
  implementation. Monomorphization eliminates ~200 lines of
  duplicated code with zero runtime overhead.

## Fuzzy Matching

The DP aligner supports three word-comparison modes via `MatchMode`:

```mermaid
flowchart LR
    words["Word pair:\ntranscript 'gonna'\nASR 'gona'"]

    subgraph Exact["MatchMode::Exact"]
        exact_cmp["'gonna' == 'gona'?"]
        exact_no["NO match\ncost = 2 (substitution)"]
    end

    subgraph CaseInsensitive["MatchMode::CaseInsensitive"]
        ci_cmp["'gonna' =~ 'gona'?\n(case-insensitive)"]
        ci_no["NO match\ncost = 2"]
    end

    subgraph Fuzzy["MatchMode::Fuzzy threshold=0.85"]
        fast["Fast path:\nexact case-insensitive?"]
        jw["Jaro-Winkler similarity:\nJW('gonna','gona') = 0.95"]
        threshold{"0.95 >= 0.85?"}
        fuzzy_yes["YES match!\ncost = 0"]
    end

    words --> exact_cmp --> exact_no
    words --> ci_cmp --> ci_no
    words --> fast -->|no| jw --> threshold -->|yes| fuzzy_yes
```

Jaro-Winkler compares two strings by counting matching characters
within a distance window, penalizing transpositions, and boosting
for common prefixes. Returns 0.0 (different) to 1.0 (identical).
Better than Levenshtein for short words because it doesn't
penalize length differences as harshly.

Without fuzzy matching, a single ASR substitution ("gonna" vs
"gona") forces the DP to treat it as a gap (cost 2) rather than a
match (cost 0). This can cascade — the mismatched word shifts
subsequent alignments, potentially leaving entire utterances
unmatched. With fuzzy matching, the substitution is recognized as
a match, keeping the alignment anchored.

The default threshold (0.85) is tuned empirically across 6 corpora
and 59 files. Examples at this threshold:

| Pair | JW | Match at 0.85? |
|---|---|---|
| "gonna" / "gona" | 0.95 | Yes |
| "went" / "wen" | 0.94 | Yes |
| "yesterday" / "yestarday" | 0.92 | Yes |
| "mhm" / "mmhm" | 0.83 | No |
| "the" / "da" | 0.00 | No |
| "he" / "she" | 0.61 | No |
| "cat" / "dog" | 0.00 | No |

## UTR Two-Pass Overlap-Aware Alignment

When a file has overlap markers (`+<` linkers or CA `⌊` markers),
UTR uses a two-pass strategy to handle backchannel timing
recovery.

```mermaid
flowchart TD
    file["CHAT file with overlaps"]
    density{"Overlap density\n> 30%?"}

    subgraph Pass1["Pass 1: Global DP"]
        exclude["Exclude overlap utterances\nfrom word sequence"]
        include["Include ALL utterances\n(density too high to exclude)"]
        global["Hirschberg DP alignment\n(fuzzy matching 0.85)"]
    end

    subgraph Pass2["Pass 2: Backchannel Recovery"]
        find_pred["Find predecessor utterance"]
        ca_check{"Predecessor has\nCA marker ⌈?"}
        full_window["Search full predecessor\ntime range"]
        narrow_window["Narrow search to\nonset position ± buffer"]
        windowed_dp["Small windowed DP\non ASR tokens in window"]
    end

    subgraph Fallback["Best-of-both"]
        compare{"Two-pass creates\nfewer FA groups?"}
        keep["Keep two-pass result"]
        revert["Revert to global result"]
    end

    file --> density
    density -->|"no (≤ 30%)"| exclude --> global
    density -->|"yes (> 30%)"| include --> global
    global --> find_pred --> ca_check
    ca_check -->|yes| narrow_window --> windowed_dp
    ca_check -->|no| full_window --> windowed_dp
    windowed_dp --> compare
    compare -->|"no (equal or more)"| keep
    compare -->|"yes (fewer groups)"| revert
```

### Configurable parameters

| Flag | Default | Controls |
|---|---|---|
| `--utr-strategy` | `auto` | `auto` (detect overlaps) / `global` / `two-pass` |
| `--utr-ca-markers` | `enabled` | Use ⌈⌉⌊⌋ for onset windowing |
| `--utr-density-threshold` | `0.30` | Max overlap fraction before skipping exclusion |
| `--utr-tight-buffer` | `500` | Pass-2 tight window ±ms |
| `--utr-fuzzy` | `0.85` | Jaro-Winkler similarity threshold |

### CA marker onset estimation

When a predecessor utterance has ⌈ markers, the proportional
position of ⌈ among the utterance's words estimates when overlap
begins:

```mermaid
flowchart LR
    utt["*SPK: no pues de lo que sea ⌈tengo media hora⌉\n12660_15585"]
    frac["⌈ at word 7 of 10\nfraction = 0.6"]
    onset["onset = 12660 + 0.6 × (15585-12660)\n= 14415ms"]
    window["Search window:\n14415 ± 500ms\n(vs full 12660-15585)"]

    utt --> frac --> onset --> window
```

Narrows the pass-2 search window from the full predecessor range
(~3 seconds in this example) to ~1 second around the estimated
onset — roughly a 3× reduction in search space.

## Known DP Failure Modes

1. **Crossing alignments / rapid overlaps.** Global monotonic
   aligners cannot represent crossing matches; one side is dropped
   or mis-assigned.
2. **Repeated-token ambiguity.** Repeated words create many
   equal-cost paths; deterministic tie-breaks may pick semantically
   wrong matches.
3. **ASR drift and hallucinations.** Large payload/reference
   divergence causes sparse matches and low timing coverage.
4. **Tokenization or normalization mismatch.** Char-level DP may
   align surprising spans when punctuation/case/tokenization
   differ.
5. **Temporal validity vs textual order.** Correct per-utterance
   times can still violate CHAT monotonicity when transcript order
   diverges from audio order.

### Mitigations in code

- **Monotonicity enforcement (E362)** — `enforce_monotonicity()`
  strips timing from regressions after FA.
- **Same-speaker overlap enforcement (E704)** —
  `strip_e704_same_speaker_overlaps()` strips earlier conflicting
  timing.
- **Untimed fallback windows** — proportional boundary estimates
  keep FA from skipping all untimed utterances. See
  [Proportional FA Estimation](../alignment/proportional-fa-estimation.md).
- **Retokenize diagnostics + safe fallback** — invalid token
  mappings keep original words and mark parse taint.

## Allowlist Policy

`batchalign/tests/test_dp_allowlist.py` fails CI if new runtime DP
call sites appear outside the allowlist:

| Call site | Purpose |
|---|---|
| `crates/batchalign-pyo3/src/pyfunctions.rs` | PyO3 bridge (1 call) |
| `crates/batchalign/src/benchmark.rs` | WER (1 call) |
| `crates/batchalign/src/compare.rs` | Transcript comparison (1 call) |
| `crates/batchalign/src/chat_ops/fa/utr.rs` | UTR timing recovery (1 call) |

DP-related behavior is also protected by golden tests:

```bash
uv run pytest batchalign/tests/golden/test_dp_golden.py -m golden -v -n 0
```

For intentional behavior changes:

```bash
uv run pytest batchalign/tests/golden/test_dp_golden.py -m golden -v -n 0 --update-golden
```
