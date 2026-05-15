# Rev.AI Language Quality Strategy

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Background

Rev.AI's transcription API advertises support for ~70 languages via an
ISO-639-1 language hint parameter. In practice the quality of those models
varies dramatically: some are production-grade (English, Spanish, French),
some are usable-but-noisy, and some return output that is unusable for any
downstream CHAT pipeline.

The failure mode we care about is not "ASR accuracy is mediocre" — CHAT can
tolerate many transcription errors and still be useful. The failure mode is
*output that cannot be represented as CHAT at all*: tokens containing Unicode
replacement characters (U+FFFD), tokens in the wrong script entirely, bare
punctuation returned as if it were a word, digit sequences mixed into
alphabetic tokens. These violate CHAT word-legality rules (E220, E330,
etc.) at the final "belt after the braces" validator in
`transcript_from_asr_utterances`, and the user sees confusing per-token
validation errors with no way to tell that the ASR backend, not the
transcript, is the source of the problem.

### Observed failure mode

A short Malayalam audio sample submitted to Rev.AI with `language=ml`
returned text elements comprising:

- Hangul characters mixed with Malayalam vowel signs (e.g. `모두െ`).
- A long tail of bare Gurmukhi/Punjabi tokens (`ਅਤੁਂਦੇ`, `ਵਾਲੇ`, …) —
  an entirely unrelated script.
- Stray Latin words posing as Malayalam (`occurrence`, `Moo`, `Take`,
  `Me`, `ganhar`, `segueiasm`).
- Cyrillic fragments (`анти`).
- U+FFFD replacement characters embedded in tokens (`);�`,
  `philan�ുടഖ഻ിറ്`, `ക�антиച്`).
- Bare punctuation as a "word" (`);�`).

None of this is usable. The audio content was coherent Malayalam; Rev.AI's
Malayalam model produced the garbage. Our language mapping
(`try_revai_language_hint`) was correct — `mal → ml` — so this is not a
client-side bug.

The downstream CHAT validator (`ChatWordText::try_from_lang` via
`asr_postprocess`) correctly refuses these tokens, but the user-visible
result is a cryptic `[E220] "611었" is not a legal word in language(s)
"mal"` rather than an actionable "Rev.AI's Malayalam model is broken; use
`--asr-engine whisper`."

## Problem

Given no published quality manifest from Rev.AI, how should batchalign3
decide which `(engine, language)` pairs to accept, which to reject, and
how to tell users which alternative to use?

## Options considered

The options below are ordered from cheapest to most principled. Each has a
distinct cost / latency / coverage tradeoff.

### Option A — Hand-curated deny-list (reactive)

Maintain a small, committed static table in `revai/preflight.rs` listing
`(language, reason, recommended_engine)` tuples for pairs we have observed
to be broken. `validate_language_support()` rejects matching job
submissions at preflight with an error message naming the recommended
alternative and linking to this doc.

- **Cost:** trivial — one static, one branch, one doc entry per incident.
- **Latency to catch a bad pair:** unbounded. Depends on a user reporting
  the breakage.
- **Coverage:** only pairs we have already been burned by.
- **Succession friendliness:** high — the table is in git with dated
  provenance comments; the successor understands the shape in minutes.
- **Risk:** the static table becomes stale if Rev.AI *fixes* a language
  and we never retest (a previously-broken pair would remain denied).

### Option B — Runtime script-coherence gate (per-file backstop)

After Rev.AI returns tokens for a job, but before CHAT assembly, run a
pure function `check_asr_script_coherence(tokens, lang)` that looks up the
declared language's canonical script and fails the file with a typed
`AsrQualityError` if the token distribution contradicts it (too many
tokens in the wrong script, or any U+FFFD in any token). No-op for Latin-
script languages (English legitimately code-switches).

- **Cost:** ~100 LOC plus a 16-entry script table. No external resources.
- **Latency:** catches bad pairs on the *first* affected file — no waiting
  for a bug report.
- **Coverage:** catches only *cross-script* failures. Rev.AI can still
  return same-script gibberish (wrong-word-in-right-script) that this
  gate would pass but a human would reject.
- **Succession friendliness:** medium — the threshold tuning (what
  fraction counts as "cross-script") is a judgment call that ages poorly.
- **Risk:** false positives on real code-switching transcripts.

### Option C — Empirical capability probe (Stanza-parallel)

Periodic harness that submits a small reference audio clip (e.g. from
Mozilla Common Voice) in each Rev.AI-supported language, scores the
result on proxies that don't require gold transcripts (script coherence,
U+FFFD rate, CHAT-legality pass rate, token-length distribution), and
emits a committed `revai_language_quality.json` table. Preflight loads
the table at startup and rejects pairs classified `broken`, with a
provenance field showing when the classification was last confirmed.

This mirrors how Stanza per-language processor availability is computed
at worker startup from `resources.json` (`_stanza_capabilities.py`) —
Rev.AI has no such upstream manifest, so we produce our own.

