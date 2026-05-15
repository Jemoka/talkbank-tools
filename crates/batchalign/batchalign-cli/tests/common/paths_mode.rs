use std::path::PathBuf;

use batchalign::api::{
    JobInfo, JobStatus, JobSubmission, LanguageSpec, NumSpeakers, ReleasedCommand,
};
use batchalign::options::CommandOptions;

/// Submit a paths-mode job to a live server and return the completed job info
/// plus the content of the output files.
pub async fn submit_paths_and_complete(
    client: &reqwest::Client,
    base_url: &str,
    command: ReleasedCommand,
    lang: &str,
    source_paths: Vec<String>,
    output_paths: Vec<String>,
    options: CommandOptions,
) -> (JobInfo, Vec<String>) {
    assert_eq!(
        source_paths.len(),
        output_paths.len(),
        "source_paths and output_paths must have equal length"
    );

    let submission = JobSubmission {
        command,
        lang: LanguageSpec::try_from(lang)
            .expect("test lang must be a valid ISO 639-3 code or \"auto\""),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: true,
        source_paths: source_paths.iter().map(|s| s.as_str().into()).collect(),
        output_paths: output_paths.iter().map(|s| s.as_str().into()).collect(),
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(
        resp.status(),
        200,
        "Paths-mode job submission should succeed"
    );
    let info: JobInfo = resp.json().await.expect("parse initial JobInfo");

    let final_info = super::poll_job_done(client, base_url, &info.job_id).await;

    if final_info.status != JobStatus::Completed {
        eprintln!(
            "PATHS JOB FAILED: status={:?}, job_id={}",
            final_info.status, final_info.job_id
        );
        if let Ok(resp) = client
            .get(format!("{base_url}/jobs/{}/results", final_info.job_id))
            .send()
            .await
            && let Ok(text) = resp.text().await
        {
            eprintln!("  Results response: {}", &text[..text.len().min(500)]);
        }
    }

    let outputs: Vec<String> = if final_info.status == JobStatus::Completed {
        source_paths
            .iter()
            .zip(output_paths.iter())
            .map(|(source_path, output_path)| {
                let expected_path = expected_paths_mode_result_path(source_path, output_path);

                if let Ok(content) = std::fs::read_to_string(&expected_path) {
                    return content;
                }

                let mut nearby_outputs = Vec::new();
                if let Some(dir) = expected_path.parent()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "cha" || e == "csv") {
                            nearby_outputs.push(path.display().to_string());
                        }
                    }
                }

                panic!(
                    "Failed to read expected output file {} for source {} and requested output {}; nearby outputs: {:?}",
                    expected_path.display(),
                    source_path,
                    output_path,
                    nearby_outputs
                )
            })
            .collect()
    } else {
        vec![String::new(); output_paths.len()]
    };

    (final_info, outputs)
}

/// Submit a paths-mode job to a live direct session and return completed output contents.
pub async fn submit_paths_and_complete_direct(
    session: &super::LiveDirectSession,
    command: ReleasedCommand,
    lang: &str,
    source_paths: Vec<String>,
    output_paths: Vec<String>,
    options: CommandOptions,
) -> (JobInfo, Vec<String>) {
    assert_eq!(
        source_paths.len(),
        output_paths.len(),
        "source_paths and output_paths must have equal length"
    );

    let submission = JobSubmission {
        command,
        lang: LanguageSpec::try_from(lang)
            .expect("test lang must be a valid ISO 639-3 code or \"auto\""),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: true,
        source_paths: source_paths.iter().map(|s| s.as_str().into()).collect(),
        output_paths: output_paths.iter().map(|s| s.as_str().into()).collect(),
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let (info, detail) = session.run_submission(submission).await;

    if info.status != JobStatus::Completed {
        eprintln!(
            "DIRECT PATHS JOB FAILED: status={:?}, job_id={}",
            info.status, info.job_id
        );
        eprintln!("  File results: {}", detail.results.len());
    }

    let outputs: Vec<String> = if info.status == JobStatus::Completed {
        source_paths
            .iter()
            .zip(output_paths.iter())
            .map(|(source_path, output_path)| {
                let expected_path = expected_paths_mode_result_path(source_path, output_path);

                if let Ok(content) = std::fs::read_to_string(&expected_path) {
                    return content;
                }

                let mut nearby_outputs = Vec::new();
                if let Some(dir) = expected_path.parent()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "cha" || e == "csv") {
                            nearby_outputs.push(path.display().to_string());
                        }
                    }
                }

                panic!(
                    "Failed to read expected direct output file {} for source {} and requested output {}; nearby outputs: {:?}",
                    expected_path.display(),
                    source_path,
                    output_path,
                    nearby_outputs
                )
            })
            .collect()
    } else {
        vec![String::new(); output_paths.len()]
    };

    (info, outputs)
}

