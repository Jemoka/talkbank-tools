# implementation plan

## todo
- [x] Runs typed pre- and post-stage gates and treats pre-serialization failure as a hard error.
- [x] Fixes single-word replacement retraces, `@Media` hashing, parser recovery spans, and Japanese token merging.
- [x] Fixed BA3 output passes `chatter validate`, making the validator the final output-integrity gate.
- [x] Expands compound fillers and recovers their audio spans between recognized words.
- [x] Detects long-file drift and monotonicity violations, then re-anchors or repairs timing. | FA recovery and repair passes
- [x] Integrates Cantonese FA through Jyutping and wav2vec2 (no need for the common recovery layer).
- [x] Fails unsupported primary languages per file instead of silently skipping them.
- [x] Synthesizes non-analyzable special forms such as `@q` and `@n` from typed `form_type` data.
- [x] Rolls back invalid L2 splices, skips `NoAlign` words, and provides a fallback harness where secondary Stanza coverage is absent.
- [x] Detects bogus Stanza lemmas, missing fields, and invalid analyses, preserving the surface form and recording anomalies.
- [x] Applies numbered, retireable Stanza workarounds for Italian compound imperatives and related defects.
- [x] Corrects known English transcribe patterns, Chinese bogus lemmas, and Japanese token merging.
- [x] Preflights large Rev.AI batches up front instead of submitting every file independently. (Please do this without moving revai to rust; keep it in Python.)
- [x] Avoids Whisper MPS `bfloat16` crashes on Apple Silicon.
- [x] Coordinates transcribe memory, splits work safely, and prevents OOM-created zombie workers.
- [x] Supports Qwen3-ASR plus Qwen3-ForcedAligner and fixes HK_QWEN backend/type-stub integration.
- [x] Propagates utterance metadata to children and keeps `ReplacedWord` atomic across splits.
- [x] Uses a sliding dispatch window so huge ASR input lists do not become huge in-flight sets.

## additional differences
- [x] 1/10 Sanitizes CHAT-illegal ASR tokens without failing the transcript.
- [x] 2/10 Suppresses terminator-only translation tiers.
- [x] 3/10 Excludes paired CA segment repetitions from lexical text.
- [x] 4/10 Keeps retraces with their retry across utterance splits.
- [x] 5/10 Projects comparison candidates to one majority utterance.
- [x] 6/10 Attributes matched comparison POS from the gold transcript.
- [x] 7/10 Keeps experimental review tiers off by default.
- [x] 8/10 Skips AppleDouble CHAT sidecars in CLI discovery and align preflight.
- [x] 9/10 Removes wrapped `%mor`/`%gra` tiers without orphan continuations.
- [x] 10/10 Schedules the largest batch inputs first.
- [x] 11/40 Places generated provenance after constant participant headers.
- [x] 12/40 Exposes Apple MPS only through an explicit, warned opt-in.
- [x] 13/40 Refreshes stale same-version Stanza resource catalogs safely.
- [x] 14/40 Makes unsupported-language Stanza UtSeg fallback explicit and functional.
- [x] 15/40 Rescues English contracted copula progressives in `%mor` and `%gra` together.
- [x] 16/40 Makes batch input concurrency explicitly configurable.
- [x] 17/40 Invalidates cached task output when the compiled engine changes.
- [x] 18/40 Keeps experimental two-pass overlap UTR out of automatic selection.
- [x] 19/40 Exposes standalone speaker diarization as deterministic turns JSON.
- [x] 20/40 Collapses known Italian Stanza Defect 6 false MWT expansions.
- [x] 21/40 Repairs the Italian sentence-initial `la` false MWT expansion.
- [x] 22/40 Rewrites the mis-tagged verb head in Italian `dagliela` MWTs.
- [x] 23/40 Canonicalizes the verb lemma in Italian `posala` and `posalo` MWTs.
- [x] 24/40 Preserves CHAT's virtual zero head for `%gra` root relations.
- [x] 25/40 Rejects UD analyses without exactly one dependency root.
- [x] 26/40 Reuses complete exact-match `%wor` timing without rerunning FA.
- [x] 27/40 Rejects reusable `%wor` spans that overrun the next utterance.
- [x] 28/40 Rejects near-zero word spans from FA timing reuse.
- [x] 29/40 Rejects one-word dominance in reusable FA timing.
- [x] 30/40 Rejects backward word order in reusable FA timing.
- [x] 31/40 Clears stale zero-duration authoritative bullets after untimed FA.
- [x] 32/40 Replaces provisional UTR windows with successful FA word spans.

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
- **new**: yes/no (new, not from the original TODO, or old from the above)

Discussion here, notes, things for me to review. Please keep this example block around.

### Fail unsupported primary morphotag languages per file
- **component**: `batchalign-core` morphosyntax task runner
- **summary**: Rejects unsupported primary `@Languages` values before backend dispatch with an actionable per-file validation error; `@Options: CA` remains a legitimate unchanged pass-through.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/input/unsupported-srp.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/tbt-output/cli-output.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/ba3-pre-output/unsupported-srp.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/unsupported-primary-language/ba3-post-output/cli-output.txt
- **depends on**: []
- **commit**: 871c8d4
- **new**: no

The fixture is deliberately `srp`: the local Stanza 1.12 installation can tag it, so pre-edit BA3 reported success and generated `%mor`/`%gra`; the fork intentionally admits only its known-complete static language set and fails the file. Post-edit BA3 now matches that deterministic gate. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors` and the Bazel-backed `just batchalign cli` fixture run.

### Use float32 for CHATWhisper on Apple MPS
- **component**: Python CHATWhisper ASR backend
- **summary**: Selects `torch.float32` before constructing a CHATWhisper pipeline on MPS, avoiding a late `bfloat16` attention crash while preserving the existing `bfloat16` to `float16` fallback on other devices.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/input/device.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/tbt-output/dtype.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/ba3-pre-output/dtype.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/whisper-mps-dtype/ba3-post-output/dtype.txt
- **depends on**: []
- **commit**: c425fe8
- **new**: no

The example isolates the loader policy so it is deterministic on machines without Apple GPU access and does not require downloading a model. Targeted verification: `bazel test //python/batchalign:pytest --test_arg=-q --test_arg=python/batchalign/tests/test_chatwhisper_device.py --test_output=errors`.

### Bound the pipeline dispatch-future window
- **component**: `batchalign-engine` pipeline scheduler
- **summary**: Feeds inputs through an eight-item ordered/unordered stream window instead of eagerly polling or retaining a future for every file; the existing semaphore remains the execution limit and result ordering remains stable.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/input/manifest.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/tbt-output/dispatch.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/ba3-pre-output/dispatch.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/sliding-dispatch-window/ba3-post-output/dispatch.txt
- **depends on**: []
- **commit**: d7c5d49
- **new**: no

The regression blocks 100 synthetic per-input futures immediately after their first poll. Pre-edit scheduling admitted all 100 even though only eight could execute; post-edit scheduling admits exactly eight, then slides forward as slots complete, and still returns `0..99` in input order. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors`.

### Expand compound fillers for FA and restore one source span
- **component**: `batchalign-core` forced-alignment task runner
- **summary**: Sends underscore-separated compound filler parts to FA as independently recognizable words, then consumes and merges their returned timings into one `%wor` word spanning the first recognized onset through the last recognized offset.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/input/compound-filler.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/tbt-output/fa-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/ba3-pre-output/fa-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compound-filler-fa/ba3-post-output/fa-seam.txt
- **depends on**: []
- **commit**: 503525d
- **new**: no

The fixture exercises `&-you_know`: FA now receives `you`, `know`, `today`, while typed injection restores the source word domain as `you_know` at `100_350` and `today` at `400_600`. The same cursor policy applies to original words inside `ReplacedWord`. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors`.

