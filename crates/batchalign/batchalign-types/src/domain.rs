//! Domain newtypes and small enums shared across batchalign crates.
//!
//! These are re-exported from [`super::api`] for backward compatibility.

use std::borrow::Cow;

use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

// ---------------------------------------------------------------------------
// Domain newtypes (shared across modules, re-exported from lib.rs)
// ---------------------------------------------------------------------------

validated_string_id!(
    /// Server-assigned identifier for a job (non-empty).
    pub JobId
);

/// Closed released command vocabulary used at all Rust seams.
///
/// This is the single canonical command type. Unknown command strings are
/// rejected at deserialization boundaries (HTTP 422, DB recovery skip).
#[derive(
    Debug,
    Clone,
    Copy,
    PartialEq,
    Eq,
    Hash,
    serde::Serialize,
    serde::Deserialize,
    utoipa::ToSchema,
    schemars::JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum ReleasedCommand {
    Align,
    Transcribe,
    TranscribeS,
    Translate,
    Morphotag,
    Coref,
    Utseg,
    Benchmark,
    Opensmile,
    Compare,
    Avqi,
}

/// Error returned when one string is not a released command name.
#[derive(Debug, Clone, thiserror::Error)]
#[error("unknown released command \"{0}\"")]
pub struct InvalidReleasedCommand(pub String);

impl ReleasedCommand {
    /// All released commands in a stable contributor-facing order.
    pub const ALL: [Self; 11] = [
        Self::Align,
        Self::Transcribe,
        Self::TranscribeS,
        Self::Translate,
        Self::Morphotag,
        Self::Coref,
        Self::Utseg,
        Self::Benchmark,
        Self::Opensmile,
        Self::Compare,
        Self::Avqi,
    ];

    /// Parse one untrusted released-command token.
    pub fn parse_untrusted(value: &str) -> Result<Self, InvalidReleasedCommand> {
        Self::try_from(value.trim())
    }

    /// Return the canonical snake_case released command name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Align => "align",
            Self::Transcribe => "transcribe",
            Self::TranscribeS => "transcribe_s",
            Self::Translate => "translate",
            Self::Morphotag => "morphotag",
            Self::Coref => "coref",
            Self::Utseg => "utseg",
            Self::Benchmark => "benchmark",
            Self::Opensmile => "opensmile",
            Self::Compare => "compare",
            Self::Avqi => "avqi",
        }
    }

    /// Return the canonical wire/storage spelling.
    pub const fn as_wire_name(self) -> &'static str {
        self.as_str()
    }

    /// Return whether this released command requires client-local audio access.
    pub const fn uses_local_audio(self) -> bool {
        matches!(
            self,
            Self::Transcribe | Self::TranscribeS | Self::Benchmark | Self::Avqi
        )
    }
}

impl std::fmt::Display for ReleasedCommand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl AsRef<str> for ReleasedCommand {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

impl PartialEq<&str> for ReleasedCommand {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

impl TryFrom<&str> for ReleasedCommand {
    type Error = InvalidReleasedCommand;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "align" => Ok(Self::Align),
            "transcribe" => Ok(Self::Transcribe),
            "transcribe_s" => Ok(Self::TranscribeS),
            "translate" => Ok(Self::Translate),
            "morphotag" => Ok(Self::Morphotag),
            "coref" => Ok(Self::Coref),
            "utseg" => Ok(Self::Utseg),
            "benchmark" => Ok(Self::Benchmark),
            "opensmile" => Ok(Self::Opensmile),
            "compare" => Ok(Self::Compare),
            "avqi" => Ok(Self::Avqi),
            other => Err(InvalidReleasedCommand(other.to_owned())),
        }
    }
}

/// Borrowed CHAT document text at a contributor-facing boundary.
///
/// This wrapper is intentionally lightweight: it prevents workflow/request
/// types from collapsing back into raw `&str` while still borrowing the
/// underlying document text without allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ChatText<'a>(&'a str);

impl<'a> ChatText<'a> {
    /// Wrap one borrowed CHAT document string.
    pub fn new(text: &'a str) -> Self {
        Self(text)
    }

    /// Borrow the underlying CHAT string.
    pub fn as_str(self) -> &'a str {
        self.0
    }
}

impl std::fmt::Display for ChatText<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.0)
    }
}

impl<'a> From<&'a str> for ChatText<'a> {
    fn from(value: &'a str) -> Self {
        Self::new(value)
    }
}

impl std::ops::Deref for ChatText<'_> {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl AsRef<str> for ChatText<'_> {
    fn as_ref(&self) -> &str {
        self.0
    }
}

// ---------------------------------------------------------------------------
// LanguageCode3 — validated 3-letter ISO 639-3 language code
// ---------------------------------------------------------------------------

/// 3-letter ISO 639-3 language code (e.g. `"eng"`, `"spa"`).
///
/// Construction validates that the value is exactly 3 ASCII alphabetic
/// characters, lowercased. Sentinel values like `"auto"` are rejected — use
/// [`LanguageSpec`] at boundaries where auto-detection is meaningful.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, utoipa::ToSchema, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct LanguageCode3(pub String);

