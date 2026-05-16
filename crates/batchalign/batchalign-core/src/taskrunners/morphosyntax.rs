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
//! 3. Attaches the resulting `%mor:` / `%gra:` tiers to the corresponding
//!    [`Utterance`] via [`Utterance::with_mor`] / [`Utterance::with_gra`].
//!
//! ## Retokenize semantics
//!
//! Two modes, controlled by `MorphosyntaxConfig::retokenize`:
//!
//! - `retokenize = false` (default). Mirrors BA2's `retokenize=False` path.
//!   The upstream main-tier tokenization is authoritative; the backend MUST
//!   produce exactly one `%mor` item per input token. This is the right mode
//!   for already-segmented CHAT documents, where word boundaries are
//!   linguistically meaningful (split contractions, marked compounds, etc.).
//! - `retokenize = true`. Mirrors BA2's `retokenize=True` path. The backend
//!   is allowed to resplit tokens — for example expanding `gonna` into
//!   `going to`, or recovering an MWT it knows about. The runner then
//!   propagates the (possibly different) token count into the `%mor`/`%gra`
//!   tiers it injects.
//!
//! In both modes the runner ships the raw token list AND the joined text so
//! the backend can reconstruct whichever signal it needs.
//!
//! ## UD-only `%mor` syntax
//!
//! Output uses Universal Dependencies syntax exclusively: `verb|run-Past`,
//! `noun|cat-Plur`. CLAN MOR's `&`-style fusional markers (`aux|be&PRES`) are
//! never emitted. See `CLAUDE.md` §17.3 (project policy: UD-only).

use crate::base::Chat;
use crate::utils::{BAError, BAResult};
use crate::base::{ProgressEvent, ProgressSink};
use crate::proto::asr::LanguageSpec;
use crate::proto::morphosyntax::{MorphosyntaxInput, MorphosyntaxOutput, MorphosyntaxToken};
use crate::base::Task;
use crate::base::{Dispatcher, TaskRunner};
use crate::base::BAValue;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_model::alignment::helpers::{WordItem, walk_words};
use talkbank_model::{DependentTier, Line, NonEmptyString, Utterance};

/// Runner that drops `%mor:` and `%gra:` tiers on a CHAT document.
pub struct MorphosyntaxTaskRunner;

/// Per-task config.
///
/// Both fields are tunable per Pipeline-run via the user task list:
/// `[(Task::Morphosyntax, {"language": "per-file", "retokenize": false})]`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MorphosyntaxConfig {
    /// `"eng"` etc. — pins a concrete language code; or `"per-file"` to
    /// resolve from the CHAT's `@Languages:` header at dispatch time;
    /// or `"auto"` to let the backend guess.
    #[serde(default = "default_per_file_language")]
    pub language: String,
    /// Allow the backend to resplit tokens (BA2's `retokenize=True`).
    /// Default `false` — preserves the upstream main-tier tokenization.
    #[serde(default)]
    pub retokenize: bool,
}

fn default_per_file_language() -> String {
    "per-file".to_owned()
}

impl Default for MorphosyntaxConfig {
    fn default() -> Self {
        Self {
            language: default_per_file_language(),
            retokenize: false,
        }
    }
}

#[async_trait]
impl TaskRunner for MorphosyntaxTaskRunner {
    const TASK: Task = Task::Morphosyntax;
    type Config = MorphosyntaxConfig;

