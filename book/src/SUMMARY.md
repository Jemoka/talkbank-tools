# Summary

**Status:** Current
**Last updated:** 2026-04-30 19:25 EDT

[Introduction](introduction.md)
[Install](install/index.md)
[Quickstart](quickstart/index.md)

---

# Migration Book (Batchalign2 → Batchalign3)

- [Batchalign2 -> Batchalign3](batchalign/migration/index.md)
- [User Workflow Migration](batchalign/migration/user-migration.md)
- [Developer Architecture Migration](batchalign/migration/developer-migration.md)
- [BA2 Compare Migration](batchalign/migration/ba2-compare-migration.md)
- [BA2 Architecture Reference](batchalign/migration/ba2-architecture-reference.md)
- [Algorithms, Language, and Alignment Migration](batchalign/migration/algorithms-and-language.md)
- [Why Replace Python CHAT Handling with Rust](batchalign/migration/python-to-rust-rationale.md)
- [Persistent State and Behavioral Changes](batchalign/migration/persistent-state.md)
- [BA2 CLI Reference (Baseline)](batchalign/migration/ba2-cli-reference.md)
- [Debugging and Tracing](batchalign/migration/debugging-and-tracing.md)

# Batchalign3 — User Guide

- [Installation](batchalign/user-guide/installation.md)
- [Model Downloads and Caching](batchalign/user-guide/model-downloads.md)
- [Batchalign Desktop (Experimental)](batchalign/user-guide/desktop-app.md)
- [Web Dashboard](batchalign/user-guide/dashboard.md)
- [Progress and Feedback](batchalign/user-guide/progress-and-feedback.md)
- [Quick Start](batchalign/user-guide/quick-start.md)
- [CLI Reference](batchalign/user-guide/cli-reference.md)
  - [align](batchalign/user-guide/commands/align.md)
  - [transcribe](batchalign/user-guide/commands/transcribe.md)
  - [morphotag](batchalign/user-guide/commands/morphotag.md)
  - [utseg](batchalign/user-guide/commands/utseg.md)
  - [translate](batchalign/user-guide/commands/translate.md)
  - [coref](batchalign/user-guide/commands/coref.md)
  - [compare](batchalign/user-guide/commands/compare.md)
  - [benchmark](batchalign/user-guide/commands/benchmark.md)
  - [opensmile](batchalign/user-guide/commands/opensmile.md)
  - [avqi](batchalign/user-guide/commands/avqi.md)
  - [eval l2-morphotag](batchalign/user-guide/commands/eval.md)
- [Python API](batchalign/user-guide/python-api.md)
- [Cantonese Engines](batchalign/user-guide/cantonese-processing.md)
- [Caching](batchalign/user-guide/caching.md)
- [Server Mode](batchalign/user-guide/server-mode.md)
- [Server and Fleet Setup](batchalign/user-guide/server-setup.md)
- [Rev.AI Integration](batchalign/user-guide/rev-ai.md)
- [Performance](batchalign/user-guide/performance.md)
- [Processing Provenance](batchalign/user-guide/provenance.md)
- [Review Tiers: %xalign and %xrev](batchalign/user-guide/review-tiers-guide.md)
- [Worker Tuning](batchalign/user-guide/worker-tuning.md)
- [Doctor: Diagnostics and Config Validation](batchalign/user-guide/doctor.md)
- [Troubleshooting](batchalign/user-guide/troubleshooting.md)

# Batchalign3 — Architecture

