//! Validate the shared analyze-command fixture against the generated schema.
//!
//! This keeps the extension-side canonical request fixture aligned with the
//! Rust-owned `AnalyzeCommandPayload` contract and the checked-in JSON Schema.

use std::fmt;
use std::path::PathBuf;

use jsonschema::ValidationError as JsonSchemaValidationError;
use serde_json::Value;
use chatter_lsp::backend::contracts::AnalyzeCommandPayload;

/// Checked-in schema path for the `talkbank/analyze` transport contract.
const ANALYZE_COMMAND_SCHEMA_PATH: &str = "schemas/chat-file/analyze-command.schema.json";

/// Shared TypeScript fixture path for the canonical analyze-command payload.
const ANALYZE_COMMAND_FIXTURE_PATH: &str = "apps/vscode-extension/src/test/fixtures/analyzeCommandPayload.json";

/// One schema violation found while validating the shared analyze-command fixture.
#[derive(Clone, Debug, Eq, PartialEq)]
struct AnalyzeCommandFixtureSchemaViolation {
    /// JSON Pointer-like path to the invalid instance location.
    instance_path: String,
    /// JSON Pointer-like path to the schema keyword that failed.
    schema_path: String,
    /// Human-readable validation message from the JSON Schema validator.
    message: String,
}

impl AnalyzeCommandFixtureSchemaViolation {
    /// Convert one borrowed `jsonschema` validation error into an owned domain error.
    fn from_validation_error(error: &JsonSchemaValidationError<'_>) -> Self {
        Self {
            instance_path: error.instance_path().to_string(),
            schema_path: error.schema_path().to_string(),
            message: error.to_string(),
        }
    }
}

impl fmt::Display for AnalyzeCommandFixtureSchemaViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (instance path: {}, schema path: {})",
            self.message, self.instance_path, self.schema_path
        )
    }
}

/// Owned collection of schema violations for the shared analyze-command fixture.
#[derive(Debug)]
struct AnalyzeCommandFixtureSchemaViolations {
    /// Concrete validation failures found while checking the fixture.
    violations: Vec<AnalyzeCommandFixtureSchemaViolation>,
}

impl AnalyzeCommandFixtureSchemaViolations {
    /// Build an owned validation report from one iterator of borrowed schema errors.
    fn from_validation_errors<'a>(
        errors: impl IntoIterator<Item = JsonSchemaValidationError<'a>>,
    ) -> Option<Self> {
        let violations = errors
            .into_iter()
            .map(|error| AnalyzeCommandFixtureSchemaViolation::from_validation_error(&error))
            .collect::<Vec<_>>();

        if violations.is_empty() {
            None
        } else {
            Some(Self { violations })
        }
    }
}

impl fmt::Display for AnalyzeCommandFixtureSchemaViolations {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(
            f,
            "shared analyze-command fixture violates the generated schema with {} error(s):",
            self.violations.len()
        )?;
        for violation in &self.violations {
            writeln!(f, "- {violation}")?;
        }
        Ok(())
    }
}

impl std::error::Error for AnalyzeCommandFixtureSchemaViolations {}

