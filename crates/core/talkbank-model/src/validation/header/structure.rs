//! Structural/header-level validation for CHAT file preambles.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>

use crate::model::{Header, SpeakerCode};
use crate::{ErrorCode, ErrorContext, ErrorSink, ParseError, Severity, SourceLocation, Span};
use std::collections::{HashMap, HashSet};

/// Returns a gem label as `&str`, or `""` for unlabeled gems.
fn label_or_empty(label: Option<&str>) -> &str {
    // DEFAULT: Unlabeled gems are represented by an empty label string.
    label.unwrap_or_default()
}

/// Internal function: validate headers collection with ErrorSink
///
/// `source_len` is the byte length of the original source. When provided,
/// "missing @End" errors point at end-of-file instead of offset 0.
pub(crate) fn check_headers(
    headers: &[(&Header, Span)],
    errors: &impl ErrorSink,
    source_len: Option<usize>,
) {
    let mut header_counts: HashMap<String, (usize, Span)> = HashMap::new();
    let mut declared_participants: HashSet<SpeakerCode> = HashSet::new();
    let mut id_speakers: Vec<(SpeakerCode, Span)> = Vec::new();

    for (header, span) in headers {
        let name_lower = header.name().to_lowercase();
        header_counts
            .entry(name_lower)
            .and_modify(|(count, _)| *count += 1)
            .or_insert((1, *span));

        if let Header::Participants { entries } = header {
            for entry in entries {
                declared_participants.insert(entry.speaker_code.clone());
            }
        }

        if let Header::ID(id_header) = header {
            id_speakers.push((id_header.speaker.clone(), *span));
        }
    }

    let single_only_headers = ["Types", "Media", "Videos", "UTF8", "Begin", "End"];
    for name in &single_only_headers {
        let name_lower = name.to_lowercase();
        if let Some(&(count, span)) = header_counts.get(&name_lower)
            && count > 1
        {
            let mut err = ParseError::new(
                ErrorCode::DuplicateHeader,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(*name, 0..name.len(), *name),
                format!(
                    "Duplicate @{} header: found {} occurrences, but only one is allowed",
                    name, count
                ),
            )
            .with_suggestion(format!(
                "Remove the extra @{} headers so only one remains",
                name
            ));
            err.location.span = span;
            errors.report(err);
        }
    }

    // For missing-header errors, point at end-of-file when source_len is
    // available, otherwise fall back to offset 0.
    let eof_offset = source_len.unwrap_or(0);

    let required_headers = ["Begin", "Languages", "Participants"];
    for required in &required_headers {
        let required_lower = required.to_lowercase();
        if !header_counts.contains_key(&required_lower) {
            errors.report(
                ParseError::new(
                    ErrorCode::MissingRequiredHeader,
                    Severity::Error,
                    SourceLocation::at_offset(eof_offset),
                    ErrorContext::new("", 0..0, ""),
                    format!("Missing required @{} header in file preamble", required),
                )
                .with_suggestion(format!(
                    "Add an @{} line to the file header section (before any utterances)",
                    required
                )),
            );
        }
    }

    // E503: @UTF8 must be present (spec requires it as the first line)
    if !header_counts.contains_key("utf8") {
        errors.report(
            ParseError::new(
                ErrorCode::MissingUTF8Header,
                Severity::Error,
                SourceLocation::at_offset(eof_offset),
                ErrorContext::new("", 0..0, ""),
                "Missing @UTF8 header: every CHAT file must declare its encoding",
            )
            .with_suggestion("Add @UTF8 as the very first line of the file"),
        );
    }

    if !header_counts.contains_key("end") {
        errors.report(
            ParseError::new(
                ErrorCode::MissingEndHeader,
                Severity::Error,
                SourceLocation::at_offset(eof_offset),
                ErrorContext::new("", 0..0, ""),
                "Missing @End header at end of file",
            )
            .with_suggestion("Add @End as the last line of the file"),
        );
    }

    // E543: Check header ordering — @Participants must precede @Options and @ID
    check_header_order(headers, errors);

    for (speaker, span) in &id_speakers {
        if !speaker.as_str().is_empty() && !declared_participants.contains(speaker) {
            let speaker_str = speaker.as_str();
            let mut err = ParseError::new(
                ErrorCode::SpeakerNotDefined,
                Severity::Error,
                SourceLocation::at_offset(span.start as usize),
                ErrorContext::new(speaker_str, 0..speaker_str.len(), speaker_str),
                format!(
                    "Speaker '{}' referenced in @ID header but not declared in @Participants",
                    speaker_str
                ),
            )
            .with_suggestion(format!(
                "Add '{}' to the @Participants line, or remove this @ID header",
                speaker_str
            ));
            err.location.span = *span;
            errors.report(err);
        }
    }

    // E526, E527, E528: Validate @Bg/@Eg matching
    check_gem_balance(headers, errors);
}

