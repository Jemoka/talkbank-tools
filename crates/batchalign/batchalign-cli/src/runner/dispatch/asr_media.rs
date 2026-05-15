use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api::RevAiJobId;
use crate::error::ServerError;

pub(crate) fn resolve_paths_mode_or_staging_input(
    filesystem: &crate::store::RunnerFilesystemConfig,
    file_index: usize,
    filename: &str,
) -> PathBuf {
    if filesystem.paths_mode && file_index < filesystem.source_paths.len() {
        filesystem.source_paths[file_index]
            .assume_shared_filesystem()
            .as_path()
            .to_owned()
    } else {
        filesystem
            .staging_dir
            .as_path()
            .join("input")
            .join(filename)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PreparedAsrMediaInput {
    pub original_audio_path: PathBuf,
    pub inference_audio_path: PathBuf,
    pub media_name: Option<String>,
    pub rev_job_id: Option<RevAiJobId>,
}

pub(crate) fn preserved_media_name_for_chat(
    original_audio_path: &Path,
    inference_audio_path: &Path,
) -> Option<String> {
    original_audio_path
        .file_stem()
        .or_else(|| original_audio_path.file_name())
        .or_else(|| inference_audio_path.file_stem())
        .or_else(|| inference_audio_path.file_name())
        .map(|s| s.to_string_lossy().into_owned())
}

pub(crate) async fn prepare_asr_media_input(
    original_audio_path: PathBuf,
    rev_job_ids: &HashMap<PathBuf, RevAiJobId>,
    media_name: Option<String>,
    context_label: &str,
) -> Result<PreparedAsrMediaInput, ServerError> {
    let inference_audio_path = crate::ensure_wav::ensure_wav(&original_audio_path, None)
        .await
        .map_err(|error| {
            ServerError::Validation(format!(
                "Media conversion failed for {context_label}: {error}"
            ))
        })?;

    Ok(PreparedAsrMediaInput {
        rev_job_id: rev_job_ids.get(&original_audio_path).cloned(),
        original_audio_path,
        inference_audio_path,
        media_name,
    })
}

#[cfg(test)]
mod tests {
    use super::{preserved_media_name_for_chat, resolve_paths_mode_or_staging_input};
    use crate::store::RunnerFilesystemConfig;
    use std::path::PathBuf;

    #[test]
    fn resolve_paths_mode_or_staging_input_prefers_explicit_source_path() {
        let filesystem = RunnerFilesystemConfig {
            paths_mode: true,
            source_paths: vec![batchalign_types::paths::ClientPath::new(
                "/shared/in/clip.mp3".to_string(),
            )],
            output_paths: vec![],
            before_paths: vec![],
            staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
        };

        let path = resolve_paths_mode_or_staging_input(&filesystem, 0, "ignored.mp3");
        assert_eq!(path, PathBuf::from("/shared/in/clip.mp3"));
    }

    #[test]
    fn resolve_paths_mode_or_staging_input_falls_back_to_staging_input() {
        let filesystem = RunnerFilesystemConfig {
            paths_mode: false,
            source_paths: vec![],
            output_paths: vec![],
            before_paths: vec![],
            staging_dir: batchalign_types::paths::ServerPath::new("/tmp/staging"),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
        };

        let path = resolve_paths_mode_or_staging_input(&filesystem, 0, "clip.mp3");
        assert_eq!(path, PathBuf::from("/tmp/staging/input/clip.mp3"));
    }

    #[test]
    fn preserved_media_name_prefers_original_basename() {
        let original = PathBuf::from("/resources/corpus/interview.mp4");
        let inference = PathBuf::from("/cache/worker/interview.wav");
        assert_eq!(
            preserved_media_name_for_chat(&original, &inference).as_deref(),
            Some("interview")
        );
    }
}
