//! Typed `workspace/executeCommand` protocol shared by the LSP backend.
//!
//! This module centralizes the server-side command names advertised during
//! `initialize` and the decoding logic that turns raw JSON argument vectors into
//! typed Rust request structs. Keeping those concerns in one place reduces drift
//! between capability advertisement, request dispatch, and individual feature
//! handlers.

use std::path::PathBuf;

use serde::Deserialize;
use serde_json::Value;
use tower_lsp::lsp_types::{ExecuteCommandParams, Position, Url};

use super::LspBackendError;
use super::contracts::AnalyzeCommandPayload;
use super::execute_command_args::{
    expect_string_argument, parse_json_argument, parse_position_argument, parse_uri_argument,
    parse_uri_string,
};

pub(crate) type AnalyzeRequest = AnalyzeCommandPayload;

#[cfg(test)]
pub(crate) type AnalysisOptionsRequest = super::contracts::AnalysisOptionsPayload;

/// One execute-command identifier supported by the TalkBank language server.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecuteCommandName {
    /// Generate a dependency graph for the utterance at a cursor position.
    ShowDependencyGraph,
    /// Produce the alignment-sidecar payload for one document.
    GetAlignmentSidecar,
    /// Run one CLAN-style analysis command.
    Analyze,
    /// Discover KidEval normative databases in a CLAN library directory.
    KidevalDatabases,
    /// Discover Eval or Eval-D normative databases in a CLAN library directory.
    EvalDatabases,
    /// Extract `@ID` participant entries from a document.
    GetParticipants,
    /// Format one `@ID` line from field values.
    FormatIdLine,
    /// Extract declared speaker metadata from `@Participants`.
    GetSpeakers,
    /// Filter a document down to selected speakers.
    FilterDocument,
    /// Return utterance metadata used by coder mode.
    GetUtterances,
    /// Format one timing bullet insertion.
    FormatBulletLine,
    /// Execute semantically scoped search in one document.
    ScopedFind,
}

impl ExecuteCommandName {
    /// Ordered list of command names advertised during server initialization.
    pub(crate) const ALL: [Self; 12] = [
        Self::ShowDependencyGraph,
        Self::GetAlignmentSidecar,
        Self::Analyze,
        Self::KidevalDatabases,
        Self::EvalDatabases,
        Self::GetParticipants,
        Self::FormatIdLine,
        Self::GetSpeakers,
        Self::FilterDocument,
        Self::GetUtterances,
        Self::FormatBulletLine,
        Self::ScopedFind,
    ];

    /// Return the wire-format LSP command identifier.
    pub(crate) const fn as_str(self) -> &'static str {
        match self {
            Self::ShowDependencyGraph => "talkbank/showDependencyGraph",
            Self::GetAlignmentSidecar => "talkbank/getAlignmentSidecar",
            Self::Analyze => "talkbank/analyze",
            Self::KidevalDatabases => "talkbank/kidevalDatabases",
            Self::EvalDatabases => "talkbank/evalDatabases",
            Self::GetParticipants => "talkbank/getParticipants",
            Self::FormatIdLine => "talkbank/formatIdLine",
            Self::GetSpeakers => "talkbank/getSpeakers",
            Self::FilterDocument => "talkbank/filterDocument",
            Self::GetUtterances => "talkbank/getUtterances",
            Self::FormatBulletLine => "talkbank/formatBulletLine",
            Self::ScopedFind => "talkbank/scopedFind",
        }
    }

    /// Parse one wire-format command identifier into the corresponding enum.
    pub(crate) fn parse(name: &str) -> Result<Self, LspBackendError> {
        match name {
            "talkbank/showDependencyGraph" => Ok(Self::ShowDependencyGraph),
            "talkbank/getAlignmentSidecar" => Ok(Self::GetAlignmentSidecar),
            "talkbank/analyze" => Ok(Self::Analyze),
            "talkbank/kidevalDatabases" => Ok(Self::KidevalDatabases),
            "talkbank/evalDatabases" => Ok(Self::EvalDatabases),
            "talkbank/getParticipants" => Ok(Self::GetParticipants),
            "talkbank/formatIdLine" => Ok(Self::FormatIdLine),
            "talkbank/getSpeakers" => Ok(Self::GetSpeakers),
            "talkbank/filterDocument" => Ok(Self::FilterDocument),
            "talkbank/getUtterances" => Ok(Self::GetUtterances),
            "talkbank/formatBulletLine" => Ok(Self::FormatBulletLine),
            "talkbank/scopedFind" => Ok(Self::ScopedFind),
            _ => Err(LspBackendError::UnknownCommand {
                name: name.to_string(),
            }),
        }
    }

    /// Return the command names advertised in `InitializeResult`.
    pub(crate) fn advertised_commands() -> Vec<String> {
        Self::ALL
            .iter()
            .map(|command| command.as_str().to_string())
            .collect()
    }
}

