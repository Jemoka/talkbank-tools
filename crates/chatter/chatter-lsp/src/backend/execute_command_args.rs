//! Argument parsers for `workspace/executeCommand` request decoding.
//!
//! These helpers take raw `serde_json::Value` lists — as the LSP
//! client supplies them — and lift each position into a typed value
//! (`String`, [`Url`], deserialized struct, or [`Position`]). Factored
//! out of `execute_commands.rs` so the top-level request-dispatch
//! file stays focused on command identifiers and request-shape
//! definitions; these parsers have no state beyond their arguments
//! and are reused by several request decoders.

use serde::de::DeserializeOwned;
use serde_json::Value;
use tower_lsp::lsp_types::{Position, Url};

use crate::backend::error::LspBackendError;

/// Parse a required string argument at one position.
pub(super) fn expect_string_argument(
    arguments: &[Value],
    index: usize,
    label: &'static str,
) -> Result<String, LspBackendError> {
    arguments
        .get(index)
        .and_then(Value::as_str)
        .map(str::to_string)
        .ok_or(LspBackendError::ArgumentMissing { label })
}

/// Parse a required URI argument at one position.
pub(super) fn parse_uri_argument(
    arguments: &[Value],
    index: usize,
    label: &'static str,
) -> Result<Url, LspBackendError> {
    let uri = expect_string_argument(arguments, index, label)?;
    parse_uri_string(&uri, label)
}

/// Parse a URI string value into a typed [`Url`].
pub(super) fn parse_uri_string(uri: &str, label: &'static str) -> Result<Url, LspBackendError> {
    Url::parse(uri).map_err(LspBackendError::invalid_uri_parse(label))
}

/// Parse a required JSON object argument into one typed payload.
///
/// `serde_json::Error` is wrapped explicitly into
/// [`LspBackendError::ArgumentInvalid`] rather than routed through
/// `JsonSerializeFailed`, so user-input failures stay classified as
/// user-facing.
pub(super) fn parse_json_argument<T: DeserializeOwned>(
    arguments: &[Value],
    index: usize,
    label: &'static str,
) -> Result<T, LspBackendError> {
    let value = arguments
        .get(index)
        .ok_or(LspBackendError::ArgumentMissing { label })?;
    serde_json::from_value(value.clone()).map_err(|error| LspBackendError::ArgumentInvalid {
        label,
        reason: error.to_string(),
    })
}

/// Parse an optional position argument, defaulting to the start of the document.
pub(super) fn parse_position_argument(argument: Option<&Value>) -> Position {
    if let Some(Value::Object(object)) = argument
        && let (Some(Value::Number(line)), Some(Value::Number(character))) =
            (object.get("line"), object.get("character"))
    {
        return Position {
            line: line.as_u64().unwrap_or(0) as u32,
            character: character.as_u64().unwrap_or(0) as u32,
        };
    }

    Position {
        line: 0,
        character: 0,
    }
}
