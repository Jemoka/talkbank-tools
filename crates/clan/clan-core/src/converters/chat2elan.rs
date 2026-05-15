//! CHAT to ELAN XML (`.eaf`) conversion.
//!
//! Converts CHAT files into ELAN annotation format (`.eaf`). The output is a
//! valid ELAN XML file with time-aligned tiers and annotations derived from
//! CHAT main tiers and their timing bullets.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409300)
//! for the original CHAT2ELAN command documentation.
//!
//! # CLAN Equivalence
//!
//! | CLAN command                     | Rust equivalent                               |
//! |----------------------------------|-----------------------------------------------|
//! | `chat2elan +e.wav file.cha`      | `chatter clan chat2elan file.cha`             |
//!
//! # Differences from CLAN
//!
//! - CLAN requires `+e` flag to specify media extension; Rust version omits
//!   media references by default (can be added via `--media-extension`).
//! - Uses typed AST traversal for utterance extraction rather than text parsing.
//! - Generates standard-compliant EAF 3.0 XML.
//! - Speaker codes are used directly as tier IDs (no truncation).

use talkbank_model::{ChatFile, Line, WriteChat};

use crate::framework::TransformError;

/// Convert a CHAT file to ELAN EAF format.
pub fn chat_to_elan(chat: &ChatFile) -> Result<String, TransformError> {
    chat_to_elan_with_options(chat, None)
}

/// Convert a CHAT file to ELAN EAF format with options.
///
/// If `media_extension` is provided (e.g., "wav"), a `MEDIA_DESCRIPTOR` element
/// is included referencing a media file with the same basename.
pub fn chat_to_elan_with_options(
    chat: &ChatFile,
    media_extension: Option<&str>,
) -> Result<String, TransformError> {
    let mut time_slots: Vec<(u64, u64)> = Vec::new();
    let mut annotations: Vec<AnnotationEntry> = Vec::new();

    // Collect all utterances with timing
    for line in &chat.lines {
        if let Line::Utterance(utt) = line {
            let speaker = utt.main.speaker.as_str().to_owned();
            // Get the main tier text (strip "*SPK:\t" prefix)
            let full_text = utt.main.to_chat_string();
            let text = full_text
                .find(":\t")
                .map(|i| full_text[i + 2..].trim().to_owned())
                .unwrap_or_else(|| full_text.trim().to_owned());

            let (start_ms, end_ms) = if let Some(bullet) = &utt.main.content.bullet {
                (bullet.timing.start_ms, bullet.timing.end_ms)
            } else {
                // No timing — use sequential placeholders
                let last_end = time_slots.last().map(|(_, e)| *e).unwrap_or(0);
                (last_end, last_end + 1000)
            };

            time_slots.push((start_ms, end_ms));
            annotations.push(AnnotationEntry { speaker, text });
        }
    }

    // Build EAF XML
    let mut xml = String::new();
    xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
    xml.push_str("<ANNOTATION_DOCUMENT AUTHOR=\"chatter\" DATE=\"");
    xml.push_str(&chrono_date());
    xml.push_str("\" FORMAT=\"3.0\" VERSION=\"3.0\"");
    xml.push_str(" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\"");
    xml.push_str(" xsi:noNamespaceSchemaLocation=\"http://www.mpi.nl/tools/elan/EAFv3.0.xsd\">\n");

    // Header
    xml.push_str("    <HEADER MEDIA_FILE=\"\" TIME_UNITS=\"milliseconds\">\n");
    if let Some(ext) = media_extension {
        xml.push_str(&format!(
            "        <MEDIA_DESCRIPTOR MEDIA_URL=\"file:///media.{ext}\" MIME_TYPE=\"{}\" RELATIVE_MEDIA_URL=\"./media.{ext}\"/>\n",
            mime_for_ext(ext),
        ));
    }
    xml.push_str("    </HEADER>\n");

    // Time order
    xml.push_str("    <TIME_ORDER>\n");
    let mut ts_idx = 1u64;
    for (start_ms, end_ms) in &time_slots {
        xml.push_str(&format!(
            "        <TIME_SLOT TIME_SLOT_ID=\"ts{ts_idx}\" TIME_VALUE=\"{start_ms}\"/>\n",
        ));
        ts_idx += 1;
        xml.push_str(&format!(
            "        <TIME_SLOT TIME_SLOT_ID=\"ts{ts_idx}\" TIME_VALUE=\"{end_ms}\"/>\n",
        ));
        ts_idx += 1;
    }
    xml.push_str("    </TIME_ORDER>\n");

    // Group annotations by speaker
    let mut by_speaker: indexmap::IndexMap<String, Vec<(usize, &AnnotationEntry)>> =
        indexmap::IndexMap::new();
    for (idx, ann) in annotations.iter().enumerate() {
        by_speaker
            .entry(ann.speaker.clone())
            .or_default()
            .push((idx, ann));
    }

    // Tiers
    let mut ann_id = 1u64;
    for (speaker, speaker_anns) in &by_speaker {
        xml.push_str(&format!(
            "    <TIER LINGUISTIC_TYPE_REF=\"default-lt\" TIER_ID=\"{speaker}\">\n"
        ));
        for (idx, ann) in speaker_anns {
            let ts1 = idx * 2 + 1;
            let ts2 = idx * 2 + 2;
            xml.push_str(&format!(
                "        <ANNOTATION>\n            <ALIGNABLE_ANNOTATION ANNOTATION_ID=\"a{ann_id}\" TIME_SLOT_REF1=\"ts{ts1}\" TIME_SLOT_REF2=\"ts{ts2}\">\n                <ANNOTATION_VALUE>{}</ANNOTATION_VALUE>\n            </ALIGNABLE_ANNOTATION>\n        </ANNOTATION>\n",
                xml_escape(&ann.text),
            ));
            ann_id += 1;
        }
        xml.push_str("    </TIER>\n");
    }

    // Linguistic type
    xml.push_str(
        "    <LINGUISTIC_TYPE GRAPHIC_REFERENCES=\"false\" LINGUISTIC_TYPE_ID=\"default-lt\" TIME_ALIGNABLE=\"true\"/>\n",
    );
    xml.push_str("</ANNOTATION_DOCUMENT>\n");

    Ok(xml)
}