/// Check that headers appear in canonical order.
///
/// The CHAT spec requires:
/// - `@Participants` must appear before `@Options`
/// - `@Participants` must appear before `@ID`
///
/// This corresponds to CLAN CHECK errors 61 and 125.
fn check_header_order(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    let mut saw_participants = false;

    for (header, span) in headers {
        match header {
            Header::Participants { .. } => {
                saw_participants = true;
            }
            Header::Options { .. } if !saw_participants => {
                let mut err = ParseError::new(
                    ErrorCode::HeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@Options", 0.."@Options".len(), "@Options"),
                    "@Options must appear after @Participants",
                )
                .with_suggestion("Move @Options to after the @Participants header");
                err.location.span = *span;
                errors.report(err);
            }
            Header::ID(_) if !saw_participants => {
                let mut err = ParseError::new(
                    ErrorCode::HeaderOutOfOrder,
                    Severity::Error,
                    SourceLocation::at_offset(span.start as usize),
                    ErrorContext::new("@ID", 0.."@ID".len(), "@ID"),
                    "@ID must appear after @Participants",
                )
                .with_suggestion("Move @ID to after the @Participants header");
                err.location.span = *span;
                errors.report(err);
            }
            _ => {}
        }
    }
}

/// Validate that @Bg (Begin Gem) and @Eg (End Gem) markers are properly matched.
///
/// Checks:
/// - E526: Every @Bg has a matching @Eg
/// - E527: Every @Eg has a matching @Bg
/// - E528: Labels match between paired @Bg/@Eg
/// - E529: Nested @Bg with same label (opening @Bg while already in that scope)
/// - E530: @G (lazy gem) inside @Bg/@Eg scope
fn check_gem_balance(headers: &[(&Header, Span)], errors: &impl ErrorSink) {
    use std::collections::HashMap;

    // Track open scopes by label (None for unlabeled gems)
    let mut open_scopes: HashMap<Option<String>, usize> = HashMap::new();

    for (header, span) in headers {
        match header {
            Header::BeginGem { label } => {
                let key = label.as_ref().map(|l| l.as_str().to_string());
                let current_count = open_scopes.get(&key).copied().unwrap_or(0);

                // E529: Nested @Bg with the same label is not allowed.
                // Different labels are permitted (stack-based LIFO scoping).
                if current_count > 0 {
                    let label_str = label_or_empty(key.as_deref());
                    let mut err = ParseError::new(
                        ErrorCode::NestedBeginGem,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "Nested @Bg: cannot open a new @Bg while already inside a @Bg scope with the same label"
                                .to_string()
                        } else {
                            format!(
                                "Nested @Bg:{0}: cannot open a new @Bg:{0} while already inside a @Bg:{0} scope",
                                label_str
                            )
                        },
                    )
                    .with_suggestion(
                        "Close the current @Bg scope with @Eg before opening another @Bg with the same label"
                            .to_string(),
                    );
                    err.location.span = *span;
                    errors.report(err);
                }

                *open_scopes.entry(key).or_insert(0) += 1;
            }
            Header::LazyGem { label } => {
                // E530: Check if any @Bg scope is open
                let any_scope_open = open_scopes.values().any(|&count| count > 0);
                if any_scope_open {
                    let label_str = label_or_empty(label.as_ref().map(|l| l.as_str()));
                    let mut err = ParseError::new(
                        ErrorCode::LazyGemInsideScope,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "@G (lazy gem) cannot appear inside @Bg/@Eg scope".to_string()
                        } else {
                            format!(
                                "@G:{} (lazy gem) cannot appear inside @Bg/@Eg scope",
                                label_str
                            )
                        },
                    )
                    .with_suggestion(
                        "Move @G outside of @Bg/@Eg scope, or use @Bg/@Eg markers instead"
                            .to_string(),
                    );
                    err.location.span = *span;
                    errors.report(err);
                }
            }
            Header::EndGem { label } => {
                let key = label.as_ref().map(|l| l.as_str().to_string());
                let has_any_open_scope = open_scopes.values().any(|&count| count > 0);
                let count = open_scopes.get_mut(&key);

                if let Some(count) = count {
                    if *count > 0 {
                        *count -= 1;
                    } else {
                        if has_any_open_scope {
                            let label_str = label_or_empty(key.as_deref());
                            let mut err = ParseError::new(
                                ErrorCode::GemLabelMismatch,
                                Severity::Error,
                                SourceLocation::at_offset(span.start as usize),
                                ErrorContext::new("", 0..0, ""),
                                if label_str.is_empty() {
                                    "Gem label mismatch between @Bg/@Eg markers".to_string()
                                } else {
                                    format!(
                                        "Gem label mismatch: @Eg:{} does not match active @Bg scope",
                                        label_str
                                    )
                                },
                            );
                            err.location.span = *span;
                            errors.report(err);
                        }
                        // End without matching begin
                        let label_str = label_or_empty(key.as_deref());
                        let mut err = ParseError::new(
                            ErrorCode::UnmatchedEndGem,
                            Severity::Error,
                            SourceLocation::at_offset(span.start as usize),
                            ErrorContext::new("", 0..0, ""),
                            if label_str.is_empty() {
                                "Unmatched @Eg (no matching @Bg)".to_string()
                            } else {
                                format!(
                                    "Unmatched @Eg:{} (no matching @Bg:{})",
                                    label_str, label_str
                                )
                            },
                        )
                        .with_suggestion(if label_str.is_empty() {
                            "Add a matching @Bg before this @Eg".to_string()
                        } else {
                            format!(
                                "Add a matching @Bg:{} before this @Eg:{}",
                                label_str, label_str
                            )
                        });
                        err.location.span = *span;
                        errors.report(err);
                    }
                } else {
                    if has_any_open_scope {
                        let label_str = label_or_empty(key.as_deref());
                        let mut err = ParseError::new(
                            ErrorCode::GemLabelMismatch,
                            Severity::Error,
                            SourceLocation::at_offset(span.start as usize),
                            ErrorContext::new("", 0..0, ""),
                            if label_str.is_empty() {
                                "Gem label mismatch between @Bg/@Eg markers".to_string()
                            } else {
                                format!(
                                    "Gem label mismatch: @Eg:{} does not match active @Bg scope",
                                    label_str
                                )
                            },
                        );
                        err.location.span = *span;
                        errors.report(err);
                    }
                    // End without any matching begin (different label)
                    let label_str = label_or_empty(key.as_deref());
                    let mut err = ParseError::new(
                        ErrorCode::UnmatchedEndGem,
                        Severity::Error,
                        SourceLocation::at_offset(span.start as usize),
                        ErrorContext::new("", 0..0, ""),
                        if label_str.is_empty() {
                            "Unmatched @Eg (no matching @Bg)".to_string()
                        } else {
                            format!(
                                "Unmatched @Eg:{} (no matching @Bg:{})",
                                label_str, label_str
                            )
                        },
                    )
                    .with_suggestion(if label_str.is_empty() {
                        "Add a matching @Bg before this @Eg".to_string()
                    } else {
                        format!(
                            "Add a matching @Bg:{} before this @Eg:{}",
                            label_str, label_str
                        )
                    });
                    err.location.span = *span;
                    errors.report(err);
                }
            }
            _ => {}
        }
    }

    // Check for unclosed scopes — no specific header to point at (scope was opened earlier)
    for (label_opt, count) in open_scopes {
        if count > 0 {
            let label_str = label_or_empty(label_opt.as_deref());
            errors.report(
                ParseError::new(
                    ErrorCode::UnmatchedBeginGem,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new("", 0..0, ""),
                    if label_str.is_empty() {
                        format!("Unmatched @Bg: {} @Bg without matching @Eg", count)
                    } else {
                        format!(
                            "Unmatched @Bg:{}: {} @Bg:{} without matching @Eg:{}",
                            label_str, count, label_str, label_str
                        )
                    },
                )
                .with_suggestion(if label_str.is_empty() {
                    format!("Add {} matching @Eg marker(s)", count)
                } else {
                    format!("Add {} matching @Eg:{} marker(s)", count, label_str)
                }),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ErrorCollector;
    use crate::model::GemLabel;

    // ── Header ordering tests ──────────────────────────────────────

    /// `@Options` before `@Participants` emits `E543`.
    #[test]
    fn test_e543_options_before_participants() {
        use crate::model::{ChatOptionFlag, ChatOptionFlags};

        let options = Header::Options {
            options: ChatOptionFlags::new(vec![ChatOptionFlag::Ca]),
        };
        let participants = Header::Participants {
            entries: vec![].into(),
        };
        let headers = vec![(&options, Span::DUMMY), (&participants, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_header_order(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::HeaderOutOfOrder);
        assert!(error_vec[0].message.contains("@Options"));
    }

    /// `@Options` after `@Participants` is valid — no error.
    #[test]
    fn test_options_after_participants_ok() {
        use crate::model::{ChatOptionFlag, ChatOptionFlags};

        let participants = Header::Participants {
            entries: vec![].into(),
        };
        let options = Header::Options {
            options: ChatOptionFlags::new(vec![ChatOptionFlag::Ca]),
        };
        let headers = vec![(&participants, Span::DUMMY), (&options, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_header_order(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// `@ID` before `@Participants` emits `E543`.
    #[test]
    fn test_e543_id_before_participants() {
        use crate::model::IDHeader;

        let id = Header::ID(IDHeader::new("eng", "CHI", "Target_Child"));
        let participants = Header::Participants {
            entries: vec![].into(),
        };
        let headers = vec![(&id, Span::DUMMY), (&participants, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_header_order(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::HeaderOutOfOrder);
        assert!(error_vec[0].message.contains("@ID"));
    }

    // ── Gem balance tests ──────────────────────────────────────────

    /// Unmatched `@Bg` markers emit `E526`.
    ///
    /// The error message should preserve the gem label so users can identify the open scope.
    #[test]
    fn test_e526_unmatched_begin_gem() {
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![(&h1, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnmatchedBeginGem);
        assert!(error_vec[0].message.contains("episode1"));
    }

    /// Unmatched `@Eg` markers emit `E527`.
    ///
    /// Label text is included in the diagnostic to aid recovery.
    #[test]
    fn test_e527_unmatched_end_gem() {
        let h1 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![(&h1, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::UnmatchedEndGem);
        assert!(error_vec[0].message.contains("episode1"));
    }

    /// Mismatched begin/end labels emit `E528` plus unmatched-scope diagnostics.
    ///
    /// This confirms the checker surfaces both the direct mismatch and residual stack errors.
    #[test]
    fn test_e528_gem_label_mismatch() {
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::EndGem {
            label: Some(GemLabel::new("episode2")),
        };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 3); // Label mismatch + unmatched begin + unmatched end
        assert!(
            error_vec
                .iter()
                .any(|e| e.code == ErrorCode::GemLabelMismatch)
        );
        assert!(
            error_vec
                .iter()
                .any(|e| e.code == ErrorCode::UnmatchedBeginGem && e.message.contains("episode1"))
        );
        assert!(
            error_vec
                .iter()
                .any(|e| e.code == ErrorCode::UnmatchedEndGem && e.message.contains("episode2"))
        );
    }

    /// Perfectly matched labeled gem pairs produce no errors.
    ///
    /// This is the baseline happy path for labeled scope balancing.
    #[test]
    fn test_balanced_gems() {
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// Unlabeled `@Bg`/`@Eg` pairs are valid when balanced.
    ///
    /// The checker treats unlabeled scopes as a separate stack domain.
    #[test]
    fn test_unlabeled_gems() {
        let h1 = Header::BeginGem { label: None };
        let h2 = Header::EndGem { label: None };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// Mixed labeled and unlabeled scopes can nest cleanly in LIFO order.
    ///
    /// This protects corpus patterns that combine both scope styles.
    #[test]
    fn test_mixed_labeled_unlabeled() {
        // @Bg:episode1, @Bg, @Eg, @Eg:episode1 — different labels nest cleanly (LIFO)
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::BeginGem { label: None };
        let h3 = Header::EndGem { label: None };
        let h4 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![
            (&h1, Span::DUMMY),
            (&h2, Span::DUMMY),
            (&h3, Span::DUMMY),
            (&h4, Span::DUMMY),
        ];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// Re-entering the same gem label before closing emits `E529`.
    ///
    /// Nested duplicate labels are treated as structural ambiguity.
    #[test]
    fn test_e529_nested_begin_gem_same_label() {
        // @Bg:episode1, @Bg:episode1 (nested same label is error)
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h3 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h4 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![
            (&h1, Span::DUMMY),
            (&h2, Span::DUMMY),
            (&h3, Span::DUMMY),
            (&h4, Span::DUMMY),
        ];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::NestedBeginGem);
        assert!(error_vec[0].message.contains("episode1"));
    }

    /// Nested unlabeled begin markers also emit `E529`.
    ///
    /// Unlabeled scopes cannot be recursively opened without an intervening close.
    #[test]
    fn test_e529_nested_begin_gem_unlabeled() {
        // @Bg, @Bg (nested unlabeled is error)
        let h1 = Header::BeginGem { label: None };
        let h2 = Header::BeginGem { label: None };
        let h3 = Header::EndGem { label: None };
        let h4 = Header::EndGem { label: None };
        let headers = vec![
            (&h1, Span::DUMMY),
            (&h2, Span::DUMMY),
            (&h3, Span::DUMMY),
            (&h4, Span::DUMMY),
        ];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::NestedBeginGem);
    }

    /// Different labels may nest when closed in strict LIFO order.
    ///
    /// This matches hierarchical gem usage in corpora like HSLLD.
    #[test]
    fn test_different_labels_allowed() {
        // @Bg:episode1, @Bg:episode2, @Eg:episode2, @Eg:episode1 — LIFO nesting with
        // different labels is valid (used by HSLLD and other corpora for hierarchical markup).
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::BeginGem {
            label: Some(GemLabel::new("episode2")),
        };
        let h3 = Header::EndGem {
            label: Some(GemLabel::new("episode2")),
        };
        let h4 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![
            (&h1, Span::DUMMY),
            (&h2, Span::DUMMY),
            (&h3, Span::DUMMY),
            (&h4, Span::DUMMY),
        ];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// Lazy gems inside an open scope emit `E530`.
    ///
    /// `@G` markers are only valid outside active `@Bg`/`@Eg` regions.
    #[test]
    fn test_e530_lazy_gem_inside_scope() {
        // @Bg:episode1, @G, @Eg:episode1 (@G inside scope is error)
        let h1 = Header::BeginGem {
            label: Some(GemLabel::new("episode1")),
        };
        let h2 = Header::LazyGem { label: None };
        let h3 = Header::EndGem {
            label: Some(GemLabel::new("episode1")),
        };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY), (&h3, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::LazyGemInsideScope);
    }

    /// Labeled lazy gems inside scope also emit `E530`.
    ///
    /// Label text should be preserved in the emitted diagnostic.
    #[test]
    fn test_e530_lazy_gem_with_label_inside_scope() {
        // @Bg, @G:labeled, @Eg (@G:labeled inside scope is error)
        let h1 = Header::BeginGem { label: None };
        let h2 = Header::LazyGem {
            label: Some(GemLabel::new("task1")),
        };
        let h3 = Header::EndGem { label: None };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY), (&h3, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        let error_vec = errors.into_vec();
        assert_eq!(error_vec.len(), 1);
        assert_eq!(error_vec[0].code, ErrorCode::LazyGemInsideScope);
        assert!(error_vec[0].message.contains("task1"));
    }

    /// Lazy gems outside scope are accepted.
    ///
    /// This confirms the checker does not over-constrain standalone `@G` usage.
    #[test]
    fn test_lazy_gem_outside_scope_ok() {
        // @G can appear outside @Bg/@Eg scope
        let h1 = Header::LazyGem {
            label: Some(GemLabel::new("task1")),
        };
        let headers = vec![(&h1, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }

    /// Lazy gems are valid after all scopes are closed.
    ///
    /// Closed-scope state must reset permissive handling for `@G`.
    #[test]
    fn test_lazy_gem_after_scope_closed_ok() {
        // @Bg, @Eg, @G (after scope closed is ok)
        let h1 = Header::BeginGem { label: None };
        let h2 = Header::EndGem { label: None };
        let h3 = Header::LazyGem { label: None };
        let headers = vec![(&h1, Span::DUMMY), (&h2, Span::DUMMY), (&h3, Span::DUMMY)];

        let errors = ErrorCollector::new();
        check_gem_balance(&headers, &errors);

        assert!(errors.into_vec().is_empty());
    }
}