- [Python–Rust Boundary](architecture/python-rust-boundary/python-rust-boundary.md)
- [Dispatch and Execution](architecture/runtime/dispatch.md)
- [Command Lifecycles](batchalign/architecture/command-lifecycles.md)
- [Command Flowcharts](batchalign/architecture/command-flowcharts.md)
- [Progress Reporting](batchalign/architecture/progress-reporting.md)
- [Replacements Handling](batchalign/architecture/replacements-handling.md)
- [Preprocessing and Postprocessing](batchalign/architecture/preprocessing-postprocessing.md)
- [ASR Token Pipeline](batchalign/architecture/asr-token-pipeline.md)
- [Number Expansion](batchalign/architecture/number-expansion.md)
- [Language Routing](architecture/language-and-multilingual/language-routing.md)
- [CHAT Parsing (Rust)](batchalign/architecture/chat-parsing.md)
- [Server Architecture](batchalign/architecture/server-architecture.md)
- [Dashboard Architecture](architecture/runtime/dashboard-architecture.md)
- [Batchalign Workers](architecture/runtime/batchalign-workers.md)
- [Cantonese and CJK — Architecture](architecture/language-and-multilingual/cantonese-and-cjk.md)
- [Audio-Task Cache](architecture/runtime/audio-task-cache.md)
- [Validation Cache](architecture/parser-and-grammar/validation-cache.md)
- [Cache Override Guide](batchalign/architecture/cache-override-guide.md)
- [Validation](architecture/errors-and-validation/validation.md)
- [Errors — Batchalign Runtime](architecture/errors-and-validation/batchalign-errors.md)
- [Errors at the Python ↔ Rust Boundary](architecture/errors-and-validation/python-rust-errors.md)
- [Command Contracts](batchalign/architecture/command-contracts.md)
- [NLP Pipeline Decision Architecture](batchalign/architecture/pipeline-decisions.md)
- [Morphotag Reconciliation Invariants](batchalign/architecture/morphotag-invariants.md)
- [Type-Driven Design](batchalign/architecture/type-driven-design.md)
- [Typed Path Provenance](batchalign/architecture/path-provenance.md)
- [Stanza Capability Registry](batchalign/architecture/stanza-capability-registry.md)
- [Stanza Defect Mitigation Map](batchalign/architecture/stanza-defect-mitigation-map.md)
- [Observability](batchalign/architecture/observability.md)
- [Incremental Processing](batchalign/architecture/incremental-processing.md)
- [Server Model Loading](batchalign/architecture/server-model-loading.md)
- [Dynamic Programming](architecture/parser-and-grammar/dynamic-programming.md)
- [Test Server and Worker Lifecycle](batchalign/architecture/test-server-lifecycle.md)
- [Align Throughput](batchalign/architecture/align-throughput.md)
- [Proportional FA Estimation](architecture/alignment/proportional-fa-estimation.md)
- [Overlap Encoding (`&*` and `+<`)](batchalign/architecture/overlap-encoding.md)
- [Algorithm Visualizations](batchalign/architecture/algorithm-visualizations.md)
- [Processing Provenance](batchalign/architecture/provenance.md)
- [Time Transparency UX Principle](batchalign/architecture/time-transparency.md)
- [Worker Failure Classification](batchalign/architecture/worker-failure-classification.md)

# Batchalign3 — Technical Reference

- [CHAT Format](batchalign/reference/chat-format.md)
- [`@Options` and Per-File Command Scoping](batchalign/reference/chat-options.md)
- [Morphosyntax Pipeline](batchalign/reference/morphosyntax.md)
- [%gra Format Conventions](batchalign/reference/gra-format.md)
- [Forced Alignment](batchalign/reference/forced-alignment.md)
- [Whisper ASR](batchalign/reference/whisper-asr.md)
- [HuggingFace Whisper Fine-tunes (`whisper_hub`)](batchalign/reference/whisper-hub-asr.md)
- [Rev.AI Language Quality Strategy](batchalign/reference/revai-language-quality-strategy.md)
- [Retrace Detection](batchalign/reference/retrace-detection.md)
- [Multilingual Support](batchalign/reference/multilingual.md)
- [Language-Specific Processing](batchalign/reference/language-specific-processing.md)
- [Language Code Resolution](batchalign/reference/language-code-resolution.md)
- [Language Data Model](batchalign/reference/language-handling.md)
- [L2 & Language Switching](batchalign/reference/l2-handling.md)
- [Multi-Word Tokens](batchalign/reference/mwt-handling.md)
- [Retokenization — Overview](batchalign/reference/retokenization-overview.md)
- [English Transcribe Corrections](batchalign/reference/english-transcribe-corrections.md)
- [Morphotag Retokenization](batchalign/reference/morphotag-retokenization.md)
- [Stanza Limitations (versioned)](batchalign/reference/stanza-limitations.md)
- [Language Support Overview](batchalign/reference/languages/overview.md)
  - [Cantonese](batchalign/reference/languages/cantonese.md)
  - [Mandarin](batchalign/reference/languages/mandarin.md)
  - [Japanese](batchalign/reference/languages/japanese.md)
  - [Hebrew](batchalign/reference/languages/hebrew.md)
  - [French](batchalign/reference/languages/french.md)
  - [Italian](batchalign/reference/languages/italian.md)
  - [Portuguese](batchalign/reference/languages/portuguese.md)
  - [Dutch](batchalign/reference/languages/dutch.md)
  - [Malayalam](batchalign/reference/languages/malayalam.md)
