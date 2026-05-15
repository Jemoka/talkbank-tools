//! Incremental forced alignment processing.
//!
//! Compares a "before" file (with existing timings) against an "after" file
//! (user-edited) and only re-aligns FA groups that still need worker or cache
//! work after stable `%wor` timing is copied forward from the old file.
//!
//! Like full-file FA, this module now depends on the transport-neutral FA
//! worker adapter instead of assembling a concrete worker payload inline. That
//! keeps the incremental path and full-file path on the same migration path as
//! the worker protocol evolves from V1 payloads to V2 prepared artifacts.

use crate::api::DurationMs;
use crate::cache::CacheBackend;
use crate::chat_ops::fa::{
    FaGroup, WordTiming, apply_fa_results, cache_key, collect_existing_fa_word_timings,
    enforce_monotonicity, expand_bullets_for_edge_fillers, group_utterances,
    refresh_existing_alignment_for_utterance,
};
use crate::chat_ops::{CacheKey, ChatFile, Line, Utterance};
use crate::error::ServerError;
use crate::params::{AudioContext, FaParams};
use crate::pipeline::PipelineServices;
use crate::runner::util::{FileStage, ProgressSender, ProgressUpdate};
use crate::types::results::FaResult;
use crate::types::traces::{FaGroupTrace, TimingTrace, ViolationTrace};
use talkbank_transform::diff::UtteranceDelta;
use talkbank_transform::diff::preserve::{TierKind, copy_dependent_tiers};
use talkbank_transform::parse::{is_dummy, is_no_align, parse_lenient};
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};
use tracing::{info, warn};

use super::transport::{FaWorkerBatch, FaWorkerTransport};
use super::{CACHE_TASK, collect_final_timings, process_fa};

