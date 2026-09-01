//! `MorphosyntaxTaskRunner` — UD POS / `%mor` / `%gra` tagger (spec2.md §5, §6).
//!
//! ## What it does
//!
//! For each utterance in a `BAValue::Chat`:
//!
//! 1. Walks the main tier with [`walk_words`] to recover the upstream
//!    tokenization (Word / ReplacedWord items only — separators and punctuation
//!    on the main tier do not align with `%mor` slots).
//! 2. Builds a per-utterance [`MorphosyntaxInput`] and dispatches it through
//!    the engine.
//! 3. Converts the structured [`MorphosyntaxOutput`] into **typed**
//!    `talkbank_model` [`MorTier`] / [`GraTier`] values and attaches them as
//!    [`DependentTier::Mor`] / [`DependentTier::Gra`].
//!
//! ## Typed construction, never strings
//!
//! The backend returns a structured analysis (per word: a head morpho-unit plus
//! `~`-post-clitics; per chunk: a `%gra` triple). This runner maps that onto the
//! typed model with [`try_align_mor_gra`], which guarantees `%mor`/`%gra`
//! chunk-count alignment as a construction invariant, then lets the official
//! CHAT writer serialize the tiers. We never assemble `%mor:`/`%gra:` text by
//! hand — building CHAT by string concatenation is forbidden (see `CLAUDE.md`).
//!
//! ## Retokenize semantics
//!
//! Two modes, controlled by `MorphosyntaxConfig::retokenize`:
//!
//! - `retokenize = false` (default). Mirrors BA2's `retokenize=False` path.
//!   The upstream main-tier tokenization is authoritative; the backend produces
//!   one [`MorphosyntaxToken`] per input token (clitics live inside a token's
//!   `units`, so a contraction still counts as one main-tier word). This is the
//!   right mode for already-segmented CHAT documents.
//! - `retokenize = true`. Mirrors BA2's `retokenize=True` path. The backend may
//!   resplit tokens (expanding `gonna` → `going to`). The number of emitted
//!   `%mor` words may then differ from the input token count.
//!
//! In both modes the runner ships the raw token list AND the joined text so
//! the backend can reconstruct whichever signal it needs.
//!
//! ## UD-only `%mor` syntax
//!
//! Output uses Universal Dependencies syntax exclusively: `verb|run-Past`,
//! `noun|cat-Plur`. CLAN MOR's `&`-style fusional markers (`aux|be&PRES`) are
//! never emitted. See `CLAUDE.md` §17.3 (project policy: UD-only).

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::Task;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::LanguageSpec;
use crate::proto::morphosyntax::{
    MorphosyntaxInput, MorphosyntaxOutput, MorphosyntaxToken, MorphosyntaxUnit,
};
use crate::utils::{BAError, BAResult};
use async_trait::async_trait;
use futures::{StreamExt, TryStreamExt};
use smol_str::SmolStr;
use talkbank_model::ParseError;
use talkbank_model::Span;
use talkbank_model::alignment::helpers::TierDomain;
use talkbank_model::alignment::{MorGraTerminatorSlot, align_main_to_mor, try_align_mor_gra};
use talkbank_model::model::{
    ChatOptionFlag, DependentTier, GraTier, GrammaticalRelation, Mor, MorFeature, MorStem, MorTier,
    MorWord, PosCategory, Terminator,
};
use talkbank_model::validation::GoverningMarkKind;
use talkbank_model::{Line, Utterance};
use talkbank_transform::extract::collect_utterance_content;

/// Keep each CHAT's pending utterances bounded at the same default admission
/// budget as the engine. Eight requests are enough for the backend batcher to
/// coalesce same-language work without materializing one future per utterance.
const MORPHOTAG_DISPATCH_WINDOW: usize = 8;

/// Runner that drops typed `%mor` and `%gra` tiers on a CHAT document.
pub struct MorphosyntaxTaskRunner;

#[async_trait]
impl TaskRunner for MorphosyntaxTaskRunner {
    const TASK: Task = Task::Morphosyntax;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        match value {
            BAValue::Chat(chat) => process_chat(chat, dispatcher, sink.clone()).await,
            // `Paired` is what Compare consumes; running morphosyntax over it
            // means tagging both main and gold, so the downstream
            // CompareBackend can lift POS off the `%mor` tier per token.
            BAValue::Paired(p) => {
                let (main, gold) = p.as_mut_parts();
                process_chat(main, dispatcher, sink.clone()).await?;
                process_chat(gold, dispatcher, sink.clone()).await?;
                Ok(())
            }
            BAValue::Failed { .. } => Ok(()),
            other => Err(BAError::Internal(format!(
                "Morphosyntax expects BAValue::Chat or BAValue::Paired, got {}",
                other.kind()
            ))),
        }
    }
}