- [Chinese/Cantonese Word Segmentation](batchalign/reference/chinese-word-segmentation.md)
- [Japanese Morphosyntax (detailed)](batchalign/reference/japanese-morphosyntax.md)
- [Hebrew Morphosyntax (detailed)](batchalign/reference/hebrew-morphosyntax.md)
- [Number Expansion](batchalign/reference/number-expansion.md)
- [Utterance Segmentation](batchalign/reference/utterance-segmentation.md)
- [%wor Tier](batchalign/reference/wor-tier.md)
- [TextGrid Format](batchalign/reference/textgrid.md)
- [Media Conversion](batchalign/reference/media-conversion.md)
- [Command I/O Parity](batchalign/reference/command-io.md)
- [Filesystem Paths](batchalign/reference/filesystem-paths.md)
- [Overlapping Speech](batchalign/reference/overlap-markers.md)
- [Benchmarks](batchalign/reference/benchmarks.md)
- [L2 Morphotag (code-switching)](batchalign/reference/l2-morphotag.md)
  - [Status](batchalign/reference/l2-morphotag-status.md)
- [Transcriber $POS Hints](batchalign/reference/pos-hints.md)
- [L2 Morphotag Literature Review](batchalign/reference/l2-morphotag-literature.md)
- [NLP Engine Text Input Contract](batchalign/reference/nlp-engine-text-input.md)
- [Cross-Repo Dependency Contract](batchalign/reference/cross-repo-dependency.md)
- [Platform Support](batchalign/reference/platform-support.md)

# Batchalign3 — Developer Guide

- [Building & Development](batchalign/developer/building.md)
- [BA3 Cutover Landing Status](batchalign/developer/landing-status.md)
- [Testing](batchalign/developer/testing.md)
  - [Deterministic Simulation (turmoil)](batchalign/developer/testing-turmoil.md)
  - [Regression Fixtures](batchalign/developer/regression-fixtures.md)
  - [Investigation Probe Harnesses](batchalign/developer/investigation-probe-harnesses.md)
- [Model Downloads and Caching](batchalign/developer/model-downloads-and-caching.md)
- [API Stability](batchalign/developer/api-stability.md)
- [Adding New Engines](batchalign/developer/adding-engines.md)
- [Adding Language Support](batchalign/developer/adding-language-support.md)
- [Command Developer Reference](batchalign/developer/adding-commands.md)
  - [align](batchalign/developer/commands/align.md)
  - [transcribe](batchalign/developer/commands/transcribe.md)
  - [morphotag](batchalign/developer/commands/morphotag.md)
  - [utseg](batchalign/developer/commands/utseg.md)
  - [translate](batchalign/developer/commands/translate.md)
  - [coref](batchalign/developer/commands/coref.md)
  - [compare](batchalign/developer/commands/compare.md)
  - [benchmark](batchalign/developer/commands/benchmark.md)
  - [opensmile](batchalign/developer/commands/opensmile.md)
  - [avqi](batchalign/developer/commands/avqi.md)