/// Process a CHAT file through forced alignment incrementally.
///
/// Compares `before_text` (previous file with timings) against `after_text`
/// (user-edited version) and only re-aligns FA groups that contain changed
/// utterances. Unchanged groups preserve their existing timings.
///
/// Falls back to full processing if no "before" is available.
pub(crate) async fn process_fa_incremental(
    before_text: &str,
    after_text: &str,
    audio: &AudioContext<'_>,
    worker_lang: &crate::api::LanguageCode3,
    services: PipelineServices<'_>,
    fa_params: &FaParams,
    progress: Option<&ProgressSender>,
) -> Result<FaResult, ServerError> {
    use talkbank_transform::diff::{DiffSummary, diff_chat};

    let parser = crate::chat_parser();
    let (before_file, _) = parse_lenient(&parser, before_text);
    let (after_file, _) = parse_lenient(&parser, after_text);

    let deltas = diff_chat(&before_file, &after_file);
    let summary = DiffSummary::from_deltas(&deltas);

    info!(
        unchanged = summary.unchanged,
        words_changed = summary.words_changed,
        inserted = summary.inserted,
        deleted = summary.deleted,
        "Incremental FA diff"
    );

    // If there is no unchanged, speaker-only-changed, or timing-only region to
    // preserve from the previous file, the incremental path has nothing to
    // reuse and should fall back to the regular full-file align path.
    if summary.unchanged == 0 && summary.speaker_changed == 0 && summary.timing_only == 0 {
        return process_fa(
            after_text,
            audio,
            worker_lang,
            services,
            fa_params,
            progress,
        )
        .await;
    }

    // Group the "after" file's utterances
    let (mut chat_file, parse_errors) = parse_lenient(&parser, after_text);

    if is_dummy(&chat_file) || is_no_align(&chat_file) {
        return Ok(FaResult {
            chat_text: to_chat_string(&chat_file),
            groups: Vec::new(),
            pre_injection_timings: Vec::new(),
            timing_mode: fa_params.timing_mode,
            violations: Vec::new(),
            fallback_events: Vec::new(),
        });
    }

    if let Err(errors) = validate_to_level(&chat_file, &parse_errors, ValidityLevel::MainTierValid)
    {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(ServerError::Validation(format!(
            "align pre-validation failed: {}",
            msgs.join("; ")
        )));
    }

    let reusable_after_indices = reuse_stable_wor_timing_from_before(
        &before_file,
        &mut chat_file,
        &deltas,
        fa_params.wor_tier.should_write(),
    );

    expand_bullets_for_edge_fillers(&mut chat_file);

    let groups = group_utterances(
        &chat_file,
        fa_params.max_group_ms.0,
        audio.total_audio_ms.map(|ms| ms.0),
    );
    if groups.is_empty() {
        return Ok(FaResult {
            chat_text: to_chat_string(&chat_file),
            groups: Vec::new(),
            pre_injection_timings: Vec::new(),
            timing_mode: fa_params.timing_mode,
            violations: Vec::new(),
            fallback_events: Vec::new(),
        });
    }

    // Determine which groups still need re-alignment after stable `%wor`
    // regions from the "before" file were copied into the edited file.
    let mut group_needs_realign: Vec<bool> = Vec::with_capacity(groups.len());
    let mut realign_count = 0usize;
    let mut reused_group_count = 0usize;
    for group in &groups {
        let needs = group
            .utterance_indices
            .iter()
            .any(|idx| !reusable_after_indices.contains(&idx.raw()));
        if needs {
            realign_count += 1;
        } else {
            reused_group_count += 1;
        }
        group_needs_realign.push(needs);
    }

    info!(
        total_groups = groups.len(),
        realign_groups = realign_count,
        reused_groups = reused_group_count,
        "Incremental FA: selective group re-alignment with stable %wor reuse"
    );

    // Build cache keys and timing storage for all groups
    let word_texts: Vec<Vec<String>> = groups
        .iter()
        .map(|g| g.words.iter().map(|w| w.text.clone()).collect())
        .collect();

    let cache_keys: Vec<CacheKey> = groups
        .iter()
        .zip(word_texts.iter())
        .map(|(g, words)| {
            cache_key(
                words,
                audio.audio_identity,
                g.audio_start_ms(),
                g.audio_end_ms(),
                fa_params.timing_mode,
                fa_params.engine,
            )
        })
        .collect();

    let mut all_timings: Vec<Option<Vec<Option<WordTiming>>>> = vec![None; groups.len()];

    // Reused groups already have current main-tier word timing in `chat_file`.
    // Everything else still needs a cache lookup or worker call.
    let key_strings: Vec<String> = cache_keys.iter().map(|k| k.as_str().to_string()).collect();
    let cached = if fa_params.cache_policy.should_skip() {
        std::collections::HashMap::new()
    } else {
        match services
            .cache
            .get_batch(&key_strings, CACHE_TASK.as_str(), services.engine_version)
            .await
        {
            Ok(map) => map,
            Err(e) => {
                warn!(error = %e, "FA cache batch lookup failed");
                std::collections::HashMap::new()
            }
        }
    };

    // Populate reused groups and cache hits.
    let mut miss_indices: Vec<usize> = Vec::new();
    for (i, key) in cache_keys.iter().enumerate() {
        if !group_needs_realign[i]
            && let Some(timings) = collect_preserved_group_timings(&chat_file, &groups[i])
        {
            all_timings[i] = Some(timings);
            continue;
        }

        if let Some(cached_data) = cached.get(key.as_str()) {
            match serde_json::from_value::<Vec<Option<WordTiming>>>(cached_data.clone()) {
                Ok(timings) => {
                    all_timings[i] = Some(timings);
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, group = i, "Failed to deserialize cached FA timings");
                }
            }
        }
        miss_indices.push(i);
    }

    let reused_or_cached_groups = groups.len() - miss_indices.len();
    if reused_or_cached_groups > 0 || !miss_indices.is_empty() {
        info!(
            reused_or_cached = reused_or_cached_groups,
            misses = miss_indices.len(),
            "FA incremental partition"
        );
    }

    if let Some(tx) = progress {
        let _ = tx.send(ProgressUpdate::new(
            FileStage::Aligning,
            Some(reused_or_cached_groups as i64),
            Some(groups.len() as i64),
        ));
    }

    let transport = FaWorkerTransport::production(services);
    let mut fallback_events = Vec::new();

    // Send miss groups through the shared FA worker transport adapter.
    if !miss_indices.is_empty() {
        let parsed_results = transport
            .infer_groups(FaWorkerBatch {
                word_texts: &word_texts,
                groups: &groups,
                miss_indices: &miss_indices,
                audio_path: audio.audio_path,
                worker_lang: worker_lang.into(),
                engine: fa_params.engine,
                timing_mode: fa_params.timing_mode,
            })
            .await?;

        for (parsed_idx, parsed_result) in parsed_results.iter().enumerate() {
            let miss_idx = parsed_result.group_index;
            let timings = parsed_result.timings.clone();
            if let Some(event) = parsed_result.fallback_event.clone() {
                fallback_events.push(event);
            }

            let ba_version = env!("CARGO_PKG_VERSION");
            if let Ok(cache_data) = serde_json::to_value(&timings)
                && let Err(error) = services
                    .cache
                    .put_batch(
                        &[(cache_keys[miss_idx].as_str().to_string(), cache_data)],
                        CACHE_TASK.as_str(),
                        services.engine_version,
                        ba_version,
                    )
                    .await
            {
                warn!(error = %error, "Failed to cache FA result (non-fatal)");
            }

            all_timings[miss_idx] = Some(timings);

            if let Some(tx) = progress {
                let done = reused_or_cached_groups + parsed_idx + 1;
                let _ = tx.send(ProgressUpdate::new(
                    FileStage::Aligning,
                    Some(done as i64),
                    Some(groups.len() as i64),
                ));
            }
        }
    }

    // Apply all results
    let final_timings = collect_final_timings(all_timings, "incremental forced alignment")?;

    let pre_injection_timings: Vec<Vec<Option<TimingTrace>>> = final_timings
        .iter()
        .map(|group| {
            group
                .iter()
                .map(|t| {
                    t.as_ref().map(|wt| TimingTrace {
                        start_ms: wt.start_ms as i64,
                        end_ms: wt.end_ms as i64,
                    })
                })
                .collect()
        })
        .collect();

    let _fa_decisions = apply_fa_results(
        &mut chat_file,
        &groups,
        &final_timings,
        fa_params.timing_mode,
        fa_params.wor_tier.should_write(),
    );

    // Strip backward timestamps (E362/E704 violations).  The full FA path
    // (`run_fa_from_ast`) calls this unconditionally after injection; the
    // incremental path must do the same.  Without this call, anchor drift from
    // UTR (e.g., repeated scripted phrases matched to an earlier audio window)
    // survives into the output — as seen in APROCSA 2256_T4.cha (2026-04-09).
    enforce_monotonicity(&mut chat_file);

    // Post-FA bullet repair (experimental, opt-in via --bullet-repair).
    let repair_decisions = if fa_params.bullet_repair {
        let repair_result = crate::chat_ops::fa::repair_bullets(&mut chat_file, false);
        tracing::info!(%repair_result.stats, "bullet repair applied (incremental)");
        repair_result.decisions
    } else {
        Vec::new()
    };

    // Always strip stale decision tiers from previous runs before injecting new
    // ones.  inject_review_tiers does this internally, but only when
    // review_level != None.  Stripping unconditionally prevents stale tiers from
    // accumulating when review_level == None or when no new decisions are made.
    talkbank_transform::decisions::strip_decision_tiers(&mut chat_file);
    if fa_params.review_level != crate::chat_ops::fa::ReviewLevel::None {
        crate::chat_ops::fa::inject_review_tiers(
            &mut chat_file,
            &repair_decisions,
            fa_params.review_level,
        );
    }

    let violations = if let Err(errors) = validate_output(&chat_file, "align") {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        warn!(errors = ?msgs, "align post-validation warnings (non-fatal)");
        errors
            .iter()
            .map(|e| ViolationTrace {
                code: format!("L{}", e.level as u8),
                message: e.message.clone(),
                utterance_index: None,
            })
            .collect()
    } else {
        Vec::new()
    };

    let group_traces: Vec<FaGroupTrace> = groups
        .iter()
        .map(|g| FaGroupTrace {
            audio_start_ms: DurationMs(g.audio_start_ms()),
            audio_end_ms: DurationMs(g.audio_end_ms()),
            utterance_indices: g.utterance_indices.iter().map(|idx| idx.0).collect(),
            words: g.words.iter().map(|w| w.text.clone()).collect(),
        })
        .collect();

    Ok(FaResult {
        chat_text: to_chat_string(&chat_file),
        groups: group_traces,
        pre_injection_timings,
        timing_mode: fa_params.timing_mode,
        violations,
        fallback_events,
    })
}

