//! `CompareTaskRunner` — thin glue around the `CompareBackend`.
//!
//! The Compare task aligns a candidate transcript (`main`) against a gold
//! reference (`gold`) and annotates `main` with `%xref:` / `%xcmp:` tiers
//! plus a `@Comment: ba.compare.summary:` header. Spec2.md §19.
//!
//! All heavy lifting (token extraction, `conform()`, bag-of-words window
//! search, Levenshtein with traceback, summary aggregation, AST tier
//! injection) lives in `crates/batchalign/batchalign-core/src/backends/compare.rs`
//! so the engine's batcher can fan many `(main, gold)` pairs out concurrently
//! across the runtime's thread pool — no GIL, no Python round-trip.
//!
//! What the runner does here:
//!   1. Pull `Paired { main, gold }` out of `BAValue`.
//!   2. Serialize both transcripts to CHAT text.
//!   3. Build a `CompareInput`, dispatch it.
//!   4. Re-parse `annotated_main` back into a `Chat<Validated>` and replace
//!      the `BAValue` with `BAValue::Chat`.

use crate::base::Chat;
use crate::base::Paired;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{BAValue, Dispatcher, TaskRunner};
use crate::metrics::{MetricsArtifact, MetricsKind, MetricsRow, MetricsTable};
use crate::proto::compare::{CompareInput, CompareOutput};
use crate::utils::{BAError, BAResult, SourceId};
use async_trait::async_trait;
use smol_str::SmolStr;
use std::collections::BTreeMap;
use std::mem;

pub struct CompareTaskRunner;

#[async_trait]
impl TaskRunner for CompareTaskRunner {
    const TASK: Task = Task::Compare;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()> {
        let sid = value.source_id();
        sink.emit(ProgressEvent::stage_started(&sid, Task::Compare));

        // Take ownership of the Paired without leaving the BAValue in a
        // moved-from state — if the runner errors out we restore the original.
        let placeholder = BAValue::Failed {
            source_id: sid.clone(),
            error: BAError::Internal("compare: in-flight placeholder".into()),
            partial: None,
        };
        let taken = mem::replace(value, placeholder);
        let paired: Paired = match taken {
            BAValue::Paired(p) => p,
            BAValue::Failed { .. } => {
                // Already poisoned upstream — propagate.
                *value = BAValue::Failed {
                    source_id: sid,
                    error: BAError::Internal("compare: upstream failure".into()),
                    partial: None,
                };
                return Ok(());
            }
            other => {
                let kind = other.kind();
                *value = other;
                return Err(BAError::Internal(format!(
                    "CompareTaskRunner expected BAValue::Paired, got {kind}"
                )));
            }
        };

        let (mut main_chat, mut gold_chat) = paired.into_parts();
        // BA2's compare output carries %mor/%xsrep/%xsmor but NOT %gra. Drop
        // %gra before the round-trip: the morphosyntax %gra reproduces BA2's
        // ROOT-head quirk (head → last chunk), which is circular and trips
        // E724 when the annotated text is re-parsed. The compare itself only
        // needs %mor (POS), so this matches BA2 and keeps the document valid.
        strip_gra_tiers(&mut main_chat);
        strip_gra_tiers(&mut gold_chat);
        let main_text = main_chat.to_chat();
        let gold_text = gold_chat.to_chat();
        let main_source_id = main_chat.source_id().clone();

        let input = CompareInput {
            source_id: main_source_id.clone(),
            main_chat: main_text,
            gold_chat: gold_text,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Compare(input)).await?;
        let output: CompareOutput = output_raw.try_into()?;

        let annotated: Chat =
            Chat::parse(&output.annotated_main, main_source_id.clone()).map_err(|e| {
                BAError::Worker(format!("compare: failed to re-parse annotated_main: {e}"))
            })?;
        // Keep media reference if the upstream Paired carried one (won't
        // typically be the case for Compare, but harmless and consistent).
        let attached = match main_chat.media() {
            Some(m) => annotated.with_media(m.clone()),
            None => annotated,
        };

        // Build a wide-format MetricsArtifact from CompareOutput.metrics so
        // the driver writes `<source>.compare.csv` next to `<source>.cha`.
        let metrics_artifact = compare_metrics_to_artifact(&output, &main_source_id);

        // List shape: Cons(Chat, Cons(Metrics, Nil)). BAValue::write walks
        // this and routes each element to its own file (Chat → .cha,
        // Metrics → .compare.csv).
        *value = BAValue::list(vec![
            BAValue::Chat(attached),
            BAValue::Metrics(metrics_artifact),
        ]);

        sink.emit(ProgressEvent::stage_injected(
            &main_source_id,
            Task::Compare,
        ));
        Ok(())
    }
}

