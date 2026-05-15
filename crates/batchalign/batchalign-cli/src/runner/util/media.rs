//! Media resolution, preflight validation, and output path handling.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api::ReleasedCommand;
use crate::options::{CommandOptions, UtrEngine};
use crate::store::{PendingJobFile, RunnerJobSnapshot};
use batchalign_types::paths::ClientPath;

use super::auto_tune::KNOWN_MEDIA_EXTENSIONS;

/// Check if a job should use Rev.AI preflight submission.
///
/// Preflight pre-submits all audio files to Rev.AI before processing,
/// allowing Rev.AI to process them in parallel server-side.
pub(in crate::runner) fn should_preflight(
    command: ReleasedCommand,
    typed_options: Option<&CommandOptions>,
) -> bool {
    match (command, typed_options) {
        (
            ReleasedCommand::Transcribe | ReleasedCommand::TranscribeS,
            Some(CommandOptions::Transcribe(t) | CommandOptions::TranscribeS(t)),
        ) => t.effective_asr_engine().is_revai(),
        (ReleasedCommand::Transcribe | ReleasedCommand::TranscribeS, None) => true,
        (ReleasedCommand::Benchmark, Some(CommandOptions::Benchmark(b))) => {
            b.effective_asr_engine().is_revai()
        }
        (ReleasedCommand::Benchmark, None) => true,
        (ReleasedCommand::Align, Some(CommandOptions::Align(a))) => {
            matches!(a.utr_engine, Some(UtrEngine::RevAi))
        }
        (ReleasedCommand::Align, None) => true,
        _ => false,
    }
}

/// Pre-validate media files before dispatch.
///
/// For non-CHAT files in paths_mode, checks:
/// 1. File exists on disk
/// 2. File is non-empty
/// 3. File extension is a known audio/video format
///
/// Returns the set of file indices that failed validation.
pub(in crate::runner) async fn preflight_validate_media(
    file_list: &[PendingJobFile],
    source_paths: &[ClientPath],
    paths_mode: bool,
) -> HashMap<usize, String> {
    if !paths_mode {
        return HashMap::new();
    }

    let mut failures = HashMap::new();

    for file in file_list {
        // Only validate non-CHAT (media) files
        if file.has_chat {
            continue;
        }

        let Some(client_path) = source_paths.get(file.file_index) else {
            failures.insert(file.file_index, "No source path for file index".to_string());
            continue;
        };

        // In paths_mode the client and server share a filesystem.
        let path = client_path.assume_shared_filesystem();

        // Check file exists and non-empty via metadata (one syscall)
        match tokio::fs::metadata(&path).await {
            Err(_) => {
                failures.insert(file.file_index, format!("File not found: {}", path));
                continue;
            }
            Ok(meta) if meta.len() == 0 => {
                failures.insert(file.file_index, format!("File is empty: {}", path));
                continue;
            }
            Ok(_) => {}
        }

        // Check known extension
        let ext = path
            .as_path()
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.to_lowercase());
        if let Some(ref ext) = ext {
            if !KNOWN_MEDIA_EXTENSIONS.contains(&ext.as_str()) {
                failures.insert(
                    file.file_index,
                    format!("Unknown media format '.{ext}': {}", path),
                );
            }
        } else {
            failures.insert(file.file_index, format!("File has no extension: {}", path));
        }
    }

    failures
}

/// Collect the original media paths that should be pre-submitted to Rev.AI for
/// one job.
///
/// The returned paths must be the original provider-visible files, not
/// temporary WAV conversions, because preflight submission happens before the
/// per-file processing pipeline starts.
pub(in crate::runner) async fn collect_preflight_audio_paths(
    command: ReleasedCommand,
    job: &RunnerJobSnapshot,
    file_list: &[PendingJobFile],
) -> Vec<PathBuf> {
    match command {
        ReleasedCommand::Align => collect_align_preflight_audio_paths(job, file_list).await,
        _ => file_list
            .iter()
            .filter(|file| !file.has_chat)
            .filter_map(|file| {
                if job.filesystem.paths_mode && file.file_index < job.filesystem.source_paths.len()
                {
                    Some(
                        job.filesystem.source_paths[file.file_index]
                            .assume_shared_filesystem()
                            .as_path()
                            .to_owned(),
                    )
                } else {
                    None
                }
            })
            .collect(),
    }
}

