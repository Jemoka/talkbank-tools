//! CHECK — CHAT file validation.
//!
//! Validates CHAT files for structural correctness, checking headers, tier
//! formatting, bracket matching, bullet consistency, and more.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                | Rust equivalent                              |
//! |-----------------------------|----------------------------------------------|
//! | `check file.cha`            | `chatter clan check file.cha`                |
//! | `check +c0 file.cha`        | `chatter clan check --bullets 0 file.cha`    |
//! | `check +e file.cha`         | `chatter clan check --list-errors file.cha`  |
//! | `check +e6 file.cha`        | `chatter clan check --error 6 file.cha`      |
//! | `check -e6 file.cha`        | `chatter clan check --exclude-error 6 file.cha` |
//! | `check +g2 file.cha`        | `chatter clan check --check-target file.cha` |
//! | `check +g4 file.cha`        | `chatter clan check --check-id file.cha`     |
//! | `check +g5 file.cha`        | `chatter clan check --check-unused file.cha` |
//! | `check +u file.cha`         | `chatter clan check --check-ud file.cha`     |
//!
//! # Differences from CLAN
//!
//! - **Parsing**: Uses tree-sitter grammar for CHAT parsing, which is more
//!   rigorous and consistent than CLAN's hand-written character-by-character
//!   parser. Many of CHECK's 161 error numbers correspond to parse errors that
//!   our parser catches structurally.
//! - **Error numbering**: CLAN CHECK uses a flat numbering system (1–161).
//!   We map our typed error codes to CHECK's numbers where a clear
//!   correspondence exists. Errors without a CHECK equivalent get number 0.
//! - **Two-pass architecture**: CLAN CHECK uses two passes: `check_OverAll`
//!   (basic structure) and `check_CheckRest` (semantic validation). Our parser
//!   combines both passes into a single streaming parse+validate pipeline.
//! - **depfile.cut**: CLAN CHECK reads `depfile.cut` for tier/code templates.
//!   We validate against the CHAT specification directly without external
//!   template files.
//! - **Output format**: In default mode, we emit CLAN-compatible output:
//!   `*** File "path": line N.` followed by the tier text and error message
//!   with `(N)` error number suffix.
//! - **Bug fixes**: Several CHECK errors in the original are unreachable or
//!   duplicate (e.g., errors 51, 96 are commented out). We skip those.
//! - **`+g1` (prosodic delimiters)**: Currently a no-op — our parser always
//!   recognizes prosodic delimiters.
//! - **`+g3` (word detail checks)**: Partially implemented through our
//!   existing word validation (illegal characters, digits in words).

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::Path;

use serde::Serialize;
use talkbank_model::ParseValidateOptions;
use talkbank_model::{ChatFile, ChatOptionFlag, Header, Line};
use talkbank_model::{ErrorCollector, ParseError};

use crate::framework::CommandOutput;

mod error_map;
pub use error_map::check_error_number;

// ---------------------------------------------------------------------------
// Configuration
// ---------------------------------------------------------------------------

/// Configuration for the CHECK command.
#[derive(Debug, Clone)]
pub struct CheckConfig {
    /// Bullet consistency check level: `None` = skip, `Some(0)` = full,
    /// `Some(1)` = missing bullets only.
    pub bullets: Option<u8>,
    /// Only report errors with these numbers (empty = report all).
    pub include_errors: BTreeSet<u16>,
    /// Exclude errors with these numbers.
    pub exclude_errors: BTreeSet<u16>,
    /// If true, list all error numbers and their messages, then exit.
    pub list_errors: bool,
    /// `+g2`: Check that CHI has Target_Child role.
    pub check_target_child: bool,
    /// `+g4`: Check for missing @ID tiers.
    pub check_missing_id: bool,
    /// `+g5`: Check for unused speakers.
    pub check_unused_speakers: bool,
    /// `+u`: Validate UD features on %mor tier.
    pub check_ud_features: bool,
}

