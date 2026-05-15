use std::path::Path;

use batchalign::api::{FilePayload, FileResult, JobInfo, ReleasedCommand};
use batchalign::options::CommandOptions;

/// Small test-side helper for direct-session submissions.
///
/// This keeps direct-mode tests focused on behavior and fixture setup rather
/// than on which submission helper they need to call.
pub struct LiveDirectJobClient<'a> {
    session: &'a super::LiveDirectSession,
}

impl<'a> LiveDirectJobClient<'a> {
    pub fn new(session: &'a super::LiveDirectSession) -> Self {
        Self { session }
    }

    pub fn state_dir(&self) -> &Path {
        self.session.state_dir()
    }

    pub async fn submit_content_job(
        &self,
        command: ReleasedCommand,
        lang: &str,
        files: Vec<FilePayload>,
        options: CommandOptions,
    ) -> (JobInfo, Vec<FileResult>) {
        super::submit_and_complete_direct(self.session, command, lang, files, options).await
    }

    pub async fn submit_paths_job(
        &self,
        command: ReleasedCommand,
        lang: &str,
        source_paths: Vec<String>,
        output_paths: Vec<String>,
        options: CommandOptions,
    ) -> (JobInfo, Vec<String>) {
        super::submit_paths_and_complete_direct(
            self.session,
            command,
            lang,
            source_paths,
            output_paths,
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
        super::submit_paths_with_before_and_complete_direct(
            self.session,
            command,
            lang,
            source_paths,
            output_paths,
            before_paths,
            options,
        )
        .await
    }
}
