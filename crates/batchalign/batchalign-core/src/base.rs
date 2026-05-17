//! Core pipeline base types: `Task` DAG, runner traits, closed-set TaskInput /
//! TaskOutput unions, the `BAValue` flow type (with `Paired` inlined), the
//! typestate `Chat<S>` wrapper, and the progress-event channel.
//!
//! Consolidated from `task.rs`, `task_runner.rs`, `union.rs`, `value.rs`,
//! `paired.rs`, `chat.rs`, and `progress.rs` per the spec2.md reorg.

use crate::metrics::MetricsArtifact;
use crate::proto::asr::{AsrInput, AsrOutput};
use crate::proto::avqi::{AvqiInput, AvqiOutput};
use crate::proto::compare::{CompareInput, CompareOutput};
use crate::proto::coref::{CorefInput, CorefOutput};
use crate::proto::fa::{FaInput, FaOutput};
use crate::proto::morphosyntax::{MorphosyntaxInput, MorphosyntaxOutput};
use crate::proto::opensmile::{OpenSmileInput, OpenSmileOutput};
use crate::proto::speaker::{SpeakerInput, SpeakerOutput};
use crate::proto::translate::{TranslateInput, TranslateOutput};
use crate::proto::utseg::{UtSegInput, UtSegOutput};
use crate::utils::{BAError, BAResult, MediaInput, SourceId};
use async_trait::async_trait;
use futures::future::BoxFuture;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::marker::PhantomData;
use std::path::Path;
use talkbank_model::ChatFile;
use talkbank_model::ParseValidateOptions;
use talkbank_model::validation::Validated as ModelValidated;
use talkbank_transform::parse_and_validate;

// ---------------------------------------------------------------------------
// Task — the DAG node enum
// ---------------------------------------------------------------------------

/// The closed set of pipeline stages.
#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize, JsonSchema,
)]
#[cfg_attr(
    feature = "python",
    pyo3::pyclass(eq, eq_int, hash, frozen, rename_all = "PascalCase")
)]
pub enum Task {
    /// Automatic speech recognition.
    Asr,
    /// Forced alignment of an already-transcribed CHAT to its audio.
    Fa,
    /// Speaker diarization.
    Speaker,
    /// Utterance segmentation.
    UtSeg,
    /// Morphosyntactic tagging — adds `%mor` / `%gra` tiers.
    Morphosyntax,
    /// Machine translation.
    Translate,
    /// Coreference resolution annotation.
    Coref,
    /// Pure-AST diff against a gold-standard transcript.
    Compare,
    /// openSMILE acoustic feature extraction.
    OpenSmile,
    /// AVQI acoustic voice quality index.
    Avqi,
}

impl Task {
    /// The upstream tasks this stage requires in the DAG.
    pub const fn requires(self) -> &'static [Task] {
        match self {
            Task::Asr | Task::Speaker | Task::OpenSmile | Task::Avqi => &[],
            Task::UtSeg => &[Task::Asr],
            Task::Fa => &[Task::UtSeg],
            Task::Morphosyntax => &[Task::UtSeg],
            Task::Coref => &[Task::Morphosyntax],
            Task::Translate => &[Task::Morphosyntax],
            Task::Compare => &[],
        }
    }

    /// Stable, short name used in `@Comment:` provenance and progress events.
    pub const fn as_str(self) -> &'static str {
        match self {
            Task::Asr => "asr",
            Task::Fa => "fa",
            Task::Speaker => "speaker",
            Task::UtSeg => "utseg",
            Task::Morphosyntax => "morphosyntax",
            Task::Translate => "translate",
            Task::Coref => "coref",
            Task::Compare => "compare",
            Task::OpenSmile => "opensmile",
            Task::Avqi => "avqi",
        }
    }

    /// Every variant — useful for iteration in tests and codegen.
    pub const ALL: [Task; 10] = [
        Task::Asr,
        Task::Fa,
        Task::Speaker,
        Task::UtSeg,
        Task::Morphosyntax,
        Task::Translate,
        Task::Coref,
        Task::Compare,
        Task::OpenSmile,
        Task::Avqi,
    ];
}

// ---------------------------------------------------------------------------
// TaskInput / TaskOutput closed-union variants
// ---------------------------------------------------------------------------

