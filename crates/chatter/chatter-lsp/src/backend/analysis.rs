//! CLAN analysis integration for the LSP.
//!
//! Handles `talkbank/analyze` execute-command requests by running analysis
//! commands from `clan_core` and returning JSON results.
//!
//! This module is intentionally an adapter, not a command-construction hub.
//! `clan-core` now owns the reusable `AnalysisCommandName`,
//! `AnalysisRequestBuilder`, and `AnalysisService` boundaries, so the LSP only
//! translates execute-command payloads into typed library inputs.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use serde_json::Value;
use clan_core::database;
use clan_core::framework::DiscoveredChatFiles;
use clan_core::service::AnalysisService;
use clan_core::service_types::{AnalysisOptions, AnalysisPlan, AnalysisRequestBuilder};
use tower_lsp::jsonrpc::Result as LspResult;
use tower_lsp::lsp_types::Url;

use super::LspBackendError;
use super::execute_commands::{AnalyzeRequest, DiscoverDatabasesRequest, ExecuteCommandRequest};

/// Feature-oriented execute-command service for CLAN analysis requests.
pub(crate) struct AnalysisCommandService;

impl AnalysisCommandService {
    /// Dispatch one analysis-family execute-command request.
    pub(crate) fn dispatch(&self, request: ExecuteCommandRequest) -> LspResult<Option<Value>> {
        // Routing invariant: `ExecuteCommandRoutingService::dispatch`
        // (in `requests/execute_command.rs`) calls `request.family()`
        // and routes to the analysis service only for variants whose
        // family is `ExecuteCommandFamily::Analysis`. The wildcard arm
        // is therefore unreachable by construction. Follow-up:
        // partition `ExecuteCommandRequest` into typed sub-enums per
        // family so the compiler proves exhaustiveness — see
        // `book/src/operations/panic-audit/chatter-lsp.md`.
        #[allow(clippy::unreachable)]
        match request {
            ExecuteCommandRequest::Analyze(request) => {
                command_response(handle_analyze(&request), "Analysis error")
            }
            ExecuteCommandRequest::KidevalDatabases(request)
            | ExecuteCommandRequest::EvalDatabases(request) => command_response(
                handle_discover_databases(&request),
                "Database discovery error",
            ),
            _ => unreachable!("analysis service received unsupported execute-command request"),
        }
    }
}

fn command_response(
    result: Result<Value, LspBackendError>,
    prefix: &str,
) -> LspResult<Option<Value>> {
    match result {
        Ok(json) => Ok(Some(json)),
        Err(error) => Ok(Some(Value::String(format!("{prefix}: {error}")))),
    }
}

/// Handle a `talkbank/analyze` execute-command request.
///
/// Returns JSON output from the analysis command.
pub(crate) fn handle_analyze(request: &AnalyzeRequest) -> Result<Value, LspBackendError> {
    let target_uri =
        Url::parse(&request.target_uri).map_err(LspBackendError::invalid_uri_parse("file URI"))?;
    let file_path = target_uri
        .to_file_path()
        .map_err(LspBackendError::uri_not_file_path("file URI"))?;

    let options = build_analysis_options(request)?;
    let plan = AnalysisRequestBuilder::new(request.command_name, options)
        .build()
        .map_err(
            |error: clan_core::service_types::AnalysisServiceError| {
                LspBackendError::ExternalServiceFailed {
                    service: "Analysis plan build",
                    reason: error.to_string(),
                }
            },
        )?;
    let service = AnalysisService::new();

    match plan {
        AnalysisPlan::Service(analysis_request) => {
            let discovered_files = DiscoveredChatFiles::from_path(&file_path);
            if discovered_files.is_empty() {
                return Err(LspBackendError::ExternalServiceFailed {
                    service: "Analysis",
                    reason: "No .cha files found".to_string(),
                });
            }
            let files = discovered_files.into_files();

            service
                .execute_json(analysis_request, &files)
                .map_err(|error| LspBackendError::ExternalServiceFailed {
                    service: "Analysis",
                    reason: error.to_string(),
                })
        }
        AnalysisPlan::Rely(rely_request) => service
            .execute_rely_json(rely_request, &file_path)
            .map_err(|error| LspBackendError::ExternalServiceFailed {
                service: "Analysis (rely)",
                reason: error.to_string(),
            }),
    }
}

