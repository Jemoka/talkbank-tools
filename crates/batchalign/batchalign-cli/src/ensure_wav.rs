//! Content-fingerprinted WAV conversion cache.
//!
//! Converts non-WAV media (e.g. mp4) to WAV via ffmpeg and caches the result
//! in `~/.batchalign3/media_cache/` keyed by a fast content fingerprint. This
//! avoids writing converted files next to originals (which may be on read-only
//! NFS/SMB mounts) and deduplicates across paths that point to the same content.
//!
//!
//! # Design
//!
//! - **Fingerprint**: `BLAKE3(file_size ++ first_64KB ++ last_64KB)[:24]` —
//!   reads at most ~128 KB regardless of file size.
//! - **Locking**: per-fingerprint `.lock` file via `fs2` exclusive lock prevents
//!   concurrent ffmpeg invocations for the same source file (important for
//!   parallel FA groups).
//! - **Atomic writes**: convert to a temp file, then `rename()` into the cache.
//!
//! # Formats requiring conversion
//!
//! Only container formats that `libsndfile` (via `soundfile` in Python) cannot
//! read need conversion. Currently: `.mp4`, `.m4a`, `.webm`, `.wma`.
//! WAV, MP3, FLAC, OGG are handled natively by `soundfile`.

use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use fs2::FileExt;
use thiserror::Error;
use tracing::{debug, info};

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Extensions that `soundfile` (libsndfile) cannot read and must be converted.
const FORCED_CONVERSION: &[&str] = &["mp4", "m4a", "webm", "wma"];

/// Chunk size for fingerprinting (64 KB).
const FINGERPRINT_CHUNK: usize = 65536;

