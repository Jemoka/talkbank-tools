use crate::common::{
    LiveDirectSession, require_live_direct_warmed, require_live_direct_warmed_many,
};
use batchalign::api::{FilePayload, FileResult, JobInfo, ReleasedCommand};
use batchalign::chat_ops::{ChatFile, DependentTier};
use batchalign::options::CommandOptions;
use batchalign::worker::InferTask;
use std::sync::atomic::{AtomicU64, Ordering};
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

static LIVE_NAME_COUNTER: AtomicU64 = AtomicU64::new(0);

pub(crate) struct DirectGoldenSession {
    session: LiveDirectSession,
}

impl DirectGoldenSession {
    pub(crate) async fn submit_content_job(
        &self,
        command: ReleasedCommand,
        lang: &str,
        filename: &str,
        content: &str,
        options: CommandOptions,
    ) -> (JobInfo, Vec<FileResult>) {
        crate::common::submit_and_complete_direct(
            &self.session,
            command,
            lang,
            vec![FilePayload {
                filename: filename.into(),
                content: content.into(),
            }],
            options,
        )
        .await
    }

    pub(crate) async fn submit_files_job(
        &self,
        command: ReleasedCommand,
        lang: &str,
        files: Vec<FilePayload>,
        options: CommandOptions,
    ) -> (JobInfo, Vec<FileResult>) {
        crate::common::submit_and_complete_direct(&self.session, command, lang, files, options)
            .await
    }
}

pub(crate) async fn require_direct_session_warmed(
    task: InferTask,
    command: ReleasedCommand,
    lang: &str,
    skip_message: &str,
) -> Option<DirectGoldenSession> {
    let session = require_live_direct_warmed(task, command, lang, skip_message).await?;
    Some(DirectGoldenSession { session })
}

pub(crate) async fn require_direct_session_warmed_many(
    task: InferTask,
    warmups: Vec<(ReleasedCommand, &str)>,
    skip_message: &str,
) -> Option<DirectGoldenSession> {
    let session = require_live_direct_warmed_many(task, warmups, skip_message).await?;
    Some(DirectGoldenSession { session })
}

pub(crate) fn unique_test_dir(prefix: &str) -> String {
    let counter = LIVE_NAME_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("{prefix}_{counter}")
}

pub(crate) fn parse_output(chat: &str, label: &str) -> ChatFile {
    let parser = TreeSitterParser::new().unwrap();
    let (file, errors) = parse_lenient(&parser, chat);
    assert!(errors.is_empty(), "{label}: CHAT parse errors: {errors:?}");
    file
}

pub(crate) fn has_mor_tier(file: &ChatFile) -> bool {
    file.lines.iter().any(|line| {
        if let batchalign::chat_ops::Line::Utterance(utt) = line {
            utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Mor(_)))
        } else {
            false
        }
    })
}

pub(crate) fn has_gra_tier(file: &ChatFile) -> bool {
    file.lines.iter().any(|line| {
        if let batchalign::chat_ops::Line::Utterance(utt) = line {
            utt.dependent_tiers
                .iter()
                .any(|t| matches!(t, DependentTier::Gra(_)))
        } else {
            false
        }
    })
}

pub(crate) fn has_user_defined_tier(file: &ChatFile, label: &str) -> bool {
    file.lines.iter().any(|line| {
        if let batchalign::chat_ops::Line::Utterance(utt) = line {
            utt.dependent_tiers.iter().any(|t| match t {
                DependentTier::UserDefined(ud) => ud.label.as_ref() == label,
                _ => false,
            })
        } else {
            false
        }
    })
}

pub(crate) fn find_mor_line_for(chat: &str, at_s_text: &str) -> Option<String> {
    let lines: Vec<&str> = chat.lines().collect();
    for (i, line) in lines.iter().enumerate() {
        if line.contains(at_s_text)
            && line.starts_with('*')
            && i + 1 < lines.len()
            && lines[i + 1].starts_with("%mor:")
        {
            return Some(lines[i + 1].trim_start_matches("%mor:\t").to_string());
        }
    }
    None
}

macro_rules! assert_golden_snapshot {
    ($name:expr, $value:expr) => {
        insta::with_settings!({snapshot_path => "../snapshots"}, {
            insta::assert_snapshot!($name, $value);
        });
    };
}

pub(crate) use assert_golden_snapshot;
