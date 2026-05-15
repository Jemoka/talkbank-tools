//! Paths-mode submission preparation for local filesystem execution.

use std::path::{Path, PathBuf};

use crate::ReleasedCommand;
use crate::api::{JobSubmission, LanguageSpec};
use crate::options::CommandOptions;

use crate::cli::discover::{build_server_names, copy_nonmatching, infer_base_dir};
use crate::cli::error::CliError;

use super::helpers::{filter_files_for_command, inject_lexicon};

pub(super) struct PreparedPathsSubmission {
    pub submission: JobSubmission,
    pub effective_out: PathBuf,
    pub total_files: usize,
}

#[allow(clippy::too_many_arguments)]
pub(super) fn prepare_paths_submission(
    command: ReleasedCommand,
    lang: &str,
    num_speakers: u32,
    extensions: &[&str],
    inputs: &[std::path::PathBuf],
    out_dir: Option<&std::path::Path>,
    options: Option<&CommandOptions>,
    lexicon: Option<&str>,
    before: Option<&std::path::Path>,
    media_mapping_keys: &[String],
) -> Result<Option<PreparedPathsSubmission>, CliError> {
    let (files, outputs) =
        crate::cli::discover::discover_server_inputs(inputs, out_dir, extensions)?;
    let (files, outputs) = filter_files_for_command(command, files, outputs);

    if let Some(od) = out_dir {
        for inp in inputs {
            if Path::new(inp).is_dir() {
                copy_nonmatching(Path::new(inp), Path::new(od), extensions, command)?;
            }
        }
    }

    if files.is_empty() {
        return Ok(None);
    }

    let (server_names, _) = build_server_names(&files, &outputs, inputs)?;

    let source_paths: Vec<String> = files
        .iter()
        .map(|f| {
            std::fs::canonicalize(f)
                .map_err(CliError::Io)
                .map(|p| p.to_string_lossy().to_string())
        })
        .collect::<Result<_, _>>()?;
    let output_paths: Vec<String> = outputs
        .iter()
        .map(|f| {
            let parent = f.parent().ok_or_else(|| {
                CliError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("output path has no parent directory: {}", f.display()),
                ))
            })?;
            std::fs::create_dir_all(parent).map_err(CliError::Io)?;
            let canonical_parent = std::fs::canonicalize(parent).map_err(CliError::Io)?;
            let file_name = f.file_name().ok_or_else(|| {
                CliError::Io(std::io::Error::new(
                    std::io::ErrorKind::InvalidData,
                    format!("output path has no filename: {}", f.display()),
                ))
            })?;
            Ok(canonical_parent
                .join(file_name)
                .to_string_lossy()
                .to_string())
        })
        .collect::<Result<_, CliError>>()?;

    let base_dir = infer_base_dir(inputs)?;
    let (mapping_key, mapping_subdir) = detect_media_mapping(&base_dir, media_mapping_keys)?;

    let mut opts = options.cloned().unwrap_or_else(|| {
        CommandOptions::Morphotag(crate::options::MorphotagOptions {
            common: Default::default(),

            ..Default::default()
        })
    });
    inject_lexicon(&mut opts, lexicon)?;
    let debug_traces = opts.common().debug_dir.is_some();

    let effective_out = out_dir
        .map(PathBuf::from)
        .unwrap_or_else(|| base_dir.clone());

    let before_paths = if let Some(before_arg) = before {
        let before_path = Path::new(before_arg);
        if before_path.is_dir() {
            let mut matches = Vec::new();
            for src in &files {
                let src_path = Path::new(src);
                let Some(filename) = src_path.file_name() else {
                    return Err(CliError::Io(std::io::Error::new(
                        std::io::ErrorKind::InvalidData,
                        format!("input path has no filename: {}", src_path.display()),
                    )));
                };
                let candidate = before_path.join(filename);
                if candidate.exists() {
                    matches.push(
                        std::fs::canonicalize(&candidate)
                            .map_err(CliError::Io)?
                            .to_string_lossy()
                            .to_string(),
                    );
                }
            }
            matches
        } else if before_path.is_file() && files.len() == 1 {
            std::fs::canonicalize(before_path)
                .map_err(CliError::Io)
                .map(|p| vec![p.to_string_lossy().to_string()])?
        } else {
            eprintln!("warning: --before path is not a valid file or directory, ignoring");
            Vec::new()
        }
    } else {
        Vec::new()
    };

    Ok(Some(PreparedPathsSubmission {
        submission: JobSubmission {
            command,
            lang: LanguageSpec::try_from(lang)
                .map_err(|e| CliError::InvalidArgument(format!("invalid language: {e}")))?,
            num_speakers: num_speakers.into(),
            files: vec![],
            media_files: vec![],
            media_mapping: mapping_key.into(),
            media_subdir: mapping_subdir.into(),
            source_dir: base_dir.to_string_lossy().to_string().into(),
            options: opts,
            paths_mode: true,
            source_paths: source_paths.into_iter().map(Into::into).collect(),
            output_paths: output_paths.into_iter().map(Into::into).collect(),
            display_names: server_names,
            debug_traces,
            before_paths: before_paths.into_iter().map(Into::into).collect(),
        },
        effective_out,
        total_files: files.len(),
    }))
}

fn detect_media_mapping(
    in_dir: &Path,
    mapping_keys: &[String],
) -> Result<(String, String), CliError> {
    if mapping_keys.is_empty() {
        return Ok((String::new(), String::new()));
    }

    let abs = std::fs::canonicalize(in_dir).map_err(CliError::Io)?;
    let parts: Vec<String> = abs
        .components()
        .map(|c| c.as_os_str().to_string_lossy().to_string())
        .collect();

    for key in mapping_keys {
        if let Some(idx) = parts.iter().position(|p| p == key) {
            let subdir = if idx + 1 < parts.len() {
                parts[idx + 1..].join("/")
            } else {
                String::new()
            };
            return Ok((key.clone(), subdir));
        }
    }

    Ok((String::new(), String::new()))
}

#[cfg(test)]
mod tests {
    use super::{detect_media_mapping, prepare_paths_submission};
    use crate::ReleasedCommand;

    #[test]
    fn detect_media_mapping_keeps_local_subdir() {
        let root = tempfile::tempdir().unwrap();
        let dir = root
            .path()
            .join("slabank-data/French/Newcastle/Discussion/12");
        std::fs::create_dir_all(&dir).unwrap();

        let (key, subdir) = detect_media_mapping(&dir, &["slabank-data".to_string()]).unwrap();
        assert_eq!(key, "slabank-data");
        assert_eq!(subdir, "French/Newcastle/Discussion/12");
    }

    #[test]
    fn paths_submission_carries_detected_media_mapping() {
        let root = tempfile::tempdir().unwrap();
        let input_dir = root
            .path()
            .join("slabank-data/French/Newcastle/Discussion/12");
        let output_dir = root.path().join("out");
        std::fs::create_dir_all(&input_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        std::fs::write(input_dir.join("d01oma12a.cha"), "@Begin\n@End\n").unwrap();

        let prepared = prepare_paths_submission(
            ReleasedCommand::Align,
            "eng",
            1,
            &["cha"],
            std::slice::from_ref(&input_dir),
            Some(output_dir.as_path()),
            None,
            None,
            None,
            &["slabank-data".to_string()],
        )
        .unwrap()
        .expect("expected one prepared submission");

        assert_eq!(prepared.submission.media_mapping.as_str(), "slabank-data");
        assert_eq!(
            prepared.submission.media_subdir.as_str(),
            "French/Newcastle/Discussion/12"
        );
    }
}