/// Default cache directory name under the app data dir.
const CACHE_SUBDIR: &str = "media_cache";

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[derive(Debug, Error)]
/// Errors from [`ensure_wav`].
pub enum EnsureWavError {
    /// ffmpeg is not installed or not on `PATH`.
    #[error(
        "ffmpeg not found in PATH. Cannot convert {path} to WAV.\n\
         Hint: install ffmpeg (https://ffmpeg.org/download.html) \
         or convert your input audio to .wav beforehand."
    )]
    FfmpegNotFound {
        /// Source file that could not be converted.
        path: String,
    },

    /// ffmpeg exited with a non-zero status.
    #[error("ffmpeg conversion failed for {path}: {detail}")]
    FfmpegFailed {
        /// Source file that failed conversion.
        path: String,
        /// ffmpeg stderr output.
        detail: String,
    },

    /// Filesystem I/O error.
    #[error("I/O error during media conversion: {0}")]
    Io(#[from] std::io::Error),
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Return a path to a WAV version of `source_path`.
///
/// - If `source_path` is already `.wav` / `.mp3` / `.flac` / `.ogg` (formats
///   that `soundfile` handles natively), return it unchanged.
/// - If a cached conversion exists (by content fingerprint), return that.
/// - Otherwise convert via ffmpeg, cache the result, and return the cache path.
///
/// `cache_dir` overrides the default cache location (for testing).
pub async fn ensure_wav(
    source_path: &Path,
    cache_dir: Option<&Path>,
) -> Result<PathBuf, EnsureWavError> {
    if !needs_conversion(source_path) {
        return Ok(source_path.to_path_buf());
    }

    // Move all blocking I/O (ffmpeg check, fingerprint, fs2 lock, ffmpeg
    // subprocess, rename) onto a dedicated thread so we don't starve the
    // tokio executor.
    let source_path = source_path.to_path_buf();
    let cache_dir = cache_dir.map(Path::to_path_buf);

    tokio::task::spawn_blocking(move || -> Result<PathBuf, EnsureWavError> {
        if !ffmpeg_available() {
            return Err(EnsureWavError::FfmpegNotFound {
                path: source_path.display().to_string(),
            });
        }

        let effective_cache_dir = match cache_dir {
            Some(p) => p,
            None => default_cache_dir(),
        };
        std::fs::create_dir_all(&effective_cache_dir)?;

        let fp = media_fingerprint(&source_path)?;
        let cached_wav = effective_cache_dir.join(format!("{fp}.wav"));

        // Fast path: already converted
        if cached_wav.exists() {
            debug!(
                "Media cache hit: {} -> {}",
                source_path.display(),
                cached_wav.display()
            );
            return Ok(cached_wav);
        }

        // Convert under a per-fingerprint exclusive lock to prevent concurrent
        // ffmpeg invocations for the same source file.
        let lock_path = effective_cache_dir.join(format!("{fp}.wav.lock"));
        let lock_file = std::fs::File::create(&lock_path)?;
        lock_file.lock_exclusive()?;

        // Re-check after acquiring lock (another task may have finished).
        if cached_wav.exists() {
            lock_file.unlock()?;
            return Ok(cached_wav);
        }

        info!(
            "Converting {} -> {}",
            source_path.display(),
            cached_wav.display()
        );

        // Write to temp file, then atomic rename.
        // Uses std::process::Command (blocking) since we're already on a
        // dedicated thread and the lock must be held across the conversion.
        let tmp_path = effective_cache_dir.join(format!("{fp}.tmp.wav"));
        let result = std::process::Command::new("ffmpeg")
            .args([
                "-y".as_ref(),
                "-i".as_ref(),
                source_path.as_os_str(),
                "-acodec".as_ref(),
                "pcm_s16le".as_ref(),
                "-ar".as_ref(),
                "16000".as_ref(),
                "-ac".as_ref(),
                "1".as_ref(),
                tmp_path.as_os_str(),
            ])
            .output();

        // Release lock before error handling
        let _ = lock_file.unlock();

        let result = result?;
        if !result.status.success() {
            let _ = std::fs::remove_file(&tmp_path);
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(EnsureWavError::FfmpegFailed {
                path: source_path.display().to_string(),
                detail: stderr.to_string(),
            });
        }

        std::fs::rename(&tmp_path, &cached_wav)?;
        Ok(cached_wav)
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Extract a time-windowed audio segment to a cached WAV file.
///
/// Uses ffmpeg `-ss {start} -to {end}` with pcm_s16le 16kHz mono.
/// Result is cached in media_cache keyed by source fingerprint + time window.
pub async fn extract_audio_segment(
    source: &Path,
    start_ms: u64,
    end_ms: u64,
) -> Result<PathBuf, EnsureWavError> {
    let source = source.to_path_buf();

    tokio::task::spawn_blocking(move || -> Result<PathBuf, EnsureWavError> {
        if !ffmpeg_available() {
            return Err(EnsureWavError::FfmpegNotFound {
                path: source.display().to_string(),
            });
        }

        let cache_dir = default_cache_dir();
        std::fs::create_dir_all(&cache_dir)?;

        // Key includes source identity + time window
        let base_fp = media_fingerprint(&source).unwrap_or_else(|_| {
            // Fallback: hash the path + mtime + size
            let meta = std::fs::metadata(&source).ok();
            let size = meta.as_ref().map(|m| m.len()).unwrap_or(0);
            format!("{:024x}", size)
        });
        let segment_key = blake3::hash(format!("{base_fp}|{start_ms}|{end_ms}").as_bytes())
            .to_hex()[..24]
            .to_string();
        let cached_wav = cache_dir.join(format!("{segment_key}.wav"));

        // Fast path: already extracted
        if cached_wav.exists() {
            debug!(
                "Segment cache hit: {}[{}..{}ms] -> {}",
                source.display(),
                start_ms,
                end_ms,
                cached_wav.display()
            );
            return Ok(cached_wav);
        }

        // Convert under lock
        let lock_path = cache_dir.join(format!("{segment_key}.wav.lock"));
        let lock_file = std::fs::File::create(&lock_path)?;
        lock_file.lock_exclusive()?;

        if cached_wav.exists() {
            lock_file.unlock()?;
            return Ok(cached_wav);
        }

        let start_secs = start_ms as f64 / 1000.0;
        let end_secs = end_ms as f64 / 1000.0;

        info!(
            "Extracting segment {}[{:.3}s..{:.3}s] -> {}",
            source.display(),
            start_secs,
            end_secs,
            cached_wav.display()
        );

        let tmp_path = cache_dir.join(format!("{segment_key}.tmp.wav"));
        let ss_arg = format!("{start_secs:.3}");
        let to_arg = format!("{end_secs:.3}");
        let result = std::process::Command::new("ffmpeg")
            .args([
                "-y".as_ref(),
                "-ss".as_ref(),
                ss_arg.as_ref(),
                "-to".as_ref(),
                to_arg.as_ref(),
                "-i".as_ref(),
                source.as_os_str(),
                "-acodec".as_ref(),
                "pcm_s16le".as_ref(),
                "-ar".as_ref(),
                "16000".as_ref(),
                "-ac".as_ref(),
                "1".as_ref(),
                tmp_path.as_os_str(),
            ])
            .output();

        let _ = lock_file.unlock();

        let result = result?;
        if !result.status.success() {
            let _ = std::fs::remove_file(&tmp_path);
            let stderr = String::from_utf8_lossy(&result.stderr);
            return Err(EnsureWavError::FfmpegFailed {
                path: source.display().to_string(),
                detail: stderr.to_string(),
            });
        }

        std::fs::rename(&tmp_path, &cached_wav)?;
        Ok(cached_wav)
    })
    .await
    .map_err(std::io::Error::other)?
}

/// Remove all files in the media cache. Returns bytes freed.
pub fn clear_cache(cache_dir: Option<&Path>) -> std::io::Result<u64> {
    let dir = match cache_dir {
        Some(p) => p.to_path_buf(),
        None => default_cache_dir(),
    };
    if !dir.is_dir() {
        return Ok(0);
    }
    let mut freed = 0u64;
    for entry in std::fs::read_dir(&dir)? {
        let entry = entry?;
        let path = entry.path();
        if let Ok(meta) = std::fs::metadata(&path) {
            freed += meta.len();
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(freed)
}

/// Return `(cache_dir, total_size_bytes, file_count)`.
pub fn cache_stats(cache_dir: Option<&Path>) -> (PathBuf, u64, usize) {
    let dir = match cache_dir {
        Some(p) => p.to_path_buf(),
        None => default_cache_dir(),
    };
    if !dir.is_dir() {
        return (dir, 0, 0);
    }
    let mut total_size = 0u64;
    let mut file_count = 0usize;
    if let Ok(entries) = std::fs::read_dir(&dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "wav")
                && let Ok(meta) = std::fs::metadata(&path)
            {
                total_size += meta.len();
                file_count += 1;
            }
        }
    }
    (dir, total_size, file_count)
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// Check whether the file extension requires ffmpeg conversion.
pub fn needs_conversion(path: &Path) -> bool {
    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();
    FORCED_CONVERSION.contains(&ext.as_str())
}

/// Fast content fingerprint: `BLAKE3(file_size ++ first_64KB ++ last_64KB)[:24]`.
///
/// Reads at most ~128 KB regardless of file size. The size prefix
/// distinguishes files whose head and tail happen to be identical but
/// differ in the middle.
fn media_fingerprint(path: &Path) -> Result<String, std::io::Error> {
    let mut file = std::fs::File::open(path)?;
    let size = file.metadata()?.len();

    let mut hasher = blake3::Hasher::new();
    hasher.update(&size.to_be_bytes());

    let mut buf = vec![0u8; FINGERPRINT_CHUNK];
    let n = file.read(&mut buf)?;
    hasher.update(&buf[..n]);

    if size > FINGERPRINT_CHUNK as u64 {
        file.seek(SeekFrom::End(-(FINGERPRINT_CHUNK as i64)))?;
        let n = file.read(&mut buf)?;
        hasher.update(&buf[..n]);
    }

    let hash = hasher.finalize();
    // 24 hex chars from 12 bytes
    Ok(hash.to_hex()[..24].to_string())
}

fn default_cache_dir() -> PathBuf {
    default_cache_dir_from(
        crate::runtime_paths::media_cache_dir_override_from_env(),
        dirs::data_dir(),
    )
}

fn default_cache_dir_from(
    override_dir: Option<PathBuf>,
    platform_data_dir: Option<PathBuf>,
) -> PathBuf {
    override_dir.unwrap_or_else(|| {
        platform_data_dir
            .unwrap_or_else(|| PathBuf::from("."))
            .join("batchalign3")
            .join(CACHE_SUBDIR)
    })
}

fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .output()
        .is_ok_and(|o| o.status.success())
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_needs_no_conversion() {
        assert!(!needs_conversion(Path::new("audio.wav")));
        assert!(!needs_conversion(Path::new("audio.WAV")));
        assert!(!needs_conversion(Path::new("/path/to/audio.mp3")));
        assert!(!needs_conversion(Path::new("audio.flac")));
        assert!(!needs_conversion(Path::new("audio.ogg")));
    }

    #[test]
    fn mp4_needs_conversion() {
        assert!(needs_conversion(Path::new("video.mp4")));
        assert!(needs_conversion(Path::new("video.MP4")));
        assert!(needs_conversion(Path::new("/path/to/video.mp4")));
    }

    #[test]
    fn m4a_needs_conversion() {
        assert!(needs_conversion(Path::new("audio.m4a")));
    }

    #[test]
    fn webm_needs_conversion() {
        assert!(needs_conversion(Path::new("video.webm")));
    }

    #[test]
    fn wma_needs_conversion() {
        assert!(needs_conversion(Path::new("audio.wma")));
    }

    #[test]
    fn no_extension_needs_no_conversion() {
        assert!(!needs_conversion(Path::new("noext")));
        assert!(!needs_conversion(Path::new("/path/to/noext")));
    }

    #[test]
    fn fingerprint_deterministic() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("test.mp4");
        std::fs::write(&path, b"fake mp4 content for fingerprinting").unwrap();

        let fp1 = media_fingerprint(&path).unwrap();
        let fp2 = media_fingerprint(&path).unwrap();
        assert_eq!(fp1, fp2, "fingerprint should be deterministic");
        assert_eq!(fp1.len(), 24, "fingerprint should be 24 hex chars");
    }

    #[test]
    fn fingerprint_differs_for_different_content() {
        let dir = tempfile::tempdir().unwrap();
        let path_a = dir.path().join("a.mp4");
        let path_b = dir.path().join("b.mp4");
        std::fs::write(&path_a, b"content A").unwrap();
        std::fs::write(&path_b, b"content B").unwrap();

        let fp_a = media_fingerprint(&path_a).unwrap();
        let fp_b = media_fingerprint(&path_b).unwrap();
        assert_ne!(fp_a, fp_b);
    }

    #[test]
    fn fingerprint_large_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("large.mp4");
        // Create a file larger than FINGERPRINT_CHUNK
        let data = vec![0xABu8; FINGERPRINT_CHUNK * 3];
        std::fs::write(&path, &data).unwrap();

        let fp = media_fingerprint(&path).unwrap();
        assert_eq!(fp.len(), 24);
    }

    #[test]
    fn default_cache_dir_prefers_explicit_override() {
        let resolved = default_cache_dir_from(
            Some(PathBuf::from("/tmp/media-cache")),
            Some(PathBuf::from("/tmp/platform-data")),
        );

        assert_eq!(resolved, PathBuf::from("/tmp/media-cache"));
    }

    #[test]
    fn default_cache_dir_uses_platform_data_dir_when_no_override() {
        let resolved = default_cache_dir_from(None, Some(PathBuf::from("/tmp/platform-data")));

        assert_eq!(
            resolved,
            PathBuf::from("/tmp/platform-data")
                .join("batchalign3")
                .join(CACHE_SUBDIR)
        );
    }

    #[tokio::test]
    async fn ensure_wav_passthrough_for_wav() {
        let result = ensure_wav(Path::new("/some/path/audio.wav"), None)
            .await
            .unwrap();
        assert_eq!(result, Path::new("/some/path/audio.wav"));
    }

    #[tokio::test]
    async fn ensure_wav_passthrough_for_mp3() {
        let result = ensure_wav(Path::new("/some/path/audio.mp3"), None)
            .await
            .unwrap();
        assert_eq!(result, Path::new("/some/path/audio.mp3"));
    }

    #[tokio::test]
    async fn ensure_wav_passthrough_for_flac() {
        let result = ensure_wav(Path::new("/some/path/audio.flac"), None)
            .await
            .unwrap();
        assert_eq!(result, Path::new("/some/path/audio.flac"));
    }

    #[tokio::test]
    async fn ensure_wav_converts_mp4() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("cache");

        // Create a minimal valid mp4 via ffmpeg (short silent audio)
        let mp4_path = dir.path().join("test.mp4");
        let ffmpeg_out = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=16000:cl=mono",
                "-t",
                "0.1",
                mp4_path.to_string_lossy().as_ref(),
            ])
            .output()
            .await;
        if ffmpeg_out.is_err() || !ffmpeg_out.unwrap().status.success() {
            eprintln!("skipping: could not generate test mp4");
            return;
        }

        // First call should convert
        let wav_path = ensure_wav(&mp4_path, Some(&cache_dir)).await.unwrap();
        assert!(
            wav_path.extension().is_some_and(|e| e == "wav"),
            "should return a .wav path"
        );
        assert!(wav_path.exists(), "converted wav should exist on disk");
        assert!(
            wav_path.starts_with(&cache_dir),
            "wav should be in cache dir"
        );

        // Second call should hit cache
        let wav_path2 = ensure_wav(&mp4_path, Some(&cache_dir)).await.unwrap();
        assert_eq!(wav_path, wav_path2, "cache should return same path");
    }

    #[tokio::test]
    async fn ensure_wav_converts_m4a() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("cache");

        let m4a_path = dir.path().join("test.m4a");
        let ffmpeg_out = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=16000:cl=mono",
                "-t",
                "0.1",
                m4a_path.to_string_lossy().as_ref(),
            ])
            .output()
            .await;
        if ffmpeg_out.is_err() || !ffmpeg_out.unwrap().status.success() {
            eprintln!("skipping: could not generate test m4a");
            return;
        }

        let wav_path = ensure_wav(&m4a_path, Some(&cache_dir)).await.unwrap();
        assert!(wav_path.extension().is_some_and(|e| e == "wav"));
        assert!(wav_path.exists());
    }

    #[tokio::test]
    async fn ensure_wav_cache_stats_and_clear() {
        if !ffmpeg_available() {
            eprintln!("skipping: ffmpeg not installed");
            return;
        }

        let dir = tempfile::tempdir().unwrap();
        let cache_dir = dir.path().join("cache");

        // Empty cache
        let (_, size, count) = cache_stats(Some(&cache_dir));
        assert_eq!(size, 0);
        assert_eq!(count, 0);

        // Generate and convert an mp4
        let mp4_path = dir.path().join("test.mp4");
        let ffmpeg_out = tokio::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "anullsrc=r=16000:cl=mono",
                "-t",
                "0.1",
                mp4_path.to_string_lossy().as_ref(),
            ])
            .output()
            .await;
        if ffmpeg_out.is_err() || !ffmpeg_out.unwrap().status.success() {
            return;
        }

        let _ = ensure_wav(&mp4_path, Some(&cache_dir)).await.unwrap();

        let (_, size, count) = cache_stats(Some(&cache_dir));
        assert!(size > 0, "cache should have nonzero size");
        assert_eq!(count, 1, "cache should have one file");

        // Clear
        let freed = clear_cache(Some(&cache_dir)).unwrap();
        assert!(freed > 0);
        let (_, size, count) = cache_stats(Some(&cache_dir));
        assert_eq!(size, 0);
        assert_eq!(count, 0);
    }
}
