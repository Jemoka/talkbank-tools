//! Async validation runtime orchestration for integration with async runtimes.
//!
//! Validation is inherently CPU-bound (parsing, tree traversal, checks), not I/O-bound.
//! These entry points allow running validation on async runtimes like Tokio by:
//! - Offloading to blocking thread pools (tokio::task::spawn_blocking)
//! - Reusing async-compatible error sinks from the core error subsystem
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//!
//! # Example with tokio
//!
//! ```ignore
//! use talkbank_model::{ChatFile, NotValidated};
//! use talkbank_model::validation::validate_async;
//! use talkbank_model::ErrorCollector;
//!
//! #[tokio::main]
//! async fn main() {
//!     let file: ChatFile<NotValidated> = parse_file(content);
//!     let errors = ErrorCollector::new();
//!
//!     // Run validation on blocking thread pool
//!     let _validated = validate_async(file, errors.clone(), None).await;
//!
//!     let error_vec = errors.into_vec();
//!     println!("Found {} errors", error_vec.len());
//! }
//! ```

use crate::ErrorSink;
use crate::model::ChatFile;
use crate::validation::{NotValidated, ValidationConfig};

/// Async wrapper errors emitted by validation task orchestration.
#[cfg(feature = "async")]
#[derive(Debug, thiserror::Error)]
pub enum AsyncValidationError {
    /// The Tokio blocking task failed to join successfully.
    #[error("failed to join async validation task: {0}")]
    Join(#[from] tokio::task::JoinError),
}

/// Validate a `ChatFile` on Tokio's blocking pool and await completion.
///
/// Since validation is CPU-bound, this runs the validation on a blocking thread
/// to avoid blocking the async runtime's event loop.
///
/// # Parameters
///
/// * `file` - The ChatFile to validate (consumed, returns Validated)
/// * `errors` - Error sink for collecting validation errors (must be Send + Sync + Clone)
/// * `filename` - Optional filename for media validation
///
/// # Returns
///
/// A `ChatFile<Validated>` after validation completes
///
/// # Example
///
/// ```ignore
/// use talkbank_model::validation::validate_async;
/// use talkbank_model::ErrorCollector;
///
/// let errors = ErrorCollector::new();
/// let validated = validate_async(file, errors.clone(), Some("myfile")).await?;
/// ```
#[cfg(feature = "async")]
pub async fn validate_async<S>(
    file: ChatFile<NotValidated>,
    errors: S,
    filename: Option<String>,
) -> Result<ChatFile<crate::validation::Validated>, AsyncValidationError>
where
    S: ErrorSink + Send + 'static,
{
    tokio::task::spawn_blocking(move || file.validate_into(&errors, filename.as_deref()))
        .await
        .map_err(AsyncValidationError::from)
}

/// Validate a `ChatFile` asynchronously with a custom `ValidationConfig`.
///
/// This variant mirrors `validate_async` but applies per-run severity/enablement
/// overrides before executing checks.
///
/// # Parameters
///
/// * `file` - The ChatFile to validate (cloned for async task)
/// * `config` - Validation configuration (severity overrides, disabled errors)
/// * `errors` - Error sink for collecting validation errors (must be Send + 'static)
/// * `filename` - Optional filename for media validation
///
/// # Example
///
/// ```ignore
/// use talkbank_model::validation::{validate_with_config_async, ValidationConfig};
/// use talkbank_model::{ErrorCollector, ErrorCode, Severity};
///
/// let config = ValidationConfig::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);
///
/// let errors = ErrorCollector::new();
/// validate_with_config_async(file.clone(), config, errors.clone(), Some("myfile".to_string())).await?;
/// ```
#[cfg(feature = "async")]
pub async fn validate_with_config_async<S>(
    file: ChatFile<NotValidated>,
    config: ValidationConfig,
    errors: S,
    filename: Option<String>,
) -> Result<(), AsyncValidationError>
where
    S: ErrorSink + Send + 'static,
{
    tokio::task::spawn_blocking(move || {
        file.validate_with_config(config, &errors, filename.as_deref())
    })
    .await
    .map_err(AsyncValidationError::from)
}

#[cfg(all(test, feature = "async"))]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::Span;
    use crate::model::{Header, Line};

    /// Builds a minimal syntactically valid test file.
    fn make_test_file() -> ChatFile<NotValidated> {
        ChatFile::new(vec![
            Line::header_with_span(Header::Utf8, Span::DUMMY),
            Line::header_with_span(Header::Begin, Span::DUMMY),
            Line::header_with_span(Header::End, Span::DUMMY),
        ])
    }

    /// Ensures async validation completes and returns a validated file.
    #[tokio::test]
    async fn test_validate_async() -> Result<(), AsyncValidationError> {
        let file = make_test_file();
        let errors = ErrorCollector::new();

        let _validated = validate_async(file, errors, None).await?;

        // Should complete without panicking
        Ok(())
    }

    /// Verifies validate with config async.
    #[tokio::test]
    async fn test_validate_with_config_async() -> Result<(), AsyncValidationError> {
        let file = make_test_file();
        let config = ValidationConfig::new();
        let errors = ErrorCollector::new();

        validate_with_config_async(file, config, errors, None).await?;

        // Should complete without panicking
        Ok(())
    }
}