- [Rust Contributor Onboarding](batchalign/developer/rust-contributor-onboarding.md)
- [Rust Core (batchalign_core)](batchalign/developer/rust-core.md)
- [Rust Workspace Map](batchalign/developer/rust-workspace-map.md)
- [Workflow Contributor Guide](batchalign/developer/workflow-contributor-guide.md)
- [Decision Provenance](batchalign/developer/decision-provenance.md)
- [Terminator Architecture](batchalign/developer/terminator-architecture.md)
- [Overlap-Aware Alignment Improvements](batchalign/developer/backchannel-aware-alignment.md)
- [Worker Protocol V2](batchalign/developer/worker-protocol-v2.md)
- [Host Facts Pipeline](batchalign/developer/host-facts.md)
- [Rust CLI and Server](batchalign/developer/rust-cli-and-server.md)
- [HTTP Request Body Limits](batchalign/developer/http-body-limits.md)
- [Plugin Removal Notes](batchalign/developer/plugins.md)
- [Upstream Defect Policy](batchalign/developer/upstream-defect-policy.md)
- [Non-English Workarounds](batchalign/developer/non-english-workarounds.md)
- [%gra Correctness Guarantee](batchalign/developer/gra-correctness-guarantee.md)
- [Python Versioning](batchalign/developer/python-versioning.md)
- [IPC Type Sync (Rust→Python)](batchalign/developer/ipc-type-sync.md)
- [Tracing and Debugging](batchalign/developer/tracing-and-debugging.md)
- [Debugging Infrastructure](batchalign/developer/debugging-infrastructure.md)
- [CHAT Validation Failures](batchalign/developer/chat-validation-failures.md)
- [Apple MPS Workarounds](batchalign/developer/apple-mps-workarounds.md)
- [Arena Allocators](batchalign/developer/arena-allocators.md)
- [Maturin Build and PyO3 Surface](batchalign/developer/maturin-pyo3-surface.md)
- [Tauri + React Dashboard](batchalign/developer/tauri-react-dashboard.md)
- [Memory Safety](batchalign/developer/memory-safety.md)
- [Release Checklist](batchalign/developer/release-checklist.md)
- [Release Contract](batchalign/developer/release-contract.md)
- [Reliability Program](batchalign/developer/reliability-program.md)

# Batchalign3 — Design Decisions

- [Models Training Runtime ADR](batchalign/decisions/models-training-runtime-adr.md)
- [Lenient Parsing](batchalign/decisions/lenient-parsing.md)
- [Trait-Based Dispatch](batchalign/decisions/trait-based-dispatch.md)
- [Release State Machine](batchalign/decisions/release-state-machine.md)

# chatter — User Guide

- [Installation](chatter/user-guide/installation.md)
  - [Quick Start](chatter/user-guide/quick-start.md)
- [CLI Reference](chatter/user-guide/cli-reference.md)
- [Migrating from CLAN](chatter/user-guide/migrating-from-clan.md)
- [Validation Errors](chatter/user-guide/validation-errors.md)
- [VS Code Extension](chatter/user-guide/vscode-extension.md)
- [Chatter Desktop (Experimental)](chatter/user-guide/desktop-app.md)
- [CLAN Line Numbering](chatter/user-guide/clan-line-numbering.md)
- [Batch Workflows](chatter/user-guide/batch-workflows.md)
- [CI Integration](chatter/user-guide/ci-integration.md)
- [CHAT Processing Playbook (Editors & Analysts)](chatter/user-guide/chat-processing-playbook.md)
- [Sanitize (Protected Corpora)](chatter/user-guide/sanitize.md)

# VS Code — Getting Started

- [Installation](apps/vscode-extension/getting-started/installation.md)
- [Your First CHAT File](apps/vscode-extension/getting-started/first-file.md)
- [Quick Reference](apps/vscode-extension/getting-started/quick-reference.md)

# VS Code — Editing

- [Syntax Highlighting](apps/vscode-extension/editing/syntax-highlighting.md)
- [Real-Time Validation](apps/vscode-extension/editing/validation.md)
- [Quick Fixes](apps/vscode-extension/editing/quick-fixes.md)
- [Code Completion & Snippets](apps/vscode-extension/editing/completion.md)
- [Special Characters](apps/vscode-extension/editing/special-characters.md)
- [Participant Editor](apps/vscode-extension/editing/participant-editor.md)

# VS Code — Navigation

- [Document Symbols](apps/vscode-extension/navigation/symbols.md)
- [Go to Definition](apps/vscode-extension/navigation/go-to-definition.md)
- [Cross-Tier Alignment](apps/vscode-extension/navigation/alignment.md)
- [Dependency Graphs](apps/vscode-extension/navigation/dependency-graphs.md)
- [Speaker Filtering](apps/vscode-extension/navigation/speaker-filtering.md)
- [Scoped Find](apps/vscode-extension/navigation/scoped-find.md)