macro_rules! union_input_output {
    (
        input  { $( $variant:ident($input_ty:ty) => $task:ident ),* $(,)? }
        output { $( $ovariant:ident($output_ty:ty) ),* $(,)? }
    ) => {
        /// The closed set of inputs that cross to a backend.
        #[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
        #[serde(tag = "task", content = "data")]
        pub enum TaskInput {
            $( $variant($input_ty), )*
        }

        impl TaskInput {
            /// The task discriminator this input services.
            pub fn task(&self) -> Task {
                match self { $( TaskInput::$variant(_) => Task::$task, )* }
            }

            /// Borrow the source-id carried inside the variant.
            pub fn source_id(&self) -> &SourceId {
                match self { $( TaskInput::$variant(i) => &i.source_id, )* }
            }
        }

        $(
            impl From<$input_ty> for TaskInput {
                fn from(i: $input_ty) -> Self { TaskInput::$variant(i) }
            }
        )*

        /// The closed set of outputs from a backend.
        #[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
        #[serde(tag = "task", content = "data")]
        pub enum TaskOutput {
            $( $ovariant($output_ty), )*
        }

        impl TaskOutput {
            /// The task discriminator this output came from.
            pub fn task(&self) -> Task {
                match self { $( TaskOutput::$ovariant(_) => Task::$ovariant, )* }
            }
        }

        $(
            impl From<$output_ty> for TaskOutput {
                fn from(o: $output_ty) -> Self { TaskOutput::$ovariant(o) }
            }
        )*
    };
}

union_input_output! {
    input {
        Asr(AsrInput) => Asr,
        Fa(FaInput) => Fa,
        Speaker(SpeakerInput) => Speaker,
        UtSeg(UtSegInput) => UtSeg,
        Morphosyntax(MorphosyntaxInput) => Morphosyntax,
        Translate(TranslateInput) => Translate,
        Coref(CorefInput) => Coref,
        OpenSmile(OpenSmileInput) => OpenSmile,
        Avqi(AvqiInput) => Avqi,
        Compare(CompareInput) => Compare,
    }
    output {
        Asr(AsrOutput),
        Fa(FaOutput),
        Speaker(SpeakerOutput),
        UtSeg(UtSegOutput),
        Morphosyntax(MorphosyntaxOutput),
        Translate(TranslateOutput),
        Coref(CorefOutput),
        OpenSmile(OpenSmileOutput),
        Avqi(AvqiOutput),
        Compare(CompareOutput),
    }
}

/// Hand-rolled `TryFrom<TaskOutput>` impls so runners can downcast cleanly.
macro_rules! try_from_output {
    ($( $variant:ident($ty:ty) ),* $(,)?) => { $(
        impl TryFrom<TaskOutput> for $ty {
            type Error = BAError;
            fn try_from(o: TaskOutput) -> Result<Self, Self::Error> {
                match o {
                    TaskOutput::$variant(x) => Ok(x),
                    other => Err(BAError::Worker(format!(
                        "expected {} got {:?}", stringify!($variant), other.task()
                    ))),
                }
            }
        }
    )* };
}

try_from_output! {
    Asr(AsrOutput),
    Fa(FaOutput),
    Speaker(SpeakerOutput),
    UtSeg(UtSegOutput),
    Morphosyntax(MorphosyntaxOutput),
    Translate(TranslateOutput),
    Coref(CorefOutput),
    OpenSmile(OpenSmileOutput),
    Avqi(AvqiOutput),
    Compare(CompareOutput),
}

// ---------------------------------------------------------------------------
// Chat<S> — typestate wrapper around ChatFile<Validated>
// ---------------------------------------------------------------------------

/// Marker for a validated CHAT document.
#[derive(Debug, Clone, Copy)]
pub struct Validated;

/// Marker for a not-yet-validated CHAT document. Internal-only.
#[derive(Debug, Clone, Copy)]
pub struct NotValidated;

/// Typestate-tagged CHAT document.
#[derive(Debug)]
pub struct Chat<S = Validated> {
    ast: ChatFile<ModelValidated>,
    source_id: SourceId,
    /// Originating audio reference, threaded through the pipeline so
    /// downstream stages (FA, speaker, opensmile, avqi) can decode PCM
    /// without re-parsing `@Media` headers.
    media: Option<MediaInput>,
    _state: PhantomData<S>,
}

