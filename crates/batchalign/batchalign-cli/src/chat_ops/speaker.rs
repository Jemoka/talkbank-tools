//! Speaker diarization application over a CHAT AST.
//!
//! The Rust control plane owns when diarization runs and how raw segments are
//! applied to utterances. Python returns only raw `start/end/speaker` segments;
//! this module rewrites utterance speaker codes plus the corresponding
//! `@Participants` / `@ID` headers.

use std::collections::HashMap;

use talkbank_model::model::{
    ChatFile, Header, IDHeader, Line, ParticipantEntries, ParticipantEntry, ParticipantName,
    ParticipantRole, SpeakerCode,
};

/// One raw diarization segment to apply to utterance bullets.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, PartialEq, Eq)]
pub struct SpeakerSegment {
    /// Segment start in milliseconds.
    pub start_ms: u64,
    /// Segment end in milliseconds.
    pub end_ms: u64,
    /// Stable speaker label emitted by the model host.
    pub speaker: String,
}

/// Reassign utterance speaker codes from diarization segments.
///
/// Segment assignment follows the historical batchalign2/PyO3 heuristic used by
/// the Pyannote path: each utterance takes the last diarization segment whose
/// end time is at or before the utterance end bullet, with a first-segment
/// fallback when no segment ends before the utterance.
pub fn reassign_speakers(
    chat_file: &mut ChatFile,
    segments: &[SpeakerSegment],
    lang: &str,
    participant_ids: &[String],
) {
    if segments.is_empty() {
        return;
    }

    let mut seen_speakers: Vec<String> = Vec::new();
    for segment in segments {
        if !seen_speakers.contains(&segment.speaker) {
            seen_speakers.push(segment.speaker.clone());
        }
    }

    let speaker_to_code: HashMap<&str, String> = seen_speakers
        .iter()
        .enumerate()
        .map(|(index, speaker)| {
            let code = participant_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("SP{index}"));
            (speaker.as_str(), code)
        })
        .collect();

    let mut lines: Vec<Line> = chat_file.lines.iter().cloned().collect();

    for line in &mut lines {
        let utterance = match line {
            Line::Utterance(utterance) => utterance,
            _ => continue,
        };

        let Some(bullet) = &utterance.main.content.bullet else {
            continue;
        };

        let utterance_end = bullet.timing.end_ms;
        let mut best_speaker: Option<&str> = None;
        for segment in segments {
            if segment.end_ms <= utterance_end {
                best_speaker = Some(&segment.speaker);
            }
        }

        let Some(speaker) =
            best_speaker.or_else(|| segments.first().map(|segment| segment.speaker.as_str()))
        else {
            continue;
        };
        let Some(code) = speaker_to_code.get(speaker) else {
            continue;
        };
        utterance.main.speaker = SpeakerCode::new(code);
    }

    let participant_entries: Vec<ParticipantEntry> = seen_speakers
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let code = participant_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("SP{index}"));
            ParticipantEntry {
                speaker_code: SpeakerCode::new(code),
                name: Some(ParticipantName::new("Participant")),
                role: ParticipantRole::new("Participant"),
            }
        })
        .collect();
    let id_headers: Vec<IDHeader> = seen_speakers
        .iter()
        .enumerate()
        .map(|(index, _)| {
            let code = participant_ids
                .get(index)
                .cloned()
                .unwrap_or_else(|| format!("SP{index}"));
            IDHeader::new(lang, code.as_str(), "Participant").with_corpus("corpus_name")
        })
        .collect();

    let mut rebuilt_lines: Vec<Line> = Vec::with_capacity(lines.len() + id_headers.len());
    let mut inserted_participants = false;
    for line in lines {
        match &line {
            Line::Header { header, .. } => match header.as_ref() {
                Header::Participants { .. } => {
                    if !inserted_participants {
                        rebuilt_lines.push(Line::header(Header::Participants {
                            entries: ParticipantEntries::new(participant_entries.clone()),
                        }));
                        for id in &id_headers {
                            rebuilt_lines.push(Line::header(Header::ID(id.clone())));
                        }
                        inserted_participants = true;
                    }
                }
                Header::ID(_) => {}
                _ => rebuilt_lines.push(line),
            },
            _ => rebuilt_lines.push(line),
        }
    }

    *chat_file = ChatFile::new(rebuilt_lines);
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::parse::{TreeSitterParser, parse_strict};
    use talkbank_transform::serialize::to_chat_string;

    #[test]
    fn reassign_speakers_rewrites_utterances_and_headers() {
        let parser = TreeSitterParser::new().unwrap();
        let input = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant Participant
@ID:\teng|corpus_name|PAR|||||Participant|||
*PAR:\thello . \u{0015}100_500\u{0015}
*PAR:\tworld . \u{0015}1000_2000\u{0015}
@End
";
        let mut chat_file = parse_strict(&parser, input).expect("chat should parse");
        let segments = vec![
            SpeakerSegment {
                start_ms: 0,
                end_ms: 600,
                speaker: "SPEAKER_0".into(),
            },
            SpeakerSegment {
                start_ms: 800,
                end_ms: 1900,
                speaker: "SPEAKER_1".into(),
            },
        ];

        reassign_speakers(
            &mut chat_file,
            &segments,
            "eng",
            &["PAR".to_string(), "INV".to_string()],
        );

        let output = to_chat_string(&chat_file);
        assert!(output.contains("*PAR:\thello ."));
        assert!(output.contains("*INV:\tworld ."));
        assert!(
            output.contains(
                "@Participants:\tPAR Participant Participant, INV Participant Participant"
            )
        );
        assert_eq!(output.matches("@ID:").count(), 2);
        assert!(
            !output.contains("|SP"),
            "unexpected fallback speaker code: {output}"
        );
    }

    #[test]
    fn reassign_speakers_is_noop_for_empty_segments() {
        let parser = TreeSitterParser::new().unwrap();
        let input = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant Participant
@ID:\teng|corpus_name|PAR|||||Participant|||
*PAR:\thello . \u{0015}100_500\u{0015}
@End
";
        let mut chat_file = parse_strict(&parser, input).expect("chat should parse");
        let before = to_chat_string(&chat_file);

        reassign_speakers(&mut chat_file, &[], "eng", &["PAR".to_string()]);

        assert_eq!(to_chat_string(&chat_file), before);
    }
}
