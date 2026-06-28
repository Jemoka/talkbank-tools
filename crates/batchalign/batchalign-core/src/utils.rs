//! Cross-cutting utility types for batchalign-core: errors, the validated
//! `SourceId` newtype, media-input shapes, and PCM preparation.
//!
//! Consolidated from the original `error.rs`, `media.rs`, `audio_prep.rs`,
//! and the `SourceId` portion of `value.rs` per the spec2.md reorg.

use anyhow::{Context, Result, anyhow};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use std::fmt;
use std::path::PathBuf;
use symphonia::core::audio::AudioBuffer;
use symphonia::core::codecs::DecoderOptions;
use symphonia::core::errors::Error as SymphoniaError;
use symphonia::core::formats::FormatOptions;
use symphonia::core::io::MediaSourceStream;
use symphonia::core::meta::MetadataOptions;
use symphonia::core::probe::Hint;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Audio-prep failure surface. Concrete decoder errors are wrapped as strings
/// so this type can live in core without pulling symphonia into the public
/// boundary.
#[derive(Debug, Error)]
pub enum AudioError {
    /// The input file path could not be opened or read.
    #[error("audio I/O at {path:?}: {source}")]
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// Decoder rejected the file (unknown codec, malformed container, etc.).
    #[error("audio decode failed: {0}")]
    Decode(String),
    /// Resampling / channel layout transformation failed.
    #[error("audio resample failed: {0}")]
    Resample(String),
}

/// Errors that originate inside batchalign-core or its direct callers.
#[derive(Debug, Error)]
pub enum BAError {
    /// CHAT parser failure — wraps talkbank-parser diagnostics as a string.
    #[error("CHAT parse error: {0}")]
    Parse(String),

    /// Validation failure reported by `talkbank-model::Validate`.
    #[error("CHAT validation failed: {0}")]
    Validation(String),

    /// CHAT serialization failure.
    #[error("CHAT serialize error: {0}")]
    Serialize(String),

    /// JSON serialization / deserialization failure crossing the cache or FFI.
    #[error("json error: {0}")]
    Json(String),

    /// Worker backend or dispatch path refused / errored.
    #[error("worker dispatch failed: {0}")]
    Worker(String),

    /// Audio decoding, resampling, or probing failed.
    #[error("audio: {0}")]
    Audio(#[source] AudioError),

    /// The requested language/engine combination is not supported.
    #[error("unsupported language/engine combo: {lang} on {engine}")]
    Capability {
        /// Resolved language code.
        lang: SmolStr,
        /// Backend name that was tried.
        engine: SmolStr,
    },

    /// User-initiated cancellation propagated through the poison-pill chain.
    #[error("operation cancelled")]
    Cancelled,

    /// Catch-all for invariants broken inside the kernel.
    #[error("internal: {0}")]
    Internal(String),

    /// I/O error reading or writing CHAT files, metrics, or media.
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
}

/// Convenience alias.
pub type BAResult<T> = std::result::Result<T, BAError>;

impl From<serde_json::Error> for BAError {
    fn from(e: serde_json::Error) -> Self {
        BAError::Json(e.to_string())
    }
}

impl From<anyhow::Error> for BAError {
    fn from(e: anyhow::Error) -> Self {
        match e.downcast::<BAError>() {
            Ok(ba) => ba,
            Err(other) => BAError::Internal(format!("{other:#}")),
        }
    }
}

// ---------------------------------------------------------------------------
// SourceId
// ---------------------------------------------------------------------------

/// Validated non-empty identifier for a media file / CHAT input / metrics row.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(eq, hash, frozen))]
pub struct SourceId(SmolStr);

impl SourceId {
    /// Construct a `SourceId`, rejecting empty / whitespace-only strings.
    pub fn try_new<S: AsRef<str>>(s: S) -> BAResult<Self> {
        let s = s.as_ref().trim();
        if s.is_empty() {
            return Err(BAError::Internal("source_id must be non-empty".into()));
        }
        Ok(Self(SmolStr::new(s)))
    }

