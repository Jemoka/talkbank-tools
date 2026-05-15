//! Rust-side adapters for batched text worker-protocol V2 results.
//!
//! The text-task orchestrators still own cache policy, postprocessing, and CHAT
//! mutation in Rust. This module keeps the worker boundary typed by parsing the
//! batched `execute_v2` result variants before task-local orchestration logic
//! reconstructs established Rust domains.

use crate::types::worker_v2::{
    CorefResultV2, ExecuteOutcomeV2, ExecuteResponseV2, MorphosyntaxResultV2, TaskResultV2,
    TranslationResultV2, UtsegResultV2,
};

/// Parse one V2 morphosyntax execute response into the typed batch result.
pub fn parse_morphosyntax_result_v2(
    response: &ExecuteResponseV2,
) -> Result<&MorphosyntaxResultV2, String> {
    match require_success_result(response, "morphosyntax")? {
        TaskResultV2::MorphosyntaxResult(result) => Ok(result),
        _ => Err("worker protocol V2 morphosyntax response returned the wrong result type".into()),
    }
}

/// Parse one V2 utterance-segmentation execute response into the typed batch
/// result.
pub fn parse_utseg_result_v2(response: &ExecuteResponseV2) -> Result<&UtsegResultV2, String> {
    match require_success_result(response, "utseg")? {
        TaskResultV2::UtsegResult(result) => Ok(result),
        _ => Err("worker protocol V2 utseg response returned the wrong result type".into()),
    }
}

/// Parse one V2 translation execute response into the typed batch result.
pub fn parse_translate_result_v2(
    response: &ExecuteResponseV2,
) -> Result<&TranslationResultV2, String> {
    match require_success_result(response, "translate")? {
        TaskResultV2::TranslationResult(result) => Ok(result),
        _ => Err("worker protocol V2 translate response returned the wrong result type".into()),
    }
}

/// Parse one V2 coreference execute response into the typed batch result.
pub fn parse_coref_result_v2(response: &ExecuteResponseV2) -> Result<&CorefResultV2, String> {
    match require_success_result(response, "coref")? {
        TaskResultV2::CorefResult(result) => Ok(result),
        _ => Err("worker protocol V2 coref response returned the wrong result type".into()),
    }
}

/// Require that one execute response succeeded and produced a result payload.
fn require_success_result<'a>(
    response: &'a ExecuteResponseV2,
    task: &str,
) -> Result<&'a TaskResultV2, String> {
    match &response.outcome {
        ExecuteOutcomeV2::Success => {}
        ExecuteOutcomeV2::Error { code, message } => {
            return Err(format!(
                "worker protocol V2 {task} request failed with {code:?}: {message}"
            ));
        }
    }

    response
        .result
        .as_ref()
        .ok_or_else(|| format!("worker protocol V2 {task} response was missing a result payload"))
}