/// Error returned when a string is not a valid 3-letter ISO 639-3 code.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid language code \"{0}\": expected 3 ASCII letters (e.g. \"eng\", \"spa\")")]
pub struct InvalidLanguageCode(pub String);

impl LanguageCode3 {
    // -- Well-known language codes (use these instead of string literals) --

    /// English (`"eng"`).
    pub fn eng() -> Self {
        Self("eng".to_owned())
    }
    /// Spanish (`"spa"`).
    pub fn spa() -> Self {
        Self("spa".to_owned())
    }
    /// French (`"fra"`).
    pub fn fra() -> Self {
        Self("fra".to_owned())
    }
    /// Chinese / Mandarin (`"zho"`).
    pub fn zho() -> Self {
        Self("zho".to_owned())
    }
    /// Cantonese (`"yue"`).
    pub fn yue() -> Self {
        Self("yue".to_owned())
    }
    /// Japanese (`"jpn"`).
    pub fn jpn() -> Self {
        Self("jpn".to_owned())
    }
    /// German (`"deu"`).
    pub fn deu() -> Self {
        Self("deu".to_owned())
    }

    // -- Construction --

    /// Try to create a validated language code.
    ///
    /// Validation: exactly 3 ASCII alphabetic characters, lowercased.
    /// Rejects `"auto"`, `""`, `"en"`, `"english"`, etc.
    ///
    /// This is the **only** way to construct a `LanguageCode3` from
    /// untrusted input. Use well-known constants (e.g. [`Self::eng()`])
    /// for compile-time-known values.
    pub fn try_new(s: &str) -> Result<Self, InvalidLanguageCode> {
        let s = s.trim();
        if s.len() == 3 && s.bytes().all(|b| b.is_ascii_alphabetic()) {
            Ok(Self(s.to_ascii_lowercase()))
        } else {
            Err(InvalidLanguageCode(s.to_string()))
        }
    }
}

impl std::fmt::Display for LanguageCode3 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl TryFrom<String> for LanguageCode3 {
    type Error = InvalidLanguageCode;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Self::try_new(&s)
    }
}

impl TryFrom<&str> for LanguageCode3 {
    type Error = InvalidLanguageCode;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        Self::try_new(s)
    }
}

impl From<LanguageCode3> for String {
    fn from(v: LanguageCode3) -> String {
        v.0
    }
}

impl std::ops::Deref for LanguageCode3 {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for LanguageCode3 {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for LanguageCode3 {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for LanguageCode3 {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl Default for LanguageCode3 {
    fn default() -> Self {
        Self::eng()
    }
}

impl<'de> serde::Deserialize<'de> for LanguageCode3 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::try_new(&s).map_err(serde::de::Error::custom)
    }
}

// ---------------------------------------------------------------------------
// WorkerLanguage — worker-runtime language routing, not a domain language code
// ---------------------------------------------------------------------------

/// Worker-runtime language routed to Python workers.
///
/// This is intentionally distinct from [`LanguageCode3`]. The worker runtime
/// accepts a small sentinel vocabulary that is meaningful only at the process
/// bootstrap/dispatch boundary:
///
/// - `Resolved(code)` for a concrete ISO 639-3 language
/// - `Auto` for ASR auto-detection
/// - `PerFile` for text-NLP commands (morphotag/translate/coref) that
///   resolve language per-file from each CHAT file's `@Languages:`
///   header. Distinct from `Auto`: the Python worker must NOT try to
///   load language-specific models for a `PerFile` worker — language
///   pipelines are loaded lazily as files dispatch.
/// - `Unspecified` when the worker task does not consume a language hint
#[derive(Debug, Clone, PartialEq, Eq, Hash, utoipa::ToSchema)]
pub enum WorkerLanguage {
    /// Concrete ISO 639-3 language code.
    Resolved(LanguageCode3),
    /// ASR auto-detection sentinel.
    Auto,
    /// Per-file language resolution (no job-level language).
    PerFile,
    /// No worker language hint should be provided.
    Unspecified,
}

/// Error returned when a worker-runtime language string is invalid.
#[derive(Debug, Clone, thiserror::Error)]
#[error(
    "invalid worker language \"{0}\": expected 3 ASCII letters, \"auto\", \"per-file\", or an empty string"
)]
pub struct InvalidWorkerLanguage(pub String);

impl WorkerLanguage {
    /// Parse one untrusted worker-runtime language string.
    pub fn parse_untrusted(s: &str) -> Result<Self, InvalidWorkerLanguage> {
        let s = s.trim();
        if s.is_empty() {
            Ok(Self::Unspecified)
        } else if s.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else if s.eq_ignore_ascii_case("per-file") {
            Ok(Self::PerFile)
        } else {
            LanguageCode3::try_new(s)
                .map(Self::Resolved)
                .map_err(|_| InvalidWorkerLanguage(s.to_string()))
        }
    }