/// Request that targets one document URI.
#[derive(Clone, Debug)]
pub(crate) struct DocumentUriRequest {
    /// Document URI supplied over LSP.
    pub(crate) uri: Url,
}

impl DocumentUriRequest {
    /// Decode the standard single-URI execute-command argument layout.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        Ok(Self {
            uri: parse_uri_argument(arguments, 0, "document URI")?,
        })
    }
}

/// Request that targets one document URI and cursor position.
#[derive(Clone, Debug)]
pub(crate) struct DocumentPositionRequest {
    /// Document URI supplied over LSP.
    pub(crate) uri: Url,
    /// Cursor position to resolve within the document.
    pub(crate) position: Position,
}

impl DocumentPositionRequest {
    /// Decode the standard URI-plus-position execute-command argument layout.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        Ok(Self {
            uri: parse_uri_argument(arguments, 0, "document URI")?,
            position: parse_position_argument(arguments.get(1)),
        })
    }
}

/// Request for database discovery commands.
#[derive(Clone, Debug)]
pub(crate) struct DiscoverDatabasesRequest {
    /// Filesystem directory that contains CLAN database files.
    pub(crate) library_dir: PathBuf,
}

impl DiscoverDatabasesRequest {
    /// Decode the single-path argument layout used by database discovery.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        Ok(Self {
            library_dir: PathBuf::from(expect_string_argument(arguments, 0, "library directory")?),
        })
    }
}

/// Plain field values used to format one `@ID` header line.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct IdLineFieldsRequest {
    /// Language code field.
    pub(crate) language: String,
    /// Corpus name field.
    pub(crate) corpus: String,
    /// Speaker code field.
    pub(crate) speaker: String,
    /// Age field.
    pub(crate) age: String,
    /// Sex field.
    pub(crate) sex: String,
    /// Group field.
    pub(crate) group: String,
    /// SES field.
    pub(crate) ses: String,
    /// Role field.
    pub(crate) role: String,
    /// Education field.
    pub(crate) education: String,
    /// Custom field.
    pub(crate) custom: String,
}

impl IdLineFieldsRequest {
    /// Decode the object payload used by `talkbank/formatIdLine`.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        parse_json_argument(arguments, 0, "fields")
    }
}

/// Request that filters a document by speaker selection.
#[derive(Clone, Debug)]
pub(crate) struct FilterDocumentRequest {
    /// Document URI to filter.
    pub(crate) uri: Url,
    /// Speaker codes to retain in the output.
    pub(crate) speakers: Vec<String>,
}

/// Raw JSON payload used for document filtering before URI normalization.
#[derive(Debug, Deserialize)]
struct FilterDocumentPayload {
    /// Document URI string supplied by the extension.
    uri: String,
    /// Speaker codes selected by the user.
    speakers: Vec<String>,
}

impl FilterDocumentRequest {
    /// Decode the object payload used by `talkbank/filterDocument`.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        let payload: FilterDocumentPayload = parse_json_argument(arguments, 0, "filter input")?;
        Ok(Self {
            uri: parse_uri_string(&payload.uri, "URI")?,
            speakers: payload.speakers,
        })
    }
}

/// Request for semantic scoped-find.
#[derive(Clone, Debug)]
pub(crate) struct ScopedFindRequest {
    /// Document URI to search.
    pub(crate) uri: Url,
    /// Plain-text query or regex source.
    pub(crate) query: String,
    /// Scope name such as `main`, `all`, or one dependent tier.
    pub(crate) scope: String,
    /// Optional speaker filters.
    pub(crate) speakers: Vec<String>,
    /// Whether the query should be treated as a regex.
    pub(crate) regex: bool,
}