# VS Code — Media & Transcription

- [Media Playback](apps/vscode-extension/media/playback.md)
- [Waveform Visualization](apps/vscode-extension/media/waveform.md)
- [Walker Mode](apps/vscode-extension/media/walker.md)
- [Transcription Mode](apps/vscode-extension/media/transcription.md)
- [Media Resolution](apps/vscode-extension/media/resolution.md)

# VS Code — Analysis

- [Running CLAN Commands](apps/vscode-extension/analysis/running-commands.md)
- [Profiling Commands](apps/vscode-extension/analysis/profiling.md)
- [Frequency & Distribution](apps/vscode-extension/analysis/frequency.md)
- [Assessment Tools](apps/vscode-extension/analysis/assessment.md)
- [Command Reference](apps/vscode-extension/analysis/command-reference.md)

# VS Code — Review Mode

- [Overview](apps/vscode-extension/review/overview.md)
- [Tutorial: Reviewing Aligned Files](apps/vscode-extension/review/tutorial.md)
- [Rating Utterances](apps/vscode-extension/review/rating.md)
- [Interactive Bullet Correction](apps/vscode-extension/review/correction.md)
- [Harvesting Results](apps/vscode-extension/review/harvesting.md)

# VS Code — Coder Mode

- [Overview](apps/vscode-extension/coder/overview.md)
- [Codes Files (.cut)](apps/vscode-extension/coder/codes-files.md)
- [Coding Workflow](apps/vscode-extension/coder/workflow.md)

# VS Code — Workflows

- [Corpus Validation](apps/vscode-extension/workflows/corpus-validation.md)
- [Transcription from Audio](apps/vscode-extension/workflows/transcription.md)
- [Post-Alignment Review](apps/vscode-extension/workflows/post-alignment-review.md)
- [Batch Processing](apps/vscode-extension/workflows/batch-processing.md)

# VS Code — Configuration

- [Settings Reference](apps/vscode-extension/configuration/settings.md)
- [Keyboard Shortcuts](apps/vscode-extension/configuration/keyboard-shortcuts.md)
- [Cache Management](apps/vscode-extension/configuration/cache.md)

# VS Code — Troubleshooting

- [Common Issues](apps/vscode-extension/troubleshooting/common-issues.md)
- [LSP Connection](apps/vscode-extension/troubleshooting/lsp.md)
- [Media Not Found](apps/vscode-extension/troubleshooting/media.md)

---

# VS Code — For Developers

- [Architecture](apps/vscode-extension/developer/architecture.md)
- [LSP Protocol](apps/vscode-extension/developer/lsp-protocol.md)
- [Adding Features](apps/vscode-extension/developer/adding-features.md)
- [Custom Commands](apps/vscode-extension/developer/custom-commands.md)
- [Testing](apps/vscode-extension/developer/testing.md)
- [Releasing](apps/vscode-extension/developer/releasing.md)
- [CLAN Feature Parity](apps/vscode-extension/developer/clan-parity.md)

---

# VS Code — Reference

- [Alignment Index Spaces](apps/vscode-extension/reference/alignment-indices.md)
- [RPC Contracts](apps/vscode-extension/reference/rpc-contracts.md)
- [Webview Message Contracts](apps/vscode-extension/reference/webview-contracts.md)
- [Command Catalog](apps/vscode-extension/reference/commands.md)

---

# VS Code — Design Decisions (ADRs)

- [ADR-001: LSP over Embedded Parser](apps/vscode-extension/design/adr-001-lsp-over-embedded-parser.md)
- [ADR-002: Effect-based Command Runtime](apps/vscode-extension/design/adr-002-effect-runtime.md)
- [ADR-003: Webview Panels over TreeView](apps/vscode-extension/design/adr-003-webview-panels-over-treeview.md)
- [ADR-004: Bundled LSP Binary](apps/vscode-extension/design/adr-004-bundled-lsp-binary.md)

# CHAT Format

- [Overview](chat-format/overview.md)
- [Headers](chat-format/headers.md)
- [Utterances](chat-format/utterances.md)
- [Retraces and Repetitions](chat-format/retraces.md)
- [Replacements](chat-format/replacements.md)
- [Untranscribed Markers (xxx, yyy, www)](chat-format/untranscribed-markers.md)
- [Postcodes](chat-format/postcodes.md)
- [Dependent Tiers](chat-format/dependent-tiers.md)
  - [The %mor Tier](chat-format/mor-tier.md)
  - [Phon Tiers](chat-format/phon-tiers.md)