/// Copy reusable `%wor` timing from the "before" file into the edited file.
///
/// Only utterances whose words are unchanged are candidates. That includes
/// plain unchanged utterances, speaker-only changes, and timing-only edits
/// where a rerun should restore timing from the durable `%wor` layer instead of
/// trusting the edited utterance bullet. Each reused utterance receives the
/// `%wor` tier from the "before" file and is then refreshed back onto the main
/// tier so later grouping sees current utterance bullets and word timings.
fn reuse_stable_wor_timing_from_before(
    before_file: &ChatFile,
    after_file: &mut ChatFile,
    deltas: &[UtteranceDelta],
    write_wor: bool,
) -> std::collections::HashSet<usize> {
    let mut reused = std::collections::HashSet::new();

    for delta in deltas {
        let (before_idx, after_idx) = match delta {
            UtteranceDelta::Unchanged {
                before_idx,
                after_idx,
            }
            | UtteranceDelta::TimingOnly {
                before_idx,
                after_idx,
            }
            | UtteranceDelta::SpeakerChanged {
                before_idx,
                after_idx,
            } => (*before_idx, *after_idx),
            _ => continue,
        };

        copy_dependent_tiers(
            before_file,
            before_idx,
            after_file,
            after_idx,
            &[TierKind::Wor],
        );

        let Some(utterance) = get_utterance_mut(after_file, after_idx.raw()) else {
            continue;
        };
        if refresh_existing_alignment_for_utterance(utterance, write_wor) {
            reused.insert(after_idx.raw());
        }
    }

    reused
}