/// Raw JSON payload used for scoped-find before URI normalization.
#[derive(Clone, Debug, Deserialize)]
struct ScopedFindPayload {
    /// Document URI string supplied by the extension.
    uri: String,
    /// Plain-text query or regex source.
    query: String,
    /// Scope name such as `main`, `all`, or one dependent tier.
    scope: String,
    /// Optional speaker filters.
    #[serde(default)]
    speakers: Vec<String>,
    /// Whether the query should be treated as a regex.
    #[serde(default)]
    regex: bool,
}

impl ScopedFindRequest {
    /// Decode the object payload used by `talkbank/scopedFind`.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        let payload: ScopedFindPayload = parse_json_argument(arguments, 0, "search input")?;
        Ok(Self {
            uri: parse_uri_string(&payload.uri, "URI")?,
            query: payload.query,
            scope: payload.scope,
            speakers: payload.speakers,
            regex: payload.regex,
        })
    }
}

/// Request for server-side bullet formatting.
#[derive(Clone, Debug, Deserialize)]
pub(crate) struct FormatBulletLineRequest {
    /// Previous segment timestamp in milliseconds.
    pub(crate) prev_ms: u64,
    /// Current segment timestamp in milliseconds.
    pub(crate) current_ms: u64,
    /// Speaker code to scaffold on the next line.
    pub(crate) speaker: String,
}

impl FormatBulletLineRequest {
    /// Decode the object payload used by `talkbank/formatBulletLine`.
    fn from_arguments(arguments: &[Value]) -> Result<Self, LspBackendError> {
        parse_json_argument(arguments, 0, "bullet input")
    }
}

/// Fully decoded execute-command request ready for feature dispatch.
#[derive(Clone, Debug)]
pub(crate) enum ExecuteCommandRequest {
    /// Request a dependency graph.
    ShowDependencyGraph(DocumentPositionRequest),
    /// Request an alignment sidecar.
    GetAlignmentSidecar(DocumentUriRequest),
    /// Run one analysis command.
    Analyze(Box<AnalyzeRequest>),
    /// Discover KidEval databases.
    KidevalDatabases(DiscoverDatabasesRequest),
    /// Discover Eval or Eval-D databases.
    EvalDatabases(DiscoverDatabasesRequest),
    /// Fetch participant entries.
    GetParticipants(DocumentUriRequest),
    /// Format one `@ID` line.
    FormatIdLine(IdLineFieldsRequest),
    /// Fetch speaker metadata.
    GetSpeakers(DocumentUriRequest),
    /// Filter a document by speaker.
    FilterDocument(FilterDocumentRequest),
    /// Fetch utterance metadata.
    GetUtterances(DocumentUriRequest),
    /// Format one timing bullet.
    FormatBulletLine(FormatBulletLineRequest),
    /// Execute scoped search.
    ScopedFind(ScopedFindRequest),
}

/// One feature family that owns a subset of execute-command requests.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ExecuteCommandFamily {
    /// Document-local graph and alignment commands.
    Documents,
    /// CLAN analysis and database discovery commands.
    Analysis,
    /// Participant extraction and `@ID` formatting commands.
    Participants,
    /// Speaker, filtering, utterance, bullet, and scoped-find commands.
    ChatOps,
}