/// Run morphosyntax on one CHAT in place. Used both by the `BAValue::Chat`
/// path and twice in the `BAValue::Paired` path (main then gold). Language
/// defaults from the chat's `@Languages:` header and is overridden by an
/// utterance precode such as `[- hin]`; backends that want to pin a language
/// do so via their own constructor.
async fn process_chat(
    chat: &mut Chat,
    dispatcher: &dyn Dispatcher,
    sink: std::sync::Arc<dyn ProgressSink>,
) -> BAResult<()> {
    use crate::base::ScaledProgress;
    use std::sync::Arc;

    let source_id = chat.source_id().clone();
    sink.emit(ProgressEvent::stage_started(&source_id, Task::Morphosyntax));
    let t_start = std::time::Instant::now();

    // Conversation Analysis transcripts deliberately bypass morphotagging.
    // CA notation is not ordinary lexical input, and this is a legitimate
    // not-applicable case rather than an inference failure.
    if chat
        .ast()
        .options
        .iter()
        .any(|option| matches!(option, ChatOptionFlag::Ca))
    {
        sink.emit(ProgressEvent::stage_injected(
            &source_id,
            Task::Morphosyntax,
        ));
        return Ok(());
    }

    let t_phase1 = std::time::Instant::now();
    let batch = MorphotagBatch::new(chat)?;
    tracing::info!(
        target: "batchalign::morphosyntax",
        sid = %source_id,
        phase = "extract_tokens",
        utterances = batch.len(),
        elapsed_ms = t_phase1.elapsed().as_millis() as u64,
    );

    // Phase 2: dispatch only for utterances missing `%mor`. Track the source
    // utterance index alongside each output so injection can apply them to
    // the right slots while leaving pre-tagged utterances untouched.
    //
    // Progress: this runner owns the per-utterance loop and the backend
    // (Stanza) reports nothing — so we drive the outer step via
    // `ScaledProgress::start_step` and pass the same Arc as the
    // `BackendProgress` handle to `dispatch_with_progress`. The backend
    // ignores it; only `start_step` advances the bar. `total_to_tag`
    // excludes already-tagged utterances — the bar reflects real work
    // to do, matching BA2's `status_hook` semantics.
    let total_to_tag = batch.len() as u64;
    let progress = Arc::new(ScaledProgress::new(
        sink.clone(),
        source_id.clone(),
        Task::Morphosyntax,
        total_to_tag,
    ));
    let progress_dyn: Arc<dyn crate::base::BackendProgress> = progress.clone();
    let t_dispatch = std::time::Instant::now();
    let dispatched = batch
        .dispatch(dispatcher, progress.clone(), progress_dyn)
        .await?;
    // Final ceiling tick so the bar reaches 100% after the loop.
    progress.finish();

    tracing::info!(
        target: "batchalign::morphosyntax",
        sid = %source_id,
        phase = "dispatch",
        dispatched = dispatched.len(),
        elapsed_ms = t_dispatch.elapsed().as_millis() as u64,
    );

    // Phase 3: build typed tiers and inject into the utterances we tagged
    // — but only when main↔`%mor` count agrees. Mismatched utterances are
    // skipped (no partial `%mor`/`%gra`); the main tier is left untouched
    // and a per-utterance warning is logged. This is the "skip-per-utt"
    // contract: one bad utterance doesn't fail the whole file.
    let t_inject = std::time::Instant::now();
    let inject_stats = inject_mor_gra_tiers_selective(chat, &dispatched)?;
    tracing::info!(
        target: "batchalign::morphosyntax",
        sid = %source_id,
        phase = "inject",
        injected = inject_stats.injected,
        skipped_mismatch = inject_stats.skipped_count_mismatch,
        skipped_empty = inject_stats.skipped_empty_output,
        elapsed_ms = t_inject.elapsed().as_millis() as u64,
    );
    if inject_stats.skipped_count_mismatch > 0 {
        tracing::info!(
            target: "batchalign::morphosyntax",
            sid = %source_id,
            "morphotag: skipped %mor/%gra on {} utterance(s) due to main↔%mor count mismatch",
            inject_stats.skipped_count_mismatch,
        );
    }

    tracing::info!(
        target: "batchalign::morphosyntax",
        sid = %source_id,
        phase = "total",
        elapsed_ms = t_start.elapsed().as_millis() as u64,
    );

    sink.emit(ProgressEvent::stage_injected(
        &source_id,
        Task::Morphosyntax,
    ));
    Ok(())
}

/// ISO-639-3 languages for which the Batchalign Stanza configuration has a
/// known complete tokenize/POS/lemma/depparse pipeline. Keep this sorted so
/// both membership checks and the user-facing error remain deterministic.
const SUPPORTED_STANZA_CODES: &[&str] = &[
    "afr", "ara", "bul", "cat", "ces", "cmn", "cym", "dan", "deu", "ell", "eng", "est", "eus",
    "fas", "fin", "fra", "gla", "gle", "glg", "heb", "hin", "hrv", "hun", "hye", "ind", "isl",
    "ita", "jpn", "kat", "kor", "lat", "lav", "lit", "mlt", "nld", "nor", "pol", "por", "ron",
    "rus", "slk", "slv", "spa", "swe", "tam", "tel", "tha", "tur", "ukr", "urd", "vie", "yue",
    "zho",
];

/// The untagged utterances from one CHAT, ready for bounded backend dispatch.
/// Existing `%mor` tiers never enter the batch, so applying morphotag twice is
/// idempotent and does not wake Stanza for already-complete work.
struct MorphotagBatch {
    inputs: Vec<MorphosyntaxInput>,
}

