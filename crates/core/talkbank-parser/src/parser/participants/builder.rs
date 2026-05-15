//! Test-only participant map builder from parsed header lines.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>

use indexmap::IndexMap;
use smallvec::SmallVec;
use talkbank_model::model::{Header, Line, Participant, SpeakerCode};
use talkbank_model::{ErrorCode, ErrorContext, ParseError, Severity, SourceLocation, Span};

#[cfg(test)]
use super::birth::find_birth_header;

type ErrorVec = SmallVec<[ParseError; 2]>;

/// Build participant map from headers
///
/// Matches each @Participants entry with its corresponding @ID header.
/// Associates optional @Birth of <CODE> headers.
///
/// # Returns
///
/// `(participants_map, errors)` where:
/// - `participants_map`: IndexMap of speaker code -> Participant (matches @Participants order)
/// - `errors`: Collection of validation errors (E522, E523, E524)
///
/// # Example
///
/// ```rust,ignore
/// let headers = vec![
///     Header::Participants { entries: vec![...] },
///     Header::ID(...),
///     Header::Birth { participant: "CHI", date: "28-JUN-2001" },
/// ];
///
/// let (participants, errors) = build_participants(&headers);
/// assert_eq!(participants.len(), 1);
/// assert!(errors.is_empty());
/// ```
#[cfg(test)]
pub fn build_participants(headers: &[Header]) -> (IndexMap<SpeakerCode, Participant>, ErrorVec) {
    let mut errors = ErrorVec::new();
    let mut participants = IndexMap::new();

    // Find @Participants entries. If header is missing, we cannot build the map.
    let Some(entries) = headers.iter().find_map(|header| match header {
        Header::Participants { entries } => Some(entries),
        _ => None,
    }) else {
        return (participants, errors);
    };

    // Collect all @ID headers for matching
    let id_headers: Vec<&Header> = headers
        .iter()
        .filter(|h| matches!(h, Header::ID(_)))
        .collect();

    // For each @Participants entry, find matching @ID
    for entry in entries {
        let speaker_code = entry.speaker_code.clone();
        let speaker_str = speaker_code.as_str();

        // Find @ID header for this speaker
        let matching_id = id_headers.iter().find_map(|h| match h {
            Header::ID(id) if id.speaker.as_str() == speaker_str => Some(id.clone()),
            _ => None,
        });

        match matching_id {
            Some(id) => {
                // Create participant from entry + ID
                let mut participant = Participant::new(entry.clone(), id);

                // Find matching @Birth of <CODE> header
                if let Some(birth_date) = find_birth_header(speaker_str, headers) {
                    participant = participant.with_birth_date(birth_date);
                }

                participants.insert(speaker_code, participant);
            }
            None => {
                // E522: Missing @ID for participant
                errors.push(
                    ParseError::new(
                        ErrorCode::SpeakerNotDefined,
                        Severity::Error,
                        SourceLocation::at_offset(0),
                        ErrorContext::new(speaker_str, 0..speaker_str.len(), speaker_str),
                        format!(
                            "Speaker '{}' declared in @Participants but has no matching @ID header",
                            speaker_str
                        ),
                    )
                    .with_suggestion(format!(
                        "Add @ID header: @ID:\t<lang>|<corpus>|{}|<age>|<sex>|<group>|<ses>|{}|<edu>|<custom>|",
                        speaker_str, entry.role
                    )),
                );
            }
        }
    }

    // Check for orphan @ID headers (ID without @Participants entry)
    for id_header in &id_headers {
        if let Header::ID(id) = id_header
            && !participants.contains_key(&id.speaker)
        {
            // E523: Orphan @ID header (warning)
            let speaker_str = id.speaker.to_string();
            errors.push(
                ParseError::new(
                    ErrorCode::OrphanIDHeader,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(&speaker_str, 0..speaker_str.len(), &speaker_str),
                    format!(
                        "@ID header for '{}' but speaker not in @Participants",
                        speaker_str
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>",
                    speaker_str
                )),
            );
        }
    }

    // Check for orphan @Birth headers
    let birth_headers: Vec<&Header> = headers
        .iter()
        .filter(|h| matches!(h, Header::Birth { .. }))
        .collect();

    for birth_header in birth_headers {
        if let Header::Birth {
            participant: speaker,
            ..
        } = birth_header
            && !participants.contains_key(speaker)
        {
            // E524: @Birth for unknown participant (warning)
            errors.push(
                ParseError::new(
                    ErrorCode::BirthUnknownParticipant,
                    Severity::Error,
                    SourceLocation::at_offset(0),
                    ErrorContext::new(speaker.as_str(), 0..speaker.len(), speaker.as_str()),
                    format!(
                        "@Birth header for '{}' but speaker not a declared participant",
                        speaker
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>, or remove @Birth header",
                    speaker
                )),
            );
        }
    }

    (participants, errors)
}

