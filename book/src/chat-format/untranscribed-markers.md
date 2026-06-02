# Untranscribed Markers: `xxx`, `yyy`, `www`

**Status:** Reference
**Last updated:** 2026-05-11 23:21 EDT

CHAT reserves three short word-level markers for material the human
transcriber cannot or chose not to render as words on the main tier.
Each one has a specific meaning. Tools that emit CHAT — including ASR
pipelines, format converters, and editor heuristics — must respect
those meanings, because every downstream consumer (researchers,
validators, and aggregate-statistics tools like CLAN's `freq`,
`kideval`, `mlu`) reads them at face value.

| Marker | Meaning | Emitter |
|---|---|---|
| `xxx` | Transcriber listened to the audio and **could not make out** what was said. The speech is unintelligible to the human ear at this point. | Human transcriber only. |
| `yyy` | Transcriber heard a discrete utterance but could not write it as ordinary CHAT words. Used when the surface form resists orthography (mumbled, slurred, foreign with no equivalent). The phonetic content typically appears on the `%pho` tier. | Human transcriber only. |
| `www` | Transcriber chose **not to transcribe** this stretch — usually for privacy, off-topic content, or because the segment is irrelevant to the corpus's purpose. | Human transcriber only. |

The shared property: each marker is the human transcriber telling
later readers something specific about *their experience listening to
the audio*. None of them mean "tooling could not process this token".

## Why this matters

When a researcher loads a CHAT corpus and counts `xxx` occurrences, the
result is a *measure of human listening difficulty*: it tells them how
much of the audio resisted human transcription. That number feeds into
methodology decisions ("can we get reliable MLU from this corpus?",
"what's the noise floor on this child's speech?", "should we re-record
in a quieter environment next time?"). It is a load-bearing signal in
language-development research.

If an ASR pipeline emits `xxx` whenever it can't sanitize a token —
for example, substituting `xxx` for any word that fails CHAT
validation under a strict language profile — every `xxx` count in the
corpus becomes a meaningless mixture of "human couldn't tell" and
"pipeline gave up". Researchers then reading those counts are silently
misled. The signal is destroyed for the entire history of that
corpus, because the corruption is indistinguishable from real
unintelligibility once committed.

The same reasoning applies to `yyy` and `www`. A converter or
post-processor that emits any of these three markers because *the
tooling* couldn't handle a token is committing semantic vandalism
against the whole field.

## Rules for tooling

1. **Never emit `xxx`, `yyy`, or `www` from a tool to mean "could not
   process".** These markers are reserved for human transcriber
   judgment.
2. **When a token cannot be validated as legal CHAT under the
   declared language**, prefer one of:
   - Pass the token through verbatim and let the CHAT validator
     (or CLAN's `check`) flag it for human review. The transcriber
     listens, decides, and corrects.
   - Fail loud — abort the file rather than emit corrupted output.
   - Apply only purely orthographic, semantically null repairs
     (e.g., stripping a stray boundary quote mark from `"My`). These
     are safe because no information is lost.
3. **Never sanitize a token by replacing it with one of the three
   markers.** That is exactly the corrupting behavior this document
   prohibits.
4. **Never delete a token to "fix" a validation failure.** Deletion
   loses data without any flag.

## What ASR pipelines should do instead

The `transcript_from_asr_utterances` gate (defined in
`crates/core/talkbank-transform/src/asr_postprocess/cleanup.rs`, exercised
by the pipeline assembled in
`crates/core/talkbank-transform/src/build_chat/`) demonstrates the right
pattern:

1. The pipeline silently fixes only orthographically inarguable
   problems (Stage 2c boundary-quote strip).
2. For tokens that fail language-level validation but are
   structurally legal CHAT (e.g., `C-3PO` under English: tree-sitter
   accepts the digit-hyphen compound but `Word::validate` fires E220
   "numeric digits not allowed"), the gate falls back to the
   structural-only construction path. The token ships verbatim. The
   downstream full-file validator and `check` fire E220 on the same
   word, the file ends up in the human review queue, and the
   transcriber listens to the audio and decides what was actually
   said.
3. For tokens that fail *structural* parsing (tree-sitter rejects),
   the gate fails loud — emitting malformed CHAT would corrupt the
   file beyond the validator's ability to flag it.

The division of labor is: ASR fixes only what's mechanically
unambiguous; CHECK and the human transcriber handle everything that
requires judgment about what the speaker said.

## Related rules

- `xxx` / `yyy` / `www` survive the transcript through all NLP passes
  (morphotag, utseg, translate, coref) without re-interpretation.
  Tools that walk the AST treat them as opaque tokens — they have no
  POS tag, no lemma, no dependency parent, no translation.
- `%wor` excludes all three (no phoneme sequence to align). `%pho`
  may reference `yyy` directly because the phonetic content is the
  whole point of the marker.
- See `word-syntax.md` for grammar; this document is the policy
  reference for *who is allowed to emit them and why*.