- [Word Syntax](chat-format/word-syntax.md)
- [Word Internals](chat-format/word-internals.md)
- [Symbols](chat-format/symbols.md)


# CLAN — Getting Started

- [Installation](clan-reference/getting-started/installation.md)
- [Quick Start](clan-reference/getting-started/quick-start.md)
- [Migrating from CLAN](clan-reference/getting-started/migration.md)
- [Flag Translation Guide](clan-reference/getting-started/flag-translation.md)

# CLAN — User Guide

- [Running Commands](clan-reference/user-guide/running-commands.md)
- [Filtering](clan-reference/user-guide/filtering.md)
  - [Speaker Filtering](clan-reference/user-guide/filtering-speakers.md)
  - [Tier Filtering](clan-reference/user-guide/filtering-tiers.md)
  - [Word Filtering](clan-reference/user-guide/filtering-words.md)
  - [Gem Filtering](clan-reference/user-guide/filtering-gems.md)
  - [Utterance Range](clan-reference/user-guide/filtering-range.md)
- [Output Formats](clan-reference/user-guide/output-formats.md)
- [Working with Directories](clan-reference/user-guide/directories.md)
- [Pipelines and Scripting](clan-reference/user-guide/pipelines.md)

# CLAN — Validation

- [CHECK — CHAT File Validation](clan-reference/commands/check.md)

# CLAN — Analysis Commands

- [FREQ — Word Frequency](clan-reference/commands/freq.md)
- [GEMFREQ — Word Frequency Within Gem Segments](clan-reference/commands/gemfreq.md)
- [MLU — Mean Length of Utterance](clan-reference/commands/mlu.md)
- [MLT — Mean Length of Turn](clan-reference/commands/mlt.md)
- [VOCD — Vocabulary Diversity](clan-reference/commands/vocd.md)
- [DSS — Developmental Sentence Scoring](clan-reference/commands/dss.md)
- [EVAL — Language Sample Evaluation](clan-reference/commands/eval.md)
- [EVAL-D — Evaluation (DementiaBank)](clan-reference/commands/eval-d.md)
- [KIDEVAL — Child Language Evaluation](clan-reference/commands/kideval.md)
- [IPSYN — Index of Productive Syntax](clan-reference/commands/ipsyn.md)
- [FLUCALC — Fluency Calculation](clan-reference/commands/flucalc.md)
- [SUGAR — Grammatical Analysis](clan-reference/commands/sugar.md)
- [MORTABLE — Morphology Tables](clan-reference/commands/mortable.md)
- [KWAL — Keyword and Line](clan-reference/commands/kwal.md)
- [COMBO — Boolean Search](clan-reference/commands/combo.md)
- [CODES — Code Frequency](clan-reference/commands/codes.md)
- [COMPLEXITY — Syntactic Complexity](clan-reference/commands/complexity.md)
- [CORELEX — Core Vocabulary](clan-reference/commands/corelex.md)
- [CHAINS — Clause Chain Analysis](clan-reference/commands/chains.md)
- [CHIP — Interaction Profile](clan-reference/commands/chip.md)
- [COOCCUR — Word Co-occurrence](clan-reference/commands/cooccur.md)
- [DIST — Word Distribution](clan-reference/commands/dist.md)
- [FREQPOS — Positional Frequency](clan-reference/commands/freqpos.md)
- [GEMLIST — Gem Segments](clan-reference/commands/gemlist.md)
- [KEYMAP — Contingency Tables](clan-reference/commands/keymap.md)
- [MAXWD — Longest Words](clan-reference/commands/maxwd.md)
- [MODREP — Model/Replica Comparison](clan-reference/commands/modrep.md)
- [PHONFREQ — Phonological Frequency](clan-reference/commands/phonfreq.md)
- [RELY — Inter-rater Agreement](clan-reference/commands/rely.md)
- [SCRIPT — Template Comparison](clan-reference/commands/script.md)
- [TIMEDUR — Time Duration](clan-reference/commands/timedur.md)
- [TRNFIX — Tier Comparison](clan-reference/commands/trnfix.md)
- [UNIQ — Repeated Utterances](clan-reference/commands/uniq.md)
- [WDLEN — Word Length Distribution](clan-reference/commands/wdlen.md)
- [WDSIZE — Word Size Distribution](clan-reference/commands/wdsize.md)