impl Default for CheckConfig {
    fn default() -> Self {
        Self {
            bullets: None,
            include_errors: BTreeSet::new(),
            exclude_errors: BTreeSet::new(),
            list_errors: false,
            check_target_child: false,
            check_missing_id: true, // CLAN default: +g4 is on
            check_unused_speakers: false,
            check_ud_features: false,
        }
    }
}

impl CheckConfig {
    /// Returns true if the given CHECK error number should be reported.
    fn should_report(&self, error_num: u16) -> bool {
        if !self.include_errors.is_empty() {
            return self.include_errors.contains(&error_num);
        }
        if self.exclude_errors.contains(&error_num) {
            return false;
        }
        true
    }
}

// ---------------------------------------------------------------------------
// Result
// ---------------------------------------------------------------------------

/// A single CHECK error in CLAN-compatible format.
#[derive(Debug, Clone, Serialize)]
pub struct CheckError {
    /// CHECK error number (1–161), or 0 for unmapped errors.
    pub error_number: u16,
    /// Line number in the file (1-based), or 0 for file-level errors.
    pub line: usize,
    /// The error message text.
    pub message: String,
    /// The tier/line text where the error occurred (if available).
    pub context: String,
    /// Our internal error code (e.g., "E502").
    pub error_code: String,
}

/// Output of the CHECK command.
#[derive(Debug, Clone, Serialize)]
pub struct CheckResult {
    /// Path to the file that was checked.
    pub file: String,
    /// All errors found (after filtering).
    pub errors: Vec<CheckError>,
    /// True if any errors were found (including filtered ones).
    pub has_errors: bool,
}

impl CommandOutput for CheckResult {
    fn render_text(&self) -> String {
        let mut out = String::new();
        if self.errors.is_empty() && !self.has_errors {
            return out;
        }
        for err in &self.errors {
            let _ = writeln!(out, "*** File \"{}\": line {}.", self.file, err.line);
            if !err.context.is_empty() {
                let _ = writeln!(out, "{}", err.context);
            }
            if err.error_number > 0 {
                let _ = writeln!(out, "{}({})", err.message, err.error_number);
            } else {
                let _ = writeln!(out, "{} [{}]", err.message, err.error_code);
            }
        }
        out
    }

    fn render_clan(&self) -> String {
        self.render_text()
    }
}

// ---------------------------------------------------------------------------
// Error listing
// ---------------------------------------------------------------------------

/// Returns the full list of CHECK error numbers and their messages.
///
/// This corresponds to `check +e` which lists all error numbers.
pub fn list_all_errors() -> String {
    let mut out = String::new();
    for n in 1..=161u16 {
        let msg = error_map::check_error_message(n);
        let _ = writeln!(out, "{n:3}: {msg}");
    }
    out
}

// ---------------------------------------------------------------------------
// Running CHECK
// ---------------------------------------------------------------------------

