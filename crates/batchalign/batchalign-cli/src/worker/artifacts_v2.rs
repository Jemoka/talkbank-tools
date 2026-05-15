//! File-backed prepared-artifact store for worker protocol V2.
//!
//! Worker protocol V2 splits the boundary into:
//!
//! - a control plane with typed envelopes
//! - a data plane with prepared-artifact descriptors
//!
//! This module owns the data-plane side of worker protocol V2. It provides both
//! a simple file-backed store for prepared audio/text artifacts and a scoped
//! runtime that keeps those artifact files alive for one request family while
//! Rust dispatch code talks to Python workers.

use std::fs::{self, File};
use std::io::{self, BufWriter, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use thiserror::Error;

use crate::api::DurationMs;
use crate::types::worker_v2::{
    ArtifactRefV2, ByteLengthV2, ByteOffsetV2, ChannelCountV2, FrameCountV2,
    PreparedAudioEncodingV2, PreparedAudioRefV2, PreparedTextEncodingV2, PreparedTextRefV2,
    SampleRateHzV2, WorkerArtifactIdV2, WorkerArtifactPathV2,
};

/// Directory names used inside one prepared-artifact root.
const AUDIO_DIR_NAME: &str = "audio";
const TEXT_DIR_NAME: &str = "text";

/// Errors produced while materializing prepared artifacts for protocol V2.
#[derive(Debug, Error)]
pub enum PreparedArtifactErrorV2 {
    /// `ffmpeg` is not installed or not visible on `PATH`.
    #[error("ffmpeg not found in PATH while preparing audio artifact from {path}")]
    FfmpegNotFound {
        /// Source media path that required ffmpeg.
        path: String,
    },

    /// `ffmpeg` exited with a non-zero status while producing an artifact.
    #[error("ffmpeg failed while preparing audio artifact from {path}: {detail}")]
    FfmpegFailed {
        /// Source media path that failed conversion.
        path: String,
        /// ffmpeg stderr output.
        detail: String,
    },

    /// The requested audio segment falls entirely beyond the end of the source
    /// file.  `ffmpeg` exits with code 0 in this case but writes zero bytes,
    /// which would cause downstream ML models to crash on empty tensors.
    ///
    /// Callers that encounter this error should skip the group rather than
    /// propagating a hard failure — the utterances will remain unaligned.
    #[error("empty audio segment: [{start_ms}ms..{end_ms}ms) falls past end of {path}")]
    EmptyAudioSegment {
        /// Source media path.
        path: String,
        /// Requested segment start (milliseconds from file start).
        start_ms: u64,
        /// Requested segment end (milliseconds from file start).
        end_ms: u64,
    },

    /// Filesystem or process-management error.
    #[error("prepared artifact I/O error: {0}")]
    Io(#[from] io::Error),
}

/// File-backed prepared-artifact store for protocol V2.
#[derive(Debug, Clone)]
pub struct PreparedArtifactStoreV2 {
    root: PathBuf,
}

impl PreparedArtifactStoreV2 {
    /// Create or open a prepared-artifact store rooted at `root`.
    pub fn new(root: impl AsRef<Path>) -> io::Result<Self> {
        let root = root.as_ref().to_path_buf();
        fs::create_dir_all(root.join(AUDIO_DIR_NAME))?;
        fs::create_dir_all(root.join(TEXT_DIR_NAME))?;
        Ok(Self { root })
    }

    /// Return the filesystem root for this store.
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Create an inline JSON attachment without touching the filesystem.
    pub fn inline_json(&self, id: WorkerArtifactIdV2, value: serde_json::Value) -> ArtifactRefV2 {
        ArtifactRefV2::InlineJson(crate::types::worker_v2::InlineJsonRefV2 { id, value })
    }

    /// Write mono or multi-channel float32 PCM audio and return the descriptor.
    pub fn write_prepared_audio_f32le(
        &self,
        id: &WorkerArtifactIdV2,
        samples: &[f32],
        sample_rate_hz: SampleRateHzV2,
        channels: ChannelCountV2,
    ) -> io::Result<PreparedAudioRefV2> {
        let channel_count = channels.0 as usize;
        if channel_count == 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prepared audio channels must be greater than zero",
            ));
        }
        if !samples.len().is_multiple_of(channel_count) {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prepared audio sample count must divide evenly by channels",
            ));
        }

        let path = self.audio_path_for(id);
        let file = File::create(&path)?;
        let mut writer = BufWriter::new(file);
        for sample in samples {
            writer.write_all(&sample.to_le_bytes())?;
        }
        writer.flush()?;

        Ok(PreparedAudioRefV2 {
            id: id.clone(),
            path: WorkerArtifactPathV2(path.to_string_lossy().into_owned()),
            encoding: PreparedAudioEncodingV2::PcmF32le,
            channels,
            sample_rate_hz,
            frame_count: FrameCountV2((samples.len() / channel_count) as u64),
            byte_offset: ByteOffsetV2(0),
            byte_len: ByteLengthV2(std::mem::size_of_val(samples) as u64),
        })
    }

    /// Write structured UTF-8 JSON text and return the descriptor.
    pub fn write_prepared_text_json<T: Serialize>(
        &self,
        id: &WorkerArtifactIdV2,
        value: &T,
    ) -> io::Result<PreparedTextRefV2> {
        let path = self.text_path_for(id);
        let bytes = serde_json::to_vec(value)
            .map_err(|error| io::Error::new(io::ErrorKind::InvalidInput, error))?;
        fs::write(&path, &bytes)?;

        Ok(PreparedTextRefV2 {
            id: id.clone(),
            path: WorkerArtifactPathV2(path.to_string_lossy().into_owned()),
            encoding: PreparedTextEncodingV2::Utf8Json,
            byte_offset: ByteOffsetV2(0),
            byte_len: ByteLengthV2(bytes.len() as u64),
        })
    }

    /// Extract a mono 16 kHz float32 PCM audio span using ffmpeg.
    ///
    /// This is the first concrete data-plane step toward worker protocol V2:
    /// Rust materializes the model-ready audio span up front, and the future
    /// Python worker will consume the resulting descriptor rather than reading
    /// and chunking source media on its own.
    pub async fn extract_prepared_audio_segment_f32le(
        &self,
        id: &WorkerArtifactIdV2,
        source: &Path,
        start_ms: DurationMs,
        end_ms: DurationMs,
    ) -> Result<PreparedAudioRefV2, PreparedArtifactErrorV2> {
        if end_ms <= start_ms {
            return Err(PreparedArtifactErrorV2::Io(io::Error::new(
                io::ErrorKind::InvalidInput,
                "prepared audio segment end must be greater than start",
            )));
        }

        let root = self.root.clone();
        let id = id.clone();
        let source = source.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if !ffmpeg_available() {
                return Err(PreparedArtifactErrorV2::FfmpegNotFound {
                    path: source.display().to_string(),
                });
            }

            fs::create_dir_all(root.join(AUDIO_DIR_NAME))?;
            let output_path = root.join(AUDIO_DIR_NAME).join(format!("{id}.pcm"));
            let start_secs = format!("{:.3}", start_ms.0 as f64 / 1000.0);
            let end_secs = format!("{:.3}", end_ms.0 as f64 / 1000.0);

            let output = std::process::Command::new("ffmpeg")
                .args([
                    "-y".as_ref(),
                    "-ss".as_ref(),
                    start_secs.as_ref(),
                    "-to".as_ref(),
                    end_secs.as_ref(),
                    "-i".as_ref(),
                    source.as_os_str(),
                    "-f".as_ref(),
                    "f32le".as_ref(),
                    "-acodec".as_ref(),
                    "pcm_f32le".as_ref(),
                    "-ar".as_ref(),
                    "16000".as_ref(),
                    "-ac".as_ref(),
                    "1".as_ref(),
                    output_path.as_os_str(),
                ])
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let _ = fs::remove_file(&output_path);
                return Err(PreparedArtifactErrorV2::FfmpegFailed {
                    path: source.display().to_string(),
                    detail: stderr,
                });
            }

            let byte_len = fs::metadata(&output_path)?.len();
            let sample_bytes = std::mem::size_of::<f32>() as u64;
            let frame_count = byte_len / sample_bytes;

            // ffmpeg exits with code 0 even when the requested segment falls
            // entirely past the end of the source file, but produces an empty
            // output.  An empty tensor would cause downstream ML models (e.g.,
            // Wave2Vec) to crash with opaque kernel-size errors.  Return a typed
            // error so callers can skip the group gracefully instead.
            if frame_count == 0 {
                let _ = fs::remove_file(&output_path);
                return Err(PreparedArtifactErrorV2::EmptyAudioSegment {
                    path: source.display().to_string(),
                    start_ms: start_ms.0,
                    end_ms: end_ms.0,
                });
            }

            Ok(PreparedAudioRefV2 {
                id,
                path: WorkerArtifactPathV2(output_path.to_string_lossy().into_owned()),
                encoding: PreparedAudioEncodingV2::PcmF32le,
                channels: ChannelCountV2(1),
                sample_rate_hz: SampleRateHzV2(16_000),
                frame_count: FrameCountV2(frame_count),
                byte_offset: ByteOffsetV2(0),
                byte_len: ByteLengthV2(byte_len),
            })
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Convert a full media file into a mono 16 kHz float32 PCM artifact.
    ///
    /// This is the whole-file counterpart to
    /// [`extract_prepared_audio_segment_f32le`](Self::extract_prepared_audio_segment_f32le).
    /// It is used when Rust wants to own ASR audio preparation but the model
    /// should still see the entire file instead of one explicit subspan.
    pub async fn prepare_audio_file_f32le(
        &self,
        id: &WorkerArtifactIdV2,
        source: &Path,
    ) -> Result<PreparedAudioRefV2, PreparedArtifactErrorV2> {
        let root = self.root.clone();
        let id = id.clone();
        let source = source.to_path_buf();

        tokio::task::spawn_blocking(move || {
            if !ffmpeg_available() {
                return Err(PreparedArtifactErrorV2::FfmpegNotFound {
                    path: source.display().to_string(),
                });
            }

            fs::create_dir_all(root.join(AUDIO_DIR_NAME))?;
            let output_path = root.join(AUDIO_DIR_NAME).join(format!("{id}.pcm"));

            let output = std::process::Command::new("ffmpeg")
                .args([
                    "-y".as_ref(),
                    "-i".as_ref(),
                    source.as_os_str(),
                    "-f".as_ref(),
                    "f32le".as_ref(),
                    "-acodec".as_ref(),
                    "pcm_f32le".as_ref(),
                    "-ar".as_ref(),
                    "16000".as_ref(),
                    "-ac".as_ref(),
                    "1".as_ref(),
                    output_path.as_os_str(),
                ])
                .output()?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr).to_string();
                let _ = fs::remove_file(&output_path);
                return Err(PreparedArtifactErrorV2::FfmpegFailed {
                    path: source.display().to_string(),
                    detail: stderr,
                });
            }

            let byte_len = fs::metadata(&output_path)?.len();
            let sample_bytes = std::mem::size_of::<f32>() as u64;
            let frame_count = byte_len / sample_bytes;

            Ok(PreparedAudioRefV2 {
                id,
                path: WorkerArtifactPathV2(output_path.to_string_lossy().into_owned()),
                encoding: PreparedAudioEncodingV2::PcmF32le,
                channels: ChannelCountV2(1),
                sample_rate_hz: SampleRateHzV2(16_000),
                frame_count: FrameCountV2(frame_count),
                byte_offset: ByteOffsetV2(0),
                byte_len: ByteLengthV2(byte_len),
            })
        })
        .await
        .map_err(io::Error::other)?
    }

    /// Return the canonical audio path for one artifact id.
    fn audio_path_for(&self, id: &WorkerArtifactIdV2) -> PathBuf {
        self.root.join(AUDIO_DIR_NAME).join(format!("{id}.pcm"))
    }

    /// Return the canonical text path for one artifact id.
    fn text_path_for(&self, id: &WorkerArtifactIdV2) -> PathBuf {
        self.root.join(TEXT_DIR_NAME).join(format!("{id}.json"))
    }
}

