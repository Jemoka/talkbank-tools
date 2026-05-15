//! Prepare a local staging directory for rsync transfer.
//!
//! Given a list of CHAT file paths, this module:
//! 1. Copies each CHAT file into a staging directory
//! 2. Resolves the `@Media:` reference for each file
//! 3. Copies the resolved audio file alongside the CHAT file
//!
//! After preparation, the staging directory contains everything needed
//! for a remote server to process the job — CHAT files and their media.

use std::path::{Path, PathBuf};

use tracing::{info, warn};

use crate::api::JobId;
use crate::runner::util::KNOWN_MEDIA_EXTENSIONS;

use super::rsync::StagingError;

/// Prepare a staging directory with CHAT files and their resolved media.
///
/// Creates `<jobs_dir>/<job_id>/staged-input/` and populates it with copies
/// of the source CHAT files and any adjacent audio/video files.
///
/// Returns the path to the prepared staging directory.
pub async fn prepare_staging_dir(
    job_id: &JobId,
    source_paths: &[PathBuf],
    jobs_dir: &Path,
) -> Result<PathBuf, StagingError> {
    let staging_dir = jobs_dir.join(job_id.as_ref()).join("staged-input");
    tokio::fs::create_dir_all(&staging_dir).await?;

    let mut staged_count = 0;
    let mut media_count = 0;

    for source_path in source_paths {
        // Copy the CHAT file
        let filename = source_path.file_name().ok_or_else(|| {
            StagingError::ConfigIncomplete(format!(
                "source path has no filename: {}",
                source_path.display()
            ))
        })?;
        let dest = staging_dir.join(filename);
        tokio::fs::copy(source_path, &dest).await?;
        staged_count += 1;

        // Resolve and copy the media file (audio/video)
        if let Some(media_path) = resolve_adjacent_media(source_path).await {
            // PathBuf invariant: `resolve_adjacent_media` only
            // returns `Some(path)` for paths it resolved by sibling
            // lookup, which always yields an absolute path with a
            // file_name component. A `Some(...)` without a file_name
            // would be impossible given that contract.
            #[allow(clippy::expect_used)]
            let media_filename = media_path
                .file_name()
                .expect("resolved media path has a filename");
            let media_dest = staging_dir.join(media_filename);
            tokio::fs::copy(&media_path, &media_dest).await?;
            media_count += 1;
        } else {
            warn!(
                chat_file = %source_path.display(),
                "No adjacent media file found for staging"
            );
        }
    }

    info!(
        staged_files = staged_count,
        media_files = media_count,
        staging_dir = %staging_dir.display(),
        "Staging directory prepared"
    );

    Ok(staging_dir)
}

/// Find an audio/video file adjacent to a CHAT file (same directory, same stem).
///
/// This is a simplified version of `runner::util::media::resolve_audio_for_chat`
/// that doesn't require the runner module's visibility scope.
async fn resolve_adjacent_media(chat_path: &Path) -> Option<PathBuf> {
    let stem = chat_path.file_stem()?.to_str()?;
    let dir = chat_path.parent()?;

    for ext in KNOWN_MEDIA_EXTENSIONS {
        let candidate = dir.join(format!("{stem}.{ext}"));
        if tokio::fs::try_exists(&candidate).await.unwrap_or(false) {
            return Some(candidate);
        }
    }
    None
}
