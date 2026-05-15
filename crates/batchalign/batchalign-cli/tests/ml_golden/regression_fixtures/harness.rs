use std::path::{Path, PathBuf};

use crate::common::regression_manifest::{
    DiscoveredFixture, FixtureAssertion, FixtureCommand, FixtureManifest,
    FixtureTranscribeAsrEngine, FixtureWorPolicy, MainTierIndex, MaxLeadMs, MaxOverrunMs,
    MaxProportion, MaxWordCount, MinDurationMs, MinSpeakerCount, MinUtrCoveragePercent,
    MinUtteranceCount,
};
use crate::common::{require_live_direct, require_revai_key, submit_paths_and_complete_direct};
use batchalign::api::{JobStatus, ReleasedCommand};
use batchalign::chat_ops::ChatFile;
use batchalign::chat_ops::TierDomain;
use batchalign::options::{
    AlignOptions, AsrEngineName, CommandOptions, CommonOptions, FaEngineName, TranscribeOptions,
    WorTierPolicy,
};
use batchalign::worker::InferTask;
use talkbank_transform::extract::extract_words;
use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

struct FixtureExecutionOutput {
    raw_chat: String,
    parsed_chat: ChatFile,
}

pub fn load_fixture(command_dir: &str, bug_dir: &str) -> Option<DiscoveredFixture> {
    let candidates = resolve_fixture_candidates(command_dir, bug_dir);
    for dir in candidates {
        let manifest_path = dir.join("source.json");
        if !manifest_path.exists() {
            continue;
        }
        let manifest = FixtureManifest::load(&manifest_path)
            .unwrap_or_else(|e| panic!("loading manifest at {}: {e}", manifest_path.display()));
        return Some(DiscoveredFixture { dir, manifest });
    }
    eprintln!(
        "SKIP: regression fixture {command_dir}/{bug_dir} not found in any candidate \
         location (set BATCHALIGN3_PRIVATE_FIXTURES_DIR to the private fixture \
         checkout to enable this test)"
    );
    None
}

fn batchalign3_repo_root() -> PathBuf {
    let mut cursor = std::env::current_dir().expect("cwd readable");
    loop {
        if cursor.join("test-fixtures").is_dir() && cursor.join("Cargo.toml").is_file() {
            return cursor;
        }
        if !cursor.pop() {
            panic!("could not locate batchalign3 repo root from cwd");
        }
    }
}

fn resolve_fixture_candidates(command_dir: &str, bug_dir: &str) -> Vec<PathBuf> {
    let mut candidates = Vec::new();
    if let Ok(private_root) = std::env::var("BATCHALIGN3_PRIVATE_FIXTURES_DIR") {
        let private_path = PathBuf::from(private_root)
            .join(command_dir)
            .join("regressions")
            .join(bug_dir);
        candidates.push(private_path);
    }
    let root = batchalign3_repo_root();
    let in_tree = root
        .join("test-fixtures")
        .join(command_dir)
        .join("regressions")
        .join(bug_dir);
    candidates.push(in_tree);
    candidates
}

fn command_options_for(manifest: &FixtureManifest) -> CommandOptions {
    match manifest.command {
        FixtureCommand::Align => CommandOptions::Align(AlignOptions {
            common: CommonOptions {
                override_media_cache: true,
                ..CommonOptions::default()
            },
            fa_engine: FaEngineName::Wave2Vec,
            wor: WorTierPolicy::Include,
            ..AlignOptions::default()
        }),
        FixtureCommand::Transcribe => CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions {
                override_media_cache: true,
                ..CommonOptions::default()
            },
            asr_engine: match manifest.transcribe.unwrap_or_default().asr_engine {
                FixtureTranscribeAsrEngine::Whisper => AsrEngineName::Whisper,
                FixtureTranscribeAsrEngine::RevAi => AsrEngineName::RevAi,
            },
            diarize: manifest.transcribe.unwrap_or_default().diarize,
            wor: match manifest.transcribe.unwrap_or_default().wor {
                FixtureWorPolicy::Include => WorTierPolicy::Include,
                FixtureWorPolicy::Omit => WorTierPolicy::Omit,
            },
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        other => panic!(
            "regression_fixtures runner does not yet support command {other:?} — add the dispatch arm"
        ),
    }
}