    /// Construct without validation (escape hatch for the Python boundary).
    pub fn new_unchecked<S: Into<SmolStr>>(s: S) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0.as_str())
    }
}

impl AsRef<str> for SourceId {
    fn as_ref(&self) -> &str {
        self.0.as_str()
    }
}

// ---------------------------------------------------------------------------
// Media inputs
// ---------------------------------------------------------------------------

/// A reference to media-on-disk that the pipeline starts from.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct MediaInput {
    /// The pipeline-wide identifier for this media file.
    pub source_id: SourceId,
    /// Path to the audio file. Decoded lazily by the engine.
    pub path: PathBuf,
    /// Optional ISO-639-3 language hint for source-creating ASR pipelines.
    #[serde(default)]
    pub language: Option<String>,
}

impl MediaInput {
    /// Construct a `MediaInput` from a path and an explicit `SourceId`.
    pub fn new(source_id: SourceId, path: PathBuf) -> Self {
        Self {
            source_id,
            path,
            language: None,
        }
    }

    /// Attach an ISO-639-3 language hint for ASR transcript construction.
    pub fn with_language(mut self, language: impl Into<String>) -> Self {
        self.language = Some(language.into());
        self
    }
}

/// A reference to an existing CHAT file on disk. Compare-, FA-, morphotag-,
/// translate-style pipelines start here instead of from `MediaInput`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct ChatInput {
    /// Pipeline identifier for this transcript.
    pub source_id: SourceId,
    /// Path to the `.cha` file.
    pub path: PathBuf,
}

impl ChatInput {
    pub fn new(source_id: SourceId, path: PathBuf) -> Self {
        Self { source_id, path }
    }
}

/// A CHAT file plus per-run instruction for `Task::Ai`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct AiChatInput {
    /// Pipeline identifier for this transcript.
    pub source_id: SourceId,
    /// Path to the `.cha` file.
    pub path: PathBuf,
    /// Instruction sent to the AI backend.
    pub instruction: String,
}

impl AiChatInput {
    pub fn new(source_id: SourceId, path: PathBuf, instruction: String) -> Self {
        Self {
            source_id,
            path,
            instruction,
        }
    }
}

/// A pair `(main, gold)` of CHAT files. Compare pipelines start here.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all, set_all))]
pub struct PairedInput {
    /// Pipeline identifier (typically the main file's stem).
    pub source_id: SourceId,
    /// Path to the candidate (main) transcript.
    pub main: PathBuf,
    /// Path to the gold reference transcript.
    pub gold: PathBuf,
}

impl PairedInput {
    pub fn new(source_id: SourceId, main: PathBuf, gold: PathBuf) -> Self {
        Self {
            source_id,
            main,
            gold,
        }
    }
}

/// Decoded PCM ready to ship to a backend.
///
/// `pcm_f32le` rides the JSON wire as a base64 string (`format: "byte"` in
/// JSON Schema) so it lands on the Python side as `bytes` and stays
/// `np.frombuffer`-compatible. The byte-array fallback that serde_json picks
/// for `Vec<u8>` would arrive as `list[int]` — broken for every backend that
/// does `np.frombuffer(audio.pcm_f32le, ...)`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct PreparedAudio {
    /// Interleaved little-endian f32 PCM samples.
    #[serde(with = "base64_pcm")]
    #[schemars(schema_with = "base64_pcm::json_schema")]
    pub pcm_f32le: Vec<u8>,
    /// Sampling rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u16,
    /// Number of frames (samples per channel).
    pub frame_count: u64,
}

crate::register_proto_schema!(PreparedAudio);

