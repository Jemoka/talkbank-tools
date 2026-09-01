//! `SpeakerTaskRunner` — assigns utterances to speakers via diarization.

use crate::base::BAValue;
use crate::base::Chat;
use crate::base::ProgressEvent;
use crate::base::ProgressSink;
use crate::base::Task;
use crate::base::TaskInput;
use crate::base::{Dispatcher, TaskRunner};
use crate::proto::speaker::{DiarizationSegment, SpeakerInput, SpeakerOutput};
use crate::segmentation::split_utterance;
use crate::utils::{BAError, BAResult, MediaInput, prepare_pcm};
use async_trait::async_trait;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use talkbank_model::alignment::helpers::{TierDomain, WordItem, walk_words};
use talkbank_model::model::{
    Bullet, Header, IDHeader, Participant, ParticipantEntries, ParticipantEntry, ParticipantRole,
    Utterance,
};
use talkbank_model::{Line, SpeakerCode};

const SIBLING_MEDIA_EXTS: &[&str] = &[
    "wav", "mp3", "mp4", "m4a", "flac", "ogg", "aac", "wma", "mov", "m4v", "avi", "mpg", "mpeg",
];

/// Resolve media for CHAT loaded from disk. Prefer the typed `@Media`
/// basename, then fall back to the transcript's own stem.
fn sibling_media(chat: &Chat) -> Option<MediaInput> {
    let chat_path = Path::new(chat.source_id().as_str());
    let mut stems = Vec::new();
    if let (Some(parent), Some(header)) = (chat_path.parent(), chat.ast().media.as_deref()) {
        stems.push(parent.join(header.filename.as_str()));
    }
    stems.push(chat_path.with_extension(""));

    for stem in stems {
        for ext in SIBLING_MEDIA_EXTS {
            for candidate_ext in [ext.to_string(), ext.to_ascii_uppercase()] {
                let candidate = stem.with_extension(candidate_ext);
                if candidate.is_file() {
                    return Some(MediaInput::new(chat.source_id().clone(), candidate));
                }
            }
        }
    }
    None
}

pub struct SpeakerTaskRunner;

#[async_trait]
impl TaskRunner for SpeakerTaskRunner {
    const TASK: Task = Task::Speaker;

    async fn apply(
        &self,
        value: &mut BAValue,
        dispatcher: &dyn Dispatcher,
        sink: std::sync::Arc<dyn ProgressSink>,
    ) -> BAResult<()> {
        let chat = match value {
            BAValue::Chat(c) => c,
            BAValue::Failed { .. } => return Ok(()),
            other => {
                return Err(BAError::Internal(format!(
                    "SpeakerTaskRunner: expected BAValue::Chat, got {}",
                    other.kind()
                )));
            }
        };

        let media = match chat.media().cloned() {
            Some(media) => media,
            None => sibling_media(chat).ok_or_else(|| {
                BAError::Internal(
                    "SpeakerTaskRunner: chat has no attached media and no @Media/sibling media file was found"
                        .into(),
                )
            })?,
        };

        sink.emit(ProgressEvent::stage_started(
            chat.source_id(),
            Task::Speaker,
        ));

        let audio =
            prepare_pcm(&media).map_err(|e| BAError::Internal(format!("audio_prep: {e:#}")))?;

        let input = SpeakerInput {
            source_id: chat.source_id().clone(),
            audio,
            // Default 0 = "let the backend decide". The backend's
            // constructor (`PyannoteBackend(num_speakers=3)`) is where the
            // user pins a specific count.
            num_speakers: 0,
        };

        let output_raw = dispatcher.dispatch(TaskInput::Speaker(input)).await?;
        let output: SpeakerOutput = output_raw.try_into()?;

        relabel_utterances_by_diarization(chat, &output, &*sink)?;

        sink.emit(ProgressEvent::stage_injected(
            chat.source_id(),
            Task::Speaker,
        ));
        Ok(())
    }
}