/// Translate the LSP's typed execute-command payload into raw library options.
fn build_analysis_options(request: &AnalyzeRequest) -> Result<AnalysisOptions, LspBackendError> {
    let options = &request.options;
    let second_file = options
        .second_file
        .as_deref()
        .map(|uri| {
            let url =
                Url::parse(uri).map_err(LspBackendError::invalid_uri_parse("second file URI"))?;
            url.to_file_path()
                .map_err(LspBackendError::uri_not_file_path("second file URI"))
        })
        .transpose()?;

    Ok(AnalysisOptions {
        mor: options.mor,
        words: options.words,
        main_tier: options.main_tier,
        limit: options.limit.map(clan_core::framework::WordLimit::from),
        keywords: options
            .keywords
            .iter()
            .map(|s| clan_core::framework::KeywordPattern::from(s.as_str()))
            .collect(),
        search: options.search.clone(),
        max_depth: options
            .max_depth
            .map(clan_core::framework::CodeDepth::from),
        tier: options
            .tier
            .as_deref()
            .map(clan_core::framework::TierKind::from),
        threshold: options
            .threshold
            .map(clan_core::framework::FrequencyThreshold::from),
        max_utterances: options
            .max_utterances
            .map(clan_core::framework::UtteranceLimit::from),
        database_path: options.database_path.clone(),
        database_filter: options.database_filter.clone().map(Into::into),
        syllable_mode: options.syllable_mode,
        dss_max_utterances: options
            .dss_max_utterances
            .map(clan_core::framework::UtteranceLimit::from),
        ipsyn_max_utterances: options
            .ipsyn_max_utterances
            .map(clan_core::framework::UtteranceLimit::from),
        rules_path: None,
        dss_rules_path: None,
        ipsyn_rules_path: None,
        script_path: options.script_path.clone(),
        second_file,
        template_path: options.template_path.clone(),
        min_utterances: options
            .min_utterances
            .map(clan_core::framework::UtteranceLimit::from),
        tier1: options
            .tier1
            .as_deref()
            .map(clan_core::framework::TierKind::from),
        tier2: options
            .tier2
            .as_deref()
            .map(clan_core::framework::TierKind::from),
        sort_by_frequency: options.sort_by_frequency,
    })
}

/// Handle a `talkbank/kidevalDatabases` request.
///
/// Returns JSON array of available databases.
pub(crate) fn handle_discover_databases(
    request: &DiscoverDatabasesRequest,
) -> Result<Value, LspBackendError> {
    let databases = database::discover_databases(&request.library_dir).map_err(|e| {
        LspBackendError::ExternalServiceFailed {
            service: "Database discovery",
            reason: e.to_string(),
        }
    })?;

    Ok(serde_json::to_value(&databases)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    use crate::backend::execute_commands::{AnalysisOptionsRequest, AnalyzeRequest};
    use clan_core::service_types::AnalysisCommandName;

    #[test]
    fn build_analysis_options_converts_second_file_uri() {
        let secondary = Url::from_file_path("/tmp/secondary.cha").expect("secondary file URL");
        let request = AnalyzeRequest {
            command_name: AnalysisCommandName::Rely,
            target_uri: "file:///tmp/primary.cha".to_owned(),
            options: AnalysisOptionsRequest {
                second_file: Some(secondary.to_string()),
                ..AnalysisOptionsRequest::default()
            },
        };

        let options = build_analysis_options(&request).expect("options should build");

        assert_eq!(
            options.second_file,
            Some(PathBuf::from("/tmp/secondary.cha"))
        );
    }

    #[test]
    fn build_analysis_options_rejects_non_file_second_uri() {
        let request = AnalyzeRequest {
            command_name: AnalysisCommandName::Rely,
            target_uri: "file:///tmp/primary.cha".to_owned(),
            options: AnalysisOptionsRequest {
                second_file: Some("https://example.com/secondary.cha".to_owned()),
                ..AnalysisOptionsRequest::default()
            },
        };

        let error = build_analysis_options(&request).expect_err("non-file URI should fail");
        assert!(
            matches!(
                &error,
                LspBackendError::UriNotFilePath {
                    label: "second file URI",
                },
            ),
            "expected UriNotFilePath {{ label: 'second file URI' }}, got {error:?}",
        );
    }
}