/// Run CHECK on a single file, returning the result.
pub fn run_check(path: &Path, content: &str, config: &CheckConfig) -> CheckResult {
    let options = ParseValidateOptions::default().with_alignment();
    let sink = ErrorCollector::new();

    let parse_result = talkbank_transform::parse_and_validate_streaming(content, options, &sink);

    let collected = sink.into_vec();
    let has_errors = !collected.is_empty() || parse_result.is_err();

    let mut errors = convert_errors(content, &collected, config);

    // Refine CHECK numbers using error message content.
    // Our internal error codes are broader than CLAN's CHECK numbers —
    // one ErrorCode may cover multiple CHECK conditions. The message text
    // distinguishes them.
    refine_check_numbers(&mut errors);

    // CLAN's first pass stops at certain fatal structural errors.
    // Mimic that behavior: suppress cascading errors.
    suppress_cascading_errors(&mut errors, content);

    // For CHECK 47 (digits in words), CLAN also emits CHECK 38
    // ("numbers should be written out in words") for standalone
    // digit words like "3". Don't emit 38 for digits embedded in
    // words like "hel3lo".
    let digit_errors: Vec<CheckError> = errors
        .iter()
        .filter(|e| {
            if e.error_number != 47 || !e.message.contains("numeric digits") {
                return false;
            }
            if let Some(start) = e.message.find('"')
                && let Some(end) = e.message[start + 1..].find('"')
            {
                let word = &e.message[start + 1..start + 1 + end];
                return word.chars().all(|c| c.is_ascii_digit());
            }
            false
        })
        .map(|e| CheckError {
            error_number: 38,
            line: e.line,
            message: "Numbers should be written out in words.".to_string(),
            context: e.context.clone(),
            error_code: e.error_code.clone(),
        })
        .collect();
    errors.extend(digit_errors);

    // Additional checks that require the parsed AST
    if let Ok(file) = &parse_result {
        // Extract filename stem for media matching (CHECK 157)
        let file_stem = path.file_stem().and_then(|s| s.to_str()).unwrap_or("");
        run_additional_checks(file, config, file_stem, &mut errors);
    }

    CheckResult {
        file: path.display().to_string(),
        errors,
        has_errors,
    }
}

/// Convert our ParseErrors to CHECK-compatible errors, applying filtering.
fn convert_errors(content: &str, errors: &[ParseError], config: &CheckConfig) -> Vec<CheckError> {
    let lines: Vec<&str> = content.lines().collect();

    errors
        .iter()
        .filter_map(|err| {
            let error_num = check_error_number(&err.code);
            if !config.should_report(error_num) {
                return None;
            }

            // Use line from SourceLocation if available, else compute from span
            let line = err
                .location
                .line
                .unwrap_or_else(|| byte_offset_to_line(content, err.location.span.start as usize));

            let context = if line > 0 && line <= lines.len() {
                lines[line - 1].to_string()
            } else {
                String::new()
            };

            Some(CheckError {
                error_number: error_num,
                line,
                message: err.message.clone(),
                context,
                error_code: err.code.as_str().to_string(),
            })
        })
        .collect()
}

/// Suppress cascading errors that result from a single root cause.
///
/// CLAN CHECK's two-pass architecture naturally suppresses cascading errors
/// because pass 1 failures prevent pass 2 from running. Our single-pass
/// architecture reports everything, so we need to manually suppress
/// cascading errors to match CLAN's output.
fn suppress_cascading_errors(errors: &mut Vec<CheckError>, content: &str) {
    // Compute presence flags upfront to avoid borrow conflicts
    let has_7 = errors.iter().any(|e| e.error_number == 7);
    let has_6 = errors.iter().any(|e| e.error_number == 6);
    let has_16 = errors.iter().any(|e| e.error_number == 16);
    let has_44 = errors.iter().any(|e| e.error_number == 44);
    let has_53 = errors.iter().any(|e| e.error_number == 53);
    let has_60 = errors.iter().any(|e| e.error_number == 60);
    let has_69 = errors.iter().any(|e| e.error_number == 69);
    let has_143 = errors.iter().any(|e| e.error_number == 143);
    let _ = has_7; // suppress unused warning

    // (16) extended chars in speaker → suppress all cascading errors
    if has_16 {
        errors.retain(|e| e.error_number == 16);
        return;
    }

    // (53) duplicate @Begin → suppress cascading structural errors
    if has_53 {
        errors.retain(|e| e.error_number == 53);
        return;
    }

    // (69) missing UTF8: suppress all others IF the file truly lacks @UTF8.
    if has_69 {
        let text = content.trim_start_matches('\u{FEFF}');
        if !text.starts_with("@UTF8") {
            // Genuine missing @UTF8 — CLAN stops here
            errors.retain(|e| e.error_number == 69);
            return;
        }
        // Spurious (69) — remove it
        errors.retain(|e| e.error_number != 69);
    }

    // (6) missing @Begin: suppress cascading parse failures
    if has_6 {
        errors.retain(|e| matches!(e.error_number, 6 | 69));
        return;
    }

    // (44) content after @End → suppress spurious (7) "missing @End"
    if has_44 {
        errors.retain(|e| e.error_number != 7);
    }

    // (143) malformed @ID → suppress (60) "missing @ID"
    if has_143 {
        errors.retain(|e| e.error_number != 60);
    }

    // (60) missing @ID → suppress (18) "speaker not defined"
    if has_60 && !has_143 {
        errors.retain(|e| e.error_number != 18);
    }
}