### Lock typed special-form synthesis when Stanza returns no sentence
- **component**: `talkbank-transform` morphosyntax injection
- **summary**: Pins the existing typed synthesis path end to end: an empty Stanza response still maps `@q` to `meta` and `@n` to `neo`, preserves the surface lemma, and emits validator-safe ROOT/PUNCT relations.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/input/special-forms.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/tbt-output/special-forms.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/ba3-pre-output/special-forms.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/special-form-synthesis/ba3-post-output/special-forms.cha
- **depends on**: []
- **commit**: 086894b
- **new**: no

Audit note: runtime parity was already present before this item, so pre- and post-edit semantic output is intentionally identical. The independent commit adds the missing injection-level regression by collecting typed `FormType` payloads, supplying an empty `UdResponse`, and validating the resulting aligned CHAT. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test --test_output=errors`.

### Repair invalid Stanza fields without losing lexical words
- **component**: Python Stanza morphology renderer and backend diagnostics
- **summary**: Normalizes missing or invalid lemma, UPOS, ID, head, and dependency-relation fields before rendering; lexical surfaces fall back to `x|surface`, dependency repairs satisfy the head/ROOT invariant, and every repair is logged with file and utterance identity.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/input/raw-stanza.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/tbt-output/renderer-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/ba3-pre-output/renderer-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-analysis-repair/ba3-post-output/renderer-seam.txt
- **depends on**: []
- **commit**: f2f60d0
- **new**: no

The seam deliberately combines a lexical `hello` with lemma `.`, null UPOS, out-of-range head `99`, and `<pad>` relation. Pre-edit BA3 silently classified it as punctuation and emitted no analysis. Post-edit BA3 preserves it as `x|hello`, repairs a validator-safe ROOT tree, and records four structured warnings. The fork's focused raw-Stanza regression proves its surface-lemma fallback; BA3 additionally repairs the invalid dependency fields instead of carrying them toward a later mapping failure. Targeted verification: `bazel test //python/batchalign:pytest --test_arg=-q --test_arg=python/batchalign/tests/test_morphotag_render.py --test_output=errors`; fork verification: `RUSTUP_TOOLCHAIN=1.95.0 cargo test -p batchalign-transform parse_raw_stanza_bogus_lemma`.

### Apply numbered, retireable Italian Stanza workarounds
- **component**: Python Stanza morphology renderer and Italian compatibility registry
- **summary**: Expands the fork's closed Defects 8, 12, and 13 allowlist into synthetic verb-plus-clitic MWTs, reindexes following dependency heads, records the numbered repair, and leaves every unknown surface unchanged.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/input/raw-stanza.json
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/tbt-output/renderer-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/ba3-pre-output/renderer-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-stanza-workarounds/ba3-post-output/renderer-seam.txt
- **depends on**: []
- **commit**: 21c30bb
- **new**: no

The regression reproduces the confirmed mid-sentence `dammela` shape (`ADJ`, lemma `dammelo`) and pins the full three-chunk output. Separate cases exercise missing-MWT `aprilo` and fabricated-lemma `leggila`; the registry test requires explicit defect numbers and retirement criteria. Targeted verification uses the Bazel-built test executable without the rule's fixed whole-suite argument: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_render.py -q` (11 passed). Fork verification: `RUSTUP_TOOLCHAIN=1.95.0 cargo test -p batchalign test_italian_defect8_dammela` (2 passed).

### Enforce typed gates at every pipeline and serialization boundary
- **component**: `batchalign-engine` pipeline boundary, `batchalign-core` CHAT writer, and typed validation API
- **summary**: Validates every CHAT carried directly, in pairs, or in artifact lists before and after each task; stage violations become per-file failures, and the exact serialized bytes must reparse and pass full validation before any disk write.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/input/stage-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/tbt-output/gate.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/ba3-pre-output/invalid-output.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/typed-stage-validation/ba3-post-output/gate.txt
- **depends on**: []
- **commit**: 6d1aaae
- **new**: no

The seam deliberately mutates an already-validated typed AST after admission by removing its main-tier terminator. Pre-edit BA3 could serialize the invalid transcript; post-edit, the shared pre/post boundary rejects it and `Chat::write` independently reparses the exact output bytes as a final hard gate. The validation helpers are now generic over both CHAT typestates, avoiding a string round-trip inside stage checks. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test //crates/batchalign/batchalign-core:batchalign_core_unit_test //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors`; Bazel product build: `just batchalign build debug`.

### Lock Python-owned Rev.AI batch preflight ordering
- **component**: Python Rev.AI ASR/speaker backend
- **summary**: Pins the existing submit-then-poll path on a 32-file batch: every unique provider job is submitted before polling begins, and duplicate atomic ASR/speaker projections reuse one submission.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/input/manifest.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/tbt-output/order.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/ba3-pre-output/order.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/revai-python-preflight/ba3-post-output/order.txt
- **depends on**: []
- **commit**: c39b3d5
- **new**: no

Audit note: the Python backend already had the intended batched runtime behavior before this item was audited; the independent commit supplies the missing large-batch regression and explicitly prevents a later return to per-file submit-and-wait. Rev.AI remains Python-owned. The hermetic fake makes no provider calls and asserts 32 submit events precede the single poll event. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_revai_preflight.py -q` (1 passed).

### Lock utterance metadata and atomic replacement splitting together
- **component**: typed `talkbank-transform` utterance segmentation
- **summary**: Pins the child-propagation policy for linkers, language code, postcodes, timing bullets, and source spans while a classifier boundary falls inside a multiword replacement; the complete `ReplacedWord` follows its first assignment.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/input/split.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/tbt-output/split-contract.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/ba3-pre-output/split-contract.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-metadata-atomic/ba3-post-output/split-contract.txt
- **depends on**: []
- **commit**: 5ebd4cc
- **new**: no

Audit note: the individual propagation and atomicity rules were already present; the new independent regression exercises their interaction in one typed split. Its five-word morphology domain assigns `I, want` to child 0 and `to, go, now` to child 1, forcing the boundary inside `wanna [: want to]`. The replacement remains wholly on child 0; utterance-scope language and spans reach both children, the prior-turn linker stays first, and end-scope postcode/bullet stay last. Targeted verification: `bazel test --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test --test_output=errors`.

### Repair long-file FA monotonicity before validation
- **component**: `batchalign-core` forced-alignment task runner
- **summary**: Runs a typed timing repair immediately after FA injection: backward utterance anchors lose both their unsafe main bullet and stale `%wor`, while forward anchors whose end crosses the next start are clamped to that start.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/input/timing-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/tbt-output/repair.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/ba3-pre-output/repair.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-long-file-monotonicity/ba3-post-output/repair.txt
- **depends on**: []
- **commit**: 5b412fa
- **new**: no

The synthetic 500-utterance regression includes one ordinary end overlap and one severe backward anchor at utterance 400. The post-pass reports one clamp and one strip, removes `%wor` together with the untrustworthy drifted main bullet, and reparses the complete 500-utterance serialization through the full CHAT validator. This follows the fork's conservative rule: preserve forward-moving conversational overlap starts, but never invent a location for a backward anchor. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors`.

### Romanize Cantonese only at the wav2vec2 alignment seam
- **component**: Python `Wav2Vec2FaBackend`
- **summary**: Converts resolved `yue` source words to tone-free, apostrophe-delimited Jyutping before MMS_FA while preserving the original Hanzi surfaces in returned CHAT; non-Cantonese and unresolved-language inputs retain the shared path unchanged.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/input/alignment-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/tbt-output/alignment-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/ba3-pre-output/alignment-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cantonese-jyutping-fa/ba3-post-output/alignment-seam.txt
- **depends on**: []
- **commit**: 2ad0e3a
- **new**: no

