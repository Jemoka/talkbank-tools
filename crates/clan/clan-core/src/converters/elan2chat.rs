//! ELAN XML (`.eaf`) to CHAT conversion.
//!
//! Converts ELAN annotation files (`.eaf`) into CHAT format. ELAN uses a
//! time-aligned annotation format stored as XML, with time slots referenced
//! by alignable annotations within tiers.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409298)
//! for the original ELAN2CHAT command documentation.
//!
//! # Conversion details
//!
//! - ELAN tier IDs are mapped to CHAT speaker codes (first 3 characters, uppercased)
//! - Time slots are resolved to millisecond timing bullets
//! - Annotations are merged across tiers and sorted by start time
//! - All speakers are assigned the `Unidentified` participant role
//!
//! # Differences from CLAN
//!
//! - Generates CHAT output via typed AST construction, ensuring well-formed
//!   output with valid headers, speaker codes, and terminators.
//! - ELAN tier IDs are mapped to 3-character uppercased speaker codes.
//! - Time alignment is preserved as millisecond timing bullets.

use std::collections::BTreeMap;

use quick_xml::Reader;
use quick_xml::events::Event;
use talkbank_model::Span;
use talkbank_model::{
    Bullet, ChatFile, Header, IDHeader, LanguageCode, LanguageCodes, Line, MainTier,
    ParticipantEntries, ParticipantEntry, ParticipantRole, SpeakerCode, Terminator, Utterance,
    UtteranceContent, Word,
};

use crate::framework::TransformError;

/// An annotation from an ELAN tier.
#[derive(Debug)]
struct ElanAnnotation {
    /// Start time in milliseconds.
    start_ms: u64,
    /// End time in milliseconds.
    end_ms: u64,
    /// Annotation text.
    text: String,
}

/// A parsed ELAN tier.
#[derive(Debug)]
struct ElanTier {
    /// Tier ID (used as speaker code).
    tier_id: String,
    /// Annotations in this tier.
    annotations: Vec<ElanAnnotation>,
}