/// Scoped prepared-artifact runtime for one V2 worker dispatch seam.
///
/// V2 request builders return descriptors that point at files on disk. The
/// runtime owns the temporary directory that backs those files and exposes the
/// regular [`PreparedArtifactStoreV2`] API for materialization, ensuring the
/// descriptors stay valid until the enclosing request/response flow completes.
#[derive(Debug)]
pub(crate) struct PreparedArtifactRuntimeV2 {
    _scratch_dir: tempfile::TempDir,
    store: PreparedArtifactStoreV2,
}

impl PreparedArtifactRuntimeV2 {
    /// Create a fresh scoped runtime rooted under a new temporary directory.
    pub(crate) fn new(scope_name: impl Into<PathBuf>) -> io::Result<Self> {
        let scratch_dir = tempfile::tempdir()?;
        let store = PreparedArtifactStoreV2::new(scratch_dir.path().join(scope_name.into()))?;
        Ok(Self {
            _scratch_dir: scratch_dir,
            store,
        })
    }

    /// Borrow the file-backed store owned by this runtime.
    pub(crate) fn store(&self) -> &PreparedArtifactStoreV2 {
        &self.store
    }

    /// Return the store root for diagnostics and tests.
    #[cfg(test)]
    pub(crate) fn root(&self) -> &Path {
        self.store.root()
    }