/// Serde + schemars adapter that ships `Vec<u8>` as a base64-encoded string.
///
/// Without this, `serde_json` defaults to a JSON array of integers — which
/// schemars then describes as `array<u8>` and which pydantic decodes as
/// `list[int]`, breaking every Python backend that calls
/// `np.frombuffer(audio.pcm_f32le, ...)`. The `format: "byte"` annotation
/// tells `datamodel-code-generator` to emit a `bytes` field that auto-decodes
/// the base64 payload at validation time.
mod base64_pcm {
    use base64::Engine as _;
    use base64::engine::general_purpose::STANDARD;
    use schemars::{Schema, SchemaGenerator, json_schema};
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(bytes: &[u8], s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&STANDARD.encode(bytes))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<Vec<u8>, D::Error> {
        let s = String::deserialize(d)?;
        STANDARD
            .decode(s.as_bytes())
            .map_err(serde::de::Error::custom)
    }

    pub fn json_schema(_g: &mut SchemaGenerator) -> Schema {
        json_schema!({
            "type": "string",
            "format": "byte",
            "contentEncoding": "base64",
            "description": "Interleaved little-endian f32 PCM samples, base64-encoded."
        })
    }
}

/// A speaker label as it travels through ASR / Speaker outputs.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
pub struct SpeakerLabel(pub SmolStr);

impl SpeakerLabel {
    /// Construct from any `Into<SmolStr>`.
    pub fn new<S: Into<SmolStr>>(s: S) -> Self {
        Self(s.into())
    }

    /// Borrow the underlying label.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl From<&str> for SpeakerLabel {
    fn from(s: &str) -> Self {
        Self(SmolStr::new(s))
    }
}

// ---------------------------------------------------------------------------
// Audio preparation (symphonia-backed PCM decode)
// ---------------------------------------------------------------------------