fn relabel_utterances_by_diarization(
    chat: &mut Chat,
    out: &SpeakerOutput,
    sink: &dyn ProgressSink,
) -> BAResult<()> {
    let segs = &out.diarization.segments;
    if segs.is_empty() {
        return Ok(());
    }
    // Diarizer labels are opaque. Map them deterministically onto the CHAT's
    // declared participant order so Rev's second (Speaker) projection does
    // not replace the valid PAR0/PAR1 codes created by its ASR projection
    // with undeclared vendor labels such as "speaker0".
    let labels: BTreeSet<&str> = segs
        .iter()
        .map(|segment| segment.speaker.as_str())
        .collect();
    let declared: Vec<SpeakerCode> = chat.ast().participants.keys().cloned().collect();
    let speaker_codes: BTreeMap<&str, SpeakerCode> = labels
        .into_iter()
        .enumerate()
        .map(|(index, label)| {
            let code = declared
                .get(index)
                .cloned()
                .unwrap_or_else(|| SpeakerCode::new(format!("PAR{index}")));
            (label, code)
        })
        .collect();
    let mut added_participants = Vec::new();
    if speaker_codes
        .values()
        .any(|code| !chat.ast().participants.contains_key(code))
    {
        let template = chat
            .ast()
            .participants
            .values()
            .next()
            .cloned()
            .ok_or_else(|| {
                BAError::Internal(
                    "SpeakerTaskRunner: cannot declare diarized speakers without a participant template"
                        .into(),
                )
            })?;
        for code in speaker_codes.values() {
            if chat.ast().participants.contains_key(code) {
                continue;
            }
            let role = ParticipantRole::new("Participant");
            let entry = ParticipantEntry {
                speaker_code: code.clone(),
                name: None,
                role: role.clone(),
            };
            let id = IDHeader::from_languages(template.id.language.clone(), code.clone(), role)
                .with_corpus(template.id.corpus.clone());
            let participant = Participant::new(entry, id);
            chat.ast_mut()
                .participants
                .insert(code.clone(), participant.clone());
            added_participants.push(participant);
        }
    }
    let participant_entries = ParticipantEntries::new(
        chat.ast()
            .participants
            .values()
            .map(|participant| ParticipantEntry {
                speaker_code: participant.code.clone(),
                name: participant.name.clone(),
                role: participant.role.clone(),
            })
            .collect(),
    );
    let source_id = chat.source_id().clone();
    let total = chat
        .ast()
        .lines
        .as_slice()
        .iter()
        .filter(|l| matches!(l, Line::Utterance(_)))
        .count() as u64;
    let mut completed: u64 = 0;
    let old_lines = chat.ast_mut().lines.take();
    let mut new_lines = Vec::with_capacity(old_lines.len());
    for line in old_lines {
        let Line::Utterance(utterance) = line else {
            let mut header_line = line;
            if let Line::Header { header, .. } = &mut header_line
                && let Header::Participants { entries } = header.as_mut()
            {
                *entries = participant_entries.clone();
            }
            new_lines.push(header_line);
            continue;
        };

        let assignments = speaker_assignments(&utterance, segs);
        for mut child in split_utterance(*utterance, &assignments) {
            if let Some((start_ms, end_ms)) = utterance_timing_ms(&child) {
                child.main.content.bullet = Some(Bullet::new(start_ms, end_ms));
                let midpoint = start_ms + (end_ms - start_ms) / 2;
                if let Some(segment_index) = segment_index_at(segs, midpoint)
                    && let Some(code) = speaker_codes.get(segs[segment_index].speaker.as_str())
                {
                    child.main.speaker = code.clone();
                }
            }
            new_lines.push(Line::Utterance(Box::new(child)));
        }
        completed += 1;
        sink.emit(ProgressEvent::stage_tick(
            &source_id,
            Task::Speaker,
            completed,
            total,
        ));
    }
    if !added_participants.is_empty() {
        let insert_at = new_lines
            .iter()
            .rposition(|line| {
                matches!(
                    line,
                    Line::Header { header, .. } if matches!(header.as_ref(), Header::ID(_))
                )
            })
            .map_or(0, |index| index + 1);
        new_lines.splice(
            insert_at..insert_at,
            added_participants
                .into_iter()
                .map(|participant| Line::header(Header::ID(participant.id))),
        );
    }
    chat.ast_mut().lines = new_lines.into();
    Ok(())
}

