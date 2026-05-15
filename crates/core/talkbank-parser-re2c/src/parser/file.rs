//! File-level parser — assembles headers, utterances, and dependent tiers.
//!
//! This module is imperative (sequential line dispatch) rather than
//! combinator-based, because the file structure is prefix-dispatched:
//! dependent tier type is determined by reading the `%mor:`, `%gra:` etc.
//! prefix text, which doesn't map cleanly to chumsky's token-variant matching.
//!
//! Sub-parsers for individual tiers are chumsky combinators from
//! `dependent_tiers` and `main_tier`.
//!
//! **Error reporting:** When a chumsky sub-parser fails, this module
//! reports the failure to the `ErrorSink` and produces best-effort
//! output (e.g., falling back to a generic text tier for unparseable
//! dependent tiers).

use chumsky::Parser as _;

use crate::ast::*;
use crate::token::{Token, TokenDiscriminants};
use talkbank_model::{ErrorSink, NullErrorSink, ParseError, Span};

use super::dependent_tiers;
use super::main_tier;

/// Parse a complete CHAT file with no error reporting.
pub fn parse_file<'a>(tokens: &'a [Token<'a>], source: &'a str) -> ChatFile<'a> {
    parse_file_with_errors(tokens, source, &NullErrorSink)
}

/// Parse a complete CHAT file from a leaked token slice, reporting
/// parse failures to the given error sink.
pub fn parse_file_with_errors<'a>(
    tokens: &'a [Token<'a>],
    source: &'a str,
    errors: &impl ErrorSink,
) -> ChatFile<'a> {
    let mut pos = 0;
    let mut lines = Vec::new();

    while pos < tokens.len() {
        let d = TokenDiscriminants::from(&tokens[pos]);
        match d {
            // No-content headers
            TokenDiscriminants::HeaderUtf8
            | TokenDiscriminants::HeaderBegin
            | TokenDiscriminants::HeaderEnd
            | TokenDiscriminants::HeaderBlank
            | TokenDiscriminants::HeaderNewEpisode => {
                let tok = tokens[pos].clone();
                pos += 1;
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header(HeaderParsed {
                    prefix: tok,
                    content: vec![],
                }));
            }

            // Headers with content
            TokenDiscriminants::HeaderPrefix
            | TokenDiscriminants::HeaderBirthOf
            | TokenDiscriminants::HeaderBirthplaceOf
            | TokenDiscriminants::HeaderL1Of => {
                let prefix = tokens[pos].clone();
                pos += 1;
                let mut content = Vec::new();
                while pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
                {
                    let tok = tokens[pos].clone();
                    pos += 1;
                    if !matches!(tok, Token::Whitespace(_)) {
                        content.push(tok);
                    }
                }
                if pos < tokens.len()
                    && TokenDiscriminants::from(&tokens[pos]) == TokenDiscriminants::Newline
                {
                    pos += 1;
                }
                lines.push(Line::Header(HeaderParsed { prefix, content }));
            }

            // Main tier
            TokenDiscriminants::Star => {
                let start = pos;
                pos = skip_to_newline(tokens, pos);
                if pos < tokens.len() {
                    pos += 1; // consume newline
                }

                let main_tier_tokens = &tokens[start..pos];
                match main_tier::main_tier_parser()
                    .parse(main_tier_tokens)
                    .into_result()
                {
                    Ok(main_tier) => {
                        let dep_tiers = parse_dependent_tiers(tokens, &mut pos, errors);
                        lines.push(Line::Utterance(Box::new(Utterance {
                            main_tier,
                            dependent_tiers: dep_tiers,
                        })));
                    }
                    Err(_) => {
                        // Report E321: unparsable utterance.
                        report_error(
                            errors,
                            talkbank_model::errors::codes::ErrorCode::UnparsableUtterance,
                            talkbank_model::Severity::Error,
                            main_tier_tokens,
                            "utterance could not be parsed",
                        );
                        // Skip any dependent tiers that follow — they're orphaned
                        // without a valid main tier.
                        while pos < tokens.len()
                            && TokenDiscriminants::from(&tokens[pos])
                                == TokenDiscriminants::TierPrefix
                        {
                            pos = skip_to_newline(tokens, pos);
                            if pos < tokens.len() {
                                pos += 1;
                            }
                        }
                    }
                }
            }

            // Skip structural tokens
            TokenDiscriminants::Whitespace
            | TokenDiscriminants::Newline
            | TokenDiscriminants::Continuation
            | TokenDiscriminants::BOM => {
                pos += 1;
            }

            // Orphan tier prefix (no preceding main tier) — report E319
            TokenDiscriminants::TierPrefix => {
                let line_start = pos;
                pos = skip_to_newline(tokens, pos);
                if pos < tokens.len() {
                    pos += 1;
                }
                report_error(
                    errors,
                    talkbank_model::errors::codes::ErrorCode::UnparsableLine,
                    talkbank_model::Severity::Error,
                    &tokens[line_start..pos],
                    "orphan dependent tier (no preceding main tier)",
                );
            }

            // Unknown tokens — report and skip
            _ => {
                let tok = &tokens[pos];
                errors.report(ParseError::new(
                    talkbank_model::errors::codes::ErrorCode::UnexpectedSyntax,
                    talkbank_model::Severity::Error,
                    talkbank_model::SourceLocation::new(Span::DUMMY),
                    None,
                    format!("unhandled token in parse_chat_file: {:?}", tok.text()),
                ));
                pos += 1;
            }
        }
    }

    ChatFile { lines, source }
}