/// Decode the file at `input.path` into little-endian f32 PCM bytes.
pub fn prepare_pcm(input: &MediaInput) -> Result<PreparedAudio> {
    let file = std::fs::File::open(&input.path)
        .with_context(|| format!("audio_prep: open {}", input.path.display()))?;
    let mss = MediaSourceStream::new(Box::new(file), Default::default());

    let mut hint = Hint::new();
    if let Some(ext) = input.path.extension().and_then(|s| s.to_str()) {
        hint.with_extension(ext);
    }

    let probed = symphonia::default::get_probe()
        .format(
            &hint,
            mss,
            &FormatOptions::default(),
            &MetadataOptions::default(),
        )
        .with_context(|| format!("audio_prep: probe {}", input.path.display()))?;

    let mut format = probed.format;
    let track = format.default_track().ok_or_else(|| {
        anyhow!(
            "audio_prep: no default audio track in {}",
            input.path.display()
        )
    })?;
    let track_id = track.id;

    let mut decoder = symphonia::default::get_codecs()
        .make(&track.codec_params, &DecoderOptions::default())
        .with_context(|| "audio_prep: codec init")?;

    let sample_rate = track.codec_params.sample_rate.unwrap_or(16_000);
    // Source channel count is informational only — we downmix to mono below
    // (BA2 parity: `torch.mean(audio.transpose(0,1), dim=1)` in
    // `batchalign2/batchalign/models/wave2vec/infer_fa.py`).

    let mut pcm_f32: Vec<f32> = Vec::new();
    loop {
        let packet = match format.next_packet() {
            Ok(p) => p,
            Err(SymphoniaError::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => {
                break;
            }
            Err(SymphoniaError::ResetRequired) => {
                let track = format
                    .tracks()
                    .iter()
                    .find(|t| t.id == track_id)
                    .ok_or_else(|| anyhow!("audio_prep: track vanished after reset"))?;
                decoder = symphonia::default::get_codecs()
                    .make(&track.codec_params, &DecoderOptions::default())?;
                continue;
            }
            Err(e) => return Err(e).context("audio_prep: read packet"),
        };
        if packet.track_id() != track_id {
            continue;
        }
        let decoded = decoder.decode(&packet).context("audio_prep: decode")?;
        let mut buf: AudioBuffer<f32> = decoded.make_equivalent();
        decoded.convert(&mut buf);
        // Downmix to mono: every consumer (FA/ASR backends) treats
        // `pcm_f32le` as a single-channel waveform indexed by
        // `(time_ms * sample_rate / 1000)`. Concatenating planar channels
        // (`[L_all, R_all]`) would double the effective length and shift
        // every utterance window. Average across planes per frame.
        let planes_holder = buf.planes();
        let planes = planes_holder.planes();
        if planes.is_empty() {
            continue;
        }
        let frames = planes[0].len();
        if planes.len() == 1 {
            pcm_f32.extend_from_slice(planes[0]);
        } else {
            let n = planes.len() as f32;
            for i in 0..frames {
                let mut acc = 0.0f32;
                for plane in planes {
                    acc += plane[i];
                }
                pcm_f32.push(acc / n);
            }
        }
    }

    let mut pcm_bytes = Vec::with_capacity(pcm_f32.len() * 4);
    for sample in &pcm_f32 {
        pcm_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    // pcm_f32 is now mono; downstream `channels` is the post-downmix value.
    let channels: u16 = 1;
    let frame_count = pcm_f32.len() as u64;

    Ok(PreparedAudio {
        pcm_f32le: pcm_bytes,
        sample_rate,
        channels,
        frame_count,
    })
}

// ---------------------------------------------------------------------------
// Provenance stamp (BA version + engine name on a `@Comment` header)
// ---------------------------------------------------------------------------

/// Prefix every provenance `@Comment` starts with. Used to detect and refresh
/// a prior stamp on retag so reruns don't accrete a comment per invocation.
pub const PROVENANCE_PREFIX: &str = "batchalign3 ";
/// Clear the `unlinked` status from any `@Media` header in the file.
///
/// CHAT requires an explicit linkage-status flag (`, unlinked` /
/// `, missing` / `, notrans`) on an `@Media` header when the file has no
/// timing evidence — see error E544 in `talkbank-model`'s validator. A
/// transcript begins life as `@Media: foo, audio, unlinked`; once a
/// pipeline stage (UTR, FA) actually injects bullets, the `unlinked`
/// marker is stale and would cause downstream tools to ignore the now-
/// recovered timing. Stage runners call this at the end of a successful
/// run so the output advertises its newly linked state.
///
/// Other status values (`missing`, `notrans`) are left alone — those
/// describe the audio file's availability, not the timing state.
pub fn clear_media_unlinked(lines: &mut [talkbank_model::Line]) {
    use talkbank_model::Line;
    use talkbank_model::model::{Header, MediaStatus};
    for line in lines.iter_mut() {
        if let Line::Header { header, .. } = line
            && let Header::Media(m) = header.as_mut()
            && m.status == Some(MediaStatus::Unlinked)
        {
            m.status = None;
        }
    }
}

/// Insert one `@Comment: batchalign3 <sha> | <stage>: <engine> | <ts>`
/// header per pipeline stage, just before the first utterance.
///
/// One stage → one comment line. A full UTR → FA run therefore produces:
///
/// ```text
/// @Comment:	batchalign3 abc1234 | utr: rev:rev_lang_en | 2026-06-02T17:04:12Z
/// @Comment:	batchalign3 abc1234 | fa: wav2vec2-fa:mms_fa-v3 | 2026-06-02T17:09:33Z
/// ```
///
/// One-line-per-stage was chosen over a single accumulated comment so the
/// audit trail stays readable as pipelines grow longer, and each stage
/// carries its own UTC timestamp (ISO-8601, second precision) — stages
/// may run on different days, or hours apart on the same day.
///
/// Re-running the *same* stage replaces that stage's comment in place
/// (matched by leading `<stage>: ` token). Other stages' comments are left
/// untouched. `engine = None` records `<stage>: <unknown>` so the stage is
/// still visible.
///
/// The git SHA is baked at compile time via `option_env!`. The Bazel path
/// goes through `BATCHALIGN_GIT_SHA`; cargo via `VERGEN_GIT_SHA`. Falls back
/// to `"unknown"` when neither is set (test runs, IDE checks).
pub fn stamp_provenance(
    lines: &mut Vec<talkbank_model::Line>,
    stage: &str,
    engine: Option<&str>,
) {
    use talkbank_model::Line;
    use talkbank_model::model::{BulletContent, Header};

    let sha = option_env!("VERGEN_GIT_SHA")
        .or(option_env!("BATCHALIGN_GIT_SHA"))
        .unwrap_or("unknown");
    let engine_label = engine.unwrap_or("<unknown>");
    let ts = current_utc_timestamp();
    let stamp = format!("{PROVENANCE_PREFIX}{sha} | {stage}: {engine_label} | {ts}");
    let stage_marker = format!("| {stage}: ");

    // If a prior comment for this exact stage exists, replace it in place.
    let mut replaced = false;
    for l in lines.iter_mut() {
        if let Some(Header::Comment { content }) = l.as_header() {
            let text = content.to_chat_string();
            if text.starts_with(PROVENANCE_PREFIX) && text.contains(&stage_marker) {
                *l = Line::header(Header::Comment {
                    content: BulletContent::from_text(stamp.clone()),
                });
                replaced = true;
                break;
            }
        }
    }
    if replaced {
        return;
    }

    let comment = Line::header(Header::Comment {
        content: BulletContent::from_text(stamp),
    });
    let insert_at = lines
        .iter()
        .position(Line::is_utterance)
        .unwrap_or_else(|| {
            // No utterances: place before `@End`, or append if no `@End`.
            lines
                .iter()
                .rposition(|l| matches!(l.as_header(), Some(Header::End)))
                .unwrap_or(lines.len())
        });
    lines.insert(insert_at, comment);
}

/// Current UTC timestamp as ISO-8601 `YYYY-MM-DDTHH:MM:SSZ`. Pure-stdlib
/// (no chrono dep) via Howard Hinnant's `civil_from_days` algorithm
/// (<http://howardhinnant.github.io/date_algorithms.html>) for the date
/// part and trivial modular arithmetic for the time-of-day part.
fn current_utc_timestamp() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0);
    let days = secs.div_euclid(86_400);
    let tod = secs.rem_euclid(86_400) as u32;
    let (y, mo, d) = civil_from_unix_days(days);
    let h = tod / 3600;
    let mi = (tod % 3600) / 60;
    let s = tod % 60;
    format!("{y:04}-{mo:02}-{d:02}T{h:02}:{mi:02}:{s:02}Z")
}