    async fn apply(
        &self,
        cfg: &Self::Config,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        // Variant gate: Morphosyntax requires an existing CHAT document.
        // Failed values are short-circuited by the pipeline; other variants
        // are an upstream wiring bug.
        let chat = match value {
            BAValue::Chat(c) => c,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "Morphosyntax expects BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        let source_id = chat.source_id().clone();
        sink.emit(ProgressEvent::stage_started(&source_id, Task::Morphosyntax));

        let language = resolve_language(&cfg.language, chat);

        // Phase 1: extract per-utterance token lists from the AST.
        let per_utt_tokens: Vec<Vec<String>> = chat
            .ast()
            .utterances()
            .map(extract_tokens)
            .collect();

        // Phase 2: dispatch one input per utterance, collect outputs in order.
        let mut outputs: Vec<MorphosyntaxOutput> = Vec::with_capacity(per_utt_tokens.len());
        for (idx, tokens) in per_utt_tokens.iter().enumerate() {
            let text = tokens.join(" ");
            let input = MorphosyntaxInput {
                source_id: source_id.clone(),
                utterance_id: idx as u32,
                language: language.clone(),
                tokens: tokens.clone(),
                retokenize: cfg.retokenize,
                text,
            };
            let task_out = dispatcher.dispatch(input.into()).await?;
            let out: MorphosyntaxOutput = task_out.try_into()?;
            outputs.push(out);
        }

        // Phase 3: inject tiers into the AST in utterance order.
        inject_mor_gra_tiers(chat, &outputs)?;

        sink.emit(ProgressEvent::stage_injected(&source_id, Task::Morphosyntax));
        Ok(())
    }
}

/// Resolve the runtime `LanguageSpec` from config + chat header.
///
/// `"per-file"` falls back to `LanguageSpec::PerFile` (the backend is
/// expected to honor it) and the runner also resolves the actual code from
/// `@Languages:` so the backend has both signals.
fn resolve_language(cfg_language: &str, chat: &Chat) -> LanguageSpec {
    match cfg_language {
        "auto" => LanguageSpec::Auto,
        "per-file" | "per_file" => {
            if let Some(code) = chat.primary_language() {
                LanguageSpec::Code(SmolStr::new(code))
            } else {
                LanguageSpec::PerFile
            }
        }
        other => LanguageSpec::Code(SmolStr::new(other)),
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

/// Build `%mor:` / `%gra:` text from a per-utterance [`MorphosyntaxOutput`].
///
/// We attach the rendered text as `UserDefined` dependent tiers so the
/// downstream serializer roundtrips correctly without us having to construct
/// fully-typed `MorTier` / `GraTier` AST values from scratch (which would
/// require synthesizing alignment indices). The serializer emits user-defined
/// tiers as `%LABEL:\t<content>` — the resulting CHAT text re-parses into
/// proper typed `%mor` / `%gra` tiers on the next round-trip.
fn render_mor(out: &MorphosyntaxOutput) -> String {
    if let Some(s) = &out.mor {
        return s.clone();
    }
    let mut parts: Vec<String> = Vec::with_capacity(out.tokens.len());
    for tok in &out.tokens {
        parts.push(render_mor_token(tok));
    }
    parts.join(" ")
}

/// Render one token in UD `%mor` syntax: `POS|lemma[-Feature]*`.
///
/// UD-only per CLAUDE.md §17.3 — no `&` fusional markers.
fn render_mor_token(tok: &MorphosyntaxToken) -> String {
    let pos = if tok.upos.is_empty() { "x" } else { tok.upos.as_str() };
    let lemma = if tok.lemma.is_empty() {
        tok.text.as_str()
    } else {
        tok.lemma.as_str()
    };
    let mut s = format!("{pos}|{lemma}");
    for feat in &tok.features {
        if feat.is_empty() {
            continue;
        }
        s.push('-');
        s.push_str(feat);
    }
    s
}

fn render_gra(out: &MorphosyntaxOutput) -> String {
    if let Some(s) = &out.gra {
        return s.clone();
    }
    // Synthesize from head/deprel when present; otherwise a flat ROOT chain.
    let mut parts: Vec<String> = Vec::with_capacity(out.tokens.len());
    for (i, tok) in out.tokens.iter().enumerate() {
        let idx = (i + 1) as u32;
        let head = tok.head.unwrap_or(0);
        let deprel = tok
            .deprel
            .as_deref()
            .filter(|s| !s.is_empty())
            .unwrap_or("ROOT");
        parts.push(format!("{idx}|{head}|{deprel}"));
    }
    parts.join(" ")
}

/// Walk lines in document order; for each utterance, append a `%mor:` and
/// `%gra:` user-defined tier from the matching `MorphosyntaxOutput`.
///
/// Idempotency note: this runner is not idempotent — running it twice would
/// duplicate tiers. The pipeline driver guarantees one execution per task per
/// source, so we keep the runner straightforward.
fn inject_mor_gra_tiers(chat: &mut Chat, outputs: &[MorphosyntaxOutput]) -> BAResult<()> {
    use talkbank_model::Span;
    use talkbank_model::model::dependent_tier::UserDefinedDependentTier;

    let mut idx = 0usize;
    for line in chat.ast_mut().lines.0.iter_mut() {
        if let Line::Utterance(u) = line {
            let Some(out) = outputs.get(idx) else {
                return Err(BAError::Internal(format!(
                    "Morphosyntax: missing output for utterance {idx}"
                )));
            };
            let mor_text = render_mor(out);
            let gra_text = render_gra(out);
            if !mor_text.is_empty() {
                let label = NonEmptyString::new("mor").ok_or_else(|| BAError::Internal("mor label empty".into()))?;
                let content = NonEmptyString::new(&mor_text).ok_or_else(|| BAError::Internal("mor content empty".into()))?;
                u.dependent_tiers
                    .push(DependentTier::UserDefined(UserDefinedDependentTier {
                        label,
                        content,
                        span: Span::DUMMY,
                    }));
            }
            if !gra_text.is_empty() {
                let label = NonEmptyString::new("gra").ok_or_else(|| BAError::Internal("gra label empty".into()))?;
                let content = NonEmptyString::new(&gra_text).ok_or_else(|| BAError::Internal("gra content empty".into()))?;
                u.dependent_tiers
                    .push(DependentTier::UserDefined(UserDefinedDependentTier {
                        label,
                        content,
                        span: Span::DUMMY,
                    }));
            }
            idx += 1;
        }
    }
    if idx != outputs.len() {
        return Err(BAError::Internal(format!(
            "Morphosyntax: utterance/output count mismatch ({idx} vs {})",
            outputs.len()
        )));
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
    use crate::utils::SourceId;
    use async_trait::async_trait;
    use std::sync::Mutex;

    /// Stub dispatcher: records inputs, returns canned outputs (token-per-token
    /// when retokenize=false; doubles each token when retokenize=true so tests
    /// can observe the difference).
    struct RecordingDispatcher {
        seen: Mutex<Vec<MorphosyntaxInput>>,
        expand_on_retokenize: bool,
    }

    impl RecordingDispatcher {
        fn new(expand_on_retokenize: bool) -> Self {
            Self {
                seen: Mutex::new(Vec::new()),
                expand_on_retokenize,
            }
        }
    }

    #[async_trait]
    impl Dispatcher for RecordingDispatcher {
        async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput> {
            let m = match input {
                TaskInput::Morphosyntax(m) => m,
                other => return Err(BAError::Internal(format!("unexpected: {:?}", other.task()))),
            };
            // Echo tokens. If retokenize, split each token by ' into two when
            // it contains an apostrophe; otherwise mirror 1:1.
            let mut out_tokens: Vec<MorphosyntaxToken> = Vec::new();
            let resplit = self.expand_on_retokenize && m.retokenize;
            for t in &m.tokens {
                if resplit && t.contains('\'') {
                    let (head, tail) = t.split_once('\'').unwrap_or((t.as_str(), ""));
                    out_tokens.push(MorphosyntaxToken {
                        text: head.to_owned(),
                        lemma: head.to_owned(),
                        upos: "pron".to_owned(),
                        features: vec![],
                        head: Some(2),
                        deprel: Some("nsubj".to_owned()),
                    });
                    out_tokens.push(MorphosyntaxToken {
                        text: tail.to_owned(),
                        lemma: "be".to_owned(),
                        upos: "aux".to_owned(),
                        features: vec!["Pres".to_owned(), "S3".to_owned()],
                        head: Some(0),
                        deprel: Some("root".to_owned()),
                    });
                } else {
                    out_tokens.push(MorphosyntaxToken {
                        text: t.clone(),
                        lemma: t.clone(),
                        upos: "noun".to_owned(),
                        features: vec![],
                        head: Some(0),
                        deprel: Some("root".to_owned()),
                    });
                }
            }
            let out = MorphosyntaxOutput {
                source_id: m.source_id.clone(),
                utterance_id: m.utterance_id,
                tokens: out_tokens,
                mor: None,
                gra: None,
            };
            self.seen.lock().expect("poisoned").push(m);
            Ok(out.into())
        }
    }

    const FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\tit's red .\n*CHI:\tcat dog .\n@End\n";

    #[tokio::test]
    async fn injects_mor_and_gra_no_retokenize() -> BAResult<()> {
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture")?)?;
        let mut value = BAValue::Chat(chat);
        let cfg = MorphosyntaxConfig {
            language: "per-file".into(),
            retokenize: false,
        };
        let dispatcher = RecordingDispatcher::new(true);
        MorphosyntaxTaskRunner
            .apply(&cfg, &mut value, &dispatcher, &NullSink)
            .await?;

        // Inspect what the dispatcher saw.
        let seen = dispatcher.seen.lock().expect("poisoned");
        assert_eq!(seen.len(), 2, "one input per utterance");
        assert!(!seen[0].retokenize);
        assert_eq!(
            seen[0].language,
            LanguageSpec::Code(SmolStr::new("eng")),
            "per-file should resolve to eng from @Languages"
        );
        assert_eq!(seen[1].utterance_id, 1);

        // Tiers landed.
        let chat = match value {
            BAValue::Chat(c) => c,
            other => panic!("expected Chat, got {}", other.kind()),
        };
        let s = chat.to_chat();
        assert!(s.contains("%mor:"), "missing %mor tier: {s}");
        assert!(s.contains("%gra:"), "missing %gra tier: {s}");
        assert!(s.contains("noun|cat"), "expected UD-rendered noun|cat: {s}");
        // retokenize=false: should NOT split it's into two tokens.
        assert!(!s.contains("aux|be"), "should not have split it's: {s}");
        Ok(())
    }

    #[tokio::test]
    async fn retokenize_true_allows_resplit() -> BAResult<()> {
        let chat = Chat::parse(FIXTURE, SourceId::try_new("fixture2")?)?;
        let mut value = BAValue::Chat(chat);
        let cfg = MorphosyntaxConfig {
            language: "eng".into(),
            retokenize: true,
        };
        let dispatcher = RecordingDispatcher::new(true);
        MorphosyntaxTaskRunner
            .apply(&cfg, &mut value, &dispatcher, &NullSink)
            .await?;

        let seen = dispatcher.seen.lock().expect("poisoned");
        assert!(seen.iter().all(|i| i.retokenize));
        assert_eq!(seen[0].language, LanguageSpec::Code(SmolStr::new("eng")));

        let chat = match value {
            BAValue::Chat(c) => c,
            other => panic!("expected Chat, got {}", other.kind()),
        };
        let s = chat.to_chat();
        // With retokenize=true, the it's token expanded → aux|be-Pres-S3 appears.
        assert!(s.contains("aux|be-Pres-S3"), "expected expanded clitic: {s}");
        Ok(())
    }

    #[tokio::test]
    async fn rejects_non_chat_variant() {
        use crate::utils::MediaInput;
        let mut value = BAValue::Media(MediaInput {
            source_id: SourceId::new_unchecked("audio"),
            path: "/dev/null".into(),
        });
        let cfg = MorphosyntaxConfig::default();
        let dispatcher = RecordingDispatcher::new(false);
        let err = MorphosyntaxTaskRunner
            .apply(&cfg, &mut value, &dispatcher, &NullSink)
            .await
            .expect_err("must reject non-Chat");
        match err {
            BAError::Internal(msg) => assert!(msg.contains("BAValue::Chat")),
            other => panic!("unexpected error: {other:?}"),
        }
    }
}