    /// Return the CLI/registry string form used by the worker runtime.
    pub fn as_worker_arg(&self) -> &str {
        match self {
            Self::Resolved(code) => code.as_ref(),
            Self::Auto => "auto",
            Self::PerFile => "per-file",
            Self::Unspecified => "",
        }
    }

    /// Return the resolved ISO language code, if present.
    pub fn as_resolved(&self) -> Option<&LanguageCode3> {
        match self {
            Self::Resolved(code) => Some(code),
            Self::Auto | Self::PerFile | Self::Unspecified => None,
        }
    }

    /// Return `true` when the worker should auto-detect the language.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Return `true` when the worker has no job-level language and
    /// should resolve per-file.
    pub fn is_per_file(&self) -> bool {
        matches!(self, Self::PerFile)
    }

    /// Return `true` when the worker should receive no language hint.
    pub fn is_unspecified(&self) -> bool {
        matches!(self, Self::Unspecified)
    }
}

impl std::fmt::Display for WorkerLanguage {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_worker_arg())
    }
}

impl TryFrom<String> for WorkerLanguage {
    type Error = InvalidWorkerLanguage;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse_untrusted(&value)
    }
}

impl TryFrom<&str> for WorkerLanguage {
    type Error = InvalidWorkerLanguage;
    fn try_from(value: &str) -> Result<Self, Self::Error> {
        Self::parse_untrusted(value)
    }
}

impl From<LanguageCode3> for WorkerLanguage {
    fn from(code: LanguageCode3) -> Self {
        Self::Resolved(code)
    }
}

impl From<&LanguageCode3> for WorkerLanguage {
    fn from(code: &LanguageCode3) -> Self {
        Self::Resolved(code.clone())
    }
}

impl From<&WorkerLanguage> for WorkerLanguage {
    fn from(value: &WorkerLanguage) -> Self {
        value.clone()
    }
}

impl serde::Serialize for WorkerLanguage {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(self.as_worker_arg())
    }
}

impl<'de> serde::Deserialize<'de> for WorkerLanguage {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Self::parse_untrusted(&s).map_err(serde::de::Error::custom)
    }
}

impl schemars::JsonSchema for WorkerLanguage {
    fn schema_name() -> Cow<'static, str> {
        "WorkerLanguage".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({
            "type": "string",
            "description": "Worker-runtime language string: ISO 639-3 code, \"auto\", or empty string."
        })
    }
}

// ---------------------------------------------------------------------------
// LanguageSpec — Auto vs Resolved(LanguageCode3)
// ---------------------------------------------------------------------------

/// Language specification from the CLI or job submission.
///
/// `Auto` means the ASR engine should detect the language. This variant must
/// be resolved to a concrete [`LanguageCode3`] before any CHAT construction
/// or NLP dispatch that requires a known language.
///
/// `PerFile` means the command has no job-level language at all: each input
/// file's processing language is read from its `@Languages:` header at the
/// start of the per-file pipeline. This is distinct from `Auto`: `Auto` is an
/// ASR-engine signal asking the model to detect the spoken language;
/// `PerFile` is a routing signal for text-NLP commands (morphotag, translate,
/// coref) whose language source is the CHAT file itself, not the job
/// submission. The 2026-05-03 morphotag incident happened because these
/// commands were forced to carry a placeholder `Resolved(eng)` value that
/// then leaked into the job record, the dashboard, and the Stanza
/// pre-warming key. `PerFile` makes the absence of a job-level language a
/// first-class state in the type system.
#[derive(Debug, Clone, PartialEq, Eq, Hash, ToSchema)]
pub enum LanguageSpec {
    /// Let the ASR engine auto-detect the language.
    Auto,
    /// A concrete ISO 639-3 language code.
    Resolved(LanguageCode3),
    /// No job-level language; resolve per-file from each CHAT file's
    /// `@Languages:` header. Used by morphotag, translate, and coref —
    /// none of which take a `--lang` CLI flag.
    PerFile,
}

impl LanguageSpec {
    /// Return the resolved language code, or `None` if `Auto` or `PerFile`.
    ///
    /// Both `Auto` and `PerFile` represent "no job-level resolved language"
    /// from the inference-dispatch perspective, but they reach that state
    /// for different reasons. Callers that need to distinguish them must
    /// match on the variant directly.
    pub fn as_resolved(&self) -> Option<&LanguageCode3> {
        match self {
            Self::Auto | Self::PerFile => None,
            Self::Resolved(code) => Some(code),
        }
    }

    /// Return the resolved language code, falling back to `fallback` if
    /// `Auto` or `PerFile`.
    pub fn resolve_or(&self, fallback: &LanguageCode3) -> LanguageCode3 {
        match self {
            Self::Auto | Self::PerFile => fallback.clone(),
            Self::Resolved(code) => code.clone(),
        }
    }