The conversion is local to the direct MMS input seam, so this language-specific route does not depend on the common recovery layer and does not alter typed output words. If the optional Cantonese dependency is absent, the backend reports an actionable extra requirement instead of silently sending Han characters to MMS. Targeted verification uses the Bazel-built test executable: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_cantonese_fa.py -q` (3 passed).

### Prove serialized Batchalign output through the Chatter validator
- **component**: `batchalign-core` final CHAT serialization gate
- **summary**: Locks the final output contract with a representative annotated document containing media, utterance and word bullets, `%mor`, and `%gra`; the exact serialized bytes re-enter the same full validation pipeline used by `chatter validate` before disk I/O.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/input/output-contract.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/tbt-output/annotated.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-pre-output/annotated.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-post-output/annotated.cha
- **depends on**: [6d1aaae]
- **commit**: 21fb255
- **new**: no

The implementation dependency made full reparsing a hard pre-write gate; this independent regression pins a realistic successful output instead of testing only rejection. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_filter=serialized_batchalign_output_passes_chatter_validation_pipeline --test_output=errors` (1 passed). The fixture also passed the actual Bazel-built CLI with `bazel-bin/crates/chatter/chatter-cli/chatter validate --force --tui-mode disable --quiet /Users/houjun/Documents/Projects/talkbank-parity/ba3/chatter-final-gate/ba3-post-output/annotated.cha`.

### Use the typed Qwen class for standalone forced alignment
- **component**: Python Qwen3 ASR/FA backends and public backend surface
- **summary**: Keeps Qwen3-ASR paired with its companion forced aligner for word timestamps, but constructs standalone FA directly as one `Qwen3ForcedAligner`; both backend types remain reachable through the lazy typed package surface.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/input/backend-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/tbt-output/backend-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/ba3-pre-output/backend-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/qwen3-backend-contract/ba3-post-output/backend-seam.txt
- **depends on**: []
- **commit**: a0b8601
- **new**: no