# CLAN — Transform Commands

- [CHSTRING — String Replacement](clan-reference/commands/chstring.md)
- [COMBTIER — Combine Tiers](clan-reference/commands/combtier.md)
- [COMPOUND — Compound Normalization](clan-reference/commands/compound.md)
- [DATACLEAN — Format Cleanup](clan-reference/commands/dataclean.md)
- [DATES — Age Computation](clan-reference/commands/dates.md)
- [DELIM — Add Terminators](clan-reference/commands/delim.md)
- [FIXIT — Normalize Formatting](clan-reference/commands/fixit.md)
- [FIXBULLETS — Timing Repair](clan-reference/commands/fixbullets.md)
- [FLO — Fluent Output](clan-reference/commands/flo.md)
- [GEM — Gem Extraction](clan-reference/commands/gem.md)
- [INDENT — Align Overlap Markers](clan-reference/commands/indent.md)
- [LINES — Line Numbers](clan-reference/commands/lines.md)
- [LONGTIER — Remove Line Wrapping](clan-reference/commands/longtier.md)
- [LOWCASE — Lowercase](clan-reference/commands/lowcase.md)
- [MAKEMOD — Model Tier](clan-reference/commands/makemod.md)
- [ORT — Orthographic Conversion](clan-reference/commands/ort.md)
- [POSTMORTEM — Mor Post-processing](clan-reference/commands/postmortem.md)
- [QUOTES — Extract Quotes](clan-reference/commands/quotes.md)
- [REPEAT — Mark Repetitions](clan-reference/commands/repeat.md)
- [RETRACE — Retrace Tier](clan-reference/commands/retrace.md)
- [ROLES — Rename Speakers](clan-reference/commands/roles.md)
- [TIERORDER — Reorder Tiers](clan-reference/commands/tierorder.md)
- [TRIM — Remove Dependent Tiers](clan-reference/commands/trim.md)

# CLAN — Format Converters

- [CHAT2TEXT — CHAT to Plain Text](clan-reference/commands/chat2text.md)
- [CHAT2ELAN — CHAT to ELAN](clan-reference/commands/chat2elan.md)
- [CHAT2PRAAT — CHAT to Praat TextGrid](clan-reference/commands/chat2praat.md)
- [CHAT2SRT — CHAT to SRT Subtitles](clan-reference/commands/chat2srt.md)
- [CHAT2VTT — CHAT to WebVTT Subtitles](clan-reference/commands/chat2vtt.md)
- [ELAN2CHAT — ELAN to CHAT](clan-reference/commands/elan2chat.md)
- [LAB2CHAT — LAB to CHAT](clan-reference/commands/lab2chat.md)
- [LENA2CHAT — LENA to CHAT](clan-reference/commands/lena2chat.md)
- [LIPP2CHAT — LIPP to CHAT](clan-reference/commands/lipp2chat.md)
- [PLAY2CHAT — PLAY to CHAT](clan-reference/commands/play2chat.md)
- [PRAAT2CHAT — Praat TextGrid](clan-reference/commands/praat2chat.md)
- [RTF2CHAT — Rich Text to CHAT](clan-reference/commands/rtf2chat.md)
- [SALT2CHAT — SALT to CHAT](clan-reference/commands/salt2chat.md)
- [SRT2CHAT — Subtitles to CHAT](clan-reference/commands/srt2chat.md)
- [TEXT2CHAT — Plain Text to CHAT](clan-reference/commands/text2chat.md)

# CLAN — Architecture & Developer Guide

- [Design Philosophy](clan-reference/architecture/philosophy.md)
- [Framework](clan-reference/architecture/framework.md)
- [Adding a Command](clan-reference/developer/adding-command.md)
- [Current Architecture Seams](clan-reference/developer/architecture-seams.md)
- [Testing Strategy](clan-reference/developer/testing.md)
- [Golden Tests](clan-reference/developer/golden-tests.md)
- [Coding Standards](clan-reference/developer/coding-standards.md)

