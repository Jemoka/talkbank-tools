//! Core pipeline base types: `Task` DAG, runner traits, closed-set TaskInput /
//! TaskOutput unions, the `BAValue` flow type (with `Paired` inlined), the
//! typestate `Chat<S>` wrapper, and the progress-event channel.
//!
//! Consolidated from `task.rs`, `task_runner.rs`, `union.rs`, `value.rs`,
//! `paired.rs`, `chat.rs`, and `progress.rs` per the spec2.md reorg.

use crate::metrics::MetricsArtifact;
use crate::proto::asr::{AsrInput, AsrOutput};
use crate::proto::compare::{CompareInput, CompareOutput};
use crate::proto::coref::{CorefInput, CorefOutput};
use crate::proto::fa::{FaInput, FaOutput};
use crate::proto::morphosyntax::{MorphosyntaxInput, MorphosyntaxOutput};
use crate::proto::speaker::{SpeakerInput, SpeakerOutput};
use crate::proto::translate::{TranslateInput, TranslateOutput};
use crate::proto::utr::{UtrInput, UtrOutput};
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
    /// Utterance Timing Recovery — runs an ASR-shaped backend to get
    /// timed tokens, then Hirschberg-aligns the CHAT word stream
    /// against those tokens to inject per-utterance bullet timings on
    /// transcripts that arrived without timing. Pre-pass for FA on
    /// human-authored or hand-edited CHATs.
    Utr,
    /// Morphosyntactic tagging — adds `%mor` / `%gra` tiers.
    Morphosyntax,
    /// Machine translation.
    Translate,
    /// Coreference resolution annotation.
    Coref,
    /// Pure-AST diff against a gold-standard transcript.
    Compare,
}

impl Task {
    /// The upstream tasks this stage requires in the DAG.
    pub const fn requires(self) -> &'static [Task] {
        match self {
            Task::Asr | Task::Speaker => &[],
            Task::UtSeg => &[Task::Asr],
            // UTR has no DAG prerequisites: it operates on a CHAT plus
            // audio and produces utterance bullets from scratch. It is
            // safe to include in any pipeline because its taskrunner
            // skips when every utterance already carries a non-zero
            // bullet (see `taskrunners/utr.rs`).
            Task::Utr => &[],
            // FA needs utterance-level bullets to slice audio; UTR
            // produces them when missing. Topo-sort orders Utr before
            // Fa whenever both are declared. (Pipelines starting from
            // a hand-timed CHAT can omit Utr; pipelines from ASR
            // already get bullets from UtSeg.)
            Task::Fa => &[Task::UtSeg, Task::Utr],
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
            Task::Utr => "utr",
            Task::Morphosyntax => "morphosyntax",
            Task::Translate => "translate",
            Task::Coref => "coref",
            Task::Compare => "compare",
        }
    }

    /// Every variant — useful for iteration in tests and codegen.
    pub const ALL: [Task; 9] = [
        Task::Asr,
        Task::Fa,
        Task::Speaker,
        Task::UtSeg,
        Task::Utr,
        Task::Morphosyntax,
        Task::Translate,
        Task::Coref,
        Task::Compare,
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

        impl $crate::cache::CacheKey for TaskInput {
            fn hash(&self, hasher: &mut ::blake3::Hasher) {
                match self {
                    $( TaskInput::$variant(i) => i.hash(hasher), )*
                }
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
        // UTR's payload is serde-transparent over `AsrInput`, so the
        // wire bytes are AsrInput-shaped. The Rust newtype keeps the
        // closed-union macro's `From<UtrInput>` impl distinct from
        // `From<AsrInput>`.
        Utr(UtrInput) => Utr,
        Morphosyntax(MorphosyntaxInput) => Morphosyntax,
        Translate(TranslateInput) => Translate,
        Coref(CorefInput) => Coref,
        Compare(CompareInput) => Compare,
    }
    output {
        Asr(AsrOutput),
        Fa(FaOutput),
        Speaker(SpeakerOutput),
        UtSeg(UtSegOutput),
        Utr(UtrOutput),
        Morphosyntax(MorphosyntaxOutput),
        Translate(TranslateOutput),
        Coref(CorefOutput),
        Compare(CompareOutput),
    }
}

crate::register_proto_schema!(TaskInput);
crate::register_proto_schema!(TaskOutput);

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
    Utr(UtrOutput),
    Morphosyntax(MorphosyntaxOutput),
    Translate(TranslateOutput),
    Coref(CorefOutput),
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
    /// downstream stages (FA, speaker) can decode PCM without re-parsing
    /// `@Media` headers.
    media: Option<MediaInput>,
    _state: PhantomData<S>,
}