impl MorphotagBatch {
    fn new(chat: &Chat) -> BAResult<Self> {
        if let Some(primary) = chat.primary_language()
            && !is_stanza_supported(&primary)
        {
            return Err(BAError::Validation(format!(
                "morphotag: primary @Languages '{primary}' is not supported by Stanza. \
                 Fix the @Languages header to use a supported ISO-639-3 code and re-run. \
                 Supported codes: {}.",
                SUPPORTED_STANZA_CODES.join(", ")
            )));
        }

        let source_id = chat.source_id().clone();
        let default_language = chat
            .primary_language()
            .map(|code| LanguageSpec::Code(SmolStr::new(code)))
            .unwrap_or(LanguageSpec::PerFile);
        let inputs = chat
            .ast()
            .utterances()
            .enumerate()
            .filter(|(_, utterance)| !utterance_has_mor_tier(utterance))
            .map(|(utterance_id, utterance)| {
                let extracted = extract_input(utterance, &default_language);
                MorphosyntaxInput {
                    source_id: source_id.clone(),
                    utterance_id: utterance_id as u32,
                    language: extracted.language,
                    tokens: extracted.tokens,
                    retokenize: false,
                    text: extracted.text,
                }
            })
            .collect();
        Ok(Self { inputs })
    }

    fn len(&self) -> usize {
        self.inputs.len()
    }

    async fn dispatch(
        self,
        dispatcher: &dyn Dispatcher,
        progress: std::sync::Arc<crate::base::ScaledProgress>,
        backend_progress: std::sync::Arc<dyn crate::base::BackendProgress>,
    ) -> BAResult<Vec<(usize, MorphosyntaxOutput)>> {
        futures::stream::iter(self.inputs.into_iter().map(|input| {
            let progress = progress.clone();
            let backend_progress = backend_progress.clone();
            async move {
                progress.start_step();
                let utterance_id = input.utterance_id as usize;
                let task_out = dispatcher
                    .dispatch_with_progress(input.into(), backend_progress)
                    .await?;
                let output = task_out.try_into()?;
                Ok::<_, BAError>((utterance_id, output))
            }
        }))
        .buffered(MORPHOTAG_DISPATCH_WINDOW)
        .try_collect()
        .await
    }
}

fn is_stanza_supported(language: &str) -> bool {
    SUPPORTED_STANZA_CODES.binary_search(&language).is_ok()
}

fn utterance_has_mor_tier(u: &Utterance) -> bool {
    if u.mor_tier().is_some() {
        return true;
    }
    u.dependent_tiers.iter().any(|t| {
        matches!(
            &t.tier,
            DependentTier::UserDefined(udt) if udt.label.as_str() == "mor"
        )
    })
}

/// NLP input projected from one typed utterance.
///
/// `tokens` keeps the clean main-tier surface forms used for alignment, while
/// `text` retains an `@s` sentinel on word-level language switches. The Stanza
/// backend deliberately masks that sentinel before inference and restores the
/// conventional `L2|xxx` analysis afterward. Keeping the two projections
/// separate prevents CHAT markup from leaking into the authoritative token
/// list while still preserving the code-switch signal across the task API.
struct ExtractedMorphosyntaxInput {
    language: LanguageSpec,
    tokens: Vec<String>,
    text: String,
}

/// Resolve the language used to tag an utterance.
///
/// A main-tier precode (`[- hin]`) is narrower than the file-wide default and
/// must win. Without one, the first `@Languages:` code remains the transcript
/// default. Inline word/span switches remain explicit `@s` sentinels in the
/// text projection and render as `L2|xxx`; they must not make a whole-document
/// language detector retag the surrounding primary-language words.
fn resolve_utterance_language(u: &Utterance, default: &LanguageSpec) -> LanguageSpec {
    u.main
        .content
        .language_code
        .as_ref()
        .map(|code| LanguageSpec::Code(SmolStr::new(code.as_str())))
        .unwrap_or_else(|| default.clone())
}

/// Extract the clean alignment tokens and the markup-aware Stanza text.
fn extract_input(u: &Utterance, default_language: &LanguageSpec) -> ExtractedMorphosyntaxInput {
    let mut extracted = Vec::new();
    collect_utterance_content(&u.main.content.content, TierDomain::Mor, &mut extracted);

    let tokens: Vec<String> = extracted.iter().map(|word| word.text.to_string()).collect();
    let text = extracted
        .iter()
        .map(|word| {
            let surface = word.text.to_string();
            if !matches!(word.language_kind(), GoverningMarkKind::Utterance) {
                format!("{surface}@s")
            } else {
                surface
            }
        })
        .collect::<Vec<_>>()
        .join(" ");

    ExtractedMorphosyntaxInput {
        language: resolve_utterance_language(u, default_language),
        tokens,
        text,
    }
}