# CLAN — Divergences & Migration

- [Why We Diverge](clan-reference/divergences/philosophy.md)
- [Framework-Level Divergences](clan-reference/divergences/framework.md)
- [Per-Command Divergences](clan-reference/divergences/per-command.md)
- [CHECK vs chatter validate](clan-reference/divergences/check-vs-validate.md)
- [Commands Not Ported](clan-reference/divergences/not-ported.md)
- [Parity Plan](clan-reference/divergences/parity-plan.md)

# CLAN — Appendices

- [Flag Translation Guide](clan-reference/appendices/flag-mapping.md)
- [CLAN Manual Audit](clan-reference/appendices/clan-manual-audit.md)
- [Command Status Matrix](clan-reference/appendices/status-matrix.md)
- [Dependent Tier Semantics](clan-reference/appendices/dependent-tier-semantics.md)
- [Transform Taxonomy](clan-reference/appendices/transform-taxonomy.md)
- [Glossary](clan-reference/appendices/glossary.md)
- [Improvements Log](clan-reference/appendices/improvements.md)

# chatter — Architecture

- [Overview](architecture/overview.md)
- [Spec System](architecture/spec-system.md)
- [Grammar](architecture/grammar.md)
- [Parsing](architecture/parsing.md)
- [CHAT Data Model](architecture/chat-model/chat-model.md)
- [Transform Pipeline](architecture/transform-pipeline.md)
- [XML Emitter](architecture/xml-emitter.md)
- [Errors — talkbank-tools](architecture/errors-and-validation/talkbank-tools-errors.md)
- [Crate Reference](architecture/crate-reference.md)
- [Repo Architecture](architecture/repo-architecture.md)
- [Grammar Governance](architecture/grammar-governance.md)
- [Parser-Model Contracts](architecture/parser-model-contracts.md)
- [Parser Backends](architecture/parser-backends.md)
- [Leniency Policy](architecture/leniency-policy.md)
- [Error Diagnostics UX](architecture/errors-and-validation/error-diagnostics-ux.md)
- [Wide Struct Audit](architecture/chat-model/wide-structs.md)
- [Spec Tooling](architecture/spec-tooling.md)
- [Symbol Registry](architecture/symbol-registry.md)
- [Bullet Validation](architecture/bullet-validation.md)
- [CA Terminator Resolution](architecture/parser-and-grammar/ca-terminator-resolution.md)
- [Alignment](architecture/alignment.md)
- [Memory and Ownership](architecture/memory-and-ownership.md)
- [Algorithms and Data Structures](architecture/algorithms.md)
- [Concurrency](architecture/runtime/concurrency.md)
- [Performance Optimizations](architecture/runtime/performance-optimizations.md)

# Contributing

- [Setup](contributing/setup.md)
- [Grammar Workflow](contributing/grammar-workflow.md)
- [Spec Workflow](contributing/spec-workflow.md)
- [Testing](contributing/testing.md)
- [Coding Standards](contributing/coding-standards.md)
- [Coding Standards (Extended)](contributing/coding-standards-extended.md)
- [Parameter Design](contributing/parameter-design.md)
- [CLI Option Wiring](contributing/cli-option-wiring.md)
- [CI and Release](contributing/ci-and-release.md)
- [Quality Gates](contributing/quality-gates.md)
- [Documentation Architecture](contributing/documentation-architecture.md)
- [CHAT Processing Playbook (Developers)](contributing/chat-processing-playbook.md)
- [Open-Source Governance](contributing/open-source-governance.md)
- [Compile Times](contributing/compile-times.md)
- [Dev Checks](contributing/dev-checks.md)
- [Branch Protection](contributing/branch-protection.md)
- [Reference Corpus](contributing/reference-corpus.md)
- [Desktop App Testing](contributing/desktop-testing.md)

# chatter — Integrating

- [Library Usage](chatter/integrating/library-usage.md)
- [JSON Output Reference](chatter/integrating/json-output.md)
- [JSON Schema](chatter/integrating/json-schema.md)
- [Diagnostic Contract](chatter/integrating/diagnostic-contract.md)