/// Collect current timings for a preserved FA group from the CHAT AST.
///
/// The caller should use this only for groups whose utterances have already
/// been refreshed from stable `%wor` timing. The returned vector matches the
/// same word order used by FA extraction and injection.
pub(super) fn collect_preserved_group_timings(
    chat_file: &ChatFile,
    group: &FaGroup,
) -> Option<Vec<Option<WordTiming>>> {
    let mut timings = Vec::new();

    for utt_idx in &group.utterance_indices {
        let utterance = get_utterance(chat_file, utt_idx.raw())?;
        timings.extend(collect_existing_fa_word_timings(utterance));
    }

    if timings.len() != group.words.len() {
        return None;
    }

    Some(timings)
}

/// Borrow one utterance immutably by utterance ordinal.
pub(super) fn get_utterance(chat_file: &ChatFile, idx: usize) -> Option<&Utterance> {
    let mut current = 0usize;
    for line in &chat_file.lines {
        if let Line::Utterance(utterance) = line {
            if current == idx {
                return Some(utterance);
            }
            current += 1;
        }
    }
    None
}

/// Borrow one utterance mutably by utterance ordinal.
fn get_utterance_mut(chat_file: &mut ChatFile, idx: usize) -> Option<&mut Utterance> {
    let mut current = 0usize;
    for line in &mut chat_file.lines {
        if let Line::Utterance(utterance) = line {
            if current == idx {
                return Some(utterance);
            }
            current += 1;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::chat_ops::fa::{FaTimingMode, FaWord, TimeSpan, apply_fa_results};
    use crate::chat_ops::{UtteranceIdx, WordIdx};
    use talkbank_transform::diff::diff_chat;

    fn parse_chat(text: &str) -> ChatFile {
        let parser = talkbank_transform::parse::TreeSitterParser::new().unwrap();
        talkbank_transform::parse::parse_lenient(&parser, text).0
    }

    fn chat_with_wor(words0: &str, words1: &str) -> String {
        format!(
            "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI|||||Target_Child|||\n*CHI:\t{words0}\n%wor:\thello \u{15}100_500\u{15} world \u{15}600_1000\u{15} .\n*CHI:\t{words1}\n%wor:\tgoodbye \u{15}1500_2000\u{15} .\n@End\n"
        )
    }

    #[test]
    fn reuse_stable_wor_timing_from_before_only_marks_unchanged_utterances() {
        let before = parse_chat(&chat_with_wor("hello world .", "goodbye ."));
        let mut after = parse_chat(&chat_with_wor("hello world .", "farewell ."));
        let deltas = diff_chat(&before, &after);

        let reused = reuse_stable_wor_timing_from_before(&before, &mut after, &deltas, true);
        assert!(reused.contains(&0));
        assert!(!reused.contains(&1));

        let utt0 = get_utterance(&after, 0).expect("missing utterance 0");
        assert_eq!(collect_existing_fa_word_timings(utt0).len(), 2);
        assert!(utt0.main.content.bullet.is_some());
    }

    #[test]
    fn collect_preserved_group_timings_reads_refreshed_main_tier_timing() {
        let before = parse_chat(&chat_with_wor("hello world .", "goodbye ."));
        let mut after = parse_chat(&chat_with_wor("hello world .", "goodbye ."));
        let deltas = diff_chat(&before, &after);
        let reused = reuse_stable_wor_timing_from_before(&before, &mut after, &deltas, true);
        assert_eq!(reused.len(), 2);

        let groups = group_utterances(&after, 20_000, Some(4_000));
        let timings = collect_preserved_group_timings(&after, &groups[0])
            .expect("group timings should exist");
        assert_eq!(timings.len(), groups[0].words.len());
        assert!(timings.iter().all(|timing| timing.is_some()));
    }

    #[test]
    fn reuse_stable_wor_timing_from_before_marks_timing_only_utterances() {
        let mut before = parse_chat(&chat_with_wor("hello world .", "goodbye ."));
        crate::chat_ops::fa::refresh_existing_alignment(&mut before, true);
        let before_text = talkbank_transform::serialize::to_chat_string(&before);
        let before = parse_chat(&before_text);
        let mut after = parse_chat(&before_text);

        let utt0 = get_utterance_mut(&mut after, 0).expect("missing utterance 0");
        utt0.main.content.bullet = None;

        let deltas = diff_chat(&before, &after);
        assert!(matches!(deltas[0], UtteranceDelta::TimingOnly { .. }));

        let reused = reuse_stable_wor_timing_from_before(&before, &mut after, &deltas, true);
        assert!(
            reused.contains(&0),
            "timing-only utterance should be reused"
        );

        let utt0 = get_utterance(&after, 0).expect("missing utterance 0");
        assert!(utt0.main.content.bullet.is_some());
        assert!(
            collect_existing_fa_word_timings(utt0)
                .iter()
                .all(|timing| timing.is_some())
        );
    }

    // ---------------------------------------------------------------------------
    // Regression test: enforce_monotonicity must be called in incremental path
    // ---------------------------------------------------------------------------
    //
    // The full FA path (`run_fa_from_ast`) calls `enforce_monotonicity`
    // unconditionally after `apply_fa_results`.  The incremental path omitted
    // this call, allowing backward timestamps to survive.
    //
    // Incident (2026-04-09): 2256_T4.cha (APROCSA aphasia protocol) produced
    // •639095_640375• immediately after •731556_733418• because the global
    // Hirschberg UTR matched repeated scripted phrases to an earlier audio
    // window.  FA injected those backward timings and `enforce_monotonicity`
    // was never called to strip them.
    //
    // Fix: `process_fa_incremental` now calls `enforce_monotonicity` after
    // `apply_fa_results`, matching the full-path invariant.

    /// `enforce_monotonicity` strips a backward timestamp injected by
    /// `apply_fa_results` when FA receives out-of-order audio windows.
    ///
    /// Two consecutive INV utterances:
    ///   utt0 "alright"  → FA assigns 731556–733418 ms  (correct, forward)
    ///   utt1 "look"     → FA assigns 639095–639300 ms  (backward — earlier
    ///                      than utt0's end time of 733418 ms)
    ///
    /// After `apply_fa_results + enforce_monotonicity`, utt1's bullet must be
    /// `None` (the backward timestamp is stripped).  Without `enforce_monotonicity`
    /// the backward 639095 ms bullet persists and produces E362/E704 violations.
    ///
    /// This regression test verifies the fix added to `process_fa_incremental`.
    #[test]
    fn test_incremental_path_enforce_monotonicity_strips_backward_timestamp() {
        let chat_text = concat!(
            "@UTF8\n",
            "@Begin\n",
            "@Languages:\teng\n",
            "@Participants:\tINV Investigator Adult_Unrelated\n",
            "@ID:\teng|test|INV||female|||Adult_Unrelated|||\n",
            "@Media:\ttest, audio\n",
            "*INV:\talright .\n",
            "*INV:\tlook .\n",
            "@End\n",
        );
        let mut chat = parse_chat(chat_text);

        // Two single-word groups: one per utterance.
        // Group 0 is forward (731556 ms); group 1 is BACKWARD (639095 < 733418).
        let groups = vec![
            FaGroup {
                audio_span: TimeSpan::new(731000, 734000),
                words: vec![FaWord {
                    utterance_index: UtteranceIdx(0),
                    utterance_word_index: WordIdx(0),
                    text: "alright".into(),
                }],
                utterance_indices: vec![UtteranceIdx(0)],
            },
            FaGroup {
                audio_span: TimeSpan::new(639000, 641000),
                words: vec![FaWord {
                    utterance_index: UtteranceIdx(1),
                    utterance_word_index: WordIdx(0),
                    text: "look".into(),
                }],
                utterance_indices: vec![UtteranceIdx(1)],
            },
        ];

        // Group 0: forward timing (correct).
        // Group 1: backward timing — earlier than group 0's end time (639095 < 733418).
        let timings = vec![
            vec![Some(crate::chat_ops::fa::WordTiming {
                start_ms: 731556,
                end_ms: 733418,
            })],
            vec![Some(crate::chat_ops::fa::WordTiming {
                start_ms: 639095,
                end_ms: 639300,
            })],
        ];

        // Replicate what the FIXED `process_fa_incremental` does: apply results,
        // then call `enforce_monotonicity` to strip non-monotonic bullets.
        apply_fa_results(
            &mut chat,
            &groups,
            &timings,
            FaTimingMode::Continuous,
            false,
        );
        // `process_fa_incremental` must call this after `apply_fa_results`;
        // the bug was that it did not.
        enforce_monotonicity(&mut chat);

        let utt0 = get_utterance(&chat, 0).expect("utterance 0 must exist");
        let utt1 = get_utterance(&chat, 1).expect("utterance 1 must exist");

        // utt0 retains its forward bullet at 731556 ms.
        let b0 = utt0
            .main
            .content
            .bullet
            .as_ref()
            .expect("utt0 must retain its forward bullet");
        assert_eq!(
            b0.timing.start_ms, 731556,
            "utt0 start must be 731556 ms after enforcement; got {}",
            b0.timing.start_ms
        );

        // utt1's backward bullet (639095 < 733418) must be stripped.
        assert!(
            utt1.main.content.bullet.is_none(),
            "backward bullet at 639095ms (< utt0 end {}ms) must be stripped by \
             enforce_monotonicity; got {:?}",
            b0.timing.end_ms,
            utt1.main.content.bullet,
        );
    }
}
