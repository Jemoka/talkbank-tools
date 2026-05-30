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
use smol_str::SmolStr;
use talkbank_model::Span;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::alignment::{MorGraTerminatorSlot, try_align_mor_gra};
use talkbank_model::model::{
    DependentTier, GraTier, GrammaticalRelation, Mor, MorFeature, MorStem, MorTier, MorWord,
    PosCategory, Terminator,
};
use talkbank_model::{Line, Utterance};

/// Runner that drops typed `%mor` and `%gra` tiers on a CHAT document.
pub struct MorphosyntaxTaskRunner;

#[async_trait]
impl TaskRunner for MorphosyntaxTaskRunner {
    const TASK: Task = Task::Morphosyntax;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        match value {
            BAValue::Chat(chat) => process_chat(chat, dispatcher, sink).await,
            // `Paired` is what Compare consumes; running morphosyntax over it
            // means tagging both main and gold, so the downstream
            // CompareBackend can lift POS off the `%mor` tier per token.
            BAValue::Paired(p) => {
                let (main, gold) = p.as_mut_parts();
                process_chat(main, dispatcher, sink).await?;
                process_chat(gold, dispatcher, sink).await?;
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
/// is resolved per-file from the chat's `@Languages:` header; backends that
/// want to pin a language do so via their own constructor.
async fn process_chat(
    chat: &mut Chat,
    dispatcher: &dyn Dispatcher,
    sink: &dyn ProgressSink,
) -> BAResult<()> {
    let source_id = chat.source_id().clone();
    sink.emit(ProgressEvent::stage_started(&source_id, Task::Morphosyntax));

    let language = resolve_per_file_language(chat);

    // Phase 1: extract per-utterance token lists AND check which utterances
    // already carry a `%mor:` tier. Pre-tagged utterances are skipped end-to-
    // end (no dispatch, no re-injection) — the existing tier is authoritative.
    // This is the "morphotag is idempotent" contract Compare relies on.
    let per_utt_tokens: Vec<Vec<String>> = chat.ast().utterances().map(extract_tokens).collect();
    let already_tagged: Vec<bool> = chat
        .ast()
        .utterances()
        .map(utterance_has_mor_tier)
        .collect();

    // Phase 2: dispatch only for utterances missing `%mor`. Track the source
    // utterance index alongside each output so injection can apply them to
    // the right slots while leaving pre-tagged utterances untouched.
    //
    // After each per-utterance dispatch we emit a `stage_tick` so the
    // per-file Rich progress bar advances incrementally instead of
    // sitting at 0 for the entire Stanza pass. `total` excludes
    // already-tagged utterances — the bar reflects real work to do,
    // matching BA2's `status_hook` semantics.
    let total_to_tag = already_tagged.iter().filter(|t| !**t).count() as u64;
    let mut dispatched: Vec<(usize, MorphosyntaxOutput)> = Vec::new();
    let mut completed_ticks: u64 = 0;
    for (idx, tokens) in per_utt_tokens.iter().enumerate() {
        if already_tagged[idx] {
            continue;
        }
        let text = tokens.join(" ");
        let input = MorphosyntaxInput {
            source_id: source_id.clone(),
            utterance_id: idx as u32,
            language: language.clone(),
            tokens: tokens.clone(),
            // Retokenize off by default — preserves upstream main-tier
            // tokenization. Backends that want to resplit (BA2's
            // `retokenize=True`) flip it via their own constructor.
            retokenize: false,
            text,
        };
        let task_out = dispatcher.dispatch(input.into()).await?;
        let out: MorphosyntaxOutput = task_out.try_into()?;
        dispatched.push((idx, out));
        completed_ticks += 1;
        sink.emit(ProgressEvent::stage_tick(
            &source_id,
            Task::Morphosyntax,
            completed_ticks,
            total_to_tag,
        ));
    }

    // Phase 3: build typed tiers and inject into the utterances we tagged.
    inject_mor_gra_tiers_selective(chat, &dispatched)?;

    sink.emit(ProgressEvent::stage_injected(
        &source_id,
        Task::Morphosyntax,
    ));
    Ok(())
}

fn utterance_has_mor_tier(u: &Utterance) -> bool {
    if u.mor_tier().is_some() {
        return true;
    }
    u.dependent_tiers.iter().any(|t| {
        matches!(
            t,
            DependentTier::UserDefined(udt) if udt.label.as_str() == "mor"
        )
    })
}

/// Read the chat's `@Languages:` header and emit a concrete `LanguageSpec`.
/// Falls back to `PerFile` (a no-op marker) when the header is absent so
/// the backend can do its own per-file resolution.
fn resolve_per_file_language(chat: &Chat) -> LanguageSpec {
    if let Some(code) = chat.primary_language() {
        LanguageSpec::Code(SmolStr::new(code))
    } else {
        LanguageSpec::PerFile
    }
}

/// Pull the alignable main-tier word surface forms from one utterance.
///
/// Uses [`walk_words`] with `domain=None` to descend into all groups
/// transparently (retraces included — Stanza still wants those tokens, the
/// domain-aware Mor gating only matters when constructing a true `%mor`
/// alignment from the parser; here we are *producing* a new `%mor`).
fn extract_tokens(u: &Utterance) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    walk_words(&u.main.content.content.0, None, &mut |item| match item {
        WordItem::Word(w) => out.push(w.cleaned_text().to_string()),
        WordItem::ReplacedWord(r) => out.push(r.word.cleaned_text().to_string()),
        WordItem::Separator(_) => {}
    });
    out
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

/// Build typed `%mor`/`%gra` tiers for the subset of utterances we dispatched
/// and attach them. `outputs` is the `(utterance_index, output)` list produced
/// by `process_chat`'s skip-already-tagged loop; utterances absent from the
/// list keep whatever tiers they already had.
fn inject_mor_gra_tiers_selective(
    chat: &mut Chat,
    outputs: &[(usize, MorphosyntaxOutput)],
) -> BAResult<()> {
    use std::collections::HashMap;

    let by_idx: HashMap<usize, &MorphosyntaxOutput> =
        outputs.iter().map(|(i, o)| (*i, o)).collect();

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        if let Line::Utterance(u) = line {
            if let Some(out) = by_idx.get(&idx) {
                // The %mor/%gra terminator kind comes from the utterance's own
                // typed terminator; default to a period when the main tier has
                // none (BA2's fallback).
                let terminator = u
                    .main
                    .content
                    .terminator
                    .clone()
                    .unwrap_or(Terminator::Period { span: Span::DUMMY });
                if let Some((mor_tier, gra_tier)) = build_tiers(out, terminator)? {
                    u.dependent_tiers.push(DependentTier::Mor(mor_tier));
                    u.dependent_tiers.push(DependentTier::Gra(gra_tier));
                }
            }
            idx += 1;
        }
    }
    Ok(())
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
            let tokens: Vec<MorphosyntaxToken> = m
                .tokens
                .iter()
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
            let out = MorphosyntaxOutput {
                source_id: m.source_id.clone(),
                utterance_id: m.utterance_id,
                tokens,
                terminator: Some(GraTerminator {
                    index: n + 1,
                    head: if n == 0 { 0 } else { 1 },
                    deprel: "PUNCT".to_owned(),
                }),
            };
            self.seen.lock().expect("poisoned").push(m);
            Ok(out.into())
        }
    }