Pre-edit standalone FA constructed `Qwen3ASRModel` from the alignment checkpoint and also passed that checkpoint as its nested `forced_aligner`, loading the same large model twice through the wrong outer class. The locked `qwen-asr` API exposes `Qwen3ForcedAligner.from_pretrained` directly; post-edit uses that supported surface with its correct `dtype` parameter. The regression also pins the ASR companion-aligner argument and the two public re-exports, covering the fork's HK_QWEN/type-stub intent in BA3's backend architecture. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_qwen3_contract.py -q` (3 passed).

### Honor NoAlign before FA work
- **component**: `batchalign-core` FA task runner
- **summary**: Treats `@Options: NoAlign` as strict byte-stable pass-through before media lookup or backend dispatch.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/input/no-align.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/tbt-output/no-align.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/ba3-pre-output/no-align.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-noalign-fallback/ba3-post-output/no-align.cha
- **depends on**: []
- **commit**: b390560
- **new**: no

Pre-edit a NoAlign transcript without media failed sibling lookup, while post-edit it reaches neither media nor a panic-on-call dispatcher and serializes identically. Targeted verification used the Bazel-built executable: `bazel-bin/crates/batchalign/batchalign-core/batchalign_core_unit_test --exact taskrunners::fa::tests::no_align_is_strict_pass_through_without_media_or_dispatch` (1 passed).

### Lock invalid L2 splice rollback snapshots
- **component**: typed L2 morphosyntax splice and Python secondary-language fallback
- **summary**: Pins the transactional fallback contract: an invalid post-splice dependency tree restores the complete host `%mor`/`%gra` snapshots and resets the affected analysis to `L2|xxx`; unavailable secondary Stanza pipelines return empty per-input responses instead of aborting the batch.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-rollback-snapshot/input/invalid-secondary.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-rollback-snapshot/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-rollback-snapshot/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/l2-rollback-snapshot/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: 9fc1358
- **new**: no

Runtime parity was already present, so the pre/post transcript evidence is intentionally identical. The independent audit commit proves the private transaction chokepoint rejects an out-of-bounds `2|99|NMOD` result, restores both typed tier snapshots exactly, and reports `RolledBack`. The existing Python fallback harness separately verifies one unavailable secondary language is attempted once, memoized, and represented by empty responses for every affected input. Targeted verification: `bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact morphosyntax::l2::splice::cardinality_tests::invalid_post_splice_tree_restores_complete_tier_snapshots` (1 passed); `bazel-bin/python/batchalign/pytest python/batchalign/tests/test_stanza_pipeline_cache.py -q` (5 passed).

### Bound decoded-audio admission at each backend route
- **component**: `batchalign-engine` backend dispatcher and transcribe chunking contracts
- **summary**: Replaces the unbounded per-backend channel with an admission-budget-sized queue whose async send applies backpressure; one backend loop continues to own model calls, long BERT inputs use bounded overlapping splits, and route shutdown wakes pending producers rather than leaving model-worker processes behind.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/input/dispatch-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/tbt-output/dispatch-seam.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/ba3-pre-output/dispatch-seam.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/transcribe-memory-admission/ba3-post-output/dispatch-seam.txt
- **depends on**: [d7c5d49]
- **commit**: 04ea516
- **new**: no

A dispatch item may own an entire decoded PCM file, so an unbounded route could defeat the earlier sliding future window when callers dispatch directly or future admission widths grow. Post-edit queue capacity comes from `EngineConfig.max_concurrent_values` (with a safe minimum of one), and closing the route causes blocked `send().await` producers to fail cleanly. BA3 runs Python model objects in-process behind one Rust batcher loop rather than spawning one process per input, so there is no OOM-created child-worker population to orphan. Targeted verification: `bazel build --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test && bazel-bin/crates/batchalign/batchalign-engine/batchalign_engine_unit_test --exact engine::tests::backend_route_capacity_matches_memory_admission_budget` (1 passed); `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_utseg_sliding_window.py -q` (6 passed).

### Exercise replacement retraces inside the Bazel parser target
- **component**: `talkbank-parser` replacement/retrace lowering
- **summary**: Locks the existing parser correction into the actual Bazel unit-test graph: a word carrying both `[: replacement]` and `[//]` lowers to a full `Retrace` containing a `ReplacedWord`, never a bare replacement that pollutes morphology alignment.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/input/retrace.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/tbt-output/shape.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/ba3-pre-output/shape.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/replacement-retrace-bazel/ba3-post-output/shape.txt
- **depends on**: [c90b9bff]
- **commit**: 0b010c0
- **new**: no

Audit note: runtime parity was already supplied by the historical focused fix, and a six-case regression existed under `crates/utils/tests`; however, that integration-test directory is explicitly deferred in `crates/utils/BUILD.bazel` and therefore was invisible to the required Bazel workflow. The new parser-local contract makes the critical AST shape part of `//crates/core/talkbank-parser:talkbank_parser_unit_test`. Targeted verification: `bazel build --config=dev //crates/core/talkbank-parser:talkbank_parser_unit_test && bazel-bin/crates/core/talkbank-parser/talkbank_parser_unit_test --exact parser::tree_parsing::main_tier::content::word::tests::replacement_with_retrace_marker_stays_a_retrace` (1 passed).

### Retain source locations on parser recovery tiers
- **component**: `talkbank-parser` malformed dependent-tier recovery
- **summary**: Carries the CST byte range into recovered empty `%mor` and `%gra` placeholders, including the synthetic morphology terminator, so later validation and regeneration diagnostics point at the malformed source tier instead of byte zero.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/input/recovery-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/tbt-output/spans.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/ba3-pre-output/spans.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/parser-recovery-spans/ba3-post-output/spans.txt
- **depends on**: []
- **commit**: 4578546
- **new**: no

Pre-edit recovery intentionally retained the dependent-tier slot but constructed its tier and morphology terminator with `Span::DUMMY`. Post-edit uses one exact `tree_sitter::Node` range for the recovered typed objects; the fallback remains empty and ordering-preserving, but it no longer fabricates a source location. Targeted verification: `bazel build --config=dev //crates/core/talkbank-parser:talkbank_parser_unit_test && bazel-bin/crates/core/talkbank-parser/talkbank_parser_unit_test --exact parser::chat_file_parser::dependent_tier_dispatch::parsed::tests::recovered_tier_placeholders_keep_source_spans` (1 passed).

### Lock Japanese Stanza splits back to one CHAT word
- **component**: `talkbank-transform` tokenizer realignment
- **summary**: Pins the character-alignment merge for Japanese: Stanza stem/auxiliary splits are recombined to the original CHAT surface and emitted with `expand_mwt=false`, preserving one morphology slot instead of allowing a second expansion.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/input/token-seam.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/tbt-output/tokens.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/ba3-pre-output/tokens.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/japanese-token-merge/ba3-post-output/tokens.txt
- **depends on**: []
- **commit**: a97cdb4
- **new**: no

Audit note: BA3 and the fork already share the same realignment implementation, but neither the generic compound case nor the English contraction case proved Japanese behavior. The focused contract uses `食べちゃう` split as `食べ + ちゃう`, checks exact surface restoration, and prevents accidental English-style MWT expansion. This one seam satisfies the Japanese-merging clause shared by both remaining original checklist lines. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact tokenizer_realign::tests::test_japanese_split_tokens_merge_back_to_chat_word` (1 passed).

### Audit known English transcribe corrections end to end
- **component**: `talkbank-transform` ASR postprocessing and CHAT construction
- **summary**: Verifies the three ordered English hooks in one output contract: pronoun-I and I-contraction capitalization, allowlisted title-period stripping before retokenization, and first lexical-word capitalization after utterance boundaries are known.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/input/raw-asr.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/tbt-output/transcript.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/ba3-pre-output/transcript.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/english-transcribe-patterns/ba3-post-output/transcript.txt
- **depends on**: []
- **commit**: 92e60bd
- **new**: no

Audit note: runtime parity was already present before this parity pass. The independent branch-local regression starts from timed raw ASR elements, crosses `process_raw_asr` and transcript description, and proves `Hello`, `I`, `Dr`, and `I'll` retain their exact source intervals after capitalization and title-period cleanup. The existing end-to-end serialization regression separately checks all three corrected utterances plus negative stale-form assertions. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact build_chat::tests::english_transcribe_corrections_preserve_source_timings` (1 passed).

### Lock bogus Chinese lemmas to their Han surface
- **component**: Python structured Stanza morphology renderer
- **summary**: Proves a Chinese lexical word whose Stanza lemma is punctuation retains its Han surface as the lemma, remains a noun with a valid ROOT relation, and records a structured lemma anomaly rather than disappearing as punctuation.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/input/raw-stanza.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/tbt-output/analysis.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/ba3-pre-output/analysis.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/chinese-bogus-lemma/ba3-post-output/analysis.txt
- **depends on**: [f2f60d0]
- **commit**: bf3035a
- **new**: no

Audit note: the repair implementation was introduced by the earlier general invalid-Stanza fix, but only Latin lexical surfaces were pinned. This independent Chinese regression supplies `苹果` with the observed bogus lemma `。`, asserts exact `%mor` surface preservation and ROOT repair, and checks the anomaly retains the Han source text. Together with the English end-to-end evidence and Japanese realignment contract above, this closes the final three-language original TODO. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_render.py -q -k chinese_bogus_punctuation_lemma` (1 passed, 11 deselected).

### Hash the complete typed `@Media` identity
- **component**: `talkbank-model` typed media header
- **summary**: Adds `Eq` and `Hash` to `MediaHeader` so typed headers can safely key deduplication and memoization; the derived identity includes filename, capture modality, and optional linkage status.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/input/media-identities.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/tbt-output/hash-set.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/ba3-pre-output/hash-set.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/media-header-hash/ba3-post-output/hash-set.txt
- **depends on**: []
- **commit**: dd92e0b
- **new**: no

Pre-edit every constituent media field was already hashable, but the composite header omitted the contract and could not be used directly in a `HashSet`/`HashMap`. The regression inserts an exact duplicate plus three single-field variations and requires four unique identities, proving no field is accidentally ignored. This closes the last clause of the first bundled original TODO; its retrace, recovery-span, and Japanese clauses are independently evidenced above. Targeted verification: `bazel build --config=dev //crates/core/talkbank-model:talkbank_model_unit_test && bazel-bin/crates/core/talkbank-model/talkbank_model_unit_test --exact model::header::media::tests::media_hash_includes_filename_type_and_status` (1 passed).

### Sanitize CHAT-illegal ASR tokens at the final word seam
- **component**: `talkbank-transform` ASR postprocessing
- **summary**: Uses the typed CHAT word parser as an oracle after number expansion, stripping illegal characters while retaining valid residue and timing; entirely structural tokens are dropped, and empty utterances created by sanitization are removed.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/input/raw-asr.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/tbt-output/transcript.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/ba3-pre-output/transcript.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/asr-chat-sanitization/ba3-post-output/transcript.txt
- **depends on**: []
- **commit**: 3ccc897
- **new**: yes

Whisper and Tencent can emit bare CHAT separators such as `:` and `~`, or glue invalid Unicode to otherwise useful text. Pre-edit the downstream typed transcript gate rejected the whole utterance. Post-edit sanitization runs only after currency/percent expansion, preserves every already-valid word byte-for-byte, keeps timestamps on repaired residue, and prevents an all-dropped utterance from becoming an invalid empty main tier. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact build_chat::tests::chat_illegal_asr_separator_does_not_fail_transcription` (1 passed).

### Suppress terminator-only translation noise
- **component**: `talkbank-transform` translation injection
- **summary**: Treats whitespace-only and bare `.`, `!`, or `?` backend responses as no translation, leaving the utterance unchanged instead of emitting a meaningless `%xtra` tier.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/input/translation.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/translate-terminator-noise/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: be2b98b
- **new**: yes

Translator APIs can answer with only the source terminator when a turn has no lexical material. Pre-edit BA3 accepted nonempty `.` and serialized `%xtra:\t.`; BA2 and the fork omit it. The trim-first guard handles all three CHAT terminators and whitespace variants without changing genuine translations or replacement of an existing tier. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact translate::tests::test_inject_bare_terminator_translation_is_noop` (1 passed).

### Exclude paired CA segment repetitions from lexical text
- **component**: `talkbank-model` cleaned words and `talkbank-parser-re2c` conversion
- **summary**: Treats text bracketed by paired `↫` markers as a repeated non-lexical fragment while preserving text inside every other CA delimiter, keeping both parser implementations consistent.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/input/stutter.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/tbt-output/lexical.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/ba3-pre-output/lexical.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/ca-segment-repetition/ba3-post-output/lexical.txt
- **depends on**: []
- **commit**: dec565c
- **new**: yes

In CHAT, `↫sch↫schaap` records a repeated onset followed by the lexical Dutch word `schaap`. Pre-edit cleaned text concatenated every text node and sent `schschaap` to morphology, alignment, comparison, and frequency consumers. Post-edit both CST conversion paths toggle a segment-repetition state and omit only bracketed material; the regression also proves ordinary CA content such as `∆snel∆` remains lexical. Targeted verification: `bazel build --config=dev //crates/core/talkbank-model:talkbank_model_unit_test //crates/core/talkbank-parser-re2c:talkbank_parser_re2c_unit_test && bazel-bin/crates/core/talkbank-model/talkbank_model_unit_test --exact model::content::word::word_type::cleaned_text_tests::segment_repetition_is_excluded_but_other_ca_content_is_lexical` (1 passed).

### Keep retraces with their following retry during UtSeg
- **component**: `talkbank-transform` utterance splitting
- **summary**: Assigns uncounted `Retrace` content forward to the next word-bearing child before generic marker back-fill, preventing `[/]`, `[//]`, or `[///]` from being stranded at the end of the preceding child.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/input/retrace.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/tbt-output/split.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/ba3-pre-output/split.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utseg-retrace-binding/ba3-post-output/split.txt
- **depends on**: [5ebd4cc]
- **commit**: a309006
- **new**: yes

Retraced words are deliberately absent from the morphology word domain, so their top-level `Retrace` nodes receive no classifier assignment. Pre-edit generic back-fill attached those nodes to the preceding assigned word, creating a dangling retrace when the classifier boundary fell before the kept retry. Post-edit retraces first inherit the next assigned content group; genuinely utterance-final retraces still use the existing fallback. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact utseg::tests::utseg_split_keeps_retrace_with_following_retry` (1 passed).

### Project comparison candidates to one utterance
- **component**: `talkbank-transform` transcript comparison
- **summary**: Projects every candidate main-token window to its majority utterance before overlap and alignment scoring, preventing a single gold utterance from collecting matches across two main utterances.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/tbt-output/metrics.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/ba3-pre-output/metrics.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-majority-window/ba3-post-output/metrics.txt
- **depends on**: []
- **commit**: 6ac567d
- **new**: yes

Pre-edit bag-of-words scoring selected `the sky this dog ran` and counted `the` from one main utterance together with `dog ran` from the next, inflating the match count from two to three. Post-edit preserves Python `Counter.most_common(1)` first-seen tie behavior, trims candidate edges to the majority utterance, and scores only the projected range. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact compare::tests::find_best_segment_does_not_score_across_utterance_boundaries && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact compare::tests::compare_does_not_steal_match_across_utterance_boundary` (2 passed).

### Attribute matched POS from the gold transcript
- **component**: `talkbank-transform` comparison annotation and metrics
- **summary**: Uses the gold word's morphology tag for every matched token, so `%xsmor` and per-POS match counts describe the reference annotation rather than silently copying a conflicting main-transcript tag.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/tbt-output/xsmor.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/ba3-pre-output/xsmor.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/compare-gold-pos/ba3-post-output/xsmor.txt
- **depends on**: []
- **commit**: 0064d0d
- **new**: yes

Pre-edit identical surfaces with disagreeing morphology produced `NOUN ADJ`, masking the gold annotation `INTJ NOUN` and charging matches to the wrong POS buckets. Post-edit matched and gold-only tokens both derive POS from gold, while main-only insertions continue to retain main POS; gold files without `%mor` correctly produce `?` instead of borrowing a tag. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test compare::tests::` (32 passed).

### Keep experimental review tiers off by default
- **component**: `talkbank-transform` decision reporting and `batchalign-core` UTR
- **summary**: Defaults decision-tier emission to `None`, preserving structured UTR decisions internally without adding `%xalign` or `%xrev` cleanup noise to ordinary CHAT output.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/input/decision.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/tbt-output/decision.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/ba3-pre-output/decision.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/review-tiers-default/ba3-post-output/decision.cha
- **depends on**: []
- **commit**: f0aa660
- **new**: yes

Pre-edit UTR hardcoded `ReviewLevel::All`, so any unmatched or zero-duration decision leaked experimental dependent tiers into finished research files. Post-edit `ReviewLevel::default()` is `None` and UTR uses that default; callers that explicitly pass `LowConfidence` or `All` to the retained injection API still receive review tiers. Targeted verification: `bazel build --config=dev //crates/core/talkbank-transform:talkbank_transform_unit_test //crates/batchalign/batchalign-core:batchalign_core_unit_test && bazel-bin/crates/core/talkbank-transform/talkbank_transform_unit_test --exact decisions::tests::default_review_level_produces_no_output_tiers` (1 passed).

### Skip AppleDouble CHAT sidecars during discovery
- **component**: Python CLI CHAT discovery and align language preflight
- **summary**: Routes align's language inference through the shared hidden-file filter and rejects explicitly supplied dotfiles, so macOS `._*.cha` metadata cannot be parsed as transcript text.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/input/corpus.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/tbt-output/discovered.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/ba3-pre-output/discovered.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/appledouble-discovery/ba3-post-output/discovered.txt
- **depends on**: []
- **commit**: 565c0bc
- **new**: yes

The normal input collector already ignored directory dotfiles, but align independently scanned `*.cha` to choose its UTR language. Since `._session.cha` sorts before `session.cha`, binary metadata raised `UnicodeDecodeError` before any real input ran. Post-edit both preflight and collection share `_walk`, and a sidecar passed as the explicit path yields a clear no-language error rather than entering the pipeline. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_chat_discovery.py -q` (2 passed).

### Remove wrapped analysis tiers completely before morphotagging
- **component**: Python morphotag `--clear-existing` staging
- **summary**: Removes every tab-led continuation belonging to an existing `%mor` or `%gra` tier while preserving following main tiers, headers, and unrelated dependent tiers.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/input/wrapped.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/tbt-output/cleared.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/ba3-pre-output/cleared.cha
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/wrapped-analysis-clear/ba3-post-output/cleared.cha
- **depends on**: []
- **commit**: 48655f8
- **new**: yes

The default refresh path stages a copy without old analysis so the engine cannot skip already-tagged utterances. Pre-edit it removed only `%mor:` and `%gra:` header lines, leaving wrapped payload lines as orphan CHAT continuations; the staged file could then fail parsing before regeneration. Post-edit a small state machine consumes the complete logical analysis tiers and stops at the next non-continuation line. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_morphotag_clear.py -q` (3 passed).

### Schedule the largest batch inputs first
- **component**: Python CLI shared CHAT/media discovery
- **summary**: Orders discovered inputs by descending byte size with a stable path tie-breaker, starting long-running work early so small files can fill later worker slots instead of leaving one tail straggler.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/largest-first-scheduling/input/files.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/largest-first-scheduling/tbt-output/order.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/largest-first-scheduling/ba3-pre-output/order.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/largest-first-scheduling/ba3-post-output/order.txt
- **depends on**: []
- **commit**: e00f548
- **new**: yes

The fork explicitly sorts batch discovery largest-first to avoid makespan dominated by a large file launched at the end. Pre-edit BA3 used lexicographic path order; the focused regression names its 5-, 11-, and 16-byte inputs `a-small.cha`, `m-medium.cha`, and `z-large.cha`, forcing alphabetical and size order to disagree. Post-edit the shared collector applies largest-first scheduling to morphotag, align, utseg, translate, AI, and transcribe inputs while retaining deterministic path ordering for equal sizes. Targeted verification: `bazel build //python/batchalign:pytest && bazel-bin/python/batchalign/pytest python/batchalign/tests/test_chat_discovery.py -q` (3 passed).

### Place generated provenance after constant participant headers
- **component**: `batchalign-core` provenance stamping
- **summary**: Inserts generated provenance immediately after the last `@ID`, `@Birth of`, `@Birthplace of`, or `@L1 of` header and before later changeable metadata, matching the fork's canonical CHAT header order.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/provenance-header-order/input/constant-headers.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/provenance-header-order/tbt-output/header-order.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/provenance-header-order/ba3-pre-output/header-order.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/provenance-header-order/ba3-post-output/header-order.txt
- **depends on**: []
- **commit**: 9e431a7
- **new**: yes

The fork's provenance regression identifies constant participant headers as the boundary between fixed participant metadata and changeable headers. Pre-edit BA3 appended provenance immediately before the first main tier, after `@Date` and any other session metadata. Post-edit BA3 selects the same constant-header boundary; documents without participant headers retain the prior safe fallback before the first utterance or `@End`. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors` (55 passed, 1 ignored).

### Expose Apple MPS only through an explicit warned opt-in
- **component**: Python align/transcribe CLI device selection
- **summary**: Adds `--allow-mps` to local alignment and transcription commands, keeps Apple GPU use opt-in, warns when engaged, rejects contradictory CPU/MPS switches, and preserves CHATWhisper's float32 MPS policy.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/apple-mps-opt-in/input/invocations.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/apple-mps-opt-in/tbt-output/device-policy.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/apple-mps-opt-in/ba3-pre-output/device-policy.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/apple-mps-opt-in/ba3-post-output/device-policy.txt
- **depends on**: [c425fe8]
- **commit**: 1c926d2
- **new**: yes

The fork changed MPS from an implicit hardware possibility into an explicit performance/stability trade-off after rare sustained-load driver stalls. BA3's affected local backends already defaulted to CPU on Apple hosts, but offered no supported way to request MPS; users could only force CPU. Post-edit the shared resolver returns `mps` only for `--allow-mps`, emits a risk warning, and rejects combining it with `--force-cpu`. Pyannote remains untouched and therefore on its safe CPU path. Targeted verification: `bazel test //python/batchalign:pytest --test_arg=python/batchalign/tests/test_cli.py --test_arg=-q --test_output=errors` (Python Bazel target passed).

### Refresh stale same-version Stanza resource catalogs safely
- **component**: Python Stanza morphosyntax backend bootstrap
- **summary**: Refreshes an existing Stanza `resources.json` once per process before pipeline construction, validates and atomically installs the response, and preserves the cached manifest on any offline or filesystem failure.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-catalog-refresh/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-catalog-refresh/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-catalog-refresh/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-catalog-refresh/ba3-post-output/result.txt
- **depends on**: []
- **commit**: 68575ff
- **new**: yes

Stanford has republished Stanza model artifacts without changing the resources version, so `REUSE_RESOURCES` can verify a new model payload against an old cached checksum and make morphotag unavailable. Pre-edit BA3 never refreshed a present manifest. Post-edit it follows the fork's worker-boundary repair at BA3's backend boundary: missing manifests remain Stanza's responsibility, successful refreshes use same-directory atomic replacement, and failures leave the old catalog intact. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors` (Python Bazel target passed, including online-success, offline-fallback, and missing-manifest regressions).

### Make unsupported-language Stanza UtSeg fallback explicit and functional
- **component**: Python UtSeg backend and transcribe/utseg CLI wiring
- **summary**: Refuses to silently omit utterance segmentation when no TalkBank boundary model exists and makes `--utseg-fallback-stanza` select a real constituency backend that applies the fork's coordinated-clause grouping policy.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-utseg-opt-in/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-utseg-opt-in/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-utseg-opt-in/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/stanza-utseg-opt-in/ba3-post-output/result.txt
- **depends on**: [68575ff]
- **commit**: b498ac0
- **new**: yes

Pre-edit transcribe returned `None` from its UtSeg builder for unsupported languages, silently keeping vendor ASR segmentation, while the standalone fallback switch was explicitly inert. Post-edit both command paths refuse that silent output change by default and expose the fork's named opt-in. The fallback groups coordinated `S` clauses, merges fragments shorter than three words, carries fixed word timings, and proportionally projects a timed parent window when word timing is absent. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors` (240 passed, 1 skipped).

### Rescue contracted copula progressives structurally
- **component**: Python UD-to-CHAT morphosyntax renderer
- **summary**: Rewrites Stanza's possessive-gerund analysis of English `<subject>'s <verb-ing>` MWTs into a finite copula plus progressive verb, updating morphology and dependencies together.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/copula-progressive-rescue/input/stanza-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/copula-progressive-rescue/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/copula-progressive-rescue/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/copula-progressive-rescue/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: e44b6d9
- **new**: yes

Stanza can interpret `sink's overflowing` as possessive `sink` plus `PART/case` and a nominal `overflowing`, producing `~part|s` and possessive dependencies. Post-edit BA3 ports the fork's guarded finite-clause invariant before structured rendering: it requires an MWT, no existing finite verb, one possessive `'s`, and exactly one `-ing` noun, then emits the finite `~aux|be-Fin-Ind-Pres-S3`, progressive verb features, and matching `NSUBJ/AUX/ROOT` relations. Genuine possessives such as `boy's coat` remain unchanged. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=python/batchalign/tests/test_morphotag_render.py` (Bazel target passed).

### Make batch worker concurrency configurable end to end
- **component**: Python CLI and Rust pipeline scheduler
- **summary**: Adds a validated global `--workers` option and uses the requested value for runtime threads, execution permits, and the bounded dispatch window across every batch-processing command.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/configurable-workers/input/invocation.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/configurable-workers/tbt-output/concurrency.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/configurable-workers/ba3-pre-output/concurrency.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/configurable-workers/ba3-post-output/concurrency.txt
- **depends on**: [d7c5d49]
- **commit**: 4d12e89
- **new**: yes

The fork exposes an operator worker limit and recently fixed a path where another concurrency cap silently overrode it. Pre-edit BA3 had the more fundamental gap: its Tokio runtime, semaphore, and resident dispatch window were three independent hardcoded eights. Post-edit one Pipeline setting controls all three, defaults compatibly to eight, rejects zero, and is forwarded by transcribe, align, morphotag, translate, AI, UtSeg, and compare. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors`; `bazel test //python/batchalign:pytest --test_output=errors --test_arg=python/batchalign/tests/test_cli.py`; and Bazel-backed `just batchalign cli --help`.

### Invalidate cached outputs across engine builds
- **component**: Rust LMDB task-output cache
- **summary**: Includes the compiled git/build identity in the central cache namespace so unchanged task input cannot retrieve output produced by older algorithm code.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cache-build-identity/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cache-build-identity/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cache-build-identity/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/cache-build-identity/ba3-post-output/result.txt
- **depends on**: []
- **commit**: 35e9aad
- **new**: yes

The fork requires its cache engine version to match before returning a hit. BA3's cache documentation instead acknowledged that code changes under a stable backend name silently served old output and required a manual name bump or cache purge. Post-edit the schema-v3 key includes the build's stamped git identity, with the package version as the non-stamped fallback, before task/backend/input identity. A released build retains normal reuse, while a new implementation cannot inherit its predecessor's transcript or analysis result. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-engine:batchalign_engine_unit_test --test_output=errors` (including distinct namespace digests for two code versions).

### Keep automatic UTR on the validated global strategy
- **component**: `batchalign-core` utterance timing recovery strategy selection
- **summary**: Stops overlap markers from automatically enabling experimental two-pass UTR and keeps ordinary recovery on the monotonic global alignment path.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utr-global-default/input/overlap.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utr-global-default/tbt-output/strategy.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utr-global-default/ba3-pre-output/strategy.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/utr-global-default/ba3-post-output/strategy.txt
- **depends on**: []
- **commit**: 8767049
- **new**: yes

The fork previously auto-selected two-pass UTR for `+<` and bottom-overlap markers, but now deliberately keeps `Auto` on global UTR until the pass-2 end-time bug is resolved and operator files validate the experiment. Pre-edit BA3 retained the older automatic behavior and even documented its incomplete FA-group tiebreaker. Post-edit the two-pass implementation remains available for future calibrated opt-in work, while the default selector consistently returns the global monotonic strategy for overlap and non-overlap files. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors` (56 passed, 1 ignored, including a parsed `+<` regression).

### Expose standalone speaker diarization as turns JSON
- **component**: Python CLI, Pyannote speaker backend, and Rust audio preparation binding
- **summary**: Adds `diarize` for media-only speaker-turn detection, writing deterministic anonymous `PAR0`… track spans without paying for transcription.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/standalone-diarize/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/standalone-diarize/tbt-output/session.turns.json
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/standalone-diarize/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/standalone-diarize/ba3-post-output/session.turns.json
- **depends on**: []
- **commit**: 26fa3f7
- **new**: yes

The fork makes its Pyannote speaker stage independently useful for acoustic speaker-attribution repair. Pre-edit BA3 already had the same typed speaker input/output and backend, but users could reach it only while transcribing. Post-edit the command decodes through BA3's existing 16 kHz mono Rust seam, preserves millisecond spans, maps sorted backend labels deterministically to anonymous `PAR0`… tracks, supports speaker-count hints, and mirrors directory outputs as `.turns.json`. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=python/batchalign/tests/test_diarize_cli.py --test_arg=python/batchalign/tests/test_cli.py`; core Bazel target; and `just batchalign cli diarize --help`.

### Collapse Italian Stanza Defect 6 false MWT expansions
- **component**: Python UD-to-CHAT Italian morphosyntax renderer
- **summary**: Replaces a closed set of ordinary Italian words that Stanza expands as fake verb-clitic MWTs with one curated lexical analysis while preserving genuine compound imperatives.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect6-collapse/input/stanza-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect6-collapse/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect6-collapse/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect6-collapse/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: 751cd4b
- **new**: yes

The fork carries a closed, retireable Defect 6 table for common words such as `piccolo`, `parla`, `pallone`, and `coccole` that Stanza can split into a spurious stem plus pronoun. Pre-edit BA3 rendered `piccolo` as `verb|picco~pron|il-S3`, adding a fake lexical unit and corrupting both tiers. Post-edit it recognizes only an actual MWT range whose original surface is in the same closed table, synthesizes the curated POS, lemma, and features, renumbers dependencies and token spans together, and emits an operator-visible anomaly. Genuine `dammela` remains a three-unit compound. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=-k --test_arg=italian_defect6` (Bazel target passed); Ruff passed on changed files apart from the repository's pre-existing E721 diagnostic outside this change.

### Repair the Italian sentence-initial `la` false MWT expansion
- **component**: Python UD-to-CHAT Italian morphosyntax renderer
- **summary**: Collapses Stanza's sentence-initial `la → il + i` expansion to one feminine singular definite article with consistent dependencies.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect7-article/input/stanza-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect7-article/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect7-article/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect7-article/ba3-post-output/tiers.txt
- **depends on**: [751cd4b]
- **commit**: ad229ab
- **new**: yes

The fork records a distinct Defect 7 failure in which Stanza analyzes sentence-initial `la` as masculine-singular `il` plus masculine-plural `i`. Pre-edit BA3 retained both components as a `~`-joined MWT and introduced an extra dependency unit. Post-edit the closed Italian range table synthesizes `det|il-Fem-Def-Art-Sing`, collapses the range, renumbers the following noun and punctuation, and reports `italian_defect_7`; ordinary one-component `la` tokens remain untouched because the repair requires an actual MWT range. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=-k --test_arg=italian_defect7` (Bazel target passed) and Ruff on the changed files (with the unrelated pre-existing E721 diagnostic excluded).

### Rewrite the mis-tagged verb head in Italian `dagliela` MWTs
- **component**: Python UD-to-CHAT Italian morphosyntax renderer
- **summary**: Changes only the head of Stanza's shape-correct `dagliela` expansion from `ADP/da` to imperative `VERB/dare`, retaining both clitic units and their arcs.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect9-dagliela/input/stanza-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect9-dagliela/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect9-dagliela/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect9-dagliela/ba3-post-output/tiers.txt
- **depends on**: []
- **commit**: b717b1e
- **new**: yes

Stanza's `dagliela` range has the correct three-piece shape but analyzes its imperative head as the homographic adposition `da`, yielding `adp|da~pron|gli~pron|la`. The fork treats this separately from false-MWT collapse: only component zero's POS, canonical lemma, and imperative features change. Post-edit BA3 follows that invariant and leaves component ids, heads, relations, and clitic analyses intact, producing `verb|dare-Fin-Imp-S2~pron|gli-Prs-S3~pron|la-Prs-S3` plus an `italian_defect_9` anomaly. The closed lookup excludes correctly analyzed siblings such as `digliela` and `portagliela`. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=-k --test_arg=italian_defect9` (Bazel target passed) and Ruff on the changed files (with the unrelated pre-existing E721 diagnostic excluded).

### Canonicalize the verb lemma in Italian `posala` and `posalo` MWTs
- **component**: Python UD-to-CHAT Italian morphosyntax renderer
- **summary**: Replaces Stanza's surface-echo `posa` lemma with canonical `posare` for the two confirmed imperative-clitic ranges while preserving their component structure.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect10-posare/input/stanza-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect10-posare/tbt-output/tiers.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect10-posare/ba3-pre-output/tiers.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/italian-defect10-posare/ba3-post-output/tiers.txt
- **depends on**: [b717b1e]
- **commit**: 4895eed
- **new**: yes

Unlike Defect 9, Stanza already tags the head of `posala` and `posalo` as an imperative verb; only the lemma is the inflected surface `posa`. Pre-edit BA3 therefore emitted `verb|posa-Fin-Imp-S2` and made lemma-based corpus searches disagree with the fork. Post-edit the component-rewrite table gives both confirmed surfaces the canonical `posare` lemma and retains their `la`/`lo` post-clitic, ids, and dependency arcs. The regression exercises both genders and requires the distinct `italian_defect_10` anomaly. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=-k --test_arg=italian_defect10` (Bazel target passed) and Ruff on the changed files (with the unrelated pre-existing E721 diagnostic excluded).

### Preserve CHAT's virtual zero head for `%gra` root relations
- **component**: Python UD-to-CHAT morphosyntax dependency renderer
- **summary**: Maps UD `head=0` directly to CHAT's virtual root instead of applying Python's `-1` list index and attaching `ROOT` to the sentence's last lexical chunk.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/virtual-root-head/input/ud-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/virtual-root-head/tbt-output/gra.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/virtual-root-head/ba3-pre-output/gra.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/virtual-root-head/ba3-post-output/gra.txt
- **depends on**: []
- **commit**: 135a622
- **new**: yes

The fork's generated-GRA contract requires exactly one `ROOT` relation whose head is the virtual node zero. BA3's faithful BA2 port indexed `actual_indicies[raw_head - 1]` for every arc; for `raw_head == 0`, Python silently selected the final lexical item, so a two-word sentence emitted `2|2|ROOT`. Post-edit the zero case bypasses lexical reindexing, while non-root heads and the terminator's attachment to the lexical root are unchanged. This also corrects roots inside MWTs and after Italian range repairs. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors` (249 passed, 1 skipped) and Ruff on the changed files (with the unrelated pre-existing E721 diagnostic excluded).

### Reject UD analyses without exactly one dependency root
- **component**: Python UD-to-CHAT morphosyntax dependency renderer
- **summary**: Stops rootless and multiple-root Stanza analyses before tier assembly instead of serializing cyclic or ambiguous `%gra` structures.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/invalid-ud-root/input/rootless-analysis.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/invalid-ud-root/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/invalid-ud-root/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/invalid-ud-root/ba3-post-output/result.txt
- **depends on**: [135a622]
- **commit**: b0ad6a2
- **new**: yes

The fork treats the absence of a dependency root as a mapping error, because no terminator attachment or valid CHAT tree can be derived; the same one-root invariant also excludes ambiguous multi-root parses. Pre-edit BA3 accepted a two-node cycle, emitted no `ROOT` triple, and attached punctuation to virtual zero. Post-edit normalized analyses must contain exactly one `head=0`/`root` pair before chunk assembly. Existing field recovery remains intact: a missing or out-of-range head can still be repaired deterministically and recorded as an anomaly before validation. Targeted verification: `bazel test //python/batchalign:pytest --test_output=errors --test_arg=-k --test_arg='rootless or multiple_ud_roots or invalid_stanza_fields'` (Bazel target passed) and Ruff on the changed files (with the unrelated pre-existing E721 diagnostic excluded).

### Reuse complete exact-match `%wor` timing without rerunning FA
- **component**: `batchalign-core` forced-alignment task runner
- **summary**: Detects a fully timed `%wor` tier whose words exactly match each current main tier, refreshes utterance bullets from its spans, and returns before media decode or backend dispatch.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-complete-wor-reuse/input/reusable.cha
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-complete-wor-reuse/tbt-output/reusable.cha
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-complete-wor-reuse/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-complete-wor-reuse/ba3-post-output/reusable.cha
- **depends on**: []
- **commit**: 031cc38
- **new**: yes

The fork has a cheap rerun path for already aligned files, whereas BA3 previously decoded the complete recording and invoked the FA model on every run. Post-edit BA3 requires every alignable utterance to have the same number and text of `%wor` words, with a positive-duration inline bullet on each; any mismatch falls through without mutation. A clean file refreshes its main bullets from the first and last word and succeeds even when the referenced media is unavailable, proving that no media or backend work occurred. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::complete_wor_reuse_skips_media_and_backend` (1 passed); `rustfmt --edition 2024 --check` passed for the changed file.

### Reject reusable `%wor` spans that overrun the next utterance
- **component**: `batchalign-core` forced-alignment reuse validation
- **summary**: Refuses the cheap rerun path when an utterance's final reused word ends after the next timed utterance begins.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-next-start/input/spans.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-next-start/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-next-start/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-next-start/ba3-post-output/result.txt
- **depends on**: [031cc38]
- **commit**: 93f4048
- **new**: yes

An exact word match is not enough to trust old timing: stale `%wor` data can claim audio already anchored to the next utterance. The fork excludes that utterance from reuse. Immediately after adding BA3's base fast path, the same input was accepted and widened the first main bullet from `100_900` to `100_1200`, overlapping the next `1000` start. Post-edit the runner finds the next available timed main-tier start across intervening lines and falls through to realignment when the reused end exceeds it. The check runs before any mutation, so a rejected whole-file reuse attempt leaves every original bullet intact. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::complete_wor_reuse_rejects_span_past_next_utterance_start` (1 passed); file-local Rust formatting passed.

### Reject near-zero word spans from FA timing reuse
- **component**: `batchalign-core` forced-alignment reuse validation
- **summary**: Requires every reused `%wor` bullet to span at least 40 ms, catching collapsed internal and final words before the cheap rerun path.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-collapsed-word/input/spans.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-collapsed-word/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-collapsed-word/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-collapsed-word/ba3-post-output/result.txt
- **depends on**: [031cc38]
- **commit**: 7bddd99
- **new**: yes

Positive duration alone does not make old word timing reusable. The fork uses a 40 ms floor because failed alignment often leaves one internal or final word with a tiny residual span that survives zero-duration validation. With only the base BA3 reuse path, a 30 ms `tiny` token was accepted and the file skipped FA. Post-edit duration is checked with saturating arithmetic for every `%wor` word; a sub-floor span rejects the whole fast-path attempt before any main bullet changes. The same invariant covers short utterances and position-independent internal/final failures. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::complete_wor_reuse_rejects_near_zero_word_span` (1 passed); file-local Rust formatting passed.

### Reject one-word dominance in reusable FA timing
- **component**: `batchalign-core` forced-alignment reuse validation
- **summary**: Rejects old timing when one word occupies more than 40% of a three-or-more-word utterance span, forcing a fresh alignment instead of preserving a characteristic stale distribution.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-dominant-word/input/spans.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-dominant-word/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-dominant-word/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-dominant-word/ba3-post-output/result.txt
- **depends on**: [031cc38]
- **commit**: 1a3287e
- **new**: yes

Some bad prior alignments have no zero or tiny word: instead, one token absorbs most of the utterance and leaves plausible positive spans for its neighbors. The fork treats that distribution as unreusable once at least three words provide enough context. Pre-edit BA3 accepted a 550 ms word inside an 800 ms total word span and skipped FA. Post-edit it measures the largest word against the minimum-start/maximum-end envelope and rejects proportions strictly above 0.4; one- and two-word utterances deliberately avoid the heuristic. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::complete_wor_reuse_rejects_dominant_word_span` (1 passed); file-local Rust formatting passed.

### Reject backward word order in reusable FA timing
- **component**: `batchalign-core` forced-alignment reuse validation
- **summary**: Requires reused `%wor` spans to advance monotonically, preventing positive-duration but backward word timing from entering the cheap rerun path.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-backward-words/input/spans.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-backward-words/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-backward-words/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-reuse-backward-words/ba3-post-output/result.txt
- **depends on**: [031cc38]
- **commit**: 475563d
- **new**: yes

Word spans can each be positive yet still form a backward sequence. The base BA3 reuse path accepted `hello 500_600; world 400_480`, then derived the invalid main bullet `500_480` and returned before the normal post-FA monotonicity repair. Post-edit every word start must be at or after the preceding word end; otherwise reuse is rejected without mutation and the normal FA path replaces the stale tier. This check is independent of the 40 ms floor, dominance ratio, and next-utterance boundary. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::complete_wor_reuse_rejects_backward_word_timing` (1 passed); file-local Rust formatting passed.

### Clear stale zero-duration authoritative bullets after untimed FA
- **component**: `batchalign-core` forced-alignment result injection
- **summary**: Removes an authoritative `T_T` main-tier bullet when FA returns no timed words, while retaining the untimed `%wor` structure for diagnostics.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-clear-zero-bullet/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-clear-zero-bullet/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-clear-zero-bullet/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-clear-zero-bullet/ba3-post-output/result.txt
- **depends on**: []
- **commit**: 438641c
- **new**: yes

The fork distinguishes a usable UTR hint from a stale authoritative bullet left by an earlier bad FA run. When every returned word is untimed, there is no new span with which to repair an authoritative `245986_245986` anchor; retaining it guarantees temporal validation failure. Pre-edit BA3 copied the segment bounds back unconditionally and preserved the invalid bullet. Post-edit the no-timing branch removes only a zero/backward authoritative bullet, leaves valid existing windows unchanged, and still replaces `%wor` with the current untimed word structure. The strict parser regression constructs the stale state in the typed AST because serialized `T_T` input is correctly rejected as E362. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::untimed_fa_result_clears_zero_duration_authoritative_bullet` (1 passed); file-local Rust formatting passed.

### Replace provisional UTR windows with successful FA word spans
- **component**: `batchalign-core` forced-alignment result injection
- **summary**: Replaces a broad UTR-generated main-tier hint with the minimum and maximum timestamps of the words that FA actually aligned.
- **input example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-overwrite-utr-hint/input/scenario.txt
- **tbt output example**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-overwrite-utr-hint/tbt-output/result.txt
- **ba3 output, pre-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-overwrite-utr-hint/ba3-pre-output/result.txt
- **ba3 output, post-edit**: /Users/houjun/Documents/Projects/talkbank-parity/ba3/fa-overwrite-utr-hint/ba3-post-output/result.txt
- **depends on**: []
- **commit**: be1af15
- **new**: yes

UTR boundaries are search hints, not authoritative transcript timing. The fork discards that broad window after successful forced alignment and derives the utterance bullet from the actual timed-word envelope. Pre-edit BA3 retained the segment bounds `800_3000` even though both aligned words occupied only `1000_2000`, leaving avoidable leading and trailing silence. Post-edit the injector tracks the minimum word start and maximum word end and overwrites only bullets marked as UTR-sourced; untimed results and authoritative transcript bullets retain their separate policies. Targeted verification: `bazel test --config=dev //crates/batchalign/batchalign-core:batchalign_core_unit_test --test_output=errors --test_filter=taskrunners::fa::tests::timed_fa_words_overwrite_provisional_utr_window` (1 passed); file-local Rust formatting passed.