impl Chat<Validated> {
    /// Parse + validate from CHAT text. The only public constructor.
    pub fn parse(text: &str, source_id: SourceId) -> BAResult<Self> {
        let options = ParseValidateOptions::default().with_validation();
        let chat_file = parse_and_validate(text, options).map_err(|e| match e {
            talkbank_transform::PipelineError::Parse(errs) => {
                BAError::Parse(format!("{} parse error(s)", errs.errors.len()))
            }
            talkbank_transform::PipelineError::Validation(errs) => {
                BAError::Validation(format!("{} validation error(s)", errs.len()))
            }
            other => BAError::Internal(format!("pipeline: {other}")),
        })?;
        let collector = talkbank_model::ErrorCollector::new();
        let validated = chat_file.validate_into(&collector, None);
        Ok(Self {
            ast: validated,
            source_id,
            media: None,
            _state: PhantomData,
        })
    }

    /// Lift an already-validated `ChatFile` (e.g. constructed by an ASR runner
    /// from scratch) into a `Chat<Validated>`.
    pub fn from_validated_ast(ast: ChatFile<ModelValidated>, source_id: SourceId) -> Self {
        Self {
            ast,
            source_id,
            media: None,
            _state: PhantomData,
        }
    }

    /// Attach an audio source so audio-dependent runners (FA, speaker, opensmile, avqi)
    /// can locate the media after the document leaves the ASR stage.
    pub fn with_media(mut self, media: MediaInput) -> Self {
        self.media = Some(media);
        self
    }

    /// The audio source originally associated with this CHAT, if any.
    pub fn media(&self) -> Option<&MediaInput> {
        self.media.as_ref()
    }

    /// Borrow the underlying validated AST.
    pub fn ast(&self) -> &ChatFile<ModelValidated> {
        &self.ast
    }

    /// Identifier the pipeline keys this CHAT by.
    pub fn source_id(&self) -> &SourceId {
        &self.source_id
    }

    /// Resolve the file's primary language from its `@Languages:` header.
    pub fn primary_language(&self) -> Option<String> {
        self.ast
            .languages
            .iter()
            .next()
            .map(|code| code.as_str().to_string())
    }

    /// Borrow the underlying validated AST mutably.
    pub fn ast_mut(&mut self) -> &mut ChatFile<ModelValidated> {
        &mut self.ast
    }

    /// Serialize back to CHAT text and write to disk.
    pub fn write(&self, path: &Path) -> BAResult<()> {
        let text = self.ast.to_chat();
        std::fs::write(path, text)?;
        Ok(())
    }

    /// Serialize back to CHAT text.
    pub fn to_chat(&self) -> String {
        self.ast.to_chat()
    }
}

// ---------------------------------------------------------------------------
// Paired — Compare-task two-CHAT input shape
// ---------------------------------------------------------------------------

/// A `(main, gold)` pair fed to the Compare task.
#[derive(Debug)]
pub struct Paired {
    main: Chat<Validated>,
    gold: Chat<Validated>,
}

impl Paired {
    /// Build a `Paired` from two validated CHATs.
    pub fn new(main: Chat<Validated>, gold: Chat<Validated>) -> Self {
        Self { main, gold }
    }

    /// Borrow the main (writable) side.
    pub fn main(&self) -> &Chat<Validated> {
        &self.main
    }

    /// Borrow the gold (reference) side.
    pub fn gold(&self) -> &Chat<Validated> {
        &self.gold
    }

    /// Consume into `(main, gold)` for runner mutation.
    pub fn into_parts(self) -> (Chat<Validated>, Chat<Validated>) {
        (self.main, self.gold)
    }

    /// Borrow both sides mutably. Used by runners that need to operate on
    /// each chat in place — most notably the morphosyntax runner running
    /// over a Paired before Compare, so both sides pick up `%mor` tiers.
    pub fn as_mut_parts(&mut self) -> (&mut Chat<Validated>, &mut Chat<Validated>) {
        (&mut self.main, &mut self.gold)
    }

    /// The pipeline-facing identity — always the main side.
    pub fn source_id(&self) -> &SourceId {
        self.main.source_id()
    }
}

// ---------------------------------------------------------------------------
// BAValue — the runtime value flowing through a pipeline
// ---------------------------------------------------------------------------

