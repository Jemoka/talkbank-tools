//! Shared roundtrip testing logic.
//!
//! The roundtrip test verifies serialization idempotency:
//!   1. Parse original → serialize → text_A
//!   2. Parse text_A → serialize → text_B
//!   3. If text_A == text_B, the roundtrip passes
//!
//! This approach is robust to tier materialization: since both passes go
//! through the same pipeline, any normalization is transparent.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ErrorCollector;
use talkbank_model::{ChatFile, WriteChat};

use super::worker::ParserDispatch;

/// Result of a roundtrip test on a single file.
#[derive(Debug)]
pub struct RoundtripResult {
    /// Whether the roundtrip passed (serialization is idempotent).
    pub passed: bool,
    /// Human-readable failure reason, if any.
    pub failure_reason: Option<String>,
    /// Text diff of first few differing lines, if any.
    pub diff: Option<String>,
}

/// Run roundtrip test: serialize → re-parse → serialize → compare.
///
/// Assumes validation already passed (caller checks for real errors first).
/// The `chat_file` is the already-parsed result from validation.
pub(super) fn run_roundtrip(chat_file: &ChatFile, parser: &ParserDispatch) -> RoundtripResult {
    // Pass 1: serialize the already-parsed ChatFile
    let mut serialized_a = String::new();
    if let Err(err) = chat_file.write_chat(&mut serialized_a) {
        return RoundtripResult {
            passed: false,
            failure_reason: Some(format!("Serialization failed (pass 1): {}", err)),
            diff: None,
        };
    }

    // Pass 2: re-parse the serialized output (parse-only, skip validation —
    // roundtrip checks serialization fidelity, not content validity)
    let reparse_sink = ErrorCollector::new();
    let reparsed = parser.parse_chat_file_streaming(&serialized_a, &reparse_sink);

    // Serialize again (pass 2 output)
    let mut serialized_b = String::new();
    if let Err(err) = reparsed.write_chat(&mut serialized_b) {
        return RoundtripResult {
            passed: false,
            failure_reason: Some(format!("Serialization failed (pass 2): {}", err)),
            diff: None,
        };
    }

    // Compare: is serialization idempotent?
    if serialized_a != serialized_b {
        let diff = build_text_diff(&serialized_a, &serialized_b);
        RoundtripResult {
            passed: false,
            failure_reason: Some("Roundtrip mismatch (serialization not idempotent)".to_string()),
            diff: Some(diff),
        }
    } else {
        RoundtripResult {
            passed: true,
            failure_reason: None,
            diff: None,
        }
    }
}

/// Build a human-readable text diff showing the first few differences
/// between two strings, line by line.
pub fn build_text_diff(text_a: &str, text_b: &str) -> String {
    let lines_a: Vec<&str> = text_a.lines().collect();
    let lines_b: Vec<&str> = text_b.lines().collect();
    let mut diffs = Vec::new();
    let max_diffs = 5;

    let max_len = lines_a.len().max(lines_b.len());
    for i in 0..max_len {
        if diffs.len() >= max_diffs {
            diffs.push(format!(
                "  ... and more (total lines: pass1={}, pass2={})",
                lines_a.len(),
                lines_b.len()
            ));
            break;
        }
        let line_a = lines_a.get(i).copied().unwrap_or("<missing>");
        let line_b = lines_b.get(i).copied().unwrap_or("<missing>");
        if line_a != line_b {
            diffs.push(format!(
                "  line {}:
    pass1: {}
    pass2: {}",
                i + 1,
                line_a,
                line_b
            ));
        }
    }

    if diffs.is_empty() {
        "no text differences found (possible trailing newline difference)".to_string()
    } else {
        diffs.join("\n")
    }
}
