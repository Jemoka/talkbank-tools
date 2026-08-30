use std::path::Path;

use talkbank_model::model::{
    Header, IDHeader, LanguageCode, LanguageCodes, Line, MediaFilename, MediaHeader, MediaType,
    ParticipantEntries, ParticipantEntry, ParticipantName, ParticipantRole, SpeakerCode,
};

use super::TranscriptDescription;

pub(super) fn build_header_lines(
    desc: &TranscriptDescription,
    langs: &[String],
) -> Result<Vec<Line>, String> {
    let participant_entries = build_participant_entries(desc);
    let language_codes = langs
        .iter()
        .map(LanguageCode::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(|error| format!("invalid transcript language: {error}"))?;
    let id_headers = build_id_headers(desc, &language_codes);
    let mut lines: Vec<Line> = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::header(Header::Languages {
            codes: LanguageCodes::new(language_codes),
        }),
        Line::header(Header::Participants {
            entries: ParticipantEntries::new(participant_entries),
        }),
    ];

    for id in id_headers {
        lines.push(Line::header(Header::ID(id)));
    }

    if let Some(media_header) = build_media_header(desc)? {
        lines.push(Line::header(Header::Media(media_header)));
    }

    Ok(lines)
}

fn build_participant_entries(desc: &TranscriptDescription) -> Vec<ParticipantEntry> {
    desc.participants
        .iter()
        .map(|participant| ParticipantEntry {
            speaker_code: SpeakerCode::new(participant.id.as_str()),
            name: participant.name.as_ref().map(ParticipantName::new),
            role: ParticipantRole::new(participant.role.as_str()),
        })
        .collect()
}

fn build_id_headers(desc: &TranscriptDescription, langs: &[LanguageCode]) -> Vec<IDHeader> {
    let lang_code = langs.first().cloned().unwrap_or_else(LanguageCode::empty);

    desc.participants
        .iter()
        .map(|participant| {
            let corpus = if participant.corpus.is_empty() {
                "corpus_name"
            } else {
                participant.corpus.as_str()
            };
            IDHeader::new(
                lang_code.clone(),
                participant.id.as_str(),
                participant.role.as_str(),
            )
            .with_corpus(corpus)
        })
        .collect()
}

fn build_media_header(desc: &TranscriptDescription) -> Result<Option<MediaHeader>, String> {
    let Some(media_name) = desc.media_name.as_ref() else {
        return Ok(None);
    };
    let normalized_media_name = normalize_media_name(media_name);
    let media_type = match desc.media_type.as_deref() {
        Some("video") => MediaType::Video,
        Some("audio") => MediaType::Audio,
        None => infer_media_type(media_name),
        other => {
            tracing::warn!(media_type = ?other, "unrecognized media_type, defaulting to audio");
            MediaType::Audio
        }
    };

    let filename = MediaFilename::parse(normalized_media_name.as_str())
        .map_err(|error| format!("invalid media filename: {error}"))?;
    Ok(Some(MediaHeader::new(filename, media_type)))
}

/// Infer the CHAT capture modality while the media filename still has its
/// extension. `@Media` serialization intentionally drops that extension, so
/// this is the last reliable construction boundary at which to distinguish a
/// movie from an audio recording without carrying duplicate metadata.
fn infer_media_type(media_name: &str) -> MediaType {
    let extension = Path::new(media_name)
        .extension()
        .and_then(|extension| extension.to_str());

    if extension.is_some_and(|extension| {
        matches!(
            extension.to_ascii_lowercase().as_str(),
            "mp4" | "mov" | "m4v" | "avi" | "mpg" | "mpeg"
        )
    }) {
        MediaType::Video
    } else {
        MediaType::Audio
    }
}

fn normalize_media_name(raw: &str) -> String {
    let candidate = Path::new(raw);
    candidate
        .file_stem()
        .filter(|stem| !stem.is_empty())
        .or_else(|| candidate.file_name())
        .filter(|name| !name.is_empty())
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| raw.to_string())
}