    /// Return `true` if this is `Auto`.
    pub fn is_auto(&self) -> bool {
        matches!(self, Self::Auto)
    }

    /// Return `true` if this is `PerFile`.
    pub fn is_per_file(&self) -> bool {
        matches!(self, Self::PerFile)
    }

    /// Convert this submission/runtime language into the worker-runtime
    /// language domain.
    ///
    /// Each `LanguageSpec` variant maps to its `WorkerLanguage`
    /// counterpart. `PerFile` does **not** collapse into `Auto`: those are
    /// semantically different states (Auto = "ASR detect a single
    /// language for this whole job"; PerFile = "no job-level language at
    /// all, dispatch per-file") and the wire format must distinguish
    /// them. Otherwise the Python worker — which parses `--lang` as a
    /// plain string — would receive `"auto"` for both cases and try to
    /// load Stanza models for the literal string `"auto"`, crashing
    /// before ready.
    pub fn to_worker_language(&self) -> WorkerLanguage {
        match self {
            Self::Auto => WorkerLanguage::Auto,
            Self::Resolved(code) => WorkerLanguage::Resolved(code.clone()),
            Self::PerFile => WorkerLanguage::PerFile,
        }
    }

    /// Parse from a DB string column. `"auto"` → `Auto`, `"per-file"`
    /// → `PerFile`, anything else → `Resolved`.
    ///
    /// Returns `(spec, true)` if the value was valid, `(spec, false)` if
    /// the stored value was invalid and fell back to `eng`. Callers should
    /// log the fallback so corrupt DB values are visible.
    pub fn parse_from_db(s: &str) -> (Self, bool) {
        if s.eq_ignore_ascii_case("auto") {
            (Self::Auto, true)
        } else if s.eq_ignore_ascii_case("per-file") {
            (Self::PerFile, true)
        } else {
            match LanguageCode3::try_new(s) {
                Ok(code) => (Self::Resolved(code), true),
                Err(_) => (Self::Resolved(LanguageCode3::eng()), false),
            }
        }
    }
}

impl std::fmt::Display for LanguageSpec {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Auto => write!(f, "auto"),
            Self::Resolved(code) => write!(f, "{code}"),
            Self::PerFile => write!(f, "per-file"),
        }
    }
}

impl From<LanguageCode3> for LanguageSpec {
    fn from(code: LanguageCode3) -> Self {
        Self::Resolved(code)
    }
}

impl TryFrom<&str> for LanguageSpec {
    type Error = InvalidLanguageCode;
    fn try_from(s: &str) -> Result<Self, Self::Error> {
        if s.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else if s.eq_ignore_ascii_case("per-file") {
            Ok(Self::PerFile)
        } else {
            LanguageCode3::try_new(s).map(Self::Resolved)
        }
    }
}

impl Serialize for LanguageSpec {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        match self {
            Self::Auto => serializer.serialize_str("auto"),
            Self::Resolved(code) => serializer.serialize_str(&code.0),
            Self::PerFile => serializer.serialize_str("per-file"),
        }
    }
}

impl<'de> Deserialize<'de> for LanguageSpec {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.eq_ignore_ascii_case("auto") {
            Ok(Self::Auto)
        } else if s.eq_ignore_ascii_case("per-file") {
            Ok(Self::PerFile)
        } else {
            LanguageCode3::try_new(&s)
                .map(Self::Resolved)
                .map_err(serde::de::Error::custom)
        }
    }
}

impl schemars::JsonSchema for LanguageSpec {
    fn schema_name() -> Cow<'static, str> {
        "LanguageSpec".into()
    }

    fn json_schema(g: &mut schemars::SchemaGenerator) -> schemars::Schema {
        // Reuse the string schema — "auto" or a 3-letter code.
        <String as schemars::JsonSchema>::json_schema(g)
    }
}

// ---------------------------------------------------------------------------
// DisplayPath — display-oriented file path within a job
// ---------------------------------------------------------------------------

/// Display path for a file within a job: either a bare basename
/// (`"sample.cha"`) for single-file input or a relative forward-slash path
/// (`"PWA/TYO_a1.cha"`) for directory input with subdirectories.
///
/// Backslashes are normalized to forward slashes on construction so the value
/// is platform-independent regardless of whether the CLI ran on Windows.
///
/// This type replaces the former `FileName` which incorrectly rejected path
/// separators during deserialization even though the system routinely carries
/// relative paths.
#[derive(
    Debug, Clone, PartialEq, Eq, Hash, serde::Serialize, utoipa::ToSchema, schemars::JsonSchema,
)]
#[serde(transparent)]
pub struct DisplayPath(pub String);

/// Error returned when a display path is empty.
#[derive(Debug, Clone, thiserror::Error)]
#[error("empty display path: \"{0}\"")]
pub struct InvalidDisplayPath(pub String);