fn civil_from_unix_days(days: i64) -> (i32, u32, u32) {
    // Shift so day 0 is 0000-03-01 (Hinnant's "civil" epoch).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097) as u64; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365; // [0, 399]
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = (if mp < 10 { mp + 3 } else { mp - 9 }) as u32;
    let y = if m <= 2 { y + 1 } else { y };
    (y as i32, m, d)
}

#[cfg(test)]
mod stamp_provenance_tests {
    //! One `@Comment` per stage; rerunning a stage replaces that stage's
    //! comment in place; other stages' comments are untouched. Each
    //! comment carries its own UTC date so stages run on different days
    //! stay auditable.
    use super::*;
    use talkbank_model::Line;
    use talkbank_model::model::Header;

    fn comment_lines(lines: &[Line]) -> Vec<String> {
        lines
            .iter()
            .filter_map(|l| match l.as_header() {
                Some(Header::Comment { content }) => Some(content.to_chat_string()),
                _ => None,
            })
            .collect()
    }

    fn ba_comments(lines: &[Line]) -> Vec<String> {
        comment_lines(lines)
            .into_iter()
            .filter(|c| c.starts_with(PROVENANCE_PREFIX))
            .collect()
    }

    #[test]
    fn one_comment_per_stage_with_date() {
        let mut lines: Vec<Line> = Vec::new();
        stamp_provenance(&mut lines, "utr", Some("rev:rev_lang_en"));
        stamp_provenance(&mut lines, "fa", Some("wav2vec2-fa:mms_fa-v3"));
        let comments = ba_comments(&lines);
        assert_eq!(comments.len(), 2, "one comment per stage: {comments:?}");
        let utr = comments.iter().find(|c| c.contains("| utr: ")).unwrap();
        let fa = comments.iter().find(|c| c.contains("| fa: ")).unwrap();
        assert!(utr.contains("rev:rev_lang_en"), "utr engine: {utr}");
        assert!(fa.contains("wav2vec2-fa:mms_fa-v3"), "fa engine: {fa}");
        assert!(has_timestamp_tail(utr), "utr ts tail: {utr}");
        assert!(has_timestamp_tail(fa), "fa ts tail: {fa}");
    }

