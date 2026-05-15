//! Server-side forced alignment orchestrator.
//!
//! Owns the full CHAT lifecycle for FA jobs:
//! parse → group → cache check → infer (audio chunks) → DP-align → inject →
//! postprocess → %wor → monotonicity/E704 → serialize.
//!
//! # Call path
//!
//! `batchalign-cli`/API submission
//! → `runner::dispatch_fa_infer`
//! → [`process_fa`]
//! → `crate::chat_ops::fa::{group_utterances, parse_fa_response, apply_fa_results}`
//! → FA worker transport adapter
//! → validation + serialization.
//!
//! # Key differences from morphosyntax/utseg/translate/coref
//!
//! - **Per-file, not cross-file**: Each file has its own audio, so no cross-file batching.
//! - **Multiple groups per file**: Utterances are grouped by time window; each group is one infer item.
//! - **Audio access**: Workers need the audio file path and time range, not just text.
//! - **DP alignment in Rust**: Model output is aligned to transcript words via Hirschberg.
//!
//! # Invariants for contributors
//!
//! - FA worker timestamps are chunk-relative; `parse_fa_response` must convert
//!   them to file-absolute ms with `audio_start_ms`.
//! - `apply_fa_results` ordering is load-bearing:
//!   inject → postprocess → utterance bullet update → `%wor` generation
//!   → monotonicity (E362) → same-speaker overlap enforcement (E704).
//! - Cache keys must include audio identity + time window + text + timing mode
//!   + engine; changing dimensions changes cache compatibility.

mod transport;

use crate::cache::CacheBackend;
use crate::chat_ops::fa::{
    WordTiming, apply_fa_results, cache_key, enforce_monotonicity, expand_bullets_for_edge_fillers,
    find_reusable_utterance_indices, group_utterances, has_reusable_wor_timing,
    refresh_existing_alignment, refresh_reusable_utterances, rescue_narrow_bullets,
    strip_wor_from_monotonicity_stripped_utterances,
};
use crate::chat_ops::{CacheKey, CacheTaskName};
use crate::params::{AudioContext, FaParams};
use crate::pipeline::PipelineServices;
use talkbank_transform::parse::{is_dummy, is_no_align, parse_lenient};
use talkbank_transform::serialize::to_chat_string;
use talkbank_transform::validate::{ValidityLevel, validate_output, validate_to_level};
use tracing::{info, warn};

use crate::api::DurationMs;
use crate::error::ServerError;
use crate::runner::util::{FileStage, ProgressSender, ProgressUpdate};
use crate::types::results::FaResult;
use crate::types::traces::{FaGroupTrace, TimingTrace, ViolationTrace};
use transport::{FaWorkerBatch, FaWorkerTransport};

/// Cache task name for FA results.
const CACHE_TASK: CacheTaskName = CacheTaskName::ForcedAlignment;

pub(super) fn collect_final_timings(
    all_timings: Vec<Option<Vec<Option<WordTiming>>>>,
    context: &str,
) -> Result<Vec<Vec<Option<WordTiming>>>, ServerError> {
    let missing_groups: Vec<usize> = all_timings
        .iter()
        .enumerate()
        .filter_map(|(index, timings)| timings.is_none().then_some(index))
        .collect();
    if !missing_groups.is_empty() {
        return Err(ServerError::Validation(format!(
            "{context} completed without timings for group(s): {missing_groups:?}"
        )));
    }

    // Safety: the None check above returned Err for any missing groups,
    // so all remaining elements are guaranteed Some.
    Ok(all_timings.into_iter().flatten().collect())
}

// ---------------------------------------------------------------------------
// Per-file FA processing
// ---------------------------------------------------------------------------

/// Process a single CHAT file through the forced alignment pipeline.
///
/// Returns a structured [`FaResult`] containing the serialized CHAT text,
/// group info, timing data, and validation results.  The caller decides
/// which parts to persist (file output, trace cache, etc.).
///
/// Algorithm outline:
/// 1. Parse leniently and run pre-validation (`MainTierValid`).
/// 2. Group utterances into FA windows.
/// 3. Resolve cache hits/misses per group.
/// 4. Send miss groups through the FA worker transport adapter.
/// 5. Parse responses and align to transcript words in Rust.
/// 6. Apply timings + postprocessing (`apply_fa_results`).
/// 7. Run full post-validation and serialize.
pub(crate) async fn process_fa(
    chat_text: &str,
    audio: &AudioContext<'_>,
    worker_lang: &crate::api::LanguageCode3,
    services: PipelineServices<'_>,
    fa_params: &FaParams,
    progress: Option<&ProgressSender>,
) -> Result<FaResult, ServerError> {
    run_fa_impl(chat_text, audio, worker_lang, services, fa_params, progress).await
}

