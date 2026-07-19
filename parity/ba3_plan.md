# implementation plan

## todo
- [ ] Runs typed pre- and post-stage gates and treats pre-serialization failure as a hard error.
- [ ] Fixes single-word replacement retraces, `@Media` hashing, parser recovery spans, and Japanese token merging.
- [ ] Fixed BA3 output passes `chatter validate`, making the validator the final output-integrity gate.
- [ ] Expands compound fillers and recovers their audio spans between recognized words.
- [ ] Detects long-file drift and monotonicity violations, then re-anchors or repairs timing. | FA recovery and repair passes
- [ ] Integrates Cantonese FA through Jyutping and wav2vec2 (no need for the common recovery layer).
- [ ] Fails unsupported primary languages per file instead of silently skipping them.
- [ ] Synthesizes non-analyzable special forms such as `@q` and `@n` from typed `form_type` data.
- [ ] Rolls back invalid L2 splices, skips `NoAlign` words, and provides a fallback harness where secondary Stanza coverage is absent.
- [ ] Detects bogus Stanza lemmas, missing fields, and invalid analyses, preserving the surface form and recording anomalies. 
- [ ] Applies numbered, retireable Stanza workarounds for Italian compound imperatives and related defects.
- [ ] Corrects known English transcribe patterns, Chinese bogus lemmas, and Japanese token merging.
- [ ] Preflights large Rev.AI batches up front instead of submitting every file independently. (Please do this without moving revai to rust; keep it in Python.)
- [ ] Avoids Whisper MPS `bfloat16` crashes on Apple Silicon. 
- [ ] Coordinates transcribe memory, splits work safely, and prevents OOM-created zombie workers.
- [ ] Supports Qwen3-ASR plus Qwen3-ForcedAligner and fixes HK_QWEN backend/type-stub integration.
- [ ] Propagates utterance metadata to children and keeps `ReplacedWord` atomic across splits.
- [ ] Uses a sliding dispatch window so huge ASR input lists do not become huge in-flight sets.

## done

### example change title
- **component**: which crate, component, etc.
- **summary**: one-line summary
- **input example**: /file/here
- **tbt output example**: /file/output/example
- **ba3 output, pre-edit**: /file/output/example
- **ba3 output, post-edit**: /file/output/example
- **depends on**: [any commit hash]
- **commit**: [commit hash for the change]

discussion here, notes, things for me to review