fn speaker_assignments(utterance: &Utterance, segments: &[DiarizationSegment]) -> Vec<usize> {
    let mut timings = Vec::new();
    walk_words(
        utterance.main.content.content.as_slice(),
        Some(TierDomain::Mor),
        &mut |word| timings.push(word_timing(&word)),
    );
    if timings.iter().all(Option::is_none)
        && let Some(wor) = utterance.wor_tier()
    {
        timings = wor
            .words()
            .map(|word| word_timing(&WordItem::Word(word)))
            .collect();
    }

    let mut segment_indices: Vec<Option<usize>> = timings
        .iter()
        .map(|timing| {
            timing.and_then(|(start_ms, end_ms)| {
                segment_index_at(segments, start_ms + (end_ms - start_ms) / 2)
            })
        })
        .collect();

    let mut prior = None;
    for segment_index in &mut segment_indices {
        if segment_index.is_some() {
            prior = *segment_index;
        } else {
            *segment_index = prior;
        }
    }
    let mut following = None;
    for segment_index in segment_indices.iter_mut().rev() {
        if segment_index.is_some() {
            following = *segment_index;
        } else {
            *segment_index = following;
        }
    }

    let mut group = 0;
    let mut prior_speaker = None;
    segment_indices
        .into_iter()
        .map(|segment_index| {
            let speaker = segment_index.map(|index| segments[index].speaker.as_str());
            if prior_speaker.is_some() && speaker != prior_speaker {
                group += 1;
            }
            prior_speaker = speaker;
            group
        })
        .collect()
}

fn segment_index_at(segments: &[DiarizationSegment], timestamp_ms: u64) -> Option<usize> {
    segments
        .iter()
        .position(|segment| timestamp_ms >= segment.start_ms && timestamp_ms <= segment.end_ms)
        .or_else(|| {
            segments
                .iter()
                .enumerate()
                .min_by_key(|(_, segment)| {
                    segment
                        .start_ms
                        .abs_diff(timestamp_ms)
                        .min(segment.end_ms.abs_diff(timestamp_ms))
                })
                .map(|(index, _)| index)
        })
}

fn utterance_timing_ms(utterance: &Utterance) -> Option<(u64, u64)> {
    let mut t0: Option<u64> = None;
    let mut t1: Option<u64> = None;
    walk_words(utterance.main.content.content.as_slice(), None, &mut |w| {
        if let Some((s, e)) = word_timing(&w) {
            t0 = Some(t0.map_or(s, |c| c.min(s)));
            t1 = Some(t1.map_or(e, |c| c.max(e)));
        }
    });
    if t0.is_none()
        && let Some(wor) = utterance.wor_tier()
    {
        for word in wor.words() {
            if let Some((s, e)) = word_timing(&WordItem::Word(word)) {
                t0 = Some(t0.map_or(s, |current| current.min(s)));
                t1 = Some(t1.map_or(e, |current| current.max(e)));
            }
        }
    }
    match (t0, t1) {
        (Some(start_ms), Some(end_ms)) if end_ms >= start_ms => Some((start_ms, end_ms)),
        _ => utterance.main.content.bullet.as_ref().and_then(|bullet| {
            let timing = &bullet.timing;
            (timing.end_ms >= timing.start_ms).then_some((timing.start_ms, timing.end_ms))
        }),
    }
}