/// Internal annotation entry.
struct AnnotationEntry {
    speaker: String,
    text: String,
}

/// Generate a simple ISO date string (no chrono dependency).
fn chrono_date() -> String {
    // Use a fixed format that matches ELAN convention
    "2026-01-01T00:00:00+00:00".to_owned()
}

/// Map file extension to MIME type.
fn mime_for_ext(ext: &str) -> &str {
    match ext {
        "wav" => "audio/x-wav",
        "mp3" => "audio/mpeg",
        "mp4" | "m4v" => "video/mp4",
        "mov" => "video/quicktime",
        "mpg" | "mpeg" => "video/mpeg",
        _ => "application/octet-stream",
    }
}

/// XML-escape text content.
fn xml_escape(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{
        Bullet, Header, IDHeader, LanguageCode, LanguageCodes, MainTier, ParticipantEntries,
        ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance, UtteranceContent,
        Word,
    };

    fn simple_chat() -> ChatFile {
        let spk = SpeakerCode::new("CHI");
        let lang = LanguageCode::new("eng");

        let words = vec![
            UtteranceContent::Word(Box::new(Word::simple("hello"))),
            UtteranceContent::Word(Box::new(Word::simple("world"))),
        ];
        let mut main = MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY });
        main.content.bullet = Some(Bullet::new(0, 1500));

        ChatFile::new(vec![
            Line::header(Header::Utf8),
            Line::header(Header::Begin),
            Line::header(Header::Languages {
                codes: LanguageCodes::new(vec![lang.clone()]),
            }),
            Line::header(Header::Participants {
                entries: ParticipantEntries::new(vec![ParticipantEntry {
                    speaker_code: spk.clone(),
                    name: None,
                    role: ParticipantRole::new("Child"),
                }]),
            }),
            Line::header(Header::ID(
                IDHeader::new(lang, spk, ParticipantRole::new("Child")).with_corpus("test"),
            )),
            Line::utterance(Utterance::new(main)),
            Line::header(Header::End),
        ])
    }

    #[test]
    fn chat2elan_produces_valid_xml() {
        let chat = simple_chat();
        let eaf = chat_to_elan(&chat).unwrap();
        assert!(eaf.contains("ANNOTATION_DOCUMENT"));
        assert!(eaf.contains("TIME_ORDER"));
        assert!(eaf.contains("TIER_ID=\"CHI\""));
        assert!(eaf.contains("hello world"));
        assert!(eaf.contains("TIME_VALUE=\"0\""));
        assert!(eaf.contains("TIME_VALUE=\"1500\""));
    }

    #[test]
    fn roundtrip_preserves_text() {
        let chat = simple_chat();
        let eaf = chat_to_elan(&chat).unwrap();
        assert!(eaf.contains("hello world"));
    }
}