/// Collect align-job media paths for Rev.AI preflight.
///
/// Align jobs usually begin from CHAT files, so preflight must resolve the
/// sibling media path first. This helper currently supports the local
/// `paths_mode` shape, which is where Rev.AI preflight provides the main
/// throughput win for large corpora.
async fn collect_align_preflight_audio_paths(
    job: &RunnerJobSnapshot,
    file_list: &[PendingJobFile],
) -> Vec<PathBuf> {
    if !job.filesystem.paths_mode {
        return Vec::new();
    }

    let mut paths = Vec::new();
    for file in file_list {
        let Some(client_path) = job.filesystem.source_paths.get(file.file_index) else {
            continue;
        };
        let server_path = client_path.assume_shared_filesystem();
        if let Some(audio_path) = resolve_audio_for_chat(server_path.as_path()).await {
            paths.push(audio_path);
        }
    }
    paths
}

/// Resolve the audio file path for a given CHAT file path.
///
/// Looks for files with the same stem and a known audio extension
/// in the same directory as the CHAT file.
pub(in crate::runner) async fn resolve_audio_for_chat(chat_path: &Path) -> Option<PathBuf> {
    resolve_audio_for_chat_with_media_dir(chat_path, None).await
}

/// Resolve the audio file for a given CHAT file path.
///
/// Search order:
/// 1. Custom `media_dir` if provided (from `--media-dir`)
/// 2. Alongside the .cha file (same directory)
pub(in crate::runner) async fn resolve_audio_for_chat_with_media_dir(
    chat_path: &Path,
    media_dir: Option<&Path>,
) -> Option<PathBuf> {
    let stem = chat_path.file_stem()?.to_str()?;

    // 1. Check custom media_dir first
    if let Some(dir) = media_dir {
        for ext in KNOWN_MEDIA_EXTENSIONS {
            let candidate = dir.join(format!("{stem}.{ext}"));
            if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
                return Some(candidate);
            }
        }
    }

    // 2. Alongside the .cha file
    let dir = chat_path.parent()?;
    for ext in KNOWN_MEDIA_EXTENSIONS {
        let candidate = dir.join(format!("{stem}.{ext}"));
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Some(candidate);
        }
    }
    None
}

/// Compute audio identity for cache keying.
///
/// Returns an [`AudioIdentity`] built from the file's resolved path,
/// modification time, and size.
pub(in crate::runner) async fn compute_audio_identity(
    audio_path: &str,
) -> Option<crate::chat_ops::fa::AudioIdentity> {
    let meta = tokio::fs::metadata(audio_path).await.ok()?;
    let size = meta.len();
    let mtime = meta
        .modified()
        .ok()?
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs();
    Some(crate::chat_ops::fa::AudioIdentity::from_metadata(
        audio_path, mtime, size,
    ))
}

/// Get audio duration in milliseconds via ffprobe.
///
/// Returns `None` if ffprobe is not available or fails.
pub(in crate::runner) async fn get_audio_duration_ms(audio_path: &str) -> Option<u64> {
    let output = tokio::process::Command::new("ffprobe")
        .args([
            "-v",
            "quiet",
            "-show_entries",
            "format=duration",
            "-of",
            "default=noprint_wrappers=1:nokey=1",
            audio_path,
        ])
        .output()
        .await
        .ok()?;

    if !output.status.success() {
        return None;
    }

    let stdout = String::from_utf8_lossy(&output.stdout);
    let duration_s: f64 = stdout.trim().parse().ok()?;
    Some((duration_s * 1000.0) as u64)
}

/// Replace the filename in `output_path` with `result_filename`.
#[cfg(test)]
pub(in crate::runner) fn apply_result_filename(
    output_path: &Path,
    result_filename: &str,
) -> PathBuf {
    let result_name = Path::new(result_filename).file_name().unwrap_or_default();
    output_path
        .parent()
        .map(|p| p.join(result_name))
        .unwrap_or_else(|| result_name.into())
}

#[cfg(test)]
mod tests {
    use super::should_preflight;
    use crate::api::ReleasedCommand;
    use crate::options::{
        AsrEngineName, BenchmarkOptions, CommandOptions, CommonOptions, TranscribeOptions,
    };

    #[test]
    fn transcribe_asr_override_disables_rev_preflight() {
        let mut common = CommonOptions::default();
        common.engine_overrides.asr = Some(AsrEngineName::HkTencent);
        let opts = CommandOptions::Transcribe(TranscribeOptions {
            common,
            asr_engine: AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        });

        assert!(!should_preflight(ReleasedCommand::Transcribe, Some(&opts)));
    }

    #[test]
    fn benchmark_asr_override_disables_rev_preflight() {
        let mut common = CommonOptions::default();
        common.engine_overrides.asr = Some(AsrEngineName::HkAliyun);
        let opts = CommandOptions::Benchmark(BenchmarkOptions {
            common,
            asr_engine: AsrEngineName::RevAi,
            wor: true.into(),
            merge_abbrev: false.into(),
        });

        assert!(!should_preflight(ReleasedCommand::Benchmark, Some(&opts)));
    }
}