/// Build participant map from parsed header lines, preserving source spans in
/// diagnostics.
///
/// Scans `lines` for `@Participants`, `@ID`, and `@Birth of` headers, then
/// cross-references them to build a complete participant map. Unlike
/// [`build_participants`] (test-only), this variant works with `Line` items that
/// carry their original source `Span`, so emitted diagnostics point to the
/// actual header line rather than offset 0.
///
/// # Parameters
///
/// - `lines`: The full sequence of parsed lines from a CHAT file. Only
///   `Line::Header` variants are inspected; utterance and dependent tier lines
///   are skipped.
///
/// # Returns
///
/// `(participants_map, errors)` where:
/// - `participants_map`: An `IndexMap<SpeakerCode, Participant>` preserving the
///   declaration order from `@Participants`. Each entry is enriched with data
///   from the matching `@ID` and optional `@Birth of` headers.
/// - `errors`: A small-vec of validation diagnostics:
///   - **E522** (`SpeakerNotDefined`): A speaker in `@Participants` has no `@ID`.
///   - **E523** (`OrphanIDHeader`): An `@ID` header has no `@Participants` entry.
///   - **E524** (`BirthUnknownParticipant`): A `@Birth of` header names a speaker
///     not in the participant map.
pub fn build_participants_from_lines(
    lines: &[Line],
) -> (IndexMap<SpeakerCode, Participant>, ErrorVec) {
    let mut errors = ErrorVec::new();
    let mut participants = IndexMap::new();

    let header_lines: Vec<(&Header, Span)> = lines
        .iter()
        .filter_map(|line| match line {
            Line::Header { header, span } => Some((header.as_ref(), *span)),
            _ => None,
        })
        .collect();

    let Some((entries, participants_span)) =
        header_lines.iter().find_map(|(header, span)| match header {
            Header::Participants { entries } => Some((entries, *span)),
            _ => None,
        })
    else {
        return (participants, errors);
    };

    let id_headers: Vec<(&Header, Span)> = header_lines
        .iter()
        .copied()
        .filter(|(header, _)| matches!(header, Header::ID(_)))
        .collect();

    for entry in entries {
        let speaker_code = entry.speaker_code.clone();
        let speaker_str = speaker_code.as_str();

        let matching_id = id_headers.iter().find_map(|(header, _)| match header {
            Header::ID(id) if id.speaker.as_str() == speaker_str => Some(id.clone()),
            _ => None,
        });

        match matching_id {
            Some(id) => {
                let mut participant = Participant::new(entry.clone(), id);

                if let Some(birth_date) = header_lines.iter().find_map(|(header, _)| match header {
                    Header::Birth { participant, date } if participant.as_str() == speaker_str => {
                        Some(date.clone())
                    }
                    _ => None,
                }) {
                    participant = participant.with_birth_date(birth_date);
                }

                participants.insert(speaker_code, participant);
            }
            None => {
                errors.push(
                    ParseError::new(
                        ErrorCode::SpeakerNotDefined,
                        Severity::Error,
                        SourceLocation::new(participants_span),
                        ErrorContext::new(speaker_str, 0..speaker_str.len(), speaker_str),
                        format!(
                            "Speaker '{}' declared in @Participants but has no matching @ID header",
                            speaker_str
                        ),
                    )
                    .with_suggestion(format!(
                        "Add @ID header: @ID:\t<lang>|<corpus>|{}|<age>|<sex>|<group>|<ses>|{}|<edu>|<custom>|",
                        speaker_str, entry.role
                    )),
                );
            }
        }
    }

    for (id_header, id_span) in &id_headers {
        if let Header::ID(id) = id_header
            && !participants.contains_key(&id.speaker)
        {
            let speaker_str = id.speaker.to_string();
            errors.push(
                ParseError::new(
                    ErrorCode::OrphanIDHeader,
                    Severity::Error,
                    SourceLocation::new(*id_span),
                    ErrorContext::new(&speaker_str, 0..speaker_str.len(), &speaker_str),
                    format!(
                        "@ID header for '{}' but speaker not in @Participants",
                        speaker_str
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>",
                    speaker_str
                )),
            );
        }
    }

    let birth_headers: Vec<(&Header, Span)> = header_lines
        .iter()
        .copied()
        .filter(|(header, _)| matches!(header, Header::Birth { .. }))
        .collect();

    for (birth_header, birth_span) in birth_headers {
        if let Header::Birth {
            participant: speaker,
            ..
        } = birth_header
            && !participants.contains_key(speaker)
        {
            errors.push(
                ParseError::new(
                    ErrorCode::BirthUnknownParticipant,
                    Severity::Error,
                    SourceLocation::new(birth_span),
                    ErrorContext::new(speaker.as_str(), 0..speaker.len(), speaker.as_str()),
                    format!(
                        "@Birth header for '{}' but speaker not a declared participant",
                        speaker
                    ),
                )
                .with_suggestion(format!(
                    "Add to @Participants: {} <name> <role>, or remove @Birth header",
                    speaker
                )),
            );
        }
    }

    (participants, errors)
}