pub(crate) async fn run_fa_impl(
    chat_text: &str,
    audio: &AudioContext<'_>,
    worker_lang: &crate::api::LanguageCode3,
    services: PipelineServices<'_>,
    fa_params: &FaParams,
    progress: Option<&ProgressSender>,
) -> Result<FaResult, ServerError> {
    // 1. Parse
    let parser = crate::chat_parser();
    let (chat_file, parse_errors) = parse_lenient(&parser, chat_text);
    if !parse_errors.is_empty() {
        warn!(
            num_errors = parse_errors.len(),
            "Parse errors in FA input (continuing with recovery)"
        );
    }

    run_fa_from_ast(
        chat_file,
        parse_errors,
        audio,
        worker_lang,
        services,
        fa_params,
        progress,
    )
    .await
}

/// Run forced alignment on a pre-parsed `ChatFile`.
///
/// This is the primary FA entry point when the caller already owns a `ChatFile`
/// AST (e.g., after UTR injection). It avoids the serialize→re-parse cycle that
/// `process_fa(&str)` performs.
pub(crate) async fn run_fa_from_ast(
    mut chat_file: crate::chat_ops::ChatFile,
    parse_errors: Vec<crate::chat_ops::ParseError>,
    audio: &AudioContext<'_>,
    worker_lang: &crate::api::LanguageCode3,
    services: PipelineServices<'_>,
    fa_params: &FaParams,
    progress: Option<&ProgressSender>,
) -> Result<FaResult, ServerError> {
    // 1a′. Suppress %wor for Conversation Analysis transcripts.
    // CA transcripts (@Options: CA) use prosodic notation (⌈⌉⌊⌋, arrows,
    // lengthening marks) that %wor cannot represent. Generating %wor for
    // these files adds noise that CA researchers must manually remove.
    let write_wor = if chat_file.options.iter().any(|f| f.enables_ca_mode()) {
        info!("@Options: CA detected — suppressing %wor generation");
        false
    } else {
        fa_params.wor_tier.should_write()
    };

    // 1b. Skip dummy files
    if is_dummy(&chat_file) {
        return Ok(FaResult {
            chat_text: to_chat_string(&chat_file),
            groups: Vec::new(),
            pre_injection_timings: Vec::new(),
            timing_mode: fa_params.timing_mode,
            violations: Vec::new(),
            fallback_events: Vec::new(),
        });
    }

    // 1c. @Options: NoAlign — strict pass-through, zero modifications.
    //
    // A researcher who sets this option has opted the file out of all
    // alignment processing.  The file is returned EXACTLY as parsed:
    // no timestamps added, removed, or adjusted, no %wor generated,
    // no decision tiers written.  This includes cleanup passes that
    // might seem safe (e.g., monotonicity enforcement) — those are
    // the researcher's responsibility.
    //
    // See book/src/developer/commands/align.md — "NoAlign: strict pass-through".
    if is_no_align(&chat_file) {
        return Ok(FaResult {
            chat_text: to_chat_string(&chat_file),
            groups: Vec::new(),
            pre_injection_timings: Vec::new(),
            timing_mode: fa_params.timing_mode,
            violations: Vec::new(),
            fallback_events: Vec::new(),
        });
    }

    // 1d. Pre-validation gate (L2: MainTierValid)
    if let Err(errors) = validate_to_level(&chat_file, &parse_errors, ValidityLevel::MainTierValid)
    {
        let msgs: Vec<String> = errors.iter().map(|e| e.to_string()).collect();
        return Err(ServerError::Validation(format!(
            "align pre-validation failed: {}",
            msgs.join("; ")
        )));
    }

    // 1e. Cheap rerun path: if the file already has complete, reusable `%wor`
    // timing, rebuild main-tier bullets and optionally regenerate `%wor`
    // without sending audio back through FA.
    if has_reusable_wor_timing(&chat_file) {
        info!("FA fast path: reusing existing %wor timing");
        refresh_existing_alignment(&mut chat_file, write_wor);

        // A previous run may have written backward `%wor` timestamps (e.g.
        // APROCSA 2256_T4.cha: UTR anchor drift placed utterances from task N
        // into task N-1's audio window).  Without these two steps, every re-run
        // reconstructs the backward main-tier bullet from the stale `%wor`
        // data, and the E362 violation persists indefinitely.
        //
        // Step 1: strip backward main-tier bullets.
        let decisions = enforce_monotonicity(&mut chat_file);
        // Step 2: remove `%wor` from stripped utterances so the next run goes
        // through full FA rather than reconstructing the backward bullet again.
        strip_wor_from_monotonicity_stripped_utterances(&mut chat_file, &decisions);

        return Ok(FaResult {
            chat_text: to_chat_string(&chat_file),
            groups: Vec::new(),
            pre_injection_timings: Vec::new(),
            timing_mode: fa_params.timing_mode,
            violations: Vec::new(),
            fallback_events: Vec::new(),
        });
    }

    // 1f. Per-utterance partial reuse: when some (but not all) utterances have
    // clean %wor, refresh those and track them so their FA groups can be skipped.
    let reusable_indices = find_reusable_utterance_indices(&chat_file);
    if !reusable_indices.is_empty() {
        info!(
            reusable = reusable_indices.len(),
            "FA partial reuse: refreshing utterances with clean %wor"
        );
        refresh_reusable_utterances(&mut chat_file, &reusable_indices, write_wor);
    }

    // 2a. Rescue catastrophically narrow utterance bullets before grouping.
    //
    // When `transcribe` writes a bullet that is physically too narrow to
    // contain its words (e.g., 22 words in 380 ms = 58 wps, impossible),
    // FA cannot align the words against that audio range. Wave2Vec rejects
    // the group with "targets length is too long for CTC" because the
    // encoder produces too few frames for the target labels, and the
    // Whisper FA fallback path produces degenerate token-level timings
    // (zero-duration words, words past the bullet end). The user sees a
    // CHAT file with a `%wor` tier full of broken timings.
    //
    // The rescue pre-pass detects under-budgeted bullets and expands them
    // into the trailing inter-utterance gap, giving FA a wide-enough audio
    // window to find the actual speech. After FA finishes,
    // `update_utterance_bullet` overwrites the rescued range with the FA
    // word span (which is tighter), so the rescue is self-healing.
    //
    // Covered by the private regression fixture set under
    // `resources/fixtures/align/regressions/` (gitignored; see
    // `book/src/developer/regression-fixtures.md`).
    let rescue_decisions = rescue_narrow_bullets(&mut chat_file);

    // 2b. Expand utterance bullets to cover edge fillers in inter-utterance gaps.
    // UTR-assigned bullets may be too narrow to include trailing/leading fillers
    // whose audio lives in the gap between utterances.
    expand_bullets_for_edge_fillers(&mut chat_file);

    // 2c. Group utterances
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

    info!(
        num_groups = groups.len(),
        total_words = groups.iter().map(|g| g.words.len()).sum::<usize>(),
        "FA grouping complete"
    );

    if let Some(tx) = progress {
        let _ = tx.send(ProgressUpdate::new(
            FileStage::CheckingCache,
            Some(0),
            Some(groups.len() as i64),
        ));
    }

    // 3. For each group: compute cache key, check cache
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

    // 4. Cache lookup
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
                warn!(error = %e, "FA cache batch lookup failed (treating all as misses)");
                std::collections::HashMap::new()
            }
        }
    };

    // 5. Partition into reused (from %wor), cache hits, and misses
    let mut all_timings: Vec<Option<Vec<Option<WordTiming>>>> = vec![None; groups.len()];
    let mut miss_indices: Vec<usize> = Vec::new();
    let mut reused_group_count = 0usize;

    for (i, key) in cache_keys.iter().enumerate() {
        // Tier 1: group fully reusable from %wor (all utterances have clean timing)
        if !reusable_indices.is_empty()
            && groups[i]
                .utterance_indices
                .iter()
                .all(|idx| reusable_indices.contains(&idx.raw()))
            && let Some(timings) =
                incremental::collect_preserved_group_timings(&chat_file, &groups[i])
        {
            all_timings[i] = Some(timings);
            reused_group_count += 1;
            continue;
        }

        // Tier 2: cache hit
        if let Some(cached_data) = cached.get(key.as_str()) {
            match serde_json::from_value::<Vec<Option<WordTiming>>>(cached_data.clone()) {
                Ok(timings) => {
                    all_timings[i] = Some(timings);
                    continue;
                }
                Err(e) => {
                    warn!(error = %e, group = i, "Failed to deserialize cached FA timings (re-computing)");
                }
            }
        }

        // Tier 3: cache miss
        miss_indices.push(i);
    }

    let cache_hits = groups.len() - miss_indices.len() - reused_group_count;
    let reused_or_cached_groups = reused_group_count + cache_hits;
    if cache_hits > 0 || reused_group_count > 0 {
        info!(
            reused = reused_group_count,
            cache_hits = cache_hits,
            misses = miss_indices.len(),
            "FA partition (reused from %wor / cache hits / misses)"
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

    // 6. Dispatch miss groups through the FA worker transport adapter
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

            // Cache the result
            let ba_version = env!("CARGO_PKG_VERSION");
            if let Ok(cache_data) = serde_json::to_value(&timings)
                && let Err(e) = services
                    .cache
                    .put_batch(
                        &[(cache_keys[miss_idx].as_str().to_string(), cache_data)],
                        CACHE_TASK.as_str(),
                        services.engine_version,
                        ba_version,
                    )
                    .await
            {
                warn!(error = %e, "Failed to cache FA result (non-fatal)");
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

    // 8. Apply all results
    if let Some(tx) = progress {
        let _ = tx.send(ProgressUpdate::new(
            FileStage::ApplyingResults,
            Some(groups.len() as i64),
            Some(groups.len() as i64),
        ));
    }

    let final_timings = collect_final_timings(all_timings, "forced alignment")?;

    // Snapshot pre-injection timings (before apply_fa_results consumes them)
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

    let fa_decisions = apply_fa_results(
        &mut chat_file,
        &groups,
        &final_timings,
        fa_params.timing_mode,
        write_wor,
    );

    // 9. Post-FA bullet repair (experimental, opt-in via --bullet-repair).
    let repair_decisions = if fa_params.bullet_repair {
        let repair_result = crate::chat_ops::fa::repair_bullets(&mut chat_file, false);
        tracing::info!(%repair_result.stats, "bullet repair applied");
        repair_result.decisions
    } else {
        Vec::new()
    };

    // 9b. (Decision tiers injected in step 9d below, after all decisions are collected.)

    // 9c. Enforce monotonicity: strip non-monotonic start times and clamp
    //    end-time overlaps. The old enforcement was removed (see comment in
    //    apply_fa_results) because it stripped too aggressively. The current
    //    version only strips start-time regressions and clamps end times to
    //    the next utterance's start — no timing is destroyed, only truncated.
    let monotonicity_decisions = crate::chat_ops::fa::enforce_monotonicity(&mut chat_file);

    // 9d. Inject decision provenance tiers (%xalign / %xrev) for all
    //    pipeline decisions that altered the output.
    {
        let mut all_decisions: Vec<talkbank_transform::decisions::DecisionRecord> = Vec::new();

        // Narrow-bullet rescue decisions (from step 2a, before grouping).
        // Surfaced via %xalign so the audit trail records which utterances
        // had their bullets pre-expanded due to transcribe under-budgeting.
        all_decisions.extend(rescue_decisions);

        // FA postprocessing decisions (word timing drops, from step 8)
        all_decisions.extend(fa_decisions);

        // Repair decisions (from bullet repair, step 9)
        all_decisions.extend(repair_decisions.iter().map(Into::into));

        // Monotonicity decisions (from step 9c)
        all_decisions.extend(monotonicity_decisions);

        // Always strip stale decision tiers from previous runs, regardless of
        // whether new decisions were made.  Without this, a clean re-run (no
        // decisions) leaves old %xalign/%xrev tiers in place; the NEXT run that
        // DOES produce decisions then appends to them, creating duplicates.
        talkbank_transform::decisions::strip_decision_tiers(&mut chat_file);
        if !all_decisions.is_empty() {
            talkbank_transform::decisions::inject_decision_tiers(
                &mut chat_file,
                &all_decisions,
                fa_params.review_level,
            );
        }
    }

    // 10. Post-validation check (warn only — cross-speaker overlap is normal in
    //    conversation data).
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

    // 10. Build group traces
    let group_traces: Vec<FaGroupTrace> = groups
        .iter()
        .map(|g| FaGroupTrace {
            audio_start_ms: DurationMs(g.audio_start_ms()),
            audio_end_ms: DurationMs(g.audio_end_ms()),
            utterance_indices: g.utterance_indices.iter().map(|idx| idx.0).collect(),
            words: g.words.iter().map(|w| w.text.clone()).collect(),
        })
        .collect();

    // 11. Serialize and return structured result
    Ok(FaResult {
        chat_text: to_chat_string(&chat_file),
        groups: group_traces,
        pre_injection_timings,
        timing_mode: fa_params.timing_mode,
        violations,
        fallback_events,
    })
}

mod incremental;
pub(crate) use incremental::process_fa_incremental;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cache_task_name_is_stable() {
        assert_eq!(CACHE_TASK.as_str(), "forced_alignment");
    }

    #[test]
    fn collect_final_timings_rejects_missing_groups() {
        let error = collect_final_timings(vec![Some(Vec::new()), None], "forced alignment")
            .expect_err("missing timing groups should fail");
        assert!(
            error
                .to_string()
                .contains("completed without timings for group(s): [1]")
        );
    }
}