impl DisplayPath {
    /// Try to create a validated display path.
    ///
    /// Validation: non-empty after trimming. Backslashes are normalized to
    /// forward slashes for cross-platform safety.
    pub fn try_new(s: &str) -> Result<Self, InvalidDisplayPath> {
        if s.is_empty() {
            return Err(InvalidDisplayPath(s.to_owned()));
        }
        Ok(Self(normalize_backslashes(s)))
    }
}

/// Normalize Windows-style backslashes to forward slashes.
fn normalize_backslashes(s: &str) -> String {
    if s.contains('\\') {
        s.replace('\\', "/")
    } else {
        s.to_owned()
    }
}

impl std::fmt::Display for DisplayPath {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for DisplayPath {
    fn from(s: String) -> Self {
        if s.contains('\\') {
            Self(s.replace('\\', "/"))
        } else {
            Self(s)
        }
    }
}

impl From<&str> for DisplayPath {
    fn from(s: &str) -> Self {
        Self(normalize_backslashes(s))
    }
}

impl From<DisplayPath> for String {
    fn from(v: DisplayPath) -> String {
        v.0
    }
}

impl std::ops::Deref for DisplayPath {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

impl AsRef<str> for DisplayPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl PartialEq<&str> for DisplayPath {
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl std::borrow::Borrow<str> for DisplayPath {
    fn borrow(&self) -> &str {
        &self.0
    }
}

impl<'de> serde::Deserialize<'de> for DisplayPath {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        if s.is_empty() {
            return Err(serde::de::Error::custom("DisplayPath must not be empty"));
        }
        Ok(Self(normalize_backslashes(&s)))
    }
}

string_id!(
    /// Identifier of a server/fleet node.
    /// Empty when the node does not report an identity (older server versions).
    pub NodeId
);

numeric_id!(
    /// Number of speakers in a recording.
    pub NumSpeakers(u32) [Eq]
);

numeric_id!(
    /// Duration measured in fractional seconds.
    pub DurationSeconds(f64)
);

numeric_id!(
    /// Unix timestamp as fractional seconds since epoch.
    pub UnixTimestamp(f64)
);

numeric_id!(
    /// Duration or audio position measured in milliseconds.
    ///
    /// Used for audio timestamps (`start_ms`, `end_ms`) and durations
    /// (`max_group_ms`, `tight_buffer_ms`) throughout the FA, ASR, and
    /// speaker pipelines. All ML worker IPC timing fields use this type.
    pub DurationMs(u64) [Eq]
);

numeric_id!(
    /// Physical memory quantity in megabytes.
    ///
    /// Used for memory gate thresholds and health-response memory readings.
    pub MemoryMb(u64) [Eq]
);

validated_string_id!(
    /// ML engine version string for cache keying (e.g. `"stanza-1.9.2"`, non-empty).
    pub EngineVersion
);

validated_string_id!(
    /// Correlation ID for tracing a job across log entries (non-empty).
    ///
    /// Usually the same as `JobId` but may differ for retried or cloned jobs.
    pub CorrelationId
);

numeric_id!(
    /// Number of parallel file-processing workers for a job.
    ///
    /// Computed by `compute_job_workers()` based on available memory and CPU.
    /// Used in dispatch runtime structs to bound concurrency via a semaphore.
    pub NumWorkers(usize) [Eq]
);

validated_string_id!(
    /// A Rev.AI server-side job identifier returned after audio submission (non-empty).
    ///
    /// Obtained during preflight batch upload and passed to polling calls so
    /// individual file tasks can retrieve results without re-uploading audio.
    pub RevAiJobId
);

validated_string_id!(
    /// Name of a Temporal task queue used by a batchalign3 server.
    ///
    /// **Architectural invariant:** this value must be unique per fleet
    /// machine. Each batchalign3 server owns a local SQLite `JobStore`, so a
    /// workflow's activities can only be executed by the server whose store
    /// persisted the job. Sharing a task queue across servers causes silent
    /// no-op completions when a non-submitter worker wins the poll race (see
    /// 2026-04-15 postmortem).
    ///
    /// The built-in default is derived from the system hostname
    /// (`batchalign3-{hostname}`); operators may override in `server.yaml`
    /// but must ensure the override is unique per host.
    pub TemporalTaskQueue
);

/// Engine category that supports backend overrides.
///
/// Currently only ASR and FA have multiple engine backends.
/// Other inference tasks (morphosyntax, utseg, translate, coref)
/// always use their single built-in engine.
/// MIME-like content discriminator for file results.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum ContentType {
    /// CHAT format output.
    #[default]
    Chat,
    /// Tabular CSV output (e.g. opensmile features).
    Csv,
    /// Plain text output (e.g. AVQI voice quality reports).
    Text,
}

impl std::fmt::Display for ContentType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Chat => write!(f, "chat"),
            Self::Csv => write!(f, "csv"),
            Self::Text => write!(f, "text"),
        }
    }
}

// ---------------------------------------------------------------------------
// Cancellation provenance
// ---------------------------------------------------------------------------