/// Build one typed [`MorWord`] from a structured unit (`pos|lemma-feat...`).
fn build_mor_word(u: &MorphosyntaxUnit) -> MorWord {
    let mut word = MorWord::new(
        PosCategory::new(u.pos.as_str()),
        MorStem::new(u.lemma.as_str()),
    );
    for feat in &u.features {
        if feat.is_empty() {
            continue;
        }
        // `flat` keeps the feature value verbatim (no `Key=Value` splitting);
        // BA2 emits flat UD feature tokens (`Past`, `S3`, …).
        word = word.with_feature(MorFeature::flat(feat.as_str()));
    }
    word
}

/// Build the `%gra` relation for a single unit/chunk, verbatim from the
/// backend's computed `(index, head, deprel)`.
fn build_relation(u: &MorphosyntaxUnit) -> GrammaticalRelation {
    GrammaticalRelation::new(u.index as usize, u.head as usize, u.deprel.as_str())
}

/// Convert one [`MorphosyntaxToken`] into a typed [`Mor`] (head + post-clitics)
/// plus the `%gra` relations for each of its chunks. Returns `None` for an
/// empty-unit token (defensive — backends should never emit one).
fn build_mor(tok: &MorphosyntaxToken) -> Option<(Mor, Vec<GrammaticalRelation>)> {
    let mut units = tok.units.iter();
    let main = units.next()?;
    let mut mor = Mor::new(build_mor_word(main));
    let mut relations = vec![build_relation(main)];
    for clitic in units {
        mor = mor.with_post_clitic(build_mor_word(clitic));
        relations.push(build_relation(clitic));
    }
    Some((mor, relations))
}

/// Assemble the typed `(MorTier, GraTier)` for one tagged utterance.
///
/// `terminator` is the utterance's typed main-tier terminator (`.`/`?`/…),
/// read from the AST by the caller; only the terminator's `%gra` triple comes
/// from the backend. Returns `Ok(None)` for a degenerate analysis (no tokens
/// or no terminator) so the caller injects nothing — matching BA2, which emits
/// no `%mor` line for such utterances.
fn build_tiers(
    out: &MorphosyntaxOutput,
    terminator: Terminator,
) -> BAResult<Option<(MorTier, GraTier)>> {
    if out.tokens.is_empty() {
        return Ok(None);
    }
    let Some(term) = out.terminator.as_ref() else {
        return Ok(None);
    };

    let mut mor_items: Vec<Mor> = Vec::with_capacity(out.tokens.len());
    let mut relations: Vec<GrammaticalRelation> = Vec::new();
    for tok in &out.tokens {
        let Some((mor, rels)) = build_mor(tok) else {
            continue;
        };
        mor_items.push(mor);
        relations.extend(rels);
    }
    if mor_items.is_empty() {
        return Ok(None);
    }

    let slot = MorGraTerminatorSlot {
        terminator,
        relation: GrammaticalRelation::new(
            term.index as usize,
            term.head as usize,
            term.deprel.as_str(),
        ),
    };
    let (mor_tier, gra_tier) =
        try_align_mor_gra(mor_items, relations, slot, Span::DUMMY).map_err(|e| {
            BAError::Internal(format!(
                "mor/gra alignment failed (utterance {}): {e}",
                out.utterance_id
            ))
        })?;
    Ok(Some((mor_tier, gra_tier)))
}

/// Per-file injection bookkeeping for tracing + telemetry.
#[derive(Default)]
struct InjectStats {
    /// Utterances where `%mor`/`%gra` were committed.
    injected: usize,
    /// Backend returned an empty analysis (no tokens / no terminator) —
    /// matches BA2's "no `%mor` line for a degenerate utterance" behavior.
    skipped_empty_output: usize,
    /// Built `(MorTier, GraTier)` candidate did NOT satisfy
    /// [`align_main_to_mor`] for this utterance (E705/E706/E716). The
    /// tiers are discarded; the main tier is left untouched. One bad
    /// utterance no longer fails the whole file.
    skipped_count_mismatch: usize,
}