fn released_command_for(command: FixtureCommand) -> ReleasedCommand {
    match command {
        FixtureCommand::Align => ReleasedCommand::Align,
        FixtureCommand::Transcribe => ReleasedCommand::Transcribe,
        other => panic!(
            "regression_fixtures runner does not yet support command {other:?} — add the released-command arm"
        ),
    }
}

fn infer_task_for(command: FixtureCommand) -> InferTask {
    match command {
        FixtureCommand::Align => InferTask::Fa,
        FixtureCommand::Transcribe => InferTask::Asr,
        other => panic!(
            "regression_fixtures runner does not yet support command {other:?} — add the infer-task arm"
        ),
    }
}

fn stage_fixture_source(fixture: &DiscoveredFixture, stage_dir: &Path) -> PathBuf {
    match fixture.manifest.command {
        FixtureCommand::Transcribe => {
            let audio_src = fixture.audio_path().unwrap_or_else(|| {
                panic!(
                    "regression fixture {}: transcribe fixture requires audio",
                    fixture.dir.display()
                )
            });
            let audio_dst = stage_dir.join(
                audio_src
                    .file_name()
                    .expect("fixture audio has a filename for transcribe"),
            );
            std::fs::copy(&audio_src, &audio_dst).expect("copy input audio to stage");
            audio_dst
        }
        _ => {
            let chat_src = fixture.input_chat_path().unwrap_or_else(|| {
                panic!(
                    "regression fixture {}: command {:?} requires input_chat",
                    fixture.dir.display(),
                    fixture.manifest.command
                )
            });
            let staged_chat = stage_dir.join(
                chat_src
                    .file_name()
                    .expect("fixture input chat has a filename"),
            );
            std::fs::copy(&chat_src, &staged_chat).expect("copy input.cha to stage");
            if let Some(audio_src) = fixture.audio_path() {
                let audio_dst =
                    stage_dir.join(audio_src.file_name().expect("fixture audio has a filename"));
                std::fs::copy(&audio_src, &audio_dst).expect("copy input audio to stage");
            }
            staged_chat
        }
    }
}

async fn execute_fixture(fixture: &DiscoveredFixture) -> Option<FixtureExecutionOutput> {
    let task = infer_task_for(fixture.manifest.command);
    let session = require_live_direct(
        task,
        &format!(
            "regression fixture {} requires {task:?} infer task",
            fixture.dir.display()
        ),
    )
    .await?;

    if fixture.manifest.transcribe.unwrap_or_default().diarize
        && !session.has_infer_task(InferTask::Speaker)
    {
        eprintln!(
            "SKIP: regression fixture {} requires speaker diarization support",
            fixture.dir.display()
        );
        return None;
    }

    if fixture.manifest.transcribe.unwrap_or_default().asr_engine
        == FixtureTranscribeAsrEngine::RevAi
        && require_revai_key().is_none()
    {
        eprintln!(
            "SKIP: regression fixture {} requires Rev.AI credentials",
            fixture.dir.display()
        );
        return None;
    }

    let stage_dir = session
        .state_dir()
        .join("regression")
        .join(fixture.dir.file_name().expect("fixture dir has a name"));
    std::fs::create_dir_all(&stage_dir).expect("mkdir stage dir");
    let staged_source = stage_fixture_source(fixture, &stage_dir);

    let output_dir = session.state_dir().join("regression_out");
    std::fs::create_dir_all(&output_dir).expect("mkdir output dir");
    let output_path = output_dir.join(format!(
        "{}.cha",
        fixture
            .dir
            .file_name()
            .expect("fixture dir has a name")
            .to_string_lossy()
    ));

    let options = command_options_for(&fixture.manifest);
    let released = released_command_for(fixture.manifest.command);

    let (info, outputs) = submit_paths_and_complete_direct(
        &session,
        released,
        &fixture.manifest.language,
        vec![staged_source.to_string_lossy().into_owned()],
        vec![output_path.to_string_lossy().into_owned()],
        options,
    )
    .await;

    assert_eq!(
        info.status,
        JobStatus::Completed,
        "regression fixture {}: {:?} job did not complete (status={:?})",
        fixture.dir.display(),
        fixture.manifest.command,
        info.status,
    );
    assert_eq!(
        outputs.len(),
        1,
        "regression fixture {}: expected exactly 1 output file, got {}",
        fixture.dir.display(),
        outputs.len(),
    );

    let parser = TreeSitterParser::new().expect("tree-sitter parser construct");
    let (file, parse_errors) = parse_lenient(&parser, &outputs[0]);
    assert!(
        parse_errors.is_empty(),
        "regression fixture {}: output CHAT failed to parse: {parse_errors:?}",
        fixture.dir.display(),
    );

    Some(FixtureExecutionOutput {
        raw_chat: outputs[0].clone(),
        parsed_chat: file,
    })
}