impl Chat<Validated> {
    /// Parse + validate from CHAT text. The only public constructor.
    pub fn parse(text: &str, source_id: SourceId) -> BAResult<Self> {
        let options = ParseValidateOptions::default().with_validation();
        let chat_file = parse_and_validate(text, options).map_err(|e| match e {
            // `ParseErrors` and each `ParseError` have rich `Display` impls
            // that include error code, line/column, and message (see
            // `talkbank-model/src/errors/parse_error.rs:328`). Pass that
            // through instead of summarising as a count — downstream Python
            // CLI surfaces it directly to the user.
            talkbank_transform::PipelineError::Parse(errs) => BAError::Parse(format!("{errs}")),
            talkbank_transform::PipelineError::Validation(errs) => {
                // Newline-separate so the Python TUI's `_try_multi_error_block`
                // renderer (cli/tui/errors.py) can engage — it requires
                // `splitlines() >= 2` to detect the bullet shape and avoid
                // collapsing dozens of E### entries onto one line.
                // Leading `\n` puts every E### entry on its own line so they
                // all render as bullets; the preamble text supplied by the
                // caller (e.g. "compare: failed to re-parse annotated_main:")
                // stays on the first line.
                let joined = errs
                    .iter()
                    .map(|err| err.to_string())
                    .collect::<Vec<_>>()
                    .join("\n");
                BAError::Validation(format!("\n{joined}"))
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

    /// Attach an audio source so audio-dependent runners (FA, speaker)
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
/// (Chat → `.cha`, Metrics → `.compare.csv` / etc.).
#[derive(Debug)]
pub enum BAValue {
    /// A reference to media on disk (the typical pipeline input).
    Media(MediaInput),
    /// A validated CHAT document.
    Chat(Chat<Validated>),
    /// Compare input: a main CHAT and a gold reference CHAT.
    Paired(Paired),
    /// Terminal metrics (compare summaries, benchmarks, …).
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
            BAValue::Nil => SourceId::try_new("nil")
                .unwrap_or_else(|_| SourceId::try_new("unknown").expect("'unknown' is non-empty")),
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
            .map(|col| row.columns.get(col).map(json_cell).unwrap_or_default())
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

    /// Convenience constructor for an in-stage progress tick.
    ///
    /// Carries `(completed, total)` so a runner with a per-unit loop
    /// (per-utterance Stanza, per-segment FA, etc.) can advance the
    /// per-file progress bar incrementally — matching BA2's
    /// `status_hook(completed, total)` semantics.
    ///
    /// Reuses `ProgressKind::StageStarted` deliberately: the Python
    /// bridge gates the progress-bar update on `ev.total > 0` rather
    /// than on `kind`, and `Task.stage_started` is idempotent in RUN
    /// state, so re-emitting it with non-zero counters is the
    /// no-schema-change path.
    pub fn stage_tick(
        source_id: &SourceId,
        task: Task,
        completed: u64,
        total: u64,
    ) -> Self {
        Self {
            source_id: source_id.clone(),
            task: Some(task),
            kind: ProgressKind::StageStarted,
            completed,
            total,
            label: format!("{completed}/{total}"),
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
// Backend-side progress: composable per-stage progress reporting
// ---------------------------------------------------------------------------
//
// Two layers report progress for a single stage:
//
//   * The *runner* (outer loop) — e.g. the morphosyntax runner dispatches
//     once per utterance; it knows the total utterance count up-front.
//   * The *backend* (inner loop) — e.g. the wav2vec2 FA backend slices
//     audio into ~15 s groups inside a single `call()`; only the backend
//     knows how many groups there are, and that count varies per call.
//
// We compose them into a single `(completed, total)` event stream via
// `ScaledProgress`. The trick is a fixed denominator (`SCALE` below):
// every outer step contributes exactly `SCALE` units regardless of how
// many inner ticks the backend emits, so the bar's `total` stays constant
// at `outer_total * SCALE` for the whole stage. Without this, the
// denominator would change every step (`outer * inner_total_of_this_call`)
// and ETA estimators / cached-max-total renderers would jitter — see the
// design discussion that led to this module for the worked example.
//
// Backends that don't want to report per-call sub-progress simply never
// invoke `BackendProgress::tick` — the bar still advances `SCALE` units
// at each `ScaledProgress::start_step` boundary, matching the previous
// runner-only ticking behavior.

/// Backend-facing progress channel.
///
/// Passed into `Backend::call_with_progress` so a backend can report
/// intra-call progress (e.g. "finished audio group `i` of `n`") without
/// having to know about `SourceId`, `Task`, or how the runner is composing
/// its work. The runner constructs a [`ScaledProgress`] that turns each
/// `tick(i, n)` into a properly scaled outer `stage_tick`.
///
/// Backends that have nothing meaningful to report just never call `tick`.
/// That is the no-op — no capability flag, no declaration. See
/// [`NullBackendProgress`] for the trivial implementation handed to
/// backends when the runner doesn't want per-call ticks.
pub trait BackendProgress: Send + Sync {
    /// Report that `completed` of `total` inner units are done.
    ///
    /// `completed` MUST be monotonically non-decreasing within a single
    /// outer step. `total` SHOULD be constant within a step but is allowed
    /// to vary across steps (the scaling absorbs the variance).
    fn tick(&self, completed: u64, total: u64);
}

/// A `BackendProgress` that drops everything. Hand this to backends when
/// the runner doesn't want per-call sub-progress (e.g. morphosyntax,
/// where the runner already ticks at outer granularity around its own
/// per-utterance dispatch loop).
#[derive(Debug, Default, Clone, Copy)]
pub struct NullBackendProgress;

impl BackendProgress for NullBackendProgress {
    fn tick(&self, _completed: u64, _total: u64) {}
}

/// Fixed denominator that every outer step contributes to the rendered
/// `(completed, total)` event. See module-level comment above for why a
/// constant denominator is load-bearing (ETA stability, renderer caching).
pub const PROGRESS_SCALE: u64 = 4;

/// Wraps a [`ProgressSink`] into a [`BackendProgress`] that composes outer
/// (runner) and inner (backend) progress into a single, monotonically
/// non-decreasing `(completed, total)` event stream.
///
/// Usage from a runner:
///
/// ```ignore
/// let prog = std::sync::Arc::new(ScaledProgress::new(
///     sink_arc, source_id, Task::Fa, outer_total,
/// ));
/// for input in inputs {
///     prog.start_step();                                       // bar → (k-1)/N
///     dispatcher
///         .dispatch_with_progress(input, prog.clone())          // backend may tick
///         .await?;
///     // the bar stays monotonic and lands at k/N when the call returns.
/// }
/// ```
///
/// Properties:
///
/// * **Monotonic.** `(k-1)*SCALE + i*SCALE/n` is non-decreasing as long
///   as the backend's `i/n` is non-decreasing within a step.
/// * **Backend-silent steps still move the bar.** `start_step` emits
///   the floor tick `(k-1)/N`, so a non-ticking backend produces N
///   visible jumps (one per step) — identical to today's
///   runner-only ticking shape.
/// * **Variable inner totals across steps.** Each step normalizes to
///   `[0, SCALE]`, so step 1's 17 groups and step 2's 3 groups don't
///   distort each other's contribution.
/// * **No protocol change.** Still `(completed, total)` on the wire.
///   Sinks and renderers don't need to know scaling happened.
///
/// `ScaledProgress` is owned via `Arc` because the inner backend call
/// runs on a `spawn_blocking` thread (Python backends acquire the GIL)
/// and the channel between runner and batcher takes ownership of the
/// progress handle. We hold the outer sink as `Arc<dyn ProgressSink>`
/// to keep the type `'static` and cheap to clone.
pub struct ScaledProgress {
    outer: std::sync::Arc<dyn ProgressSink>,
    source_id: SourceId,
    task: Task,
    outer_total: u64,
    /// Current 1-indexed outer step; 0 before any `start_step` call.
    /// `Relaxed` is enough — we publish a counter, not synchronize data;
    /// a stale read in a backend tick would just bias one tick by one
    /// step, still monotonic in practice.
    outer_step: std::sync::atomic::AtomicU64,
}

impl ScaledProgress {
    /// Construct a wrapper bound to one `(source_id, task)` pair.
    pub fn new(
        outer: std::sync::Arc<dyn ProgressSink>,
        source_id: SourceId,
        task: Task,
        outer_total: u64,
    ) -> Self {
        Self {
            outer,
            source_id,
            task,
            outer_total,
            outer_step: std::sync::atomic::AtomicU64::new(0),
        }
    }

    /// Advance to the next outer step and emit a floor tick.
    ///
    /// Call this BEFORE issuing the dispatch for step k. The bar will
    /// land at `(k-1) * SCALE / outer_total * SCALE`. The bar advances
    /// further if/when the backend calls `tick` during the dispatch.
    pub fn start_step(&self) {
        let k = self
            .outer_step
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        // Floor of step k: i=0, n=1 → inner=0 → completed = (k-1)*SCALE.
        self.emit_scaled(k - 1, 0, 1);
    }

    /// Emit a final ceiling tick at `outer_total * SCALE / outer_total * SCALE`
    /// so the bar lands at 100% after the loop finishes.
    ///
    /// Call this once after the last dispatch completes. Renderers that
    /// only track `(completed, total)` ratios need this to see the bar
    /// reach 100% — `start_step` emits the *floor* of each step, not the
    /// ceiling of the previous one.
    pub fn finish(&self) {
        // Saturate at outer_total — calling `finish` mid-stage is a
        // misuse but should still produce a sensible 100% bar.
        let total = self.outer_total;
        self.emit_scaled(total, 0, 1);
    }

    /// Emit one (completed, total) event scaled by `PROGRESS_SCALE`.
    /// `k_minus_1` is the 0-indexed outer step the inner tick belongs to.
    fn emit_scaled(&self, k_minus_1: u64, i: u64, n: u64) {
        // Inner ticks at unknown granularity (n=0) collapse to 0
        // contribution within the step — defensive, the typical n is ≥1.
        let inner = if n == 0 { 0 } else { i * PROGRESS_SCALE / n };
        let completed = k_minus_1 * PROGRESS_SCALE + inner;
        let total = self.outer_total * PROGRESS_SCALE;
        self.outer
            .emit(ProgressEvent::stage_tick(&self.source_id, self.task, completed, total));
    }
}

impl BackendProgress for ScaledProgress {
    fn tick(&self, i: u64, n: u64) {
        // Read (not bump) the current outer step. A backend ticking
        // before any `start_step` would land at k_minus_1 = -1 in
        // signed math; we floor at 0 via saturating_sub so a misuse
        // produces noise, not panic.
        let k = self
            .outer_step
            .load(std::sync::atomic::Ordering::Relaxed)
            .saturating_sub(1);
        self.emit_scaled(k, i, n);
    }
}

// ---------------------------------------------------------------------------
// TaskRunner traits + Dispatcher
// ---------------------------------------------------------------------------

/// What a `TaskRunner` talks to when it wants to call a backend.
#[async_trait]
pub trait Dispatcher: Send + Sync {
    /// Send `input` to its task's batcher and await the typed output.
    async fn dispatch(&self, input: TaskInput) -> BAResult<TaskOutput>;

    /// Like [`dispatch`] but carries a `BackendProgress` channel the
    /// backend may use to report intra-call progress (audio groups,
    /// per-item ticks, …). Default impl ignores the channel and falls
    /// through to `dispatch`, so legacy implementations stay correct.
    ///
    /// The handle is passed by `Arc` (not `&dyn`) because the engine
    /// implementation ships it across a `spawn_blocking` boundary into
    /// the backend's call thread. Runners typically construct one
    /// [`ScaledProgress`] for the whole stage and `clone` the `Arc`
    /// into each dispatch.
    async fn dispatch_with_progress(
        &self,
        input: TaskInput,
        _progress: std::sync::Arc<dyn BackendProgress>,
    ) -> BAResult<TaskOutput> {
        self.dispatch(input).await
    }

    /// Name of the backend currently registered to serve `task`, if any.
    ///
    /// Runners use this to annotate generated artifacts (e.g. ASR stamps a
    /// `@Comment: batchalign3 v… | engine: …` header on the CHAT file it
    /// produces). Default implementation returns `None` for stubs / tests
    /// that don't carry routing information.
    fn engine_name(&self, _task: Task) -> Option<String> {
        None
    }
}

/// Typed runner. Runners are *canonical* (one per `Task`) and stateless —
/// they have no per-invocation config; per-pipeline tunables live on the
/// backend constructor side, and per-file hints (language) are read off
/// the chat's `@Languages:` header at dispatch time.
///
/// `sink` is an `Arc` (not a borrow) so the runner can hand the same
/// underlying sink to [`ScaledProgress`], which lives long enough to be
/// shipped across the engine's `spawn_blocking` boundary. Borrowing
/// would prevent that — the trait object needs `'static` to cross
/// thread boundaries inside Tokio.
#[async_trait]
pub trait TaskRunner: Send + Sync {
    /// The DAG node this runner services.
    const TASK: Task;

    /// Run the stage: mutate or replace `value` using `dispatcher`.
    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
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
        sink: std::sync::Arc<dyn ProgressSink>,
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
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BoxFuture<'a, BAResult<()>> {
        Box::pin(async move { <T as TaskRunner>::apply(self, value, dispatcher, sink).await })
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
    fn fa_requires_utr_for_topological_ordering() {
        // The pipeline relies on `Task::Fa::requires() = [.., Utr]` to
        // place UTR before FA when both are declared. Lock that in.
        assert!(Task::Fa.requires().contains(&Task::Utr));
        // UTR itself has no DAG prerequisites — it operates on the raw
        // CHAT and is skip-safe when bullets already exist.
        assert!(Task::Utr.requires().is_empty());
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