/// Build typed `%mor`/`%gra` tiers for the dispatched utterances and
/// attach them — but **only** when [`align_main_to_mor`] reports
/// `is_error_free()` for the candidate tier against the utterance's
/// main tier.
///
/// `align_main_to_mor` is the spec'd validator: it knows the CHAT-manual
/// rules for fillers / nonwords / retraces / replacements / xxx / yyy / www
/// via [`Utterance::mor_alignable_word_count`], and it checks E705 / E706
/// (count) plus E716 (terminator value) in one pass. We use it here
/// per-utterance as a typed pre-commit gate — never the homemade count
/// check that loses sight of the domain rules.
///
/// Skipping (rather than failing) a mismatched utterance is intentional:
/// the morphotag pipeline is supposed to be **best-effort per utterance**.
/// A `%mor` we can't align to its main tier is worse than no `%mor` at
/// all, but it isn't a reason to abandon the other N-1 utterances in the
/// file.
fn inject_mor_gra_tiers_selective(
    chat: &mut Chat,
    outputs: &[(usize, MorphosyntaxOutput)],
) -> BAResult<InjectStats> {
    use std::collections::HashMap;

    let source_id = chat.source_id().clone();
    let by_idx: HashMap<usize, &MorphosyntaxOutput> =
        outputs.iter().map(|(i, o)| (*i, o)).collect();

    let mut stats = InjectStats::default();
    let mut utt_idx = 0usize;
    for line in chat.ast_mut().lines.as_mut_slice().iter_mut() {
        if let Line::Utterance(u) = line {
            if let Some(out) = by_idx.get(&utt_idx) {
                // The %mor/%gra terminator kind comes from the utterance's own
                // typed terminator; default to a period when the main tier has
                // none (BA2's fallback).
                let terminator = u
                    .main
                    .content
                    .terminator
                    .clone()
                    .unwrap_or(Terminator::Period { span: Span::DUMMY });
                let candidate = build_tiers(out, terminator)?;
                let Some((mor_tier, gra_tier)) = candidate else {
                    stats.skipped_empty_output += 1;
                    utt_idx += 1;
                    continue;
                };

                // Pre-commit gate: run the spec'd validator against the
                // candidate `%mor` for THIS utterance's main tier. If any
                // diagnostic fires (E705 count too small, E706 too large,
                // E716 terminator mismatch), drop the tiers on the floor
                // and leave the main tier untouched — the fork's "no
                // partial %mor" rule.
                let alignment = align_main_to_mor(&u.main, &mor_tier);
                if !alignment.is_error_free() {
                    log_skipped_alignment(&source_id, utt_idx, &alignment.errors);
                    stats.skipped_count_mismatch += 1;
                    utt_idx += 1;
                    continue;
                }

                u.dependent_tiers.push(DependentTier::Mor(mor_tier).into());
                u.dependent_tiers.push(DependentTier::Gra(gra_tier).into());
                stats.injected += 1;
            }
            utt_idx += 1;
        }
    }
    Ok(stats)
}