- **Cost:** high — reference corpus procurement, harness implementation
  (~1–2 days), recurring Rev.AI API spend (small per run, multiplicative
  over time), one operator-day per scheduled refresh.
- **Latency:** catches bad pairs at the next refresh interval (weeks).
- **Coverage:** every Rev.AI-supported language, re-verified on cadence.
  Detects both breakage *and* recovery — a language Rev.AI fixed gets
  automatically un-denied.
- **Succession friendliness:** high — the procedure is documented, the
  output is data-in-git, the decision rule is mechanical.
- **Risk:** reference audio may not match real-world acoustic conditions;
  proxies may miss same-script gibberish just like Option B.

### Option D — Require explicit opt-in for non-characterized languages

Reject *every* Rev.AI language we haven't explicitly characterized,
requiring an `--accept-rev-ai-quality-risk` flag to submit. Users
self-select into accepting unknown-quality output.

- **Cost:** trivial in code, high in UX friction.
- **Latency:** zero — everything we haven't cleared is denied.
- **Coverage:** total, at the cost of blocking legitimate use of
  languages we just haven't tested yet.
- **Succession friendliness:** poor — the friction pushes users off
  Rev.AI entirely rather than generating the signal we'd use to
  characterize more languages.
- **Risk:** users stop reporting issues because the flag makes breakage
  "their fault."

### Option E — Consume Rev.AI's advertised list as-is (status quo)

Do nothing. The `try_revai_language_hint` table continues to translate
ISO-639-3 codes to Rev.AI codes and submits whatever the user asked for.
Breakage surfaces as E220 / E330 validation errors on individual tokens.

- **Cost:** zero.
- **Latency:** infinite (nothing is caught).
- **Coverage:** none.
- **Succession friendliness:** poor — a successor debugging a
  Malayalam transcribe failure will not know to look at Rev.AI quality
  rather than our CHAT validator.
- **Risk:** recurring confused-user incidents.

## Decision

**Adopt Option A now. Keep Options B and C on the table as the
escalation path.**

Rationale:

1. We have exactly one data point today (Malayalam). Building a probe
   harness (Option C) to characterize a single known-broken language
   has a poor cost/benefit ratio, and a runtime gate (Option B) without
   a validated script-coherence threshold risks false positives on
   real code-switching.
2. Option A costs ~15 lines of code and one doc update. It directly
   resolves the reported incident.
3. The static table carries provenance comments, so a successor
   reads each entry's rationale and understands why it exists and
   when it should be re-evaluated.

### Escalation triggers

Move from Option A to Option B (add the runtime gate) when:

- We accumulate ≥3 reported bad `(engine, language)` pairs and want
  to catch novel regressions on the first file rather than the first
  report.

Move from Option A/B to Option C (build the probe harness) when:

- Rev.AI changes its model or adds/removes supported languages
  (announced via their changelog) and we need re-characterization on
  a cadence rather than ad-hoc.
- We extend this policy beyond Rev.AI to Whisper / Tencent / Aliyun
  and the per-backend manual curation cost exceeds one engineer-day
  per quarter.
- A downstream user (an external professor, a successor) asks "which
  languages does this actually work on?" and our only answer is
  "whichever ones we haven't gotten a bug report for."

## Implementation notes (Option A)

- Static table lives in `crates/batchalign/src/revai/preflight.rs`.
- Preflight hook is `validate_language_support()` in
  `crates/batchalign/src/types/request.rs`, in the existing Rev.AI
  block immediately after the `try_revai_language_hint(lang).is_none()`
  check.
- Error message names the offending language, explains the quality
  reason, recommends a specific alternative engine, and links to this
  doc.
- Every entry carries a dated provenance comment with the incident date,
  a one-line description of what Rev.AI returned, and a link to the
  evidence (kept in an operational workspace — never inline in this
  public repo).

### Recommended alternative: `whisper_hub`

The deny-list error message currently recommends `--asr-engine whisper_hub`
for languages where Rev.AI is broken. This recommendation presumes
empirical evidence that a community fine-tune works for that language.
See [`whisper-hub-asr.md`](whisper-hub-asr.md) for the engine, its
per-language default model table, and the evidence behind each
recommendation.

When adding a new deny-list entry, confirm first that the recommended
alternative actually works on a real sample — else you're redirecting
users from one broken engine to another. If no working alternative
exists yet for the offending language, the deny-list entry should name
that fact explicitly in the error message rather than point at a random
fallback.

## Cross-references

- Language code mapping: [`language-code-resolution.md`](language-code-resolution.md)
- Whisper (the current recommended alternative): [`whisper-asr.md`](whisper-asr.md)
- Stanza capability model (the analogy for Option C): see
  `python/batchalign/worker/_stanza_capabilities.py` and
  [`stanza-limitations.md`](stanza-limitations.md).