/// The runtime value flowing through a pipeline.
///
/// `Cons` + `Nil` give us a Lisp-style list so a single task can emit more
/// than one artifact for one source — e.g. Compare returns
/// `Cons(Chat(annotated), Cons(Metrics(per-pos CSV), Nil))`. The driver
/// walks the list and writes each variant to its natural file extension
/// (Chat → `.cha`, Metrics → `.compare.csv` / `.avqi.csv` / etc.).
#[derive(Debug)]
pub enum BAValue {
    /// A reference to media on disk (the typical pipeline input).
    Media(MediaInput),
    /// A validated CHAT document.
    Chat(Chat<Validated>),
    /// Compare input: a main CHAT and a gold reference CHAT.
    Paired(Paired),
    /// Terminal metrics (openSMILE, AVQI, compare summaries, …).
    Metrics(MetricsArtifact),
    /// Lisp-style list cell. Tasks that emit multiple artifacts return a
    /// chain of these terminated by `Nil`.
    Cons {
        head: Box<BAValue>,
        tail: Box<BAValue>,
    },
    /// Empty list — the only way `Cons` chains terminate.
    Nil,
    /// Poison-pill: the run died for this source.
    Failed {
        /// The source the run was associated with.
        source_id: SourceId,
        /// The error that terminated the run.
        error: BAError,
        /// The last good intermediate value, if any.
        partial: Option<Box<BAValue>>,
    },
}

impl BAValue {
    /// Lisp-style list builder: chain values into a `Cons` list terminated
    /// by `Nil`. Convenience for runners that emit more than one artifact
    /// (`BAValue::list(vec![chat, metrics])`).
    pub fn list(items: Vec<BAValue>) -> BAValue {
        let mut acc = BAValue::Nil;
        for item in items.into_iter().rev() {
            acc = BAValue::Cons {
                head: Box::new(item),
                tail: Box::new(acc),
            };
        }
        acc
    }

    /// Identify the source this value belongs to.
    pub fn source_id(&self) -> SourceId {
        match self {
            BAValue::Media(m) => m.source_id.clone(),
            BAValue::Chat(c) => c.source_id().clone(),
            BAValue::Paired(p) => p.source_id().clone(),
            BAValue::Metrics(m) => m.source_id.clone(),
            BAValue::Failed { source_id, .. } => source_id.clone(),
            // Lists adopt the head's identity. An empty list has no obvious
            // source, but downstream code shouldn't be writing one either —
            // return a placeholder so error paths don't panic.
            BAValue::Cons { head, .. } => head.source_id(),
            BAValue::Nil => SourceId::try_new("nil").unwrap_or_else(|_| {
                SourceId::try_new("unknown").expect("'unknown' is non-empty")
            }),
        }
    }

    /// `true` if this value has poison-pilled. `Cons` is failed iff any
    /// element is failed; `Nil` is never failed.
    pub fn is_failed(&self) -> bool {
        match self {
            BAValue::Failed { .. } => true,
            BAValue::Cons { head, tail } => head.is_failed() || tail.is_failed(),
            _ => false,
        }
    }

    /// Short kind tag, used in errors and progress events.
    pub fn kind(&self) -> &'static str {
        match self {
            BAValue::Media(_) => "Media",
            BAValue::Chat(_) => "Chat",
            BAValue::Paired(_) => "Paired",
            BAValue::Metrics(_) => "Metrics",
            BAValue::Failed { .. } => "Failed",
            BAValue::Cons { .. } => "Cons",
            BAValue::Nil => "Nil",
        }
    }

    /// Persist this value to `path`. `Cons` walks the list, writing each
    /// element to a path derived from the base (Chat keeps the base path,
    /// Metrics re-extensions to `<kind>.csv`, `Nil` is a no-op).
    pub fn write(&self, path: &Path) -> BAResult<()> {
        match self {
            BAValue::Chat(c) => c.write(path),
            BAValue::Paired(p) => p.main().write(path),
            BAValue::Metrics(m) => write_metrics_csv(m, path),
            BAValue::Media(_) => Err(BAError::Internal(
                "pipeline did not process media into a writable output".into(),
            )),
            BAValue::Cons { head, tail } => {
                head.write(path)?;
                tail.write(path)
            }
            BAValue::Nil => Ok(()),
            BAValue::Failed { error, partial, .. } => {
                let log_path = path.with_extension("error.log");
                std::fs::write(&log_path, format!("{error:#}\n"))?;
                if let Some(p) = partial {
                    p.write(path)?;
                }
                Ok(())
            }
        }
    }
}