fn word_timing(w: &WordItem<'_>) -> Option<(u64, u64)> {
    let word = match w {
        WordItem::Word(w) => *w,
        WordItem::ReplacedWord(r) => &r.word,
        WordItem::Separator(_) => return None,
    };
    let b = word.inline_bullet.as_ref()?;
    Some((b.timing.start_ms, b.timing.end_ms))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::base::NullSink;
    use crate::proto::speaker::Diarization;
    use crate::utils::SourceId;

    const TIMED_UTTERANCE: &str = "@UTF8\n\
@Begin\n\
@Languages:\teng\n\
@Participants:\tPAR0 Participant, PAR1 Participant\n\
@ID:\teng|test|PAR0|||||Participant|||\n\
@ID:\teng|test|PAR1|||||Participant|||\n\
@Media:\ttest, audio\n\
*PAR0:\thello there friend now . \u{15}0_1600\u{15}\n\
%wor:\thello \u{15}0_400\u{15} there \u{15}401_800\u{15} friend \u{15}801_1200\u{15} now \u{15}1201_1600\u{15} .\n\
@End\n";

    fn output(segments: Vec<DiarizationSegment>) -> SpeakerOutput {
        SpeakerOutput {
            source_id: SourceId::new_unchecked("speaker-test"),
            diarization: Diarization { segments },
        }
    }

    #[test]
    fn splits_utterance_at_speaker_boundary() {
        let source_id = SourceId::new_unchecked("speaker-test");
        let mut chat = Chat::parse(TIMED_UTTERANCE, source_id).expect("valid timed CHAT");
        let diarization = output(vec![
            DiarizationSegment {
                start_ms: 0,
                end_ms: 800,
                speaker: "speaker-a".into(),
            },
            DiarizationSegment {
                start_ms: 801,
                end_ms: 1600,
                speaker: "speaker-b".into(),
            },
        ]);

        relabel_utterances_by_diarization(&mut chat, &diarization, &NullSink)
            .expect("speaker projection succeeds");
        chat.validate_stage_output(Task::Speaker)
            .expect("split speaker output remains valid CHAT");

        let utterances: Vec<_> = chat.ast().utterances().collect();
        assert_eq!(utterances.len(), 2);
        assert_eq!(utterances[0].main.speaker.as_str(), "PAR0");
        assert_eq!(utterances[1].main.speaker.as_str(), "PAR1");
        assert_eq!(utterance_timing_ms(utterances[0]), Some((0, 800)));
        assert_eq!(utterance_timing_ms(utterances[1]), Some((801, 1600)));
    }

    #[test]
    fn adjacent_segments_for_same_speaker_do_not_split() {
        let source_id = SourceId::new_unchecked("speaker-test");
        let mut chat = Chat::parse(TIMED_UTTERANCE, source_id).expect("valid timed CHAT");
        let diarization = output(vec![
            DiarizationSegment {
                start_ms: 0,
                end_ms: 800,
                speaker: "speaker-a".into(),
            },
            DiarizationSegment {
                start_ms: 801,
                end_ms: 1600,
                speaker: "speaker-a".into(),
            },
        ]);

        relabel_utterances_by_diarization(&mut chat, &diarization, &NullSink)
            .expect("speaker projection succeeds");
        chat.validate_stage_output(Task::Speaker)
            .expect("same-speaker output remains valid CHAT");

        assert_eq!(chat.ast().utterances().count(), 1);
    }

    #[test]
    fn declares_new_participant_for_third_diarized_speaker() {
        let source_id = SourceId::new_unchecked("speaker-test");
        let mut chat = Chat::parse(TIMED_UTTERANCE, source_id).expect("valid timed CHAT");
        let diarization = output(vec![
            DiarizationSegment {
                start_ms: 0,
                end_ms: 400,
                speaker: "speaker-a".into(),
            },
            DiarizationSegment {
                start_ms: 401,
                end_ms: 800,
                speaker: "speaker-b".into(),
            },
            DiarizationSegment {
                start_ms: 801,
                end_ms: 1600,
                speaker: "speaker-c".into(),
            },
        ]);

        relabel_utterances_by_diarization(&mut chat, &diarization, &NullSink)
            .expect("speaker projection succeeds");
        chat.validate_stage_output(Task::Speaker)
            .expect("third-speaker output remains valid CHAT");

        let participant = chat
            .ast()
            .participants
            .get(&SpeakerCode::new("PAR2"))
            .expect("PAR2 participant is declared");
        assert_eq!(participant.id.speaker.as_str(), "PAR2");
        assert_eq!(participant.role.as_str(), "Participant");

        let rendered = chat.to_chat();
        assert!(
            rendered
                .contains("@Participants:\tPAR0 Participant, PAR1 Participant, PAR2 Participant")
        );
        assert!(rendered.contains("@ID:\teng|test|PAR2|||||Participant|||"));
        let reparsed = Chat::parse(&rendered, SourceId::new_unchecked("speaker-reparse"))
            .expect("serialized third-speaker CHAT reparses");
        reparsed
            .validate_stage_output(Task::Speaker)
            .expect("serialized third-speaker CHAT validates");

        let speakers: Vec<_> = chat
            .ast()
            .utterances()
            .map(|utterance| utterance.main.speaker.as_str())
            .collect();
        assert_eq!(speakers, ["PAR0", "PAR1", "PAR2"]);
    }
}