    #[test]
    fn rerunning_same_stage_replaces_not_duplicates() {
        let mut lines: Vec<Line> = Vec::new();
        stamp_provenance(&mut lines, "utr", Some("rev:rev_lang_en"));
        stamp_provenance(&mut lines, "utr", Some("whisper:large-v3"));
        let comments = ba_comments(&lines);
        assert_eq!(comments.len(), 1, "still one utr comment: {comments:?}");
        let c = &comments[0];
        assert!(!c.contains("rev:rev_lang_en"), "old engine dropped: {c}");
        assert!(c.contains("whisper:large-v3"), "new engine present: {c}");
    }

    #[test]
    fn full_pipeline_rerun_replaces_per_stage_keeps_others() {
        let mut lines: Vec<Line> = Vec::new();
        stamp_provenance(&mut lines, "utr", Some("rev:en"));
        stamp_provenance(&mut lines, "fa", Some("wav2vec2:v3"));
        stamp_provenance(&mut lines, "utr", Some("whisper:large-v3"));
        let comments = ba_comments(&lines);
        assert_eq!(comments.len(), 2, "utr replaced in place: {comments:?}");
        let utr = comments.iter().find(|c| c.contains("| utr: ")).unwrap();
        let fa = comments.iter().find(|c| c.contains("| fa: ")).unwrap();
        assert!(utr.contains("whisper:large-v3"));
        assert!(!utr.contains("rev:en"));
        assert!(fa.contains("wav2vec2:v3"));
    }

    #[test]
    fn engine_none_records_unknown() {
        let mut lines: Vec<Line> = Vec::new();
        stamp_provenance(&mut lines, "asr", None);
        let comments = ba_comments(&lines);
        assert_eq!(comments.len(), 1);
        assert!(comments[0].contains("asr: <unknown>"));
    }

    /// True iff `s` ends with ` | YYYY-MM-DDTHH:MM:SSZ`.
    fn has_timestamp_tail(s: &str) -> bool {
        let Some(tail) = s.rsplit(" | ").next() else {
            return false;
        };
        // Shape: 0123-56-89T12:45:78Z  (length 20)
        if tail.len() != 20 {
            return false;
        }
        let b = tail.as_bytes();
        let punct_ok = b[4] == b'-' && b[7] == b'-' && b[10] == b'T'
            && b[13] == b':' && b[16] == b':' && b[19] == b'Z';
        let digits_ok = [0, 1, 2, 3, 5, 6, 8, 9, 11, 12, 14, 15, 17, 18]
            .iter()
            .all(|&i| b[i].is_ascii_digit());
        punct_ok && digits_ok
    }

    #[test]
    fn civil_from_unix_days_matches_known_dates() {
        // Sanity-check the date math at a few well-known epochs.
        assert_eq!(civil_from_unix_days(0), (1970, 1, 1));
        assert_eq!(civil_from_unix_days(31), (1970, 2, 1));
        // 2000-03-01 = day 11017.
        assert_eq!(civil_from_unix_days(11_017), (2000, 3, 1));
        // 2026-06-02 = day 20_606.
        assert_eq!(civil_from_unix_days(20_606), (2026, 6, 2));
    }
}