/// Where a job-cancellation request originated. No `Default` impl —
/// every caller must explicitly state what kind of actor it is, so a
/// future "anonymous cancel" path cannot silently slip through as
/// `Api`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum CancelSource {
    /// Interactive TUI cancel (user pressed `c` then `y`).
    Tui,
    /// Non-TUI CLI cancel (e.g., a `--cancel` flag invocation).
    Cli,
    /// Web dashboard cancel button.
    Dashboard,
    /// Staging orchestrator forwarded a cancel.
    Staging,
    /// Direct REST API cancel with no caller hint (raw curl, scripts).
    Api,
    /// SIGTERM-driven graceful shutdown cancelled in-flight work.
    Signal,
}

/// Returned when a string cannot be parsed into a `CancelSource`.
#[derive(Debug, Clone, thiserror::Error)]
#[error("invalid cancel source \"{0}\": expected one of tui, cli, dashboard, staging, api, signal")]
pub struct InvalidCancelSource(pub String);

impl std::fmt::Display for CancelSource {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Tui => write!(f, "tui"),
            Self::Cli => write!(f, "cli"),
            Self::Dashboard => write!(f, "dashboard"),
            Self::Staging => write!(f, "staging"),
            Self::Api => write!(f, "api"),
            Self::Signal => write!(f, "signal"),
        }
    }
}

impl std::str::FromStr for CancelSource {
    type Err = InvalidCancelSource;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "tui" => Ok(Self::Tui),
            "cli" => Ok(Self::Cli),
            "dashboard" => Ok(Self::Dashboard),
            "staging" => Ok(Self::Staging),
            "api" => Ok(Self::Api),
            "signal" => Ok(Self::Signal),
            other => Err(InvalidCancelSource(other.to_string())),
        }
    }
}

string_id!(
    /// Hostname or remote IP of the caller who issued a cancel.
    ///
    /// Empty when the source did not report identity (older clients,
    /// localhost defaults). Persisted in `cancellations.host` and
    /// projected onto `jobs.last_cancelled_host`.
    pub CallerHost
);

string_id!(
    /// Free-form reason text attached to a cancel request.
    ///
    /// Examples: `"user-pressed-cancel"`, `"ctrl-c-shutdown"`,
    /// `"too-slow-aborting"`. Empty is allowed.
    pub CancelReason
);

impl CancelReason {
    /// The reason recorded when a `cancel_all` runs as part of graceful
    /// server shutdown (i.e. not a user gesture). Matched by the Temporal
    /// reconciler to distinguish stale system-initiated cancels from real
    /// user cancels on restart.
    pub fn server_cancel_all() -> Self {
        Self::from("server-cancel-all")
    }

    /// The reason recorded inside a Temporal activity when its context
    /// reports cancellation — typically because Temporal's workflow-cancel
    /// signal was forwarded into a running activity at shutdown. Treated
    /// the same as `server_cancel_all` by the reconciler.
    pub fn temporal_activity_forwarded() -> Self {
        Self::from("temporal-activity-forwarded")
    }
}

numeric_id!(
    /// Process identifier of the caller that issued a cancel.
    ///
    /// Unix PIDs fit in u32 on every platform we support. Persisted in
    /// `cancellations.pid` for forensics across multi-machine setups
    /// (helps distinguish an operator-laptop cancel from a fleet-internal one).
    pub CallerPid(u32) [Eq]
);

/// Server health status.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "lowercase")]
pub enum HealthStatus {
    /// Server is accepting work.
    #[default]
    Ok,
}

