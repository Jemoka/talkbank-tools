//! Generic helpers for Rust-owned text commands that can run either per-file or
//! as one cross-file batch.
//!
//! This keeps the command-facing request/output shape consistent across commands
//! like `utseg`, `translate`, and `coref` while still allowing each command to
//! keep its own orchestration internals.

use std::marker::PhantomData;

use async_trait::async_trait;

use crate::api::{ChatText, DisplayPath, LanguageCode3};
use crate::error::ServerError;

/// Owned serialized CHAT text produced by a text workflow.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OwnedChatText(String);

impl OwnedChatText {
    /// Wrap one owned CHAT string.
    pub(crate) fn new(text: String) -> Self {
        Self(text)
    }

    /// Consume into the underlying `String`.
    pub(crate) fn into_string(self) -> String {
        self.0
    }
}

impl From<String> for OwnedChatText {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl std::fmt::Display for OwnedChatText {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::ops::Deref for OwnedChatText {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl AsRef<str> for OwnedChatText {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

/// Per-file error emitted by a text workflow after file identity is already known.
#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
#[error("{message}")]
pub(crate) struct TextWorkflowFileError {
    message: String,
}

impl TextWorkflowFileError {
    /// Construct one file-scoped workflow error from a message.
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    /// Consume into the legacy message form used by older runner code.
    pub(crate) fn into_message(self) -> String {
        self.message
    }
}

impl From<String> for TextWorkflowFileError {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl From<&str> for TextWorkflowFileError {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

/// Named per-file outcome for one text workflow batch.
#[derive(Debug, Clone)]
pub(crate) struct TextBatchFileResult {
    /// Stable file identity for this output or error.
    pub filename: DisplayPath,
    /// File-local workflow outcome.
    pub result: Result<OwnedChatText, TextWorkflowFileError>,
}

/// Cross-file outputs for one text workflow family.
pub(crate) type TextBatchFileResults = Vec<TextBatchFileResult>;

impl TextBatchFileResult {
    /// Construct one successful named file result.
    pub(crate) fn ok(filename: impl Into<DisplayPath>, text: impl Into<OwnedChatText>) -> Self {
        Self {
            filename: filename.into(),
            result: Ok(text.into()),
        }
    }

    /// Construct one failed named file result.
    pub(crate) fn err(
        filename: impl Into<DisplayPath>,
        error: impl Into<TextWorkflowFileError>,
    ) -> Self {
        Self {
            filename: filename.into(),
            result: Err(error.into()),
        }
    }
}

/// Owned named input for one CHAT file in a batch workflow.
#[derive(Debug, Clone)]
pub(crate) struct TextBatchFileInput {
    /// Stable file identity for this input.
    pub filename: DisplayPath,
    /// Owned serialized CHAT document for this file.
    pub chat_text: OwnedChatText,
}

impl TextBatchFileInput {
    /// Construct one named batch input from a filename and CHAT text.
    pub(crate) fn new(
        filename: impl Into<DisplayPath>,
        chat_text: impl Into<OwnedChatText>,
    ) -> Self {
        Self {
            filename: filename.into(),
            chat_text: chat_text.into(),
        }
    }
}

/// Borrowed request bundle for one per-file text workflow execution.
pub(crate) struct TextPerFileWorkflowRequest<'a, Shared, Params> {
    /// CHAT text to process.
    pub chat_text: ChatText<'a>,
    /// Primary language shaping the text workflow.
    pub lang: &'a LanguageCode3,
    /// Shared context owned by the workflow family.
    pub shared: Shared,
    /// Command-specific parameters for this execution.
    pub params: Params,
}

/// Borrowed request bundle for one cross-file text workflow execution.
pub(crate) struct TextBatchWorkflowRequest<'a, Shared, Params> {
    /// Files and their CHAT text payloads.
    pub files: &'a [TextBatchFileInput],
    /// Primary language shaping the text workflow.
    pub lang: &'a LanguageCode3,
    /// Shared context owned by the workflow family.
    pub shared: Shared,
    /// Command-specific parameters shared across the batch.
    pub params: Params,
}

/// Command-specific behavior for a Rust-owned text workflow family.
#[async_trait]
pub(crate) trait TextBatchOperation {
    /// Shared context threaded through this workflow family.
    type Shared<'a>: Send
    where
        Self: 'a;

    /// Command-specific parameters threaded through the workflow.
    type Params<'a>: Send
    where
        Self: 'a;

    /// Run the command for one CHAT file.
    async fn run_single(
        chat_text: ChatText<'_>,
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        params: Self::Params<'_>,
    ) -> Result<String, ServerError>;

    /// Run the command over a batch of CHAT files.
    async fn run_batch(
        files: &[TextBatchFileInput],
        lang: &LanguageCode3,
        shared: Self::Shared<'_>,
        params: Self::Params<'_>,
    ) -> TextBatchFileResults;
}

/// Generic wrapper around one [`TextBatchOperation`] implementation.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TextBatchWorkflow<O>(PhantomData<O>);

impl<O> TextBatchWorkflow<O> {
    /// Construct the zero-sized workflow wrapper.
    pub(crate) const fn new() -> Self {
        Self(PhantomData)
    }
}

impl<O> TextBatchWorkflow<O>
where
    O: TextBatchOperation + Send + Sync + 'static,
{
    /// Run one per-file text workflow.
    pub(crate) async fn run_per_file<'a>(
        &self,
        request: TextPerFileWorkflowRequest<'a, O::Shared<'a>, O::Params<'a>>,
    ) -> Result<String, ServerError> {
        O::run_single(
            request.chat_text,
            request.lang,
            request.shared,
            request.params,
        )
        .await
    }

    /// Run one cross-file text workflow.
    pub(crate) async fn run_batch_files<'a>(
        &self,
        request: TextBatchWorkflowRequest<'a, O::Shared<'a>, O::Params<'a>>,
    ) -> TextBatchFileResults {
        O::run_batch(request.files, request.lang, request.shared, request.params).await
    }
}