/// Parse dependent tiers following a main tier.
///
/// When a tier-specific chumsky parser fails, the error is reported
/// and the tier falls back to a generic text tier (preserving the raw
/// content for downstream inspection).
fn parse_dependent_tiers<'a>(
    tokens: &'a [Token<'a>],
    pos: &mut usize,
    errors: &impl ErrorSink,
) -> Vec<DependentTierParsed<'a>> {
    let mut dep_tiers = Vec::new();

    while *pos < tokens.len()
        && TokenDiscriminants::from(&tokens[*pos]) == TokenDiscriminants::TierPrefix
    {
        let prefix = tokens[*pos].clone();
        let prefix_text = prefix.text();
        *pos += 1;

        let content_start = *pos;
        *pos = skip_to_newline(tokens, *pos);
        let content_end = *pos;
        if *pos < tokens.len() {
            *pos += 1; // consume newline
        }

        let tier_tokens = &tokens[content_start..content_end];

        // Malformed tier: no content after prefix (e.g., `%mor\n` without `:\t`).
        // Report E602 and produce a fallback text tier.
        if tier_tokens.is_empty() {
            errors.report(ParseError::new(
                talkbank_model::errors::codes::ErrorCode::MalformedTierHeader,
                talkbank_model::Severity::Error,
                talkbank_model::SourceLocation::new(Span::DUMMY),
                None,
                format!(
                    "malformed dependent tier: {} has no content (missing colon-tab?)",
                    prefix_text
                ),
            ));
            dep_tiers.push(DependentTierParsed::Text {
                prefix,
                content: vec![],
            });
            continue;
        }

        // Try the tier-specific parser. On failure, report error and
        // fall back to generic text tier.
        if prefix_text.starts_with("%mor") || prefix_text.starts_with("%trn") {
            match dependent_tiers::mor_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Mor(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%pho") {
            match dependent_tiers::pho_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Pho(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%mod") {
            match dependent_tiers::pho_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Mod(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%gra") {
            match dependent_tiers::gra_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Gra(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%sin") {
            match dependent_tiers::sin_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok(tier) => dep_tiers.push(DependentTierParsed::Sin(tier)),
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else if prefix_text.starts_with("%wor") {
            match dependent_tiers::wor_tier_parser()
                .parse(tier_tokens)
                .into_result()
            {
                Ok((items, terminator)) => {
                    dep_tiers.push(DependentTierParsed::Wor { items, terminator })
                }
                Err(_) => {
                    report_error(
                        errors,
                        talkbank_model::errors::codes::ErrorCode::UnparsableContent,
                        talkbank_model::Severity::Error,
                        tier_tokens,
                        &format!("failed to parse {prefix_text} tier content"),
                    );
                    dep_tiers.push(fallback_text_tier(prefix, tier_tokens));
                }
            }
        } else {
            // Generic text tier — always succeeds
            let content: Vec<Token<'a>> = tier_tokens.to_vec();
            dep_tiers.push(DependentTierParsed::Text { prefix, content });
        }
    }

    dep_tiers
}

/// Report a parse error with a specific error code.
fn report_error(
    errors: &impl ErrorSink,
    code: talkbank_model::errors::codes::ErrorCode,
    severity: talkbank_model::Severity,
    tokens: &[Token<'_>],
    context: &str,
) {
    let preview: String = tokens
        .iter()
        .take(5)
        .map(|t| t.text())
        .collect::<Vec<_>>()
        .join(" ");
    errors.report(ParseError::new(
        code,
        severity,
        talkbank_model::SourceLocation::new(Span::DUMMY),
        None,
        format!("{context}: {preview}..."),
    ));
}

/// Create a fallback text tier from raw tokens when a tier-specific
/// parser fails. This preserves the content for downstream inspection
/// rather than silently dropping it.
fn fallback_text_tier<'a>(prefix: Token<'a>, tokens: &[Token<'a>]) -> DependentTierParsed<'a> {
    let content: Vec<Token<'a>> = tokens.to_vec();
    DependentTierParsed::Text { prefix, content }
}

/// Advance position to the Newline token (or end of tokens).
fn skip_to_newline(tokens: &[Token<'_>], mut pos: usize) -> usize {
    while pos < tokens.len()
        && TokenDiscriminants::from(&tokens[pos]) != TokenDiscriminants::Newline
    {
        pos += 1;
    }
    pos
}
