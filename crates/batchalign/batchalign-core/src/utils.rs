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
}

impl MediaInput {
    /// Construct a `MediaInput` from a path and an explicit `SourceId`.
    pub fn new(source_id: SourceId, path: PathBuf) -> Self {
        Self { source_id, path }
    }
}

/// Decoded PCM ready to ship to a backend.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[cfg_attr(feature = "python", pyo3::pyclass(get_all))]
pub struct PreparedAudio {
    /// Interleaved little-endian f32 PCM samples.
    pub pcm_f32le: Vec<u8>,
    /// Sampling rate in Hz.
    pub sample_rate: u32,
    /// Channel count.
    pub channels: u16,
    /// Number of frames (samples per channel).
    pub frame_count: u64,
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
    let channels = track
        .codec_params
        .channels
        .map(|c| c.count() as u16)
        .unwrap_or(1);

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
        for plane in buf.planes().planes() {
            pcm_f32.extend_from_slice(plane);
        }
    }

    let mut pcm_bytes = Vec::with_capacity(pcm_f32.len() * 4);
    for sample in &pcm_f32 {
        pcm_bytes.extend_from_slice(&sample.to_le_bytes());
    }

    let frame_count = if channels == 0 {
        pcm_f32.len() as u64
    } else {
        pcm_f32.len() as u64 / channels as u64
    };

    Ok(PreparedAudio {
        pcm_f32le: pcm_bytes,
        sample_rate,
        channels,
        frame_count,
    })
}