/// Submit a direct paths-mode job with `before_paths`.
pub async fn submit_paths_with_before_and_complete_direct(
    session: &super::LiveDirectSession,
    command: ReleasedCommand,
    lang: &str,
    source_paths: Vec<String>,
    output_paths: Vec<String>,
    before_paths: Vec<String>,
    options: CommandOptions,
) -> (JobInfo, Vec<String>) {
    assert_eq!(
        source_paths.len(),
        output_paths.len(),
        "source_paths and output_paths must have equal length"
    );
    assert!(
        before_paths.is_empty() || before_paths.len() == source_paths.len(),
        "before_paths must be empty or match source_paths length"
    );

    let submission = JobSubmission {
        command,
        lang: LanguageSpec::try_from(lang)
            .expect("test lang must be a valid ISO 639-3 code or \"auto\""),
        num_speakers: NumSpeakers(1),
        files: vec![],
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: true,
        source_paths: source_paths.iter().map(|s| s.as_str().into()).collect(),
        output_paths: output_paths.iter().map(|s| s.as_str().into()).collect(),
        display_names: vec![],
        debug_traces: false,
        before_paths: before_paths.iter().map(|s| s.as_str().into()).collect(),
    };

    let (info, detail) = session.run_submission(submission).await;

    if info.status != JobStatus::Completed {
        eprintln!(
            "DIRECT PATHS+BEFORE JOB FAILED: status={:?}, job_id={}",
            info.status, info.job_id
        );
        eprintln!("  File results: {}", detail.results.len());
    }

    let outputs: Vec<String> = if info.status == JobStatus::Completed {
        source_paths
            .iter()
            .zip(output_paths.iter())
            .map(|(source_path, output_path)| {
                let expected_path = expected_paths_mode_result_path(source_path, output_path);

                if let Ok(content) = std::fs::read_to_string(&expected_path) {
                    return content;
                }

                let mut nearby_outputs = Vec::new();
                if let Some(dir) = expected_path.parent()
                    && let Ok(entries) = std::fs::read_dir(dir)
                {
                    for entry in entries.flatten() {
                        let path = entry.path();
                        if path.extension().is_some_and(|e| e == "cha" || e == "csv") {
                            nearby_outputs.push(path.display().to_string());
                        }
                    }
                }

                panic!(
                    "Failed to read expected direct output file {} for source {} and requested output {}; nearby outputs: {:?}",
                    expected_path.display(),
                    source_path,
                    output_path,
                    nearby_outputs
                )
            })
            .collect()
    } else {
        vec![String::new(); output_paths.len()]
    };

    (info, outputs)
}

fn expected_paths_mode_result_path(source_path: &str, output_path: &str) -> PathBuf {
    let source = PathBuf::from(source_path);
    let source_stem = source.file_stem().unwrap_or_else(|| {
        panic!("source path has no filename stem for paths-mode output derivation: {source_path}")
    });
    let source_name = source.file_name().unwrap_or_else(|| {
        panic!("source path has no filename for paths-mode output derivation: {source_path}")
    });

    let requested_output = PathBuf::from(output_path);
    let expected_name = match requested_output.extension() {
        Some(ext) => {
            let mut filename = source_stem.to_os_string();
            filename.push(".");
            filename.push(ext);
            filename
        }
        None => source_name.to_os_string(),
    };

    requested_output
        .parent()
        .map(|dir| dir.join(&expected_name))
        .unwrap_or_else(|| expected_name.into())
}

#[cfg(test)]
mod tests {
    use super::expected_paths_mode_result_path;
    use std::path::PathBuf;

    #[test]
    fn expected_paths_mode_result_path_preserves_requested_extension() {
        let expected = expected_paths_mode_result_path(
            "/tmp/input/eng_acr_first13p5.mp3",
            "/tmp/out/test.cha",
        );
        assert_eq!(expected, PathBuf::from("/tmp/out/eng_acr_first13p5.cha"));
    }

    #[test]
    fn expected_paths_mode_result_path_keeps_source_name_without_output_extension() {
        let expected =
            expected_paths_mode_result_path("/tmp/input/eng_acr_first13p5.mp3", "/tmp/out");
        assert_eq!(expected, PathBuf::from("/tmp/eng_acr_first13p5.mp3"));
    }
}