/// Refine CHECK error numbers using message content.
///
/// Our internal error codes are broader than CLAN's CHECK numbering — one
/// `ErrorCode` may cover multiple CHECK conditions. This function inspects
/// the error message text to assign the most precise CHECK number.
fn refine_check_numbers(errors: &mut [CheckError]) {
    for err in errors.iter_mut() {
        let msg = err.message.to_lowercase();

        // E305 (MissingTerminator) → 21 covers three CHECK cases:
        // 21: "Utterance delimiter expected" (missing terminator)
        // 36: "Utterance delimiter must be at the end" (text after delimiter)
        // 50: "Redundant utterance delimiter"
        if err.error_number == 21 {
            if msg.contains("redundant") {
                err.error_number = 50;
            } else if msg.contains("after") && msg.contains("delimiter") {
                err.error_number = 36;
            }
        }

        // E303 (SyntaxError) → 8, but CHECK 4 is "space instead of TAB"
        if err.error_number == 8 && msg.contains("space") && msg.contains("tab") {
            err.error_number = 4;
        }

        // E525 (UnknownHeader) → 17, but CHECK 2 is "missing colon" for headers without ':'
        if err.error_number == 17 && msg.contains("@page") {
            err.error_number = 2; // @Page without colon → "missing colon"
        }

        // E522/E523 (SpeakerNotDefined/OrphanIDHeader) → 18, but also covers:
        // 60: "@ID tier is missing" (when no @ID for a speaker)
        if err.error_number == 18 && msg.contains("@id") {
            err.error_number = 60;
        }

        // E519 (InvalidLanguageCode) → 121, but also covers:
        // 122: "Language on @ID not defined on @Languages"
        if err.error_number == 121 && msg.contains("@id") && msg.contains("@languages") {
            err.error_number = 122;
        }

        // E532 (InvalidParticipantRole) → 15, but also covers:
        // 142: "Role on @ID differs from @Participants"
        if err.error_number == 15 && msg.contains("@id") && msg.contains("@participants") {
            err.error_number = 142;
        }

        // E375 (ContentAnnotationParseError) → 48, but also covers:
        // 22: "Unmatched [" (unclosed bracket)
        // 161: "Space required before [" (missing space)
        if err.error_number == 48 {
            if msg.contains("space") && msg.contains("before") && msg.contains("bracket") {
                err.error_number = 161;
            } else if msg.contains("unmatched") || msg.contains("unclosed") {
                err.error_number = 22;
            } else if msg.contains("control character") {
                err.error_number = 48; // keep as-is, correct
            }
        }

        // E304 (MissingSpeaker) → 12, but sometimes it's really 21 (missing terminator)
        // when the parser interprets a missing terminator as a speaker issue
        if err.error_number == 12 && msg.contains("terminator") {
            err.error_number = 21;
        }

        // E501 (DuplicateHeader) → 44: when we detect "content after @End",
        // also suppress the spurious "Missing @End" (7) that follows
        // (the duplicate @End consumed the real one). Mark for removal.
        // Handled below after the loop.

        // E315 (InvalidControlCharacter) → 86, but CHECK 48 is "illegal character"
        if err.error_number == 86 && msg.contains("control character") {
            err.error_number = 48;
        }

        // E220 (IllegalDigits) → 47, but CLAN also emits 38 ("numbers should be
        // written out in words") for standalone digit words. We handle this by
        // duplicating the error with both numbers in post-processing below.

        // Speaker with non-ASCII characters → CHECK 16
        if msg.contains("non-ascii") && msg.contains("speaker") {
            err.error_number = 16;
        }

        // DuplicateHeader → 44 for "content after @End", but 53 for "duplicate @Begin"
        if err.error_number == 44 && msg.contains("@begin") {
            err.error_number = 53;
        }

        // Suppress (143) when followed by (60) for same speaker — @ID parse fail + no @ID
        // Handled in post-processing below
    }
}