/// Parse an ELAN EAF file into tiers with resolved time slots.
fn parse_elan(content: &str) -> Result<Vec<ElanTier>, TransformError> {
    let mut reader = Reader::from_str(content);

    let mut time_slots: BTreeMap<String, u64> = BTreeMap::new();
    let mut tiers: Vec<ElanTier> = Vec::new();

    // Parser state for tracking current position in the XML tree.
    let mut current_tier_id: Option<String> = None;
    let mut current_annotations: Vec<ElanAnnotation> = Vec::new();
    let mut current_ts1 = String::new();
    let mut current_ts2 = String::new();
    let mut in_annotation_value = false;
    let mut annotation_text = String::new();

    loop {
        match reader.read_event() {
            Ok(Event::Start(ref e) | Event::Empty(ref e)) => {
                match e.name().as_ref() {
                    b"TIME_SLOT" => {
                        let mut slot_id = None;
                        let mut slot_value = None;
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"TIME_SLOT_ID" => {
                                    slot_id = Some(
                                        attr.unescape_value()
                                            .map_err(|err| {
                                                TransformError::Parse(format!(
                                                    "Invalid TIME_SLOT_ID: {err}"
                                                ))
                                            })?
                                            .into_owned(),
                                    );
                                }
                                b"TIME_VALUE" => {
                                    slot_value = Some(
                                        attr.unescape_value()
                                            .map_err(|err| {
                                                TransformError::Parse(format!(
                                                    "Invalid TIME_VALUE: {err}"
                                                ))
                                            })?
                                            .into_owned(),
                                    );
                                }
                                _ => {}
                            }
                        }
                        if let (Some(id), Some(val)) = (slot_id, slot_value)
                            && let Ok(ms) = val.parse::<u64>()
                        {
                            time_slots.insert(id, ms);
                        }
                    }
                    b"TIER" => {
                        // Flush previous tier
                        if let Some(tid) = current_tier_id.take()
                            && !current_annotations.is_empty()
                        {
                            tiers.push(ElanTier {
                                tier_id: tid,
                                annotations: std::mem::take(&mut current_annotations),
                            });
                        }
                        let mut tier_id = "SPK".to_owned();
                        for attr in e.attributes().flatten() {
                            if attr.key.as_ref() == b"TIER_ID" {
                                tier_id = attr
                                    .unescape_value()
                                    .map_err(|err| {
                                        TransformError::Parse(format!("Invalid TIER_ID: {err}"))
                                    })?
                                    .into_owned();
                            }
                        }
                        current_tier_id = Some(tier_id);
                    }
                    b"ALIGNABLE_ANNOTATION" => {
                        current_ts1.clear();
                        current_ts2.clear();
                        for attr in e.attributes().flatten() {
                            match attr.key.as_ref() {
                                b"TIME_SLOT_REF1" => {
                                    current_ts1 =
                                        attr.unescape_value().unwrap_or_default().into_owned();
                                }
                                b"TIME_SLOT_REF2" => {
                                    current_ts2 =
                                        attr.unescape_value().unwrap_or_default().into_owned();
                                }
                                _ => {}
                            }
                        }
                    }
                    b"ANNOTATION_VALUE" => {
                        in_annotation_value = true;
                        annotation_text.clear();
                    }
                    _ => {}
                }
            }
            Ok(Event::Text(e)) if in_annotation_value => {
                let decoded = e.xml_content().map_err(|err| {
                    TransformError::Parse(format!("Invalid ANNOTATION_VALUE text: {err}"))
                })?;
                annotation_text.push_str(&decoded);
            }
            Ok(Event::GeneralRef(e)) if in_annotation_value => {
                let name = std::str::from_utf8(&e).unwrap_or("");
                let resolved = match name {
                    "amp" => "&",
                    "lt" => "<",
                    "gt" => ">",
                    "apos" => "'",
                    "quot" => "\"",
                    _ => "",
                };
                annotation_text.push_str(resolved);
            }
            Ok(Event::End(ref e)) => match e.name().as_ref() {
                b"ANNOTATION_VALUE" => {
                    in_annotation_value = false;
                    if !annotation_text.is_empty() {
                        let start_ms = time_slots.get(&current_ts1).copied().unwrap_or(0);
                        let end_ms = time_slots.get(&current_ts2).copied().unwrap_or(0);
                        current_annotations.push(ElanAnnotation {
                            start_ms,
                            end_ms,
                            text: std::mem::take(&mut annotation_text),
                        });
                    }
                }
                b"TIER" => {
                    if let Some(tid) = current_tier_id.take()
                        && !current_annotations.is_empty()
                    {
                        tiers.push(ElanTier {
                            tier_id: tid,
                            annotations: std::mem::take(&mut current_annotations),
                        });
                    }
                }
                _ => {}
            },
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(TransformError::Parse(format!(
                    "ELAN XML parse error at position {}: {e}",
                    reader.error_position()
                )));
            }
            _ => {}
        }
    }

    // Flush last tier
    if let Some(tid) = current_tier_id
        && !current_annotations.is_empty()
    {
        tiers.push(ElanTier {
            tier_id: tid,
            annotations: current_annotations,
        });
    }

    Ok(tiers)
}

/// Convert an ELAN EAF file to CHAT format with default options.
///
/// Uses `"eng"` as the language and `"elan_corpus"` as the corpus name.
pub fn elan_to_chat(content: &str) -> Result<ChatFile, TransformError> {
    elan_to_chat_with_options(content, "eng", "elan_corpus")
}

