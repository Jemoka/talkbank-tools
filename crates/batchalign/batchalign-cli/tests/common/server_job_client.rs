use super::poll_job_done;
use batchalign::api::{
    FilePayload, JobInfo, JobResultResponse, JobStatus, JobSubmission, LanguageSpec, NumSpeakers,
    ReleasedCommand,
};
use batchalign::options::CommandOptions;
use std::path::PathBuf;

/// Small test-side helper for the HTTP submission/poll/results loop.
///
/// This keeps server-path tests focused on behavior instead of repeating the
/// transport boilerplate for every command family.
pub struct LiveServerJobClient<'a> {
    client: &'a reqwest::Client,
    base_url: &'a str,
}

impl<'a> LiveServerJobClient<'a> {
    pub fn new(client: &'a reqwest::Client, base_url: &'a str) -> Self {
        Self { client, base_url }
    }

    pub fn from_session(session: &'a super::LiveServerSession) -> Self {
        Self::new(session.client(), session.base_url())
    }

    pub async fn job_info(&self, job_id: &str) -> JobInfo {
        let resp = self
            .client
            .get(format!("{}/jobs/{job_id}", self.base_url))
            .send()
            .await
            .expect("job info request failed");
        assert_eq!(resp.status(), 200, "job info request should succeed");
        resp.json::<JobInfo>().await.expect("job info parse failed")
    }

    pub async fn job_results(&self, job_id: &str) -> JobResultResponse {
        let resp = self
            .client
            .get(format!("{}/jobs/{job_id}/results", self.base_url))
            .send()
            .await
            .expect("job results request failed");
        assert_eq!(resp.status(), 200, "job results request should succeed");
        resp.json::<JobResultResponse>()
            .await
            .expect("job results parse failed")
    }

    pub async fn submit_content_job(
        &self,
        command: ReleasedCommand,
        lang: LanguageSpec,
        files: Vec<FilePayload>,
        options: CommandOptions,
    ) -> JobInfo {
        let submission = JobSubmission {
            command,
            lang,
            num_speakers: NumSpeakers(1),
            files,
            media_files: vec![],
            media_mapping: Default::default(),
            media_subdir: Default::default(),
            source_dir: Default::default(),
            options,
            paths_mode: false,
            source_paths: vec![],
            output_paths: vec![],
            display_names: vec![],
            debug_traces: false,
            before_paths: vec![],
        };

        let resp = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .json(&submission)
            .send()
            .await
            .expect("content job submission failed");
        assert_eq!(resp.status(), 200, "content job submission should succeed");
        resp.json::<JobInfo>()
            .await
            .expect("initial job info parse failed")
    }

    pub async fn submit_paths_job(
        &self,
        command: ReleasedCommand,
        lang: &str,
        source_paths: Vec<String>,
        output_paths: Vec<String>,
        options: CommandOptions,
    ) -> (JobInfo, Vec<String>) {
        self.submit_paths_job_with_before(
            command,
            lang,
            source_paths,
            output_paths,
            vec![],
            options,
        )
        .await
    }

    pub async fn submit_paths_job_with_before(
        &self,
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

        let resp = self
            .client
            .post(format!("{}/jobs", self.base_url))
            .json(&submission)
            .send()
            .await
            .expect("paths job submission failed");
        if resp.status() != 200 {
            let status = resp.status();
            let body = resp.text().await.unwrap_or_default();
            panic!("paths job submission should succeed; status={status}; body={body}");
        }
        let info = resp
            .json::<JobInfo>()
            .await
            .expect("initial paths job info parse failed");

        let final_info = self.poll_done(&info.job_id).await;
        let outputs: Vec<String> = if final_info.status == JobStatus::Completed {
            source_paths
                .iter()
                .zip(output_paths.iter())
                .map(|(source_path, output_path)| {
                    let expected_path =
                        expected_paths_mode_result_path(command, source_path, output_path);
                    std::fs::read_to_string(&expected_path).unwrap_or_else(|_| {
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
                            "Failed to read expected server output file {} for source {} and requested output {}; nearby outputs: {:?}",
                            expected_path.display(),
                            source_path,
                            output_path,
                            nearby_outputs
                        )
                    })
                })
                .collect()
        } else {
            vec![String::new(); output_paths.len()]
        };

        (final_info, outputs)
    }

    pub async fn post_json(&self, path: &str, body: &serde_json::Value) -> reqwest::Response {
        self.client
            .post(format!("{}{}", self.base_url, path))
            .json(body)
            .send()
            .await
            .expect("raw JSON request failed")
    }

    pub async fn poll_done(&self, job_id: &str) -> JobInfo {
        poll_job_done(self.client, self.base_url, job_id).await
    }
}

fn expected_paths_mode_result_path(
    command: ReleasedCommand,
    source_path: &str,
    output_path: &str,
) -> PathBuf {
    let source = PathBuf::from(source_path);
    let source_name = source.file_name().unwrap_or_else(|| {
        panic!("source path has no filename for paths-mode output derivation: {source_path}")
    });

    let requested_output = PathBuf::from(output_path);
    let expected_name = expected_result_filename(command, &source_name.to_string_lossy());

    requested_output
        .parent()
        .map(|dir| dir.join(&expected_name))
        .unwrap_or_else(|| expected_name.into())
}

fn expected_result_filename(command: ReleasedCommand, source_name: &str) -> String {
    let source = PathBuf::from(source_name);
    let stem = source
        .file_stem()
        .unwrap_or_else(|| {
            panic!("source filename has no stem for result derivation: {source_name}")
        })
        .to_string_lossy();
    match command {
        ReleasedCommand::Transcribe
        | ReleasedCommand::TranscribeS
        | ReleasedCommand::Align
        | ReleasedCommand::Benchmark => format!("{stem}.cha"),
        ReleasedCommand::Opensmile => format!("{stem}.opensmile.csv"),
        ReleasedCommand::Avqi => {
            let stem = stem.strip_suffix(".cs").unwrap_or(&stem);
            format!("{stem}.avqi.txt")
        }
        ReleasedCommand::Compare => format!("{stem}.compare.csv"),
        _ => source_name.to_string(),
    }
}
