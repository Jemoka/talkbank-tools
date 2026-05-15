# Malayalam Language Support

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

Malayalam (`mal`) is supported in **transcribe-only** mode. Stanza
ships no processor packages for Malayalam, so utseg / morphotag /
forced alignment via Stanza are unavailable. ASR runs via the
`whisper_hub` engine using a HuggingFace Whisper fine-tune.

This page covers Malayalam concretely; the same pattern applies to
the other Indo-Aryan / Dravidian / Austronesian languages whose
entries in Stanza's `resources.json` are charlm/lang_name stubs
without processor packages — see the Stanza capability table at
worker startup for the authoritative current list.

## Quick Reference

| Pipeline Stage | Malayalam-Specific Behavior |
|---------------|---------------------------|
| ASR | HuggingFace Whisper fine-tune via `--engine-overrides '{"asr":"whisper_hub"}'` |
| Text normalization | None — Malayalam script passed through as-is |
| Number expansion | **Rust-side `NUM2LANG` table.** Digits like `"3"` reach CHAT as `"മൂന്ന്"` and pass E220. See [Number Expansion](../../architecture/number-expansion.md) for the dispatch path; Malayalam is registered in the per-language coverage matrix on that page. |
| Retokenize | Not applied — retokenize maps a Stanza-tokenized word list back to ASR tokens; without a Stanza pipeline there is nothing to map. |
| Morphosyntax | **Not available** — Stanza ships no Malayalam pipeline |
| Utseg | **Not available** — same reason |
| Forced alignment | Stock Wave2Vec MMS works on the audio; no Malayalam-specific preprocessing |

## Running Transcribe

```bash
batchalign3 transcribe --lang mal \
    --engine-overrides '{"asr":"whisper_hub"}' \
    audio_dir/ -o output_dir/
```

The `whisper_hub` engine routes to whichever HuggingFace Whisper
fine-tune the model resolver picks for `mal`. The output CHAT file
contains a single main tier with the transcribed Malayalam text and
a media reference; no `%mor`, no `%utseg`, no per-word timing.

### Recommended model

`thennal/whisper-medium-ml` produces clean output on the test sample
the Whisper-Hub integration was validated against. Other Malayalam
fine-tunes exist on HuggingFace; the resolver in
`python/batchalign/models/resolve.py` accepts an explicit override if a
different checkpoint is preferred.

## Why no morphotag / utseg

Stanza's `resources.json` contains a Malayalam entry, but only with
`backward_charlm`, `forward_charlm`, and `lang_name` fields — no
`packages` key listing tokenizer / POS / lemma / depparse models.
The Python worker's `UnsupportedLanguageError` preflight catches
this before `stanza.Pipeline()` runs and the optional sub-stages
in transcribe are dropped from the plan at job submission.

Adding morphotag / utseg for Malayalam would require either Stanza
upstream shipping Malayalam pipelines, or training and integrating
a non-Stanza tagger — neither is in scope.

## Why a fine-tune instead of stock Whisper

Stock multilingual Whisper does include a Malayalam language ID and
can transcribe Malayalam audio, but accuracy on extended Malayalam
recordings is generally well below what a Malayalam-specific fine-tune
delivers. The `whisper_hub` engine exists specifically to route
audio to such fine-tunes when the user requests it.

For languages where stock Whisper is already strong (English,
Spanish, etc.), `whisper_hub` is unnecessary — the default
`--asr-engine whisper` is fine.

## Related languages

This transcribe-only mode applies to other languages where Stanza
has no processor packages. Check at worker startup which codes the
capability table reports as supported; codes outside that set
follow the same Malayalam pattern (transcribe via stock Whisper or
a fine-tune; no Stanza-driven analysis).

For context on Indo-Aryan / Dravidian languages that **do** have
full Stanza support — Tamil (`tam`), Hindi (`hin`), Urdu (`urd`),
Telugu (`tel`), and Thai (`tha`) ship complete processor packages
in current Stanza — morphotag and utseg work the standard way,
no `whisper_hub` needed. The exact set varies by Stanza version;
the worker's capability table at startup is authoritative.

## Resolved issues

### E220 on Whisper digit emissions (resolved)

HuggingFace Whisper fine-tunes for Malayalam (including
`thennal/whisper-medium-ml`) transcribe spoken numbers as Arabic
digits ("3", "100") rather than Malayalam script. Pre-fix, CHAT
validation rejected these with E220 because `mal` is not in the
digit-allowed language allowlist
(`talkbank-tools/crates/talkbank-model/src/validation/word/language/digits.rs`,
which permits digits only for `zho`, `cym`, `vie`, `tha`, `nan`,
`yue`, `min`, `hak`), and the Python `num2words` library has no
Malayalam backend.

**Fix:** added a Malayalam entry to `NUM2LANG` in
`crates/batchalign/data/num2lang.json` covering 0-20,
decades 30-90, plus 100/1000 anchor words. The per-word Rust pass
in `crates/batchalign/src/pipeline/transcribe.rs::prepare_asr_chunks`
calls `expand_number(text, "mal")` on every word, converting digits
to their Malayalam-script word forms. The Python IPC path no longer
exists — Malayalam expansion is end-to-end Rust. Tests at
`crates/batchalign/src/asr_postprocess/num2text.rs`
(`malayalam_*`) lock in the expected expansions.

Higher-magnitude numbers (4-digit and beyond) are decomposed by
`decompose_with_table` greedily against the anchor entries; if the
table can't fully decompose, the original digit string is returned
(matching every other language without an exhaustive table).

## Operational notes

- Audio files of arbitrary length are supported; ASR processes
  in 25-second chunks via the HuggingFace pipeline.
- A `max_new_tokens=444` cap is applied to fine-tune generations
  to prevent the rare runaway-decoder state where a model fails to
  predict end-of-utterance and would otherwise hang.
- Output CHAT can be edited by hand and re-run through other
  TalkBank tooling; the absence of `%mor` / `%utseg` does not
  affect downstream tools that don't require them.