/// Convert an ELAN EAF file to CHAT format with custom options.
pub fn elan_to_chat_with_options(
    content: &str,
    language: &str,
    corpus: &str,
) -> Result<ChatFile, TransformError> {
    let tiers = parse_elan(content)?;
    let lang = LanguageCode::new(language);

    let speakers: Vec<SpeakerCode> = tiers
        .iter()
        .map(|t| {
            let code = t.tier_id.chars().take(3).collect::<String>().to_uppercase();
            SpeakerCode::new(&code)
        })
        .collect();

    let participant_entries: Vec<ParticipantEntry> = speakers
        .iter()
        .map(|code| ParticipantEntry {
            speaker_code: code.clone(),
            name: None,
            role: ParticipantRole::new("Unidentified"),
        })
        .collect();

    let mut lines = vec![
        Line::header(Header::Utf8),
        Line::header(Header::Begin),
        Line::header(Header::Languages {
            codes: LanguageCodes::new(vec![lang.clone()]),
        }),
        Line::header(Header::Participants {
            entries: ParticipantEntries::new(participant_entries),
        }),
    ];

    for spk in &speakers {
        lines.push(Line::header(Header::ID(
            IDHeader::new(
                lang.clone(),
                spk.clone(),
                ParticipantRole::new("Unidentified"),
            )
            .with_corpus(corpus),
        )));
    }

    // Merge all annotations, sorted by start time
    let mut all_annotations: Vec<(SpeakerCode, &ElanAnnotation)> = Vec::new();
    for (tier_idx, tier) in tiers.iter().enumerate() {
        for ann in &tier.annotations {
            all_annotations.push((speakers[tier_idx].clone(), ann));
        }
    }
    all_annotations.sort_by_key(|(_, ann)| ann.start_ms);

    for (spk, ann) in &all_annotations {
        let words: Vec<UtteranceContent> = ann
            .text
            .split_whitespace()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(w))))
            .collect();

        if words.is_empty() {
            continue;
        }

        let main_tier = MainTier::new(spk.clone(), words, Terminator::Period { span: Span::DUMMY })
            .with_bullet(Bullet::new(ann.start_ms, ann.end_ms));

        lines.push(Line::utterance(Utterance::new(main_tier)));
    }

    lines.push(Line::header(Header::End));

    Ok(ChatFile::new(lines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_elan_basic() {
        let eaf = r#"<?xml version="1.0" encoding="UTF-8"?>
<ANNOTATION_DOCUMENT>
<HEADER/>
<TIME_ORDER>
    <TIME_SLOT TIME_SLOT_ID="ts1" TIME_VALUE="0"/>
    <TIME_SLOT TIME_SLOT_ID="ts2" TIME_VALUE="1500"/>
</TIME_ORDER>
<TIER TIER_ID="CHI" LINGUISTIC_TYPE_REF="default">
    <ANNOTATION>
        <ALIGNABLE_ANNOTATION ANNOTATION_ID="a1" TIME_SLOT_REF1="ts1" TIME_SLOT_REF2="ts2">
            <ANNOTATION_VALUE>hello world</ANNOTATION_VALUE>
        </ALIGNABLE_ANNOTATION>
    </ANNOTATION>
</TIER>
</ANNOTATION_DOCUMENT>"#;

        let tiers = parse_elan(eaf).unwrap();
        assert_eq!(tiers.len(), 1);
        assert_eq!(tiers[0].annotations.len(), 1);
        assert_eq!(tiers[0].annotations[0].text, "hello world");
        assert_eq!(tiers[0].annotations[0].start_ms, 0);
        assert_eq!(tiers[0].annotations[0].end_ms, 1500);
    }

    #[test]
    fn parse_elan_xml_entities() {
        let eaf = r#"<?xml version="1.0" encoding="UTF-8"?>
<ANNOTATION_DOCUMENT>
<TIME_ORDER>
    <TIME_SLOT TIME_SLOT_ID="ts1" TIME_VALUE="0"/>
    <TIME_SLOT TIME_SLOT_ID="ts2" TIME_VALUE="1000"/>
</TIME_ORDER>
<TIER TIER_ID="CHI">
    <ANNOTATION>
        <ALIGNABLE_ANNOTATION ANNOTATION_ID="a1" TIME_SLOT_REF1="ts1" TIME_SLOT_REF2="ts2">
            <ANNOTATION_VALUE>it&apos;s &amp; &lt;good&gt;</ANNOTATION_VALUE>
        </ALIGNABLE_ANNOTATION>
    </ANNOTATION>
</TIER>
</ANNOTATION_DOCUMENT>"#;

        let tiers = parse_elan(eaf).unwrap();
        assert_eq!(tiers[0].annotations[0].text, "it's & <good>");
    }
}
