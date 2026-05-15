//! Shared output-finalization helpers for audio-backed CHAT commands.
//!
//! `align` and `transcribe` both produce one primary CHAT artifact per file and
//! both optionally run `merge_abbrev` before persisting that output. Keeping
//! that policy in one place makes the per-file orchestrators easier to test and
//! reduces drift between the two highest-risk audio commands.

use crate::api::{DisplayPath, ReleasedCommand};
use crate::recipe_runner::materialize::PlannedMaterializedFile;
use crate::recipe_runner::runtime::{
    ChatOutputTarget, primary_output_artifact, write_chat_output_artifact_with_provenance_gate,
};
use crate::runner::dispatch::infer_batched::apply_merge_abbrev;
use crate::store::RunnerFilesystemConfig;

/// Apply output-finalization policy before the CHAT file is written.
pub(crate) fn finalize_chat_output(chat_text: &str, should_merge_abbrev: bool) -> String {
    if should_merge_abbrev {
        apply_merge_abbrev(chat_text)
    } else {
        chat_text.to_owned()
    }
}

/// Persist the command's primary CHAT output and return its planned artifact.
pub(crate) async fn write_primary_chat_output_artifact(
    filesystem: &RunnerFilesystemConfig,
    command: ReleasedCommand,
    file_index: usize,
    source_filename: &str,
    chat_text: &str,
    should_merge_abbrev: bool,
) -> std::io::Result<PlannedMaterializedFile> {
    let final_text = finalize_chat_output(chat_text, should_merge_abbrev);
    let primary_output = primary_output_artifact(command, &DisplayPath::from(source_filename));
    let target = ChatOutputTarget::new(filesystem, file_index, &primary_output.display_path);
    write_chat_output_artifact_with_provenance_gate(&target, &final_text, command).await?;
    Ok(primary_output)
}

#[cfg(test)]
mod tests {
    use talkbank_transform::serialize::to_chat_string;

    use super::*;
    use crate::api::ContentType;
    use batchalign_types::paths::{ClientPath, ServerPath};

    fn sample_filesystem(paths_mode: bool) -> RunnerFilesystemConfig {
        RunnerFilesystemConfig {
            paths_mode,
            source_paths: vec![ClientPath::new("/input/test.cha")],
            output_paths: vec![ClientPath::new("/tmp/output/test.cha")],
            before_paths: Vec::new(),
            staging_dir: ServerPath::new("/tmp/staging-audio-output"),
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: ClientPath::new("/input"),
        }
    }

    /// Verify that `finalize_chat_output` with `merge_abbrev=true` merges
    /// consecutive single-letter words matching a known abbreviation.
    #[test]
    fn finalize_chat_output_can_merge_abbrev() {
        let chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n*PAR:\tF B I do it .\n@End\n";
        let merged = finalize_chat_output(chat, true);
        let parser = crate::chat_parser();
        let (parsed, _) = talkbank_transform::parse::parse_lenient(&parser, &merged);
        let reparsed = to_chat_string(&parsed);
        assert!(
            reparsed.contains("FBI"),
            "merge_abbrev should collapse 'F B I' into 'FBI', got: {reparsed}"
        );
    }

    #[tokio::test]
    async fn write_primary_chat_output_artifact_uses_command_primary_path() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let filesystem = RunnerFilesystemConfig {
            output_paths: vec![ClientPath::new(
                tmp.path().join("requested/test.cha").to_string_lossy(),
            )],
            staging_dir: ServerPath::new(tmp.path().join("staging")),
            ..sample_filesystem(true)
        };
        let chat = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n*PAR:\thello .\n@End\n";

        let artifact = write_primary_chat_output_artifact(
            &filesystem,
            ReleasedCommand::Transcribe,
            0,
            "nested/test.mp3",
            chat,
            false,
        )
        .await
        .expect("write artifact");

        assert_eq!(artifact.content_type, ContentType::Chat);
        assert_eq!(artifact.display_path, DisplayPath::from("nested/test.cha"));
        let written = std::fs::read_to_string(tmp.path().join("requested/test.cha"))
            .expect("read written output");
        assert!(written.contains("*PAR:\thello ."));
    }
}