/// Emit a per-utterance warning that downstream tooling can pick up via
/// the `batchalign::morphosyntax` tracing target. Includes the validator's
/// own diagnostic code + headline so an operator can scan for E705 / E706
/// patterns by language.
fn log_skipped_alignment(
    source_id: &crate::utils::SourceId,
    utterance_idx: usize,
    errors: &[ParseError],
) {
    let codes: Vec<&str> = errors.iter().map(|e| e.code.as_str()).collect();
    let headline = errors
        .first()
        .map(|e| e.message.lines().next().unwrap_or("").trim().to_string())
        .unwrap_or_default();
    // INFO, not WARN — a per-utterance skip is the runner doing its job
    // (best-effort tagging), not an operator-actionable condition. The
    // file-level summary in `process_chat` aggregates the count.
    tracing::info!(
        target: "batchalign::morphosyntax",
        sid = %source_id,
        utterance = utterance_idx,
        codes = ?codes,
        "morphotag: skipped %mor/%gra on utterance {utterance_idx}: {headline}",
    );
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::base::{TaskInput, TaskOutput};
    use crate::proto::morphosyntax::GraTerminator;
    use crate::utils::SourceId;
    use async_trait::async_trait;
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn tagged_output(input: MorphosyntaxInput) -> TaskOutput {
        let tokens: Vec<MorphosyntaxToken> = input
            .tokens
            .iter()
            .enumerate()
            .map(|(i, token)| MorphosyntaxToken {
                text: token.clone(),
                units: vec![MorphosyntaxUnit {
                    pos: "noun".to_owned(),
                    lemma: token.clone(),
                    features: vec![],
                    index: (i + 1) as u32,
                    head: 0,
                    deprel: "ROOT".to_owned(),
                }],
            })
            .collect();
        let token_count = tokens.len() as u32;
        MorphosyntaxOutput {
            source_id: input.source_id,
            utterance_id: input.utterance_id,
            tokens,
            terminator: Some(GraTerminator {
                index: token_count + 1,
                head: usize::from(token_count > 0) as u32,
                deprel: "PUNCT".to_owned(),
            }),
        }
        .into()
    }

    /// Stub dispatcher: records inputs and returns one `noun|<token>` unit per
    /// input token, with sequential `%gra` indices and a trailing PUNCT
    /// terminator — enough to exercise the typed-tier construction path.
    struct RecordingDispatcher {
        seen: Mutex<Vec<MorphosyntaxInput>>,
    }

    impl RecordingDispatcher {
        fn new() -> Self {
            Self {
                seen: Mutex::new(Vec::new()),
            }
        }
    }

    #[async_trait]
    impl Dispatcher for RecordingDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            let m = match input {
                TaskInput::Morphosyntax(m) => m,
                other => {
                    return Err(BAError::Internal(format!("unexpected: {:?}", other.task())));
                }
            };
            self.seen.lock().expect("poisoned").push(m.clone());
            Ok(tagged_output(m))
        }
    }

    struct MeasuringDispatcher {
        active: AtomicUsize,
        maximum: AtomicUsize,
    }

    #[async_trait]
    impl Dispatcher for MeasuringDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            let TaskInput::Morphosyntax(input) = input else {
                return Err(BAError::Internal("expected morphosyntax input".into()));
            };
            let active = self.active.fetch_add(1, Ordering::SeqCst) + 1;
            self.maximum.fetch_max(active, Ordering::SeqCst);
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            self.active.fetch_sub(1, Ordering::SeqCst);
            Ok(tagged_output(input))
        }
    }

    const FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\tit's red .\n*CHI:\tcat dog .\n@End\n";

    const RETRACE_FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\tnld\n@Participants:\tCHI Child\n@ID:\tnld|corpus|CHI|||||Child|||\n*CHI:\tUh van [/] van de uh boerderij .\n@End\n";

    const UNSUPPORTED_FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\tsrp\n@Participants:\tCHI Child\n@ID:\tsrp|corpus|CHI|||||Child|||\n*CHI:\tnešto .\n@End\n";

    const CODE_SWITCH_FIXTURE: &str = include_str!(
        "../../../../../resources/test_fixtures/morphotag/code_switch_spans.input.cha"
    );
    const CODE_SWITCH_GOLD: &str = include_str!(
        "../../../../../resources/test_fixtures/morphotag/code_switch_spans.gold.json"
    );

    /// Capturing sink for tick-sequence assertions.
    struct CapturingSink {
        events: Mutex<Vec<crate::base::ProgressEvent>>,
    }
    impl CapturingSink {
        fn new() -> Self {
            Self {
                events: Mutex::new(Vec::new()),
            }
        }
    }
    impl crate::base::ProgressSink for CapturingSink {
        fn emit(&self, event: crate::base::ProgressEvent) {
            self.events.lock().expect("poisoned").push(event);
        }
    }

    #[tokio::test]
    async fn dispatch_batches_without_exceeding_the_memory_window() -> BAResult<()> {
        let source_id = SourceId::try_new("bounded")?;
        let batch = MorphotagBatch {
            inputs: (0..17)
                .map(|utterance_id| MorphosyntaxInput {
                    source_id: source_id.clone(),
                    utterance_id,
                    language: LanguageSpec::Code(SmolStr::new("eng")),
                    tokens: vec!["word".to_owned()],
                    retokenize: false,
                    text: "word".to_owned(),
                })
                .collect(),
        };
        let dispatcher = MeasuringDispatcher {
            active: AtomicUsize::new(0),
            maximum: AtomicUsize::new(0),
        };
        let progress = std::sync::Arc::new(crate::base::ScaledProgress::new(
            std::sync::Arc::new(NullSink),
            source_id,
            Task::Morphosyntax,
            batch.len() as u64,
        ));
        let outputs = batch
            .dispatch(&dispatcher, progress.clone(), progress)
            .await?;

        assert_eq!(outputs.len(), 17);
        let maximum = dispatcher.maximum.load(Ordering::SeqCst);
        assert!(maximum > 1, "dispatch must expose a batch to the engine");
        assert!(maximum <= MORPHOTAG_DISPATCH_WINDOW);
        Ok(())
    }

    #[tokio::test]
    async fn emits_per_utterance_progress_ticks() -> BAResult<()> {
        use crate::base::{PROGRESS_SCALE, ProgressSink};
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();
        let sink = std::sync::Arc::new(CapturingSink::new());
        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                sink.clone() as std::sync::Arc<dyn ProgressSink>,
            )
            .await?;

        // Pull out only ticks (StageStarted with non-zero total). The
        // initial bare `stage_started(... total=0)` and the final
        // `stage_injected(... total=0)` are gated out.
        //
        // With `ScaledProgress`, the per-step bar uses `total =
        // outer_total * PROGRESS_SCALE` so it stays constant even as
        // backends report variable inner counts. For 2 outer steps with
        // no backend ticks, we see start-of-step floors at 0 and 1*SCALE
        // plus a final ceiling tick from `finish()` at 2*SCALE.
        let evs = sink.events.lock().expect("poisoned");
        let ticks: Vec<(u64, u64)> = evs
            .iter()
            .filter(|e| e.total > 0)
            .map(|e| (e.completed, e.total))
            .collect();
        let s = PROGRESS_SCALE;
        assert_eq!(
            ticks,
            vec![(0, 2 * s), (1 * s, 2 * s), (2 * s, 2 * s)],
            "expected scaled floor ticks at start of each step + ceiling on finish"
        );
        Ok(())
    }

    #[tokio::test]
    async fn injects_typed_mor_and_gra() -> BAResult<()> {
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();
        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await?;

        // Inputs: one per utterance, language resolved per-file from the
        // `@Languages:` header, retokenize off by default.
        let seen = dispatcher.seen.lock().expect("poisoned");
        assert_eq!(seen.len(), 2, "one input per utterance");
        assert!(!seen[0].retokenize);
        assert_eq!(
            seen[0].language,
            LanguageSpec::Code(SmolStr::new("eng")),
            "per-file should resolve to eng from @Languages"
        );
        assert_eq!(seen[1].utterance_id, 1);
        drop(seen);

        let chat = match value {
            BAValue::Chat(c) => c,
            other => panic!("expected Chat, got {}", other.kind()),
        };
        let s = chat.to_chat();
        assert!(s.contains("%mor:"), "missing %mor tier: {s}");
        assert!(s.contains("%gra:"), "missing %gra tier: {s}");
        assert!(s.contains("noun|cat"), "expected typed noun|cat: {s}");
        // Terminator rendered by the typed writer (period after the last word).
        assert!(s.contains("noun|dog ."), "expected terminator on %mor: {s}");
        Ok(())
    }

    #[tokio::test]
    async fn preserves_fixture_code_switches_for_dispatch() -> BAResult<()> {
        let chat = Chat::parse(CODE_SWITCH_FIXTURE, SourceId::try_new("code-switch")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();

        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await?;

        let gold: serde_json::Value = serde_json::from_str(CODE_SWITCH_GOLD)?;
        let cases = gold["cases"].as_array().expect("gold cases must be an array");
        let primary = gold["language"]
            .as_str()
            .expect("gold language must be a string");
        let seen = dispatcher.seen.lock().expect("poisoned");
        assert_eq!(seen.len(), cases.len());

        for (input, case) in seen.iter().zip(cases) {
            let language = case["language"].as_str().unwrap_or(primary);
            let tokens: Vec<&str> = case["tokens"]
                .as_array()
                .expect("gold tokens must be an array")
                .iter()
                .map(|token| token.as_str().expect("gold token must be a string"))
                .collect();
            assert_eq!(input.language, LanguageSpec::Code(SmolStr::new(language)));
            assert_eq!(input.tokens, tokens);
            assert_eq!(
                input.text,
                case["stanza_text"]
                    .as_str()
                    .expect("gold stanza_text must be a string")
            );
        }

        let routed: Vec<&str> = seen
            .iter()
            .map(|input| match &input.language {
                LanguageSpec::Code(code) => code.as_str(),
                other => panic!("expected routed language code, got {other:?}"),
            })
            .fold(Vec::new(), |mut languages, language| {
                if !languages.contains(&language) {
                    languages.push(language);
                }
                languages
            });
        let expected_routed: Vec<&str> = gold["routed_utterance_languages"]
            .as_array()
            .expect("gold routed languages must be an array")
            .iter()
            .map(|language| language.as_str().expect("gold language must be a string"))
            .collect();
        assert_eq!(routed, expected_routed);
        Ok(())
    }

    #[tokio::test]
    async fn retraced_words_are_not_sent_to_stanza_or_counted_in_mor() -> BAResult<()> {
        let chat = Chat::parse(RETRACE_FIXTURE, SourceId::try_new("retrace")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();

        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await?;

        let seen = dispatcher.seen.lock().expect("poisoned");
        assert_eq!(seen.len(), 1);
        assert_eq!(seen[0].tokens, ["Uh", "van", "de", "uh", "boerderij"]);
        drop(seen);

        let BAValue::Chat(chat) = value else {
            panic!("expected Chat");
        };
        let output = chat.to_chat();
        assert!(output.contains("%mor:"), "missing %mor tier: {output}");
        assert!(output.contains("%gra:"), "missing %gra tier: {output}");
        assert_eq!(
            output.matches("noun|van").count(),
            1,
            "retraced van must not receive a %mor slot: {output}"
        );
        Ok(())
    }

    #[tokio::test]
    async fn unsupported_primary_language_fails_before_dispatch() -> BAResult<()> {
        let chat = Chat::parse(UNSUPPORTED_FIXTURE, SourceId::try_new("unsupported")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();
        let err = MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect_err("unsupported primary language must fail the file");

        match err {
            BAError::Validation(message) => {
                assert!(message.contains("primary @Languages 'srp'"));
                assert!(message.contains("not supported by Stanza"));
                assert!(message.contains("Fix the @Languages header"));
            }
            other => panic!("expected typed validation error, got {other:?}"),
        }
        assert!(
            dispatcher.seen.lock().expect("poisoned").is_empty(),
            "the per-file gate must run before any backend dispatch"
        );
        Ok(())
    }

    #[tokio::test]
    async fn ca_file_with_unsupported_language_is_legitimate_pass_through() -> BAResult<()> {
        let input = UNSUPPORTED_FIXTURE.replace(
            "@Participants:\tCHI Child\n",
            "@Participants:\tCHI Child\n@Options:\tCA\n",
        );
        let chat = Chat::parse(&input, SourceId::try_new("ca-unsupported")?)?;
        let before = chat.to_chat();
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();

        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await?;

        assert!(dispatcher.seen.lock().expect("poisoned").is_empty());
        let BAValue::Chat(chat) = value else {
            panic!("expected Chat pass-through");
        };
        assert_eq!(chat.to_chat(), before);
        Ok(())
    }

    /// Dispatcher that drops the last input token — produces a `%mor` shorter
    /// than the main tier, the misalignment shape `align_main_to_mor` catches.
    struct DropLastTokenDispatcher;

    #[async_trait]
    impl Dispatcher for DropLastTokenDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            let m = match input {
                TaskInput::Morphosyntax(m) => m,
                other => {
                    return Err(BAError::Internal(format!("unexpected: {:?}", other.task())));
                }
            };
            let kept: Vec<&String> = m
                .tokens
                .iter()
                .take(m.tokens.len().saturating_sub(1))
                .collect();
            let tokens: Vec<MorphosyntaxToken> = kept
                .into_iter()
                .enumerate()
                .map(|(i, t)| MorphosyntaxToken {
                    text: t.clone(),
                    units: vec![MorphosyntaxUnit {
                        pos: "noun".to_owned(),
                        lemma: t.clone(),
                        features: vec![],
                        index: (i + 1) as u32,
                        head: 0,
                        deprel: "ROOT".to_owned(),
                    }],
                })
                .collect();
            let n = tokens.len() as u32;
            Ok(MorphosyntaxOutput {
                source_id: m.source_id.clone(),
                utterance_id: m.utterance_id,
                tokens,
                terminator: Some(GraTerminator {
                    index: n + 1,
                    head: if n == 0 { 0 } else { 1 },
                    deprel: "PUNCT".to_owned(),
                }),
            }
            .into())
        }
    }

    #[tokio::test]
    async fn skips_misaligned_utterances_without_failing_file() -> BAResult<()> {
        // FIXTURE has two utterances ("it's red .", "cat dog ."). The
        // dispatcher drops the last token of each → both candidates fail
        // `align_main_to_mor`. The runner must NOT fail the file; instead
        // it skips the tier injection for those utterances and leaves the
        // main tier untouched. (Strategy #2 from the fork.)
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &DropLastTokenDispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("misaligned utterances must not fail the file");

        let chat = match value {
            BAValue::Chat(c) => c,
            other => panic!("expected Chat, got {}", other.kind()),
        };
        let s = chat.to_chat();
        assert!(
            !s.contains("%mor:"),
            "%mor must be skipped on mismatch: {s}"
        );
        assert!(
            !s.contains("%gra:"),
            "%gra must be skipped on mismatch: {s}"
        );
        // Main tier survives unchanged.
        assert!(s.contains("*CHI:\tit's red ."), "main tier missing: {s}");
        assert!(s.contains("*CHI:\tcat dog ."), "main tier missing: {s}");
        Ok(())
    }

    /// Dispatcher that drops the last token of *only one* specific
    /// utterance, leaving the others well-formed. Used to verify the
    /// runner injects tiers for the good utterance while skipping the
    /// bad one — partial success, the whole point of skip-per-utt.
    struct DropLastForUttDispatcher {
        bad_utt_id: u32,
    }

    #[async_trait]
    impl Dispatcher for DropLastForUttDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            let m = match input {
                TaskInput::Morphosyntax(m) => m,
                other => {
                    return Err(BAError::Internal(format!("unexpected: {:?}", other.task())));
                }
            };
            let drop_last = m.utterance_id == self.bad_utt_id;
            let take_n = if drop_last {
                m.tokens.len().saturating_sub(1)
            } else {
                m.tokens.len()
            };
            let tokens: Vec<MorphosyntaxToken> = m
                .tokens
                .iter()
                .take(take_n)
                .enumerate()
                .map(|(i, t)| MorphosyntaxToken {
                    text: t.clone(),
                    units: vec![MorphosyntaxUnit {
                        pos: "noun".to_owned(),
                        lemma: t.clone(),
                        features: vec![],
                        index: (i + 1) as u32,
                        head: 0,
                        deprel: "ROOT".to_owned(),
                    }],
                })
                .collect();
            let n = tokens.len() as u32;
            Ok(MorphosyntaxOutput {
                source_id: m.source_id.clone(),
                utterance_id: m.utterance_id,
                tokens,
                terminator: Some(GraTerminator {
                    index: n + 1,
                    head: if n == 0 { 0 } else { 1 },
                    deprel: "PUNCT".to_owned(),
                }),
            }
            .into())
        }
    }

    #[tokio::test]
    async fn partial_success_one_good_one_bad() -> BAResult<()> {
        // utt 0 ("it's red") tags cleanly; utt 1 ("cat dog") loses its
        // last token. The good utterance gets %mor/%gra; the bad one
        // doesn't. The file succeeds.
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &DropLastForUttDispatcher { bad_utt_id: 1 },
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect("partial success must not fail the file");

        let chat = match value {
            BAValue::Chat(c) => c,
            other => panic!("expected Chat, got {}", other.kind()),
        };
        let s = chat.to_chat();
        // utt 0 tagged.
        assert!(s.contains("noun|it"), "good utt should have %mor: {s}");
        // utt 1 not tagged (no noun|cat would appear — bad utt dropped).
        assert!(!s.contains("noun|cat"), "bad utt must be skipped: {s}");
        Ok(())
    }

    #[tokio::test]
    async fn rejects_non_chat_variant() {
        use crate::utils::MediaInput;
        let mut value = BAValue::Media(MediaInput {
            source_id: SourceId::new_unchecked("audio"),
            path: "/dev/null".into(),
            language: None,
        });
        let dispatcher = RecordingDispatcher::new();
        let err = MorphosyntaxTaskRunner
            .apply(
                &mut value,
                &dispatcher,
                std::sync::Arc::new(NullSink) as std::sync::Arc<dyn ProgressSink>,
            )
            .await
            .expect_err("must reject non-Chat or Paired");
        match err {
            BAError::Internal(msg) => assert!(msg.contains("BAValue::Chat")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