fn run_assertions(fixture: &DiscoveredFixture, output: &FixtureExecutionOutput) {
    let mut failures: Vec<String> = Vec::new();
    for assertion in &fixture.manifest.assertions {
        if let Err(msg) = run_one_assertion(fixture, output, assertion) {
            failures.push(format!("  - {msg}"));
        }
    }
    if !failures.is_empty() {
        panic!(
            "regression fixture {} FAILED — {} assertion(s) violated:\n{}\n\nBug summary: {}",
            fixture.dir.display(),
            failures.len(),
            failures.join("\n"),
            fixture.manifest.bug.summary,
        );
    }
}

fn run_one_assertion(
    fixture: &DiscoveredFixture,
    output: &FixtureExecutionOutput,
    assertion: &FixtureAssertion,
) -> Result<(), String> {
    let parsed = &output.parsed_chat;

    if let FixtureAssertion::MinMainTierUtteranceCount {
        threshold_count: MinUtteranceCount(threshold),
    } = assertion
    {
        let actual = parsed.utterances().count();
        if actual >= *threshold {
            return Ok(());
        }
        return Err(format!(
            "min_main_tier_utterance_count: output has {actual} utterance(s) (< {threshold} required)",
        ));
    }

    if let FixtureAssertion::MaxFirstMainTierWordCount {
        threshold_count: MaxWordCount(threshold),
    } = assertion
    {
        let first_utterance = parsed.utterances().next().ok_or_else(|| {
            "max_first_main_tier_word_count: output has no main-tier utterances".to_string()
        })?;
        if first_utterance.main.content.bullet.is_none() {
            return Err(
                "max_first_main_tier_word_count: first utterance has no timing bullet".to_string(),
            );
        }
        let extracted = extract_words(parsed, TierDomain::Mor);
        let first_words = extracted.first().ok_or_else(|| {
            "max_first_main_tier_word_count: missing extracted words for first utterance"
                .to_string()
        })?;
        let actual = first_words.words.len();
        if actual <= *threshold {
            return Ok(());
        }
        return Err(format!(
            "max_first_main_tier_word_count: first utterance has {actual} word(s) (> {threshold} allowed)"
        ));
    }

    if let FixtureAssertion::NoWorTiersPresent = assertion {
        let actual = parsed
            .utterances()
            .filter(|utt| utt.wor_tier().is_some())
            .count();
        if actual == 0 {
            return Ok(());
        }
        return Err(format!(
            "no_wor_tiers_present: output materialized {actual} utterance(s) with %wor tiers"
        ));
    }

    if let FixtureAssertion::MinDistinctMainTierSpeakerCount {
        threshold_count: MinSpeakerCount(threshold),
    } = assertion
    {
        let actual = parsed
            .utterances()
            .map(|utt| utt.main.speaker.to_string())
            .collect::<std::collections::BTreeSet<_>>()
            .len();
        if actual >= *threshold {
            return Ok(());
        }
        return Err(format!(
            "min_distinct_main_tier_speaker_count: output has {actual} distinct speaker(s) (< {threshold} required)"
        ));
    }

    if let FixtureAssertion::NoFaGroupInvalidAudioWindow = assertion {
        return crate::common::drift_assertions::check_no_fa_group_invalid_audio_window(
            &output.parsed_chat,
        );
    }

    if let FixtureAssertion::NoMonotonicityRescueEmitted = assertion {
        return crate::common::drift_assertions::check_no_monotonicity_rescue_emitted(
            &output.parsed_chat,
        );
    }

    if let FixtureAssertion::UtteranceBulletMonotonicityPreserved = assertion {
        return crate::common::drift_assertions::check_utterance_bullet_monotonicity_preserved(
            &output.parsed_chat,
        );
    }

    if let FixtureAssertion::MinUtrCoveragePercent {
        threshold_percent: MinUtrCoveragePercent(threshold),
    } = assertion
    {
        // Count utterances that were untimed on INPUT, then count how many of
        // those are now timed on OUTPUT. Matching is positional (by main-tier
        // index), matching the existing fixture convention.
        let input_path = fixture.input_chat_path().ok_or_else(|| {
            "min_utr_coverage_percent: fixture has no input_chat to compare against".to_string()
        })?;
        let input_text = std::fs::read_to_string(&input_path)
            .map_err(|e| format!("min_utr_coverage_percent: read input CHAT: {e}"))?;
        let parser = TreeSitterParser::new()
            .map_err(|e| format!("min_utr_coverage_percent: parser construct: {e:?}"))?;
        let (input_chat, parse_errors) = parse_lenient(&parser, &input_text);
        if !parse_errors.is_empty() {
            return Err(format!(
                "min_utr_coverage_percent: input CHAT failed to parse: {parse_errors:?}"
            ));
        }
        let input_untimed_indices: Vec<usize> = input_chat
            .utterances()
            .enumerate()
            .filter_map(|(i, utt)| {
                if utt.main.content.bullet.is_none() {
                    Some(i)
                } else {
                    None
                }
            })
            .collect();
        if input_untimed_indices.is_empty() {
            // Nothing to cover: assertion passes trivially.
            return Ok(());
        }
        let output_utts: Vec<_> = output.parsed_chat.utterances().collect();
        let input_len = input_chat.utterances().count();
        let output_len = output_utts.len();
        if input_len != output_len {
            return Err(format!(
                "min_utr_coverage_percent: input and output utterance counts differ \
                 ({input_len} vs {output_len}); positional matching is invalid"
            ));
        }
        let newly_timed = input_untimed_indices
            .iter()
            .filter(|idx| {
                output_utts
                    .get(**idx)
                    .and_then(|utt| utt.main.content.bullet.as_ref())
                    .is_some()
            })
            .count();
        let total = input_untimed_indices.len();
        // Integer division rounds down, so the effective threshold may be up
        // to ~1% stricter than the stated value at small sample sizes.
        let pct = (newly_timed as u64 * 100) / total as u64;
        if pct >= u64::from(*threshold) {
            return Ok(());
        }
        return Err(format!(
            "min_utr_coverage_percent: UTR covered {newly_timed}/{total} = {pct}% of formerly-untimed \
             utterances (< {threshold}% required)"
        ));
    }

    if let FixtureAssertion::NoSilentTimingStrip = assertion {
        return crate::common::drift_assertions::check_no_silent_timing_strip(&output.parsed_chat);
    }

    if let FixtureAssertion::MediaHeaderMatchesInputBasename = assertion {
        let audio_path = fixture.audio_path().ok_or_else(|| {
            format!(
                "media_header_matches_input_basename: fixture {} has no input audio",
                fixture.dir.display()
            )
        })?;
        let expected_basename = audio_path.file_stem().ok_or_else(|| {
            format!(
                "media_header_matches_input_basename: audio path {} has no basename",
                audio_path.display()
            )
        })?;
        let expected = format!("@Media:\t{}, audio", expected_basename.to_string_lossy());
        let actual = media_header_line(&output.raw_chat).ok_or_else(|| {
            "media_header_matches_input_basename: output has no @Media header".to_string()
        })?;
        if actual == expected {
            return Ok(());
        }
        return Err(format!(
            "media_header_matches_input_basename: expected {:?}, got {:?}",
            expected, actual
        ));
    }

    let target = assertion
        .main_tier_index()
        .expect("utterance-scoped regression assertion must provide main_tier_index");
    let utt = nth_main_tier_utterance(parsed, target).ok_or_else(|| {
        format!(
            "could not find main-tier utterance #{} in output (only {} utterances present)",
            target.0,
            parsed.utterances().count()
        )
    })?;
    let wor = utt.wor_tier().ok_or_else(|| {
        format!(
            "main-tier utterance #{} has no %wor tier — assertion {:?} requires one",
            target.0, assertion
        )
    })?;

    match assertion {
        FixtureAssertion::NoZeroDurationWorWords { .. } => {
            let zero_words: Vec<String> = wor
                .words()
                .enumerate()
                .filter_map(|(i, w)| {
                    let bullet = w.inline_bullet.as_ref()?;
                    if bullet.timing.start_ms == bullet.timing.end_ms {
                        Some(format!(
                            "word #{i} '{}' has zero duration ({}_{})",
                            w.cleaned_text(),
                            bullet.timing.start_ms,
                            bullet.timing.end_ms,
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            let untimed_words: Vec<String> = wor
                .words()
                .enumerate()
                .filter_map(|(i, w)| {
                    if w.inline_bullet.is_none() {
                        Some(format!(
                            "word #{i} '{}' has no timing bullet at all",
                            w.cleaned_text(),
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            if zero_words.is_empty() && untimed_words.is_empty() {
                return Ok(());
            }
            let mut report = format!(
                "no_zero_duration_wor_words: utterance #{} has {} zero-duration word(s) and {} untimed word(s)",
                target.0,
                zero_words.len(),
                untimed_words.len(),
            );
            for w in zero_words.iter().chain(untimed_words.iter()) {
                report.push_str(&format!("\n      {w}"));
            }
            Err(report)
        }
        FixtureAssertion::MinWorWordDurationMs {
            threshold_ms: MinDurationMs(threshold),
            ..
        } => {
            let short_words: Vec<String> = wor
                .words()
                .enumerate()
                .filter_map(|(i, w)| {
                    let bullet = w.inline_bullet.as_ref()?;
                    let duration = bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms);
                    if duration < *threshold {
                        Some(format!(
                            "word #{i} '{}' has duration {} ms (< {} ms threshold)",
                            w.cleaned_text(),
                            duration,
                            threshold,
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            if short_words.is_empty() {
                return Ok(());
            }
            let mut report = format!(
                "min_wor_word_duration_ms: utterance #{} has {} word(s) under {} ms threshold",
                target.0,
                short_words.len(),
                threshold,
            );
            for w in &short_words {
                report.push_str(&format!("\n      {w}"));
            }
            Err(report)
        }
        FixtureAssertion::MinLastWorWordDurationMs {
            threshold_ms: MinDurationMs(threshold),
            ..
        } => {
            let last_word = wor.words().last().ok_or_else(|| {
                format!(
                    "min_last_wor_word_duration_ms: utterance #{} has empty %wor",
                    target.0,
                )
            })?;
            let bullet = last_word.inline_bullet.as_ref().ok_or_else(|| {
                format!(
                    "min_last_wor_word_duration_ms: utterance #{} last word '{}' has no timing bullet",
                    target.0,
                    last_word.cleaned_text(),
                )
            })?;
            let duration = bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms);
            if duration >= *threshold {
                return Ok(());
            }
            Err(format!(
                "min_last_wor_word_duration_ms: utterance #{} last word '{}' has duration {} ms (< {} ms threshold)",
                target.0,
                last_word.cleaned_text(),
                duration,
                threshold,
            ))
        }
        FixtureAssertion::MaxWorWordDurationProportion {
            max_proportion: MaxProportion(max),
            ..
        } => {
            let utt_bullet = utt.main.content.bullet.as_ref().ok_or_else(|| {
                format!(
                    "max_wor_word_duration_proportion: utterance #{} has no main-tier bullet — cannot compute proportion",
                    target.0,
                )
            })?;
            let utt_duration = utt_bullet
                .timing
                .end_ms
                .saturating_sub(utt_bullet.timing.start_ms) as f64;
            if utt_duration <= 0.0 {
                return Err(format!(
                    "max_wor_word_duration_proportion: utterance #{} has zero-length main-tier bullet ({}_{})",
                    target.0, utt_bullet.timing.start_ms, utt_bullet.timing.end_ms,
                ));
            }
            let dominant: Vec<String> = wor
                .words()
                .enumerate()
                .filter_map(|(i, w)| {
                    let bullet = w.inline_bullet.as_ref()?;
                    let duration =
                        bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms) as f64;
                    let proportion = duration / utt_duration;
                    if proportion > *max {
                        Some(format!(
                            "word #{i} '{}' takes {:.1}% of utterance ({} ms / {} ms) — exceeds {:.1}% cap",
                            w.cleaned_text(),
                            proportion * 100.0,
                            duration as u64,
                            utt_duration as u64,
                            max * 100.0,
                        ))
                    } else {
                        None
                    }
                })
                .collect();
            if dominant.is_empty() {
                return Ok(());
            }
            let mut report = format!(
                "max_wor_word_duration_proportion: utterance #{} has {} dominant word(s) over {:.1}% threshold",
                target.0,
                dominant.len(),
                max * 100.0,
            );
            for w in &dominant {
                report.push_str(&format!("\n      {w}"));
            }
            Err(report)
        }
        FixtureAssertion::MaxMainTierLeadBeforeFirstWorMs {
            threshold_ms: MaxLeadMs(threshold),
            ..
        } => {
            let utt_bullet = utt.main.content.bullet.as_ref().ok_or_else(|| {
                format!(
                    "max_main_tier_lead_before_first_wor_ms: utterance #{} has no main-tier bullet",
                    target.0,
                )
            })?;
            let first_timed_word = wor
                .words()
                .enumerate()
                .find_map(|(i, word)| word.inline_bullet.as_ref().map(|bullet| (i, word, bullet)))
                .ok_or_else(|| {
                    format!(
                        "max_main_tier_lead_before_first_wor_ms: utterance #{} has no timed %wor words",
                        target.0,
                    )
                })?;
            let (word_index, word, bullet) = first_timed_word;
            let lead_ms = bullet
                .timing
                .start_ms
                .saturating_sub(utt_bullet.timing.start_ms);
            if lead_ms <= *threshold {
                return Ok(());
            }
            Err(format!(
                "max_main_tier_lead_before_first_wor_ms: utterance #{} main-tier start {} leads first timed %wor word #{} '{}' at {} by {} ms (> {} ms threshold)",
                target.0,
                utt_bullet.timing.start_ms,
                word_index,
                word.cleaned_text(),
                bullet.timing.start_ms,
                lead_ms,
                threshold,
            ))
        }
        FixtureAssertion::MaxLastWorOverrunPastMainEndMs {
            threshold_ms: MaxOverrunMs(threshold),
            ..
        } => {
            let utt_bullet = utt.main.content.bullet.as_ref().ok_or_else(|| {
                format!(
                    "max_last_wor_overrun_past_main_end_ms: utterance #{} has no main-tier bullet",
                    target.0,
                )
            })?;
            let last_timed_word = wor
                .words()
                .enumerate()
                .filter_map(|(i, word)| word.inline_bullet.as_ref().map(|bullet| (i, word, bullet)))
                .last()
                .ok_or_else(|| {
                    format!(
                        "max_last_wor_overrun_past_main_end_ms: utterance #{} has no timed %wor words",
                        target.0,
                    )
                })?;
            let (word_index, word, bullet) = last_timed_word;
            let overrun_ms = bullet
                .timing
                .end_ms
                .saturating_sub(utt_bullet.timing.end_ms);
            if overrun_ms <= *threshold {
                return Ok(());
            }
            Err(format!(
                "max_last_wor_overrun_past_main_end_ms: utterance #{} main-tier end {} trails last timed %wor word #{} '{}' end {} by {} ms (> {} ms threshold)",
                target.0,
                utt_bullet.timing.end_ms,
                word_index,
                word.cleaned_text(),
                bullet.timing.end_ms,
                overrun_ms,
                threshold,
            ))
        }
        FixtureAssertion::MinMainTierUtteranceCount { .. } => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::MaxFirstMainTierWordCount { .. } => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::NoWorTiersPresent => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::MinDistinctMainTierSpeakerCount { .. } => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::MediaHeaderMatchesInputBasename => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::NoFaGroupInvalidAudioWindow => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::NoMonotonicityRescueEmitted => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::UtteranceBulletMonotonicityPreserved => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::MinUtrCoveragePercent { .. } => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
        FixtureAssertion::NoSilentTimingStrip => {
            unreachable!("global assertions should return before utterance-scoped assertion setup")
        }
    }
}

fn media_header_line(chat: &str) -> Option<&str> {
    chat.lines().find(|line| line.starts_with("@Media:\t"))
}

fn nth_main_tier_utterance(
    output: &ChatFile,
    index: MainTierIndex,
) -> Option<&batchalign::chat_ops::Utterance> {
    output.utterances().nth(index.0)
}

pub async fn run_fixture(command_dir: &str, bug_dir: &str) {
    let Some(fixture) = load_fixture(command_dir, bug_dir) else {
        return;
    };
    let Some(output) = execute_fixture(&fixture).await else {
        return;
    };
    run_assertions(&fixture, &output);
}
