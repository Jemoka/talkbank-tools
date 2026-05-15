//! Pre-validation and post-validation gates for CHAT files.
//!
//! Each server orchestrator validates the parsed CHAT file to a command's
//! minimum validity level before spending compute. Invalid files are
//! rejected early with diagnostics.
//!
//! # Validity levels (cumulative)
//!
//! | Level | Name | Checks |
//! |-------|------|--------|
//! | L0 | Parseable | No parse errors (clean tree-sitter CST) |
//! | L1 | StructurallyComplete | Participants, languages, speaker codes, terminators |
//! | L2 | MainTierValid | Well-formed words, timing bullets |
//!
//! Each level includes all checks from lower levels.

use talkbank_model::ParseError;
use talkbank_model::model::{ChatFile, ChatOptionFlag, Line};
pub use talkbank_model::{GateValidationError as ValidationError, ValidityLevel};

/// Validate a CHAT file to the specified minimum validity level.
///
/// Returns `Ok(())` if the file meets the level, or `Err` with all
/// failures found (checks all levels up to the specified one).
///
/// `parse_errors` are the structured errors from the parser (typically
/// from [`crate::parse::parse_lenient`]). The L0 gate surfaces the
/// first error's code, source excerpt, and byte span in its message,
/// so end-users can locate and diagnose the problem without reading
/// daemon logs.
pub fn validate_to_level(
    file: &ChatFile,
    parse_errors: &[ParseError],
    level: ValidityLevel,
) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // L0: Parseable — no parse errors
    if let Some(first) = parse_errors.first() {
        errors.push(ValidationError {
            message: format_l0_message(first, parse_errors.len()),
            level: ValidityLevel::Parseable,
        });
    }

    if level >= ValidityLevel::StructurallyComplete {
        check_structurally_complete(file, &mut errors);
    }

    if level >= ValidityLevel::MainTierValid {
        check_main_tier_valid(file, &mut errors);
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Compose the L0 failure message, surfacing provenance from the first
/// `ParseError` so the user sees *what* broke, not just a count.
///
/// Includes the error code, byte span, offending text excerpt (if the
/// parser supplied one), the parser's own message, and a "+N more"
/// summary when multiple parse errors stacked up. Line/column info is
/// included when `ParseError.location.line` is populated; today the
/// streaming tree-sitter parser leaves that None, so the byte span is
/// the primary coordinate. See the parent assessment doc
/// (the private workspace incident report)
/// Fundamental D for the upstream issue.
fn format_l0_message(first: &ParseError, total: usize) -> String {
    let code = first.code.as_str();
    let span = &first.location.span;
    let excerpt = first
        .context
        .as_ref()
        .map(|c| c.source_text.as_str())
        .unwrap_or("");
    let line_hint = first
        .location
        .line
        .map(|l| format!(" line {l}, "))
        .unwrap_or_else(|| format!(" bytes {}..{} ", span.start, span.end));
    let more = if total > 1 {
        format!(" (+{} more)", total - 1)
    } else {
        String::new()
    };
    format!(
        "[{code}]{line_hint}{excerpt:?}: {msg}{more}",
        msg = first.message
    )
}

/// L1 checks: structural completeness.
fn check_structurally_complete(file: &ChatFile, errors: &mut Vec<ValidationError>) {
    // Check @Participants present with at least one participant
    if file.participants.is_empty() {
        errors.push(ValidationError {
            message: "@Participants header missing or has no participants".to_string(),
            level: ValidityLevel::StructurallyComplete,
        });
    }

    // Check @Languages present
    if file.languages.is_empty() {
        errors.push(ValidationError {
            message: "@Languages header missing".to_string(),
            level: ValidityLevel::StructurallyComplete,
        });
    }

    // CA files (Conversation Analysis) can have utterances without terminators —
    // incomplete turns, backchannels, trailing-off speech. Skip the terminator
    // check when @Options: CA is set.
    let is_ca = file.options.iter().any(|f| matches!(f, ChatOptionFlag::Ca));

    // Check every utterance has a terminator (non-CA) and a declared speaker
    for line in &file.lines {
        if let Line::Utterance(utt) = line {
            if !is_ca && utt.main.content.terminator.is_none() {
                let speaker = utt.main.speaker.as_str();
                errors.push(ValidationError {
                    message: format!("Utterance by *{speaker} has no terminator"),
                    level: ValidityLevel::StructurallyComplete,
                });
            }

            // Check speaker is declared in participants
            let speaker_code = utt.main.speaker.as_str();
            let declared = file.participants.keys().any(|k| k.as_str() == speaker_code);
            if !declared {
                errors.push(ValidationError {
                    message: format!("Speaker *{speaker_code} not declared in @Participants"),
                    level: ValidityLevel::StructurallyComplete,
                });
            }
        }
    }
}

/// L2 checks: main tier content validity.
fn check_main_tier_valid(file: &ChatFile, errors: &mut Vec<ValidationError>) {
    for line in &file.lines {
        if let Line::Utterance(utt) = line {
            // Check for empty main tiers (no content at all)
            if utt.main.content.content.is_empty()
                && utt.main.content.linkers.is_empty()
                && utt.main.content.language_code.is_none()
            {
                let speaker = utt.main.speaker.as_str();
                errors.push(ValidationError {
                    message: format!("Utterance by *{speaker} has an empty main tier"),
                    level: ValidityLevel::MainTierValid,
                });
            }
        }
    }
}

/// Post-validation: verify that the output file is at least as valid as the
/// input (no degradation). Returns diagnostics if the command corrupted the file.
pub fn validate_output(file: &ChatFile, command: &str) -> Result<(), Vec<ValidationError>> {
    let mut errors = Vec::new();

    // Check every utterance still has a terminator.
    // CA transcripts (@Options: CA) are exempt — terminators are optional in CA mode.
    let is_ca = file.options.iter().any(|f| f.enables_ca_mode());

    if !is_ca {
        for line in &file.lines {
            if let Line::Utterance(utt) = line
                && utt.main.content.terminator.is_none()
            {
                let speaker = utt.main.speaker.as_str();
                errors.push(ValidationError {
                    message: format!(
                        "After {command}: utterance by *{speaker} lost its terminator"
                    ),
                    level: ValidityLevel::StructurallyComplete,
                });
            }
        }
    }

    // Command-specific checks
    match command {
        "morphotag" => validate_morphotag_output(file, &mut errors),
        "align" => validate_align_output(file, &mut errors),
        _ => {}
    }

    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

/// Post-validation for morphotag: %mor word count must match main tier.
fn validate_morphotag_output(file: &ChatFile, errors: &mut Vec<ValidationError>) {
    use talkbank_model::alignment::helpers::TierDomain;

    for line in &file.lines {
        if let Line::Utterance(utt) = line {
            // Count alignable words
            let mut extracted = Vec::new();
            crate::extract::collect_utterance_content(
                &utt.main.content.content,
                TierDomain::Mor,
                &mut extracted,
            );
            let word_count = extracted.len();

            // Count %mor items
            for tier in &utt.dependent_tiers {
                if let talkbank_model::model::DependentTier::Mor(mor_tier) = tier {
                    let mor_count = mor_tier.items().len();
                    if word_count != mor_count {
                        let speaker = utt.main.speaker.as_str();
                        errors.push(ValidationError {
                            message: format!(
                                "After morphotag: *{speaker} has {word_count} words \
                                 but %mor has {mor_count} items"
                            ),
                            level: ValidityLevel::MainTierValid,
                        });
                    }
                }
            }
        }
    }
}

/// Post-validation for align: check for backwards timing only.
///
/// Cross-speaker overlap is **normal** in conversation data (speakers talk
/// over each other) and is valid CHAT. The real validator in talkbank-tools
/// handles all E362/E704 checks. We only flag clearly broken output here
/// (end < start within a single utterance).
fn validate_align_output(file: &ChatFile, errors: &mut Vec<ValidationError>) {
    for line in &file.lines {
        if let Line::Utterance(utt) = line
            && let Some(ref bullet) = utt.main.content.bullet
        {
            let start = bullet.timing.start_ms;
            let end = bullet.timing.end_ms;

            if start > end {
                let speaker = utt.main.speaker.as_str();
                errors.push(ValidationError {
                    message: format!(
                        "After align: *{speaker} has backwards timing \
                         ({start} > {end})"
                    ),
                    level: ValidityLevel::MainTierValid,
                });
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_parser::TreeSitterParser;

    const MOR_GRA_CHAT: &str = "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n\
@ID:\teng|test|CHI|3;|male|||Target_Child|||\n*CHI:\thello world .\n\
%mor:\tn|hello n|world .\n%gra:\t1|2|SUBJ 2|0|ROOT 3|2|PUNCT\n@End\n";

    fn parse_chat_file(
        text: &str,
    ) -> Result<talkbank_model::ChatFile, talkbank_model::ParseErrors> {
        let parser = TreeSitterParser::new().expect("grammar loads");
        parser.parse_chat_file(text)
    }

    #[test]
    fn test_valid_file_passes_all_levels() {
        let chat = parse_chat_file(MOR_GRA_CHAT).unwrap();
        assert!(validate_to_level(&chat, &[], ValidityLevel::MainTierValid).is_ok());
    }

    #[test]
    fn test_parse_errors_fail_l0() {
        use talkbank_model::{ErrorCode, Severity, SourceLocation, Span};
        let chat = parse_chat_file(MOR_GRA_CHAT).unwrap();
        let synthetic: Vec<ParseError> = (0..3)
            .map(|_| {
                ParseError::new(
                    ErrorCode::UnparsableContent,
                    Severity::Error,
                    SourceLocation::new(Span::from_usize(0, 1)),
                    None,
                    "synthetic parse error",
                )
            })
            .collect();
        let result = validate_to_level(&chat, &synthetic, ValidityLevel::Parseable);
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert_eq!(errors.len(), 1);
        // L0 message now surfaces the first error's code, span, and excerpt
        // instead of a bare count. "(+2 more)" summarises the remaining 2.
        assert!(
            errors[0].message.contains("E316"),
            "got: {}",
            errors[0].message
        );
        assert!(
            errors[0].message.contains("(+2 more)"),
            "got: {}",
            errors[0].message
        );
    }

    #[test]
    fn test_morphotag_output_validates_alignment() {
        let chat = parse_chat_file(MOR_GRA_CHAT).unwrap();
        // Valid morphotag output should pass
        assert!(validate_output(&chat, "morphotag").is_ok());
    }

    /// CA files with `@Options: CA` should not fail L1 validation for
    /// missing terminators — CA utterances can legitimately lack terminators
    /// (incomplete turns, backchannels, trailing-off speech).
    #[test]
    fn test_ca_file_skips_terminator_check() {
        use talkbank_model::model::Line;

        let mut chat = parse_chat_file(MOR_GRA_CHAT).unwrap();

        // Add @Options: CA and remove terminators
        chat.options.push(ChatOptionFlag::Ca);
        for line in &mut chat.lines {
            if let Line::Utterance(utt) = line {
                utt.main.content.terminator = None;
            }
        }

        // Should pass L1 because CA files skip the terminator check
        let result = validate_to_level(&chat, &[], ValidityLevel::StructurallyComplete);
        assert!(
            result.is_ok(),
            "CA files should pass L1 even without terminators: {result:?}"
        );
    }

    /// Non-CA files with missing terminators should still fail L1.
    #[test]
    fn test_non_ca_file_fails_without_terminator() {
        use talkbank_model::model::Line;

        let mut chat = parse_chat_file(MOR_GRA_CHAT).unwrap();

        // Remove terminators WITHOUT setting CA option
        for line in &mut chat.lines {
            if let Line::Utterance(utt) = line {
                utt.main.content.terminator = None;
            }
        }

        let result = validate_to_level(&chat, &[], ValidityLevel::StructurallyComplete);
        assert!(
            result.is_err(),
            "Non-CA files should fail without terminators"
        );
    }

    #[test]
    fn test_output_validation_catches_missing_terminator() {
        use talkbank_model::model::Line;

        let mut chat = parse_chat_file(MOR_GRA_CHAT).unwrap();

        // Remove the terminator to simulate corruption
        for line in &mut chat.lines {
            if let Line::Utterance(utt) = line {
                utt.main.content.terminator = None;
            }
        }

        let result = validate_output(&chat, "morphotag");
        assert!(result.is_err());
        let errors = result.unwrap_err();
        assert!(errors[0].message.contains("lost its terminator"));
    }

    // -------------------------------------------------------------------
    // RED — Fundamental C (validate_to_level signature destroys provenance)
    //
    // `validate_to_level(&ChatFile, parse_error_count: usize, level)`
    // takes the *count* of parse errors, not the errors themselves. The
    // call sites all have `Vec<ParseError>` in hand with rich location
    // (line, column, span), structured code (UnparsableContent = E316),
    // and offending-text excerpt (`source_text` inside `ErrorContext`).
    // The API throws all of that away before the L0 ValidationError
    // message is formatted, so the message can only ever say
    //   "File has N parse error(s); input may be malformed"
    // which is what the reporter saw (job c465e6e8-97c, 2026-04-22).
    //
    // This is the architectural problem, not a formatting problem:
    // fixing the message without widening the signature is impossible.
    // Per CLAUDE.md ("types are the first layer of documentation; no
    // tuple-packed / stringly / count-packed domain seams"), the
    // correct fix is to change the signature to accept `&[ParseError]`.
    //
    // The three earlier message-content tests (line / code / excerpt)
    // were symptom spread across one underlying issue. They collapse
    // into this single invariant test that exercises all three
    // properties at once, against a fixture known to produce rich
    // parse errors.
    //
    // (Fundamental C) for scope and sequencing.
    // -------------------------------------------------------------------

    #[test]
    fn red_fund_c_validate_to_level_preserves_parse_error_provenance() {
        let parser = TreeSitterParser::new().expect("grammar loads");
        // Minimal CHAT with `%` on the main tier (line 6). Same parse
        // failure The reporter's job hit on line 904 of her intermediate CHAT.
        let malformed = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|corpus|PAR|||||Participant|||
*PAR:\tremember 80% of it .
@End
";
        let (chat, parse_errors) = crate::parse::parse_lenient(&parser, malformed);

        // Precondition: the parser must actually produce at least one
        // structured parse error with the three pieces of provenance we
        // claim the gate should propagate. If this precondition fails,
        // the test is measuring the wrong thing.
        assert!(
            !parse_errors.is_empty(),
            "precondition: `%` on main tier must produce at least one \
             ParseError; got none"
        );
        let first = &parse_errors[0];
        let context_source_text = first
            .context
            .as_ref()
            .map(|c| c.source_text.clone())
            .unwrap_or_default();
        assert!(
            context_source_text.contains('%'),
            "precondition: ParseError.context.source_text must contain \
             the offending `%`; got {context_source_text:?} in \
             {first:#?}"
        );

        let result = validate_to_level(&chat, &parse_errors, ValidityLevel::Parseable);
        let errs = result.expect_err("malformed CHAT must fail L0");
        assert_eq!(errs.len(), 1, "expected exactly one L0 ValidationError");
        let msg = &errs[0].message;

        // The invariant: provenance that exists upstream must reach
        // the user-visible message. We assert three pieces survive:
        //
        //   1. a coordinate (line number if the upstream populates it,
        //      otherwise a byte span — today tree-sitter's streaming
        //      parser leaves line/column None, so byte span is the
        //      primary coordinate surface until Fundamental D updates
        //      the parser's error taxonomy),
        //   2. an error-code token or documentation link,
        //   3. an excerpt containing the offending text (here `%`).
        let has_coordinate = msg.contains("line ") || msg.contains("byte");
        let has_error_code = msg.contains("E316")
            || msg.contains("UnparsableContent")
            || msg.contains("talkbank.org/errors/");
        let has_excerpt = msg.contains('%') || msg.contains(&context_source_text);

        assert!(
            has_coordinate && has_error_code && has_excerpt,
            "L0 ValidationError message must preserve all three pieces \
             of parse-error provenance (coordinate / code / excerpt). \
             Upstream ParseError:\n{first:#?}\n\
             L0 gate message: {msg:?}\n\
             has_coordinate = {has_coordinate}\n\
             has_error_code = {has_error_code}\n\
             has_excerpt    = {has_excerpt}"
        );
    }
}