/// Additional checks that run on the parsed AST (not covered by parse+validate).
fn run_additional_checks(
    file: &ChatFile,
    config: &CheckConfig,
    file_stem: &str,
    errors: &mut Vec<CheckError>,
) {
    if config.check_target_child {
        check_target_child(file, config, errors);
    }
    if config.check_unused_speakers {
        check_unused_speakers(file, config, errors);
    }

    // CHECK 13: Duplicate speaker declaration
    check_duplicate_speakers(file, config, errors);

    // CHECK 157: Media filename must match data filename
    if !file_stem.is_empty() {
        check_media_filename(file, config, file_stem, errors);
    }
}

/// Check that CHI has Target_Child role (+g2).
fn check_target_child(file: &ChatFile, config: &CheckConfig, errors: &mut Vec<CheckError>) {
    if !config.should_report(68) {
        return;
    }

    let has_chi_target = file
        .participants
        .iter()
        .any(|(code, p)| code.as_str() == "CHI" && p.role.as_str() == "Target_Child");

    // Check for @Options: notarget
    let notarget = file.lines.iter().any(|line| {
        if let Line::Header { header, .. } = line
            && let Header::Options { options } = header.as_ref()
        {
            return options
                .iter()
                .any(|o| matches!(o, ChatOptionFlag::Unsupported(s) if s == "notarget"));
        }
        false
    });

    if !notarget && !has_chi_target {
        errors.push(CheckError {
            error_number: 68,
            line: 0,
            message: "PARTICIPANTS TIER IS MISSING \"CHI Target_Child\".".to_string(),
            context: String::new(),
            error_code: String::new(),
        });
    }
}

/// Check for speakers declared in @Participants but never used (+g5).
fn check_unused_speakers(file: &ChatFile, config: &CheckConfig, errors: &mut Vec<CheckError>) {
    let declared: BTreeSet<&str> = file.participants.keys().map(|code| code.as_str()).collect();

    let mut used: BTreeSet<&str> = BTreeSet::new();
    for line in file.lines.iter() {
        if let Line::Utterance(utt) = line {
            used.insert(utt.main.speaker.as_str());
        }
    }

    for speaker in declared.difference(&used) {
        if !config.should_report(0) {
            continue;
        }
        errors.push(CheckError {
            error_number: 0,
            line: 3, // CLAN reports this at line 3 (participants line)
            message: format!("Speaker \"{speaker}\" is not used in this file."),
            context: String::new(),
            error_code: String::new(),
        });
    }
}

/// Check for duplicate speaker codes in @Participants (CHECK 13).
fn check_duplicate_speakers(file: &ChatFile, config: &CheckConfig, errors: &mut Vec<CheckError>) {
    if !config.should_report(13) {
        return;
    }

    // Check @Participants header for duplicate speaker codes.
    // The parsed ChatFile deduplicates, so we check the raw Participants entries.
    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Participants { entries } = header.as_ref()
        {
            let mut seen: BTreeSet<&str> = BTreeSet::new();
            for entry in entries.iter() {
                let code = entry.speaker_code.as_str();
                if !code.is_empty() && !seen.insert(code) {
                    errors.push(CheckError {
                        error_number: 13,
                        line: 0,
                        message: format!("Duplicate speaker declaration for '{code}'."),
                        context: String::new(),
                        error_code: String::new(),
                    });
                }
            }
        }
    }

    // Also check for duplicate @ID tiers for the same speaker
    let mut seen_id_speakers: BTreeSet<&str> = BTreeSet::new();
    for id in file.id_headers() {
        let speaker = id.speaker.as_str();
        if !speaker.is_empty() && !seen_id_speakers.insert(speaker) {
            errors.push(CheckError {
                error_number: 13,
                line: 0,
                message: format!("Duplicate @ID declaration for speaker '{speaker}'."),
                context: String::new(),
                error_code: String::new(),
            });
        }
    }
}