/// File-extension picker per metrics kind. Mirrors `engine/metrics_writer`'s
/// table — kept here so `BAValue::write` can render metrics without a
/// dependency on the engine crate.
fn metrics_extension(kind: crate::metrics::MetricsKind) -> &'static str {
    use crate::metrics::MetricsKind;
    match kind {
        MetricsKind::Opensmile => "opensmile.csv",
        MetricsKind::Avqi => "avqi.csv",
        MetricsKind::Compare => "compare.csv",
        MetricsKind::Benchmark => "benchmark.csv",
        MetricsKind::Custom => "metrics.csv",
    }
}

/// Render one `MetricsArtifact` as CSV to `<path>.with_extension(<kind>.csv)`.
fn write_metrics_csv(artifact: &MetricsArtifact, path: &Path) -> BAResult<()> {
    use std::fmt::Write as _;
    let target = path.with_extension(metrics_extension(artifact.kind));
    let mut out = String::new();
    write_csv_row(&mut out, &artifact.table.schema);
    for row in &artifact.table.rows {
        let cells: Vec<String> = artifact
            .table
            .schema
            .iter()
            .map(|col| {
                row.columns
                    .get(col)
                    .map(json_cell)
                    .unwrap_or_default()
            })
            .collect();
        write_csv_row(&mut out, &cells);
    }
    std::fs::write(&target, out)?;
    Ok(())
}

fn json_cell(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::Null => String::new(),
        serde_json::Value::String(s) => s.clone(),
        other => other.to_string(),
    }
}

fn write_csv_row(out: &mut String, cells: &[String]) {
    use std::fmt::Write as _;
    for (i, cell) in cells.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }
        write_csv_cell(out, cell);
    }
    let _ = writeln!(out);
}

fn write_csv_cell(out: &mut String, cell: &str) {
    let needs_quotes = cell
        .chars()
        .any(|c| c == ',' || c == '"' || c == '\n' || c == '\r');
    if needs_quotes {
        out.push('"');
        for c in cell.chars() {
            if c == '"' {
                out.push_str("\"\"");
            } else {
                out.push(c);
            }
        }
        out.push('"');
    } else {
        out.push_str(cell);
    }
}

// ---------------------------------------------------------------------------
// Progress events
// ---------------------------------------------------------------------------

/// What happened to which source at which stage.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct ProgressEvent {
    /// The input this event is about.
    pub source_id: SourceId,
    /// The stage that emitted (None for end-of-run events).
    pub task: Option<Task>,
    /// What changed.
    pub kind: ProgressKind,
    /// Optional intra-stage progress.
    pub completed: u64,
    /// Optional intra-stage total.
    pub total: u64,
    /// Optional human-readable label.
    pub label: String,
}

/// Closed set of progress signals.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(eq, eq_int, hash, frozen))]
pub enum ProgressKind {
    /// A stage started for the given source.
    StageStarted,
    /// A stage successfully injected its output into the value.
    StageInjected,
    /// A stage failed and produced a `Failed` poison-pill.
    StageFailed,
    /// A stage was skipped.
    StageSkipped,
    /// Pipeline finished all stages for this source.
    SourceCompleted,
}

impl ProgressEvent {
    /// Convenience constructor for `StageStarted`.
    pub fn stage_started(source_id: &SourceId, task: Task) -> Self {
        Self {
            source_id: source_id.clone(),
            task: Some(task),
            kind: ProgressKind::StageStarted,
            completed: 0,
            total: 0,
            label: String::new(),
        }
    }

    /// Convenience constructor for `StageInjected`.
    pub fn stage_injected(source_id: &SourceId, task: Task) -> Self {
        Self {
            source_id: source_id.clone(),
            task: Some(task),
            kind: ProgressKind::StageInjected,
            completed: 0,
            total: 0,
            label: String::new(),
        }
    }

    /// Convenience constructor for `StageFailed`.
    pub fn stage_failed(source_id: &SourceId, task: Task, msg: impl Into<String>) -> Self {
        Self {
            source_id: source_id.clone(),
            task: Some(task),
            kind: ProgressKind::StageFailed,
            completed: 0,
            total: 0,
            label: msg.into(),
        }
    }