impl std::fmt::Display for HealthStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ok => write!(f, "ok"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- LanguageCode3 validation ----

    #[test]
    fn language_code3_valid() {
        assert_eq!(LanguageCode3::try_new("eng").unwrap().0, "eng");
        assert_eq!(LanguageCode3::try_new("SPA").unwrap().0, "spa");
        assert_eq!(LanguageCode3::try_new("Zho").unwrap().0, "zho");
    }

    #[test]
    fn language_code3_rejects_auto() {
        assert!(LanguageCode3::try_new("auto").is_err());
    }

    #[test]
    fn language_code3_rejects_empty() {
        assert!(LanguageCode3::try_new("").is_err());
    }

    #[test]
    fn language_code3_rejects_two_letter() {
        assert!(LanguageCode3::try_new("en").is_err());
    }

    #[test]
    fn language_code3_rejects_four_letter() {
        assert!(LanguageCode3::try_new("engl").is_err());
    }

    #[test]
    fn language_code3_rejects_digits() {
        assert!(LanguageCode3::try_new("e1g").is_err());
    }

    #[test]
    fn language_code3_serde_roundtrip() {
        let code = LanguageCode3::eng();
        let json = serde_json::to_string(&code).unwrap();
        assert_eq!(json, "\"eng\"");
        let back: LanguageCode3 = serde_json::from_str(&json).unwrap();
        assert_eq!(back, code);
    }

    #[test]
    fn language_code3_deserialize_rejects_auto() {
        let result: Result<LanguageCode3, _> = serde_json::from_str("\"auto\"");
        assert!(result.is_err());
    }

    #[test]
    fn language_code3_try_from_str_rejects_auto() {
        assert!(LanguageCode3::try_from("auto").is_err());
    }

    #[test]
    fn language_code3_try_from_string_rejects_auto() {
        assert!(LanguageCode3::try_from("auto".to_string()).is_err());
    }

    // ---- WorkerLanguage ----

    #[test]
    fn worker_language_parses_resolved_auto_and_unspecified() {
        assert_eq!(
            WorkerLanguage::parse_untrusted("eng").unwrap(),
            WorkerLanguage::Resolved(LanguageCode3::eng())
        );
        assert_eq!(
            WorkerLanguage::parse_untrusted("AUTO").unwrap(),
            WorkerLanguage::Auto
        );
        assert_eq!(
            WorkerLanguage::parse_untrusted("").unwrap(),
            WorkerLanguage::Unspecified
        );
    }

    #[test]
    fn worker_language_rejects_invalid_values() {
        assert!(WorkerLanguage::parse_untrusted("english").is_err());
        assert!(WorkerLanguage::parse_untrusted("12").is_err());
    }

    #[test]
    fn worker_language_serde_roundtrip() {
        let auto = WorkerLanguage::Auto;
        assert_eq!(serde_json::to_string(&auto).unwrap(), "\"auto\"");
        assert_eq!(
            serde_json::from_str::<WorkerLanguage>("\"\"").unwrap(),
            WorkerLanguage::Unspecified
        );
        assert_eq!(
            serde_json::from_str::<WorkerLanguage>("\"yue\"").unwrap(),
            WorkerLanguage::Resolved(LanguageCode3::yue())
        );
    }

    // ---- LanguageSpec ----

    #[test]
    fn language_spec_deserializes_auto() {
        let spec: LanguageSpec = serde_json::from_str("\"auto\"").unwrap();
        assert_eq!(spec, LanguageSpec::Auto);
    }

    #[test]
    fn language_spec_deserializes_auto_case_insensitive() {
        let spec: LanguageSpec = serde_json::from_str("\"AUTO\"").unwrap();
        assert_eq!(spec, LanguageSpec::Auto);
    }

    #[test]
    fn language_spec_deserializes_resolved() {
        let spec: LanguageSpec = serde_json::from_str("\"eng\"").unwrap();
        assert_eq!(spec, LanguageSpec::Resolved(LanguageCode3::eng()));
    }

    #[test]
    fn language_spec_serializes_auto() {
        let json = serde_json::to_string(&LanguageSpec::Auto).unwrap();
        assert_eq!(json, "\"auto\"");
    }

    #[test]
    fn language_spec_serializes_resolved() {
        let json = serde_json::to_string(&LanguageSpec::Resolved(LanguageCode3::spa())).unwrap();
        assert_eq!(json, "\"spa\"");
    }

    #[test]
    fn language_spec_roundtrip_auto() {
        let spec = LanguageSpec::Auto;
        let json = serde_json::to_string(&spec).unwrap();
        let back: LanguageSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn language_spec_roundtrip_resolved() {
        let spec = LanguageSpec::Resolved(LanguageCode3::fra());
        let json = serde_json::to_string(&spec).unwrap();
        let back: LanguageSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(spec, back);
    }

    #[test]
    fn language_spec_rejects_invalid_code() {
        let result: Result<LanguageSpec, _> = serde_json::from_str("\"xx\"");
        assert!(result.is_err());
    }

    #[test]
    fn language_spec_try_from_str_auto() {
        assert_eq!(LanguageSpec::try_from("auto").unwrap(), LanguageSpec::Auto);
    }

    #[test]
    fn language_spec_try_from_str_resolved() {
        assert_eq!(
            LanguageSpec::try_from("eng").unwrap(),
            LanguageSpec::Resolved(LanguageCode3::eng())
        );
    }

    #[test]
    fn language_spec_resolve_or_returns_resolved() {
        let spec = LanguageSpec::Resolved(LanguageCode3::spa());
        let fallback = LanguageCode3::eng();
        assert_eq!(spec.resolve_or(&fallback), LanguageCode3::spa());
    }

    #[test]
    fn language_spec_resolve_or_returns_fallback_for_auto() {
        let spec = LanguageSpec::Auto;
        let fallback = LanguageCode3::eng();
        assert_eq!(spec.resolve_or(&fallback), LanguageCode3::eng());
    }

    #[test]
    fn language_spec_display() {
        assert_eq!(LanguageSpec::Auto.to_string(), "auto");
        assert_eq!(
            LanguageSpec::Resolved(LanguageCode3::eng()).to_string(),
            "eng"
        );
    }

    #[test]
    fn language_spec_parse_from_db_valid() {
        let (spec, valid) = LanguageSpec::parse_from_db("auto");
        assert_eq!(spec, LanguageSpec::Auto);
        assert!(valid);

        let (spec, valid) = LanguageSpec::parse_from_db("eng");
        assert_eq!(spec, LanguageSpec::Resolved(LanguageCode3::eng()));
        assert!(valid);
    }

    #[test]
    fn language_spec_parse_from_db_invalid_falls_back() {
        let (spec, valid) = LanguageSpec::parse_from_db("not-a-lang");
        assert_eq!(spec, LanguageSpec::Resolved(LanguageCode3::eng()));
        assert!(!valid, "invalid DB value should report fallback");
    }

    #[test]
    fn language_spec_maps_to_worker_language() {
        assert_eq!(
            LanguageSpec::Auto.to_worker_language(),
            WorkerLanguage::Auto
        );
        assert_eq!(
            LanguageSpec::Resolved(LanguageCode3::eng()).to_worker_language(),
            WorkerLanguage::Resolved(LanguageCode3::eng())
        );
        assert_eq!(
            LanguageSpec::PerFile.to_worker_language(),
            WorkerLanguage::PerFile,
            "PerFile must NOT collapse into Auto — those are semantically \
             distinct states and the wire format must distinguish them",
        );
    }

    // ---- LanguageSpec::PerFile ----
    //
    // `PerFile` exists for commands whose processing language is not a
    // job-level concept but is resolved per-file from each CHAT file's
    // `@Languages:` header (morphotag, translate, coref). It is NOT the same
    // as `Auto` (which is an ASR-engine signal: "let the model detect the
    // spoken language"). The two must serialize differently so the wire
    // format and the dashboard distinguish "no job-level lang" from "ASR
    // auto-detect".

    #[test]
    fn language_spec_per_file_displays_as_per_file() {
        assert_eq!(LanguageSpec::PerFile.to_string(), "per-file");
    }

    #[test]
    fn language_spec_per_file_serializes_as_per_file_string() {
        let json = serde_json::to_string(&LanguageSpec::PerFile).unwrap();
        assert_eq!(json, "\"per-file\"");
    }

    #[test]
    fn language_spec_per_file_round_trips_json() {
        let spec = LanguageSpec::PerFile;
        let json = serde_json::to_string(&spec).unwrap();
        let back: LanguageSpec = serde_json::from_str(&json).unwrap();
        assert_eq!(back, spec);
    }

    #[test]
    fn language_spec_parse_from_db_per_file() {
        let (spec, valid) = LanguageSpec::parse_from_db("per-file");
        assert_eq!(spec, LanguageSpec::PerFile);
        assert!(valid);
    }

    #[test]
    fn language_spec_per_file_try_from_str() {
        assert_eq!(
            LanguageSpec::try_from("per-file").unwrap(),
            LanguageSpec::PerFile
        );
    }

    #[test]
    fn language_spec_per_file_distinct_from_auto() {
        // ASR auto-detect ≠ per-file lang resolution. Code that branches on
        // these states must not collapse them into `Option<LanguageCode3>`.
        assert_ne!(LanguageSpec::PerFile, LanguageSpec::Auto);
    }

    #[test]
    fn language_spec_per_file_as_resolved_is_none() {
        assert_eq!(LanguageSpec::PerFile.as_resolved(), None);
    }

    #[test]
    fn language_spec_per_file_is_not_auto() {
        assert!(!LanguageSpec::PerFile.is_auto());
    }

    // ---- DisplayPath ----

    #[test]
    fn display_path_accepts_bare_basename() {
        let p = DisplayPath::try_new("sample.cha").unwrap();
        assert_eq!(&*p, "sample.cha");
    }

    #[test]
    fn display_path_accepts_relative_path() {
        let p = DisplayPath::try_new("PWA/TYO_a1.cha").unwrap();
        assert_eq!(&*p, "PWA/TYO_a1.cha");
    }

    #[test]
    fn display_path_rejects_empty() {
        assert!(DisplayPath::try_new("").is_err());
    }

    #[test]
    fn display_path_normalizes_backslash_in_from() {
        let p = DisplayPath::from("PWA\\TYO.cha");
        assert_eq!(&*p, "PWA/TYO.cha");
    }

    #[test]
    fn display_path_serde_roundtrip() {
        let p = DisplayPath::try_new("sub/dir/file.cha").unwrap();
        let json = serde_json::to_string(&p).unwrap();
        let back: DisplayPath = serde_json::from_str(&json).unwrap();
        assert_eq!(p, back);
    }

    #[test]
    fn display_path_deserialize_rejects_empty() {
        let result: Result<DisplayPath, _> = serde_json::from_str("\"\"");
        assert!(result.is_err());
    }

    #[test]
    fn display_path_deserialize_normalizes_backslash() {
        let p: DisplayPath = serde_json::from_str("\"PWA\\\\TYO.cha\"").unwrap();
        assert_eq!(&*p, "PWA/TYO.cha");
    }
}