/// Check that @Media filename matches the data filename (CHECK 157).
fn check_media_filename(
    file: &ChatFile,
    config: &CheckConfig,
    file_stem: &str,
    errors: &mut Vec<CheckError>,
) {
    if !config.should_report(157) {
        return;
    }

    for line in file.lines.iter() {
        if let Line::Header { header, .. } = line
            && let Header::Media(media) = header.as_ref()
        {
            let media_name = media.filename.as_str();
            if !media_name.eq_ignore_ascii_case(file_stem) {
                errors.push(CheckError {
                    error_number: 157,
                    line: 0,
                    message: format!(
                        "Media file name '{}' does not match data file name '{}'.",
                        media_name, file_stem
                    ),
                    context: String::new(),
                    error_code: String::new(),
                });
            }
            break; // Only check first @Media
        }
    }
}

/// Convert a byte offset to a 1-based line number.
fn byte_offset_to_line(content: &str, offset: usize) -> usize {
    content[..offset.min(content.len())]
        .bytes()
        .filter(|&b| b == b'\n')
        .count()
        + 1
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_list_all_errors() {
        let listing = list_all_errors();
        assert!(listing.contains("  6:"));
        assert!(listing.contains("  7:"));
        assert!(listing.contains("161:"));
        assert_eq!(listing.lines().count(), 161);
    }

    #[test]
    fn test_config_error_filtering() {
        let mut config = CheckConfig::default();
        assert!(config.should_report(6));
        assert!(config.should_report(7));

        config.include_errors.insert(6);
        assert!(config.should_report(6));
        assert!(!config.should_report(7));

        let mut config2 = CheckConfig::default();
        config2.exclude_errors.insert(6);
        assert!(!config2.should_report(6));
        assert!(config2.should_report(7));
    }

    #[test]
    fn test_byte_offset_to_line() {
        let content = "line1\nline2\nline3\n";
        assert_eq!(byte_offset_to_line(content, 0), 1);
        assert_eq!(byte_offset_to_line(content, 5), 1);
        assert_eq!(byte_offset_to_line(content, 6), 2);
        assert_eq!(byte_offset_to_line(content, 12), 3);
    }

    #[test]
    fn test_check_valid_file() {
        let content = "\u{FEFF}@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Target_Child\n\
            @ID:\teng|test|CHI|2;00.||||Target_Child|||\n\
            *CHI:\tdog .\n@End\n";
        let config = CheckConfig::default();
        let result = run_check(Path::new("test.cha"), content, &config);
        assert!(
            result.errors.is_empty(),
            "Expected no errors, got: {:?}",
            result.errors
        );
    }

    #[test]
    fn test_check_missing_end() {
        let content = "\u{FEFF}@UTF8\n@Begin\n@Languages:\teng\n\
            @Participants:\tCHI Target_Child\n\
            @ID:\teng|test|CHI|2;00.||||Target_Child|||\n\
            *CHI:\tdog .\n";
        let config = CheckConfig::default();
        let result = run_check(Path::new("test.cha"), content, &config);
        assert!(result.has_errors);
        assert!(
            result.errors.iter().any(|e| e.error_number == 7),
            "Expected error 7 (missing @End), got: {:?}",
            result.errors
        );
    }
}