    /// Convenience constructor for `SourceCompleted`.
    pub fn source_completed(source_id: &SourceId) -> Self {
        Self {
            source_id: source_id.clone(),
            task: None,
            kind: ProgressKind::SourceCompleted,
            completed: 0,
            total: 0,
            label: String::new(),
        }
    }
}

/// Pluggable progress callback sink.
pub trait ProgressSink: Send + Sync {
    /// Emit one progress event.
    fn emit(&self, event: ProgressEvent);
}

/// A `ProgressSink` that drops everything.
#[derive(Debug, Default, Clone, Copy)]
pub struct NullSink;

impl ProgressSink for NullSink {
    fn emit(&self, _event: ProgressEvent) {}
}

// ---------------------------------------------------------------------------
// TaskRunner traits + Dispatcher
// ---------------------------------------------------------------------------

/// What a `TaskRunner` talks to when it wants to call a backend.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// Send `input` to its task's batcher and await the typed output.
    async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput>;
}

/// Typed runner. Runners are *canonical* (one per `Task`) and stateless —
/// they have no per-invocation config; per-pipeline tunables live on the
/// backend constructor side, and per-file hints (language) are read off
/// the chat's `@Languages:` header at dispatch time.
#[async_trait]
pub trait TaskRunner: Send + Sync {
    /// The DAG node this runner services.
    const TASK: Task;

    /// Run the stage: mutate or replace `value` using `dispatcher`.
    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: &dyn ProgressSink,
    ) -> BAResult<()>;
}

/// Erasure trait — what the pipeline holds in its `HashMap<Task, Box<dyn …>>`.
pub trait DynTaskRunner: Send + Sync {
    /// Which task this runner services.
    fn task(&self) -> Task;
    /// Apply the runner.
    fn apply<'a>(
        &'a self,
        value: &'a mut BAValue,
        dispatcher: &'a dyn Dispatcher,
        sink: &'a dyn ProgressSink,
    ) -> BoxFuture<'a, BAResult<()>>;
}

impl<T: TaskRunner + 'static> DynTaskRunner for T {
    fn task(&self) -> Task {
        T::TASK
    }

    fn apply<'a>(
        &'a self,
        value: &'a mut BAValue,
        dispatcher: &'a dyn Dispatcher,
        sink: &'a dyn ProgressSink,
    ) -> BoxFuture<'a, BAResult<()>> {
        Box::pin(async move {
            <T as TaskRunner>::apply(self, value, dispatcher, sink).await
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn dag_is_acyclic() {
        let mut remaining: HashSet<Task> = Task::ALL.iter().copied().collect();
        while !remaining.is_empty() {
            let leaf = remaining
                .iter()
                .copied()
                .find(|t| t.requires().iter().all(|r| !remaining.contains(r)));
            match leaf {
                Some(t) => {
                    remaining.remove(&t);
                }
                None => panic!("cycle detected among remaining: {remaining:?}"),
            }
        }
    }

    #[test]
    fn requires_targets_are_real_tasks() {
        for t in Task::ALL {
            for r in t.requires() {
                assert!(Task::ALL.contains(r), "{t:?}.requires references {r:?}");
            }
        }
    }

    #[test]
    fn source_id_rejects_empty() {
        assert!(SourceId::try_new("").is_err());
        assert!(SourceId::try_new("   ").is_err());
        assert!(SourceId::try_new("hi").is_ok());
    }

    #[test]
    fn write_media_is_error() {
        let v = BAValue::Media(MediaInput {
            source_id: SourceId::new_unchecked("x"),
            path: "/dev/null".into(),
        });
        let r = v.write(Path::new("/tmp/x.cha"));
        assert!(r.is_err());
    }

    #[test]
    fn parse_roundtrips() -> BAResult<()> {
        const FIXTURE: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Child\n@ID:\teng|corpus|CHI|||||Child|||\n*CHI:\thello .\n@End\n";
        let sid = SourceId::try_new("fixture")?;
        let chat = Chat::parse(FIXTURE, sid)?;
        let s = chat.to_chat();
        assert!(s.contains("*CHI:"));
        Ok(())
    }
}