/// Strip `%gra` dependent tiers from a CHAT in place (see the call site: BA2's
/// compare emits no `%gra`, and the morphosyntax ROOT-head quirk makes it
/// circular, so re-parsing the annotated text would trip E724).
fn strip_gra_tiers(chat: &mut crate::base::Chat) {
    use talkbank_model::Line;
    use talkbank_model::model::DependentTier;
    for line in chat.ast_mut().lines.0.iter_mut() {
        if let Line::Utterance(u) = line {
            u.dependent_tiers
                .retain(|t| !matches!(t, DependentTier::Gra(_)));
        }
    }
}

/// Materialize the structured `CompareMetrics` payload into a wide-format
/// `MetricsArtifact` (one row per file). Column order matches BA2's
/// `compare.csv`: file + 7 global cols + per-POS quartets sorted
/// alphabetically by POS.
fn compare_metrics_to_artifact(output: &CompareOutput, source_id: &SourceId) -> MetricsArtifact {
    let m = &output.metrics;

    let mut schema: Vec<String> = vec![
        "file".to_owned(),
        "wer".to_owned(),
        "accuracy".to_owned(),
        "matches".to_owned(),
        "insertions".to_owned(),
        "deletions".to_owned(),
        "total_gold_words".to_owned(),
        "total_main_words".to_owned(),
    ];
    for pos in &m.per_pos {
        schema.push(format!("{}:matches", pos.pos));
        schema.push(format!("{}:insertions", pos.pos));
        schema.push(format!("{}:deletions", pos.pos));
        schema.push(format!("{}:total", pos.pos));
    }

    let mut columns: BTreeMap<String, serde_json::Value> = BTreeMap::new();
    columns.insert(
        "file".to_owned(),
        serde_json::Value::String(m.file_label.clone()),
    );
    columns.insert(
        "wer".to_owned(),
        serde_json::Value::from(format!("{:.4}", m.wer)),
    );
    columns.insert(
        "accuracy".to_owned(),
        serde_json::Value::from(format!("{:.4}", m.accuracy)),
    );
    columns.insert("matches".to_owned(), serde_json::Value::from(m.matches));
    columns.insert(
        "insertions".to_owned(),
        serde_json::Value::from(m.insertions),
    );
    columns.insert("deletions".to_owned(), serde_json::Value::from(m.deletions));
    columns.insert(
        "total_gold_words".to_owned(),
        serde_json::Value::from(m.total_gold_words),
    );
    columns.insert(
        "total_main_words".to_owned(),
        serde_json::Value::from(m.total_main_words),
    );
    for pos in &m.per_pos {
        columns.insert(
            format!("{}:matches", pos.pos),
            serde_json::Value::from(pos.matches),
        );
        columns.insert(
            format!("{}:insertions", pos.pos),
            serde_json::Value::from(pos.insertions),
        );
        columns.insert(
            format!("{}:deletions", pos.pos),
            serde_json::Value::from(pos.deletions),
        );
        columns.insert(
            format!("{}:total", pos.pos),
            serde_json::Value::from(pos.total),
        );
    }

    MetricsArtifact {
        source_id: source_id.clone(),
        producer: SmolStr::new_static("compare:rust:v2"),
        kind: MetricsKind::Compare,
        table: MetricsTable {
            schema,
            rows: vec![MetricsRow {
                row_kind: SmolStr::new_static("per_file"),
                columns,
            }],
        },
    }
}