    const FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\tit's red .\n*CHI:\tcat dog .\n@End\n";

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
    async fn emits_per_utterance_progress_ticks() -> BAResult<()> {
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();
        let sink = CapturingSink::new();
        MorphosyntaxTaskRunner
            .apply(&mut value, &dispatcher, &sink)
            .await?;

        // Pull out only ticks (StageStarted with non-zero total). The
        // initial bare `stage_started(... total=0)` and the final
        // `stage_injected(... total=0)` are gated out.
        let evs = sink.events.lock().expect("poisoned");
        let ticks: Vec<(u64, u64)> = evs
            .iter()
            .filter(|e| e.total > 0)
            .map(|e| (e.completed, e.total))
            .collect();
        assert_eq!(
            ticks,
            vec![(1, 2), (2, 2)],
            "expected one tick per dispatched utterance with total=2"
        );
        Ok(())
    }

    #[tokio::test]
    async fn injects_typed_mor_and_gra() -> BAResult<()> {
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        let dispatcher = RecordingDispatcher::new();
        MorphosyntaxTaskRunner
            .apply(&mut value, &dispatcher, &NullSink)
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
    async fn rejects_non_chat_variant() {
        use crate::utils::MediaInput;
        let mut value = BAValue::Media(MediaInput {
            source_id: SourceId::new_unchecked("audio"),
            path: "/dev/null".into(),
        });
        let dispatcher = RecordingDispatcher::new();
        let err = MorphosyntaxTaskRunner
            .apply(&mut value, &dispatcher, &NullSink)
            .await
            .expect_err("must reject non-Chat or Paired");
        match err {
            BAError::Internal(msg) => assert!(msg.contains("BAValue::Chat")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