/// Errors surfaced while loading or validating the shared contract fixture.
#[derive(Debug, thiserror::Error)]
enum TestError {
    #[error("Failed to read analyze-command schema file: {path}")]
    ReadAnalyzeCommandSchema {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to parse analyze-command schema JSON: {path}")]
    ParseAnalyzeCommandSchema {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("Failed to read shared analyze-command fixture file: {path}")]
    ReadAnalyzeCommandFixture {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("Failed to parse shared analyze-command fixture JSON: {path}")]
    ParseAnalyzeCommandFixture {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("Failed to compile analyze-command JSON Schema validator")]
    CompileAnalyzeCommandSchemaValidator {
        source: JsonSchemaValidationError<'static>,
    },
    #[error("Shared analyze-command fixture violates the generated schema")]
    ValidateAnalyzeCommandFixtureAgainstSchema {
        source: AnalyzeCommandFixtureSchemaViolations,
    },
    #[error("Shared analyze-command fixture does not deserialize into AnalyzeCommandPayload")]
    DeserializeAnalyzeCommandFixture { source: serde_json::Error },
}

/// Read the generated analyze-command schema from disk.
fn read_analyze_command_schema_json() -> Result<Value, TestError> {
    let path_buf = PathBuf::from(ANALYZE_COMMAND_SCHEMA_PATH);
    let json_text = std::fs::read_to_string(&path_buf).map_err(|source| {
        TestError::ReadAnalyzeCommandSchema {
            path: path_buf.clone(),
            source,
        }
    })?;
    serde_json::from_str(&json_text).map_err(|source| TestError::ParseAnalyzeCommandSchema {
        path: path_buf,
        source,
    })
}

/// Read the shared TypeScript analyze-command fixture from disk.
fn read_analyze_command_fixture_json() -> Result<Value, TestError> {
    let path_buf = PathBuf::from(ANALYZE_COMMAND_FIXTURE_PATH);
    let json_text = std::fs::read_to_string(&path_buf).map_err(|source| {
        TestError::ReadAnalyzeCommandFixture {
            path: path_buf.clone(),
            source,
        }
    })?;
    serde_json::from_str(&json_text).map_err(|source| TestError::ParseAnalyzeCommandFixture {
        path: path_buf.clone(),
        source,
    })
}

/// The shared TypeScript fixture should remain valid JSON Schema instance data.
#[test]
fn analyze_command_fixture_matches_generated_schema() -> Result<(), TestError> {
    let schema = read_analyze_command_schema_json()?;
    let fixture = read_analyze_command_fixture_json()?;

    let validator = jsonschema::validator_for(&schema)
        .map_err(|source| TestError::CompileAnalyzeCommandSchemaValidator { source })?;

    if let Some(source) = AnalyzeCommandFixtureSchemaViolations::from_validation_errors(
        validator.iter_errors(&fixture),
    ) {
        return Err(TestError::ValidateAnalyzeCommandFixtureAgainstSchema { source });
    }

    Ok(())
}

/// The shared TypeScript fixture should also deserialize through the public Rust contract.
#[test]
fn analyze_command_fixture_deserializes_through_public_contract() -> Result<(), TestError> {
    let fixture = read_analyze_command_fixture_json()?;
    let payload: AnalyzeCommandPayload = serde_json::from_value(fixture)
        .map_err(|source| TestError::DeserializeAnalyzeCommandFixture { source })?;

    assert_eq!(payload.command_name.as_str(), "eval-d");
    assert_eq!(payload.target_uri, "file:///tmp/test.cha");
    assert_eq!(payload.options.max_utterances, Some(100));
    assert_eq!(
        payload
            .options
            .database_path
            .as_deref()
            .and_then(std::path::Path::to_str),
        Some("/Users/Shared/CLAN/lib/kideval/eng_toyplay_db.cut"),
    );
    assert_eq!(payload.options.dss_max_utterances, Some(75));
    assert_eq!(payload.options.ipsyn_max_utterances, Some(80));
    assert!(payload.options.sort_by_frequency);

    let filter = payload
        .options
        .database_filter
        .expect("fixture should include a database filter");
    assert_eq!(filter.language.as_deref(), Some("eng"));
    assert_eq!(filter.group.as_deref(), Some("TD"));
    assert_eq!(
        serde_json::to_value(filter.gender).expect("gender should serialize"),
        serde_json::json!("Female"),
    );
    assert_eq!(filter.age_from_months, Some(24));
    assert_eq!(filter.age_to_months, Some(48));
    assert_eq!(
        filter.speaker_codes,
        vec!["CHI".to_string(), "INV".to_string()]
    );

    Ok(())
}