    /// Return the outer scratch directory kept alive by this runtime.
    #[cfg(test)]
    pub(crate) fn scratch_root(&self) -> &Path {
        self._scratch_dir.path()
    }
}

/// Return whether ffmpeg is available for artifact extraction tests and
/// prepared-audio materialization.
fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|output| output.status.success())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Create a temporary prepared-artifact store for unit tests.
    fn test_store() -> (PreparedArtifactStoreV2, tempfile::TempDir) {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = PreparedArtifactStoreV2::new(dir.path()).expect("prepared artifact store");
        (store, dir)
    }

    /// Create a scoped prepared-artifact runtime for unit tests.
    fn test_runtime() -> PreparedArtifactRuntimeV2 {
        PreparedArtifactRuntimeV2::new("artifacts").expect("prepared artifact runtime")
    }

    #[test]
    fn creates_expected_directory_layout() {
        let runtime = test_runtime();
        assert!(runtime.root().join(AUDIO_DIR_NAME).is_dir());
        assert!(runtime.root().join(TEXT_DIR_NAME).is_dir());
        assert!(runtime.scratch_root().is_dir());
    }

    #[test]
    fn writes_prepared_audio_descriptor_and_pcm_bytes() {
        let (store, _dir) = test_store();
        let id = WorkerArtifactIdV2::from("audio-ref-1");
        let descriptor = store
            .write_prepared_audio_f32le(
                &id,
                &[0.25, -0.5, 1.0, 0.0],
                SampleRateHzV2(16_000),
                ChannelCountV2(2),
            )
            .expect("write prepared audio");

        assert_eq!(descriptor.id, id);
        assert_eq!(descriptor.channels, 2);
        assert_eq!(descriptor.frame_count, 2);
        assert_eq!(descriptor.byte_len, 16);

        let bytes = fs::read(Path::new(descriptor.path.as_ref())).expect("read prepared pcm");
        let expected: Vec<u8> = [0.25_f32, -0.5_f32, 1.0_f32, 0.0_f32]
            .into_iter()
            .flat_map(f32::to_le_bytes)
            .collect();
        assert_eq!(bytes, expected);
    }

    #[test]
    fn rejects_audio_when_sample_count_does_not_match_channels() {
        let (store, _dir) = test_store();
        let error = store
            .write_prepared_audio_f32le(
                &WorkerArtifactIdV2::from("audio-ref-2"),
                &[0.1, 0.2, 0.3],
                SampleRateHzV2(16_000),
                ChannelCountV2(2),
            )
            .expect_err("misaligned sample count should fail");

        assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    }

    #[test]
    fn writes_prepared_text_descriptor_and_json_bytes() {
        let (store, _dir) = test_store();
        let id = WorkerArtifactIdV2::from("text-ref-1");
        let descriptor = store
            .write_prepared_text_json(&id, &serde_json::json!({"words": ["hello", "world"]}))
            .expect("write prepared text");

        assert_eq!(descriptor.id, id);
        assert_eq!(descriptor.encoding, PreparedTextEncodingV2::Utf8Json);

        let raw = fs::read_to_string(Path::new(descriptor.path.as_ref())).expect("read text");
        let parsed: serde_json::Value = serde_json::from_str(&raw).expect("parse text json");
        assert_eq!(parsed, serde_json::json!({"words": ["hello", "world"]}));
        assert_eq!(descriptor.byte_offset, 0);
        assert_eq!(descriptor.byte_len, raw.len() as u64);
    }

    #[tokio::test]
    async fn extracts_prepared_audio_segment_with_ffmpeg() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let (store, dir) = test_store();
        let wav_path = dir.path().join("tone.wav");
        let ffmpeg_out = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000",
                "-t",
                "0.25",
                wav_path.to_string_lossy().as_ref(),
            ])
            .output()
            .await
            .expect("ffmpeg process should run");
        if !ffmpeg_out.status.success() {
            eprintln!("skipping: could not generate test wav");
            return;
        }

        let descriptor = store
            .extract_prepared_audio_segment_f32le(
                &WorkerArtifactIdV2::from("audio-segment-ref"),
                &wav_path,
                DurationMs(0u64),
                DurationMs(100u64),
            )
            .await
            .expect("extract prepared audio segment");

        assert_eq!(descriptor.channels, 1);
        assert_eq!(descriptor.sample_rate_hz, 16_000);
        assert!(descriptor.frame_count.0 > 0);
        assert!(descriptor.byte_len.0 > 0);
        assert!(Path::new(descriptor.path.as_ref()).exists());
    }

    /// Helper: generate a short tone WAV for artifact tests.
    async fn write_test_tone_wav(path: &std::path::Path, duration_s: f32) {
        let out = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=16000",
                "-t",
                &format!("{duration_s}"),
                path.to_string_lossy().as_ref(),
            ])
            .output()
            .await
            .expect("ffmpeg process should run");
        assert!(
            out.status.success(),
            "ffmpeg should generate the artifact test tone: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }

    /// Regression test for the Croatian/PWA empty-audio-segment crash.
    ///
    /// When an FA group's timestamps extend past the end of the audio file,
    /// ffmpeg exits with code 0 but writes zero bytes.  Before the fix, this
    /// produced a 0-frame descriptor that caused Wave2Vec to crash with
    /// "Kernel size can't be greater than actual input size".  After the fix,
    /// the extractor returns `PreparedArtifactErrorV2::EmptyAudioSegment`.
    #[tokio::test]
    async fn extract_returns_empty_audio_segment_error_when_segment_past_end_of_file() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let (store, dir) = test_store();
        let wav_path = dir.path().join("short.wav");
        // Generate a 0.1-second WAV — valid range is 0..100ms.
        write_test_tone_wav(&wav_path, 0.1).await;

        // Request a segment (500ms..600ms) entirely past the end of the file.
        let result = store
            .extract_prepared_audio_segment_f32le(
                &WorkerArtifactIdV2::from("empty-audio-test"),
                &wav_path,
                DurationMs(500u64),
                DurationMs(600u64),
            )
            .await;

        assert!(
            matches!(
                result,
                Err(PreparedArtifactErrorV2::EmptyAudioSegment {
                    start_ms: 500,
                    end_ms: 600,
                    ..
                })
            ),
            "expected EmptyAudioSegment error, got: {result:?}"
        );
    }

    #[test]
    fn builds_inline_json_attachment_without_filesystem_write() {
        let (store, dir) = test_store();
        let attachment = store.inline_json(
            WorkerArtifactIdV2::from("inline-ref-1"),
            serde_json::json!({"utterances": [{"words": ["hi"]}]}),
        );

        let ArtifactRefV2::InlineJson(inline) = attachment else {
            panic!("expected inline json attachment");
        };
        assert_eq!(inline.id, "inline-ref-1");
        assert_eq!(
            inline.value,
            serde_json::json!({"utterances": [{"words": ["hi"]}]})
        );

        let files: Vec<_> = walkdir::WalkDir::new(dir.path())
            .min_depth(1)
            .into_iter()
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_file())
            .collect();
        assert!(
            files.is_empty(),
            "inline attachments should not create files"
        );
    }
}