impl ExecuteCommandRequest {
    /// Decode `ExecuteCommandParams` into a typed request enum.
    pub(crate) fn parse(params: ExecuteCommandParams) -> Result<Self, LspBackendError> {
        let command = ExecuteCommandName::parse(params.command.as_str())?;

        match command {
            ExecuteCommandName::ShowDependencyGraph => Ok(Self::ShowDependencyGraph(
                DocumentPositionRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::GetAlignmentSidecar => Ok(Self::GetAlignmentSidecar(
                DocumentUriRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::Analyze => Ok(Self::Analyze(Box::new(parse_json_argument(
                &params.arguments,
                0,
                "analyze request",
            )?))),
            ExecuteCommandName::KidevalDatabases => Ok(Self::KidevalDatabases(
                DiscoverDatabasesRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::EvalDatabases => Ok(Self::EvalDatabases(
                DiscoverDatabasesRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::GetParticipants => Ok(Self::GetParticipants(
                DocumentUriRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::FormatIdLine => Ok(Self::FormatIdLine(
                IdLineFieldsRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::GetSpeakers => Ok(Self::GetSpeakers(
                DocumentUriRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::FilterDocument => Ok(Self::FilterDocument(
                FilterDocumentRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::GetUtterances => Ok(Self::GetUtterances(
                DocumentUriRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::FormatBulletLine => Ok(Self::FormatBulletLine(
                FormatBulletLineRequest::from_arguments(&params.arguments)?,
            )),
            ExecuteCommandName::ScopedFind => Ok(Self::ScopedFind(
                ScopedFindRequest::from_arguments(&params.arguments)?,
            )),
        }
    }

    /// Return the feature family that owns this execute-command request.
    pub(crate) const fn family(&self) -> ExecuteCommandFamily {
        match self {
            Self::ShowDependencyGraph(_) | Self::GetAlignmentSidecar(_) => {
                ExecuteCommandFamily::Documents
            }
            Self::Analyze(_) | Self::KidevalDatabases(_) | Self::EvalDatabases(_) => {
                ExecuteCommandFamily::Analysis
            }
            Self::GetParticipants(_) | Self::FormatIdLine(_) => ExecuteCommandFamily::Participants,
            Self::GetSpeakers(_)
            | Self::FilterDocument(_)
            | Self::GetUtterances(_)
            | Self::FormatBulletLine(_)
            | Self::ScopedFind(_) => ExecuteCommandFamily::ChatOps,
        }
    }
}

// Argument-parsing helpers (`expect_string_argument`,
// `parse_uri_argument`, `parse_uri_string`, `parse_json_argument`,
// `parse_position_argument`) moved to `execute_command_args` —
// imported at the top of this file.

#[cfg(test)]
mod tests {
    //! Unit tests for execute-command decoding.

    use serde_json::{from_str, json};
    use clan_core::database::Gender;
    use clan_core::service_types::AnalysisCommandName;
    use tower_lsp::lsp_types::ExecuteCommandParams;

    use super::{ExecuteCommandName, ExecuteCommandRequest, LspBackendError};

    /// The advertised command list should stay in sync with the enum.
    #[test]
    fn advertised_commands_cover_every_variant() {
        let names = ExecuteCommandName::advertised_commands();
        assert_eq!(names.len(), ExecuteCommandName::ALL.len());
        assert!(names.contains(&"talkbank/getSpeakers".to_string()));
        assert!(names.contains(&"talkbank/scopedFind".to_string()));
    }

    /// The Rust execute-command list should stay in sync with the shared manifest used by the
    /// VS Code tests.
    #[test]
    fn advertised_commands_match_shared_contract_manifest() {
        let manifest: Vec<String> = from_str(include_str!(
            "../../../../../apps/vscode-extension/src/test/fixtures/executeCommandContract.json"
        ))
        .expect("shared execute-command contract manifest should parse");

        assert_eq!(ExecuteCommandName::advertised_commands(), manifest);
    }

    /// Decoding a dependency-graph request should default the cursor to line 0.
    #[test]
    fn parse_dependency_graph_defaults_position() {
        let request = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/showDependencyGraph".to_string(),
            arguments: vec![json!("file:///tmp/test.cha")],
            work_done_progress_params: Default::default(),
        })
        .expect("dependency graph request should decode");

        let ExecuteCommandRequest::ShowDependencyGraph(request) = request else {
            panic!("expected dependency-graph request");
        };

        assert_eq!(request.uri.as_str(), "file:///tmp/test.cha");
        assert_eq!(request.position.line, 0);
        assert_eq!(request.position.character, 0);
    }

    /// Decoding scoped-find should normalize the JSON payload into a typed request.
    #[test]
    fn parse_scoped_find_payload() {
        let request = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/scopedFind".to_string(),
            arguments: vec![json!({
                "uri": "file:///tmp/test.cha",
                "query": "hello",
                "scope": "mor",
                "speakers": ["CHI"],
                "regex": true
            })],
            work_done_progress_params: Default::default(),
        })
        .expect("scoped-find request should decode");

        let ExecuteCommandRequest::ScopedFind(request) = request else {
            panic!("expected scoped-find request");
        };

        assert_eq!(request.uri.as_str(), "file:///tmp/test.cha");
        assert_eq!(request.query, "hello");
        assert_eq!(request.scope, "mor");
        assert_eq!(request.speakers, vec!["CHI".to_string()]);
        assert!(request.regex);
    }

    /// Decoding analyze should normalize the JSON options payload into typed fields.
    #[test]
    fn parse_analyze_payload() {
        let request = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/analyze".to_string(),
            arguments: vec![json!({
                "commandName": "kideval",
                "targetUri": "file:///tmp/test.cha",
                "options": {
                    "databasePath": "/tmp/eng_db.cut",
                    "databaseFilter": {
                        "gender": "Female",
                        "age_from_months": 24,
                        "age_to_months": 36
                    },
                    "dssMaxUtterances": 75,
                    "ipsynMaxUtterances": 120
                }
            })],
            work_done_progress_params: Default::default(),
        })
        .expect("analyze request should decode");

        let ExecuteCommandRequest::Analyze(request) = request else {
            panic!("expected analyze request");
        };

        assert_eq!(request.command_name, AnalysisCommandName::Kideval);
        assert_eq!(request.target_uri, "file:///tmp/test.cha");
        assert_eq!(
            request.options.database_path.as_deref(),
            Some(std::path::Path::new("/tmp/eng_db.cut")),
        );
        assert_eq!(request.options.dss_max_utterances, Some(75));
        assert_eq!(request.options.ipsyn_max_utterances, Some(120));

        let filter = request
            .options
            .database_filter
            .expect("database filter should decode");
        assert_eq!(filter.gender, Some(Gender::Female));
        assert_eq!(filter.age_from_months, Some(24));
        assert_eq!(filter.age_to_months, Some(36));
    }

    /// Missing analyze options should default to the empty option object.
    #[test]
    fn parse_analyze_payload_defaults_missing_options() {
        let request = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/analyze".to_string(),
            arguments: vec![json!({
                "commandName": "freq",
                "targetUri": "file:///tmp/test.cha"
            })],
            work_done_progress_params: Default::default(),
        })
        .expect("analyze request should decode");

        let ExecuteCommandRequest::Analyze(request) = request else {
            panic!("expected analyze request");
        };

        assert_eq!(request.command_name, AnalysisCommandName::Freq);
        assert_eq!(request.target_uri, "file:///tmp/test.cha");
        assert_eq!(request.options.keywords, Vec::<String>::new());
        assert_eq!(request.options.limit, None);
    }

    /// Unknown commands should still surface a readable error string.
    #[test]
    fn parse_unknown_command_reports_error() {
        let error = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/notReal".to_string(),
            arguments: vec![],
            work_done_progress_params: Default::default(),
        })
        .expect_err("unknown command should fail");

        assert!(
            matches!(&error, LspBackendError::UnknownCommand { name } if name == "talkbank/notReal"),
            "expected UnknownCommand with name=talkbank/notReal, got {error:?}",
        );
    }

    /// Unknown analysis names should fail at the typed CLAN command boundary.
    #[test]
    fn parse_unknown_analysis_command_reports_error() {
        let error = ExecuteCommandRequest::parse(ExecuteCommandParams {
            command: "talkbank/analyze".to_string(),
            arguments: vec![json!({
                "commandName": "not-real",
                "targetUri": "file:///tmp/test.cha",
                "options": {}
            })],
            work_done_progress_params: Default::default(),
        })
        .expect_err("unknown analysis command should fail");

        // Unknown analysis names surface as a JSON-decode failure of
        // the `AnalysisCommandName` enum; the message lives in the
        // underlying `Display` and is still substring-checked to
        // avoid over-constraining the exact wording.
        let message = error.to_string();
        assert!(
            message.contains("Unknown analysis command: not-real") || message.contains("not-real"),
            "expected unknown-analysis error message, got {message:?}",
        );
    }
}
