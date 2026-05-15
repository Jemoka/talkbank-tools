//! Pre-FA rescue for catastrophically narrow utterance bullets.
//!
//! When `transcribe` produces an utterance bullet that is physically too
//! narrow to contain the utterance's words — for example, a 22-word
//! sentence stamped as a 380 ms span (~58 words per second, impossible
//! for human speech) — the FA pipeline cannot align the words against
//! the bullet's audio range. Wave2Vec rejects the group with "targets
//! length is too long for CTC" because there are far more word labels
//! than encoder frames, and the Whisper FA fallback path produces
//! degenerate token-level timings (zero-duration words, words past the
//! bullet end). The user sees a CHAT file with a `%wor` tier full of
//! 0-duration words and an utterance bullet that still pretends the
//! utterance is 380 ms long.
//!
//! This pre-pass detects under-budgeted bullets and expands them into
//! the trailing inter-utterance gap before the grouping step runs.
//! Once the audio window is wide enough, Wave2Vec succeeds, the Whisper
//! fallback never fires, and the per-word timings are sane. After FA
//! finishes, `update_utterance_bullet` will overwrite the rescued
//! bullet with the FA word span, so the rescue is self-healing as long
//! as FA succeeds. The rescued bullet is therefore still a provisional
//! grouping hint, not an authoritative boundary to clamp FA back into.
//!
//! See also `expand_for_fillers.rs`, which performs a related but
//! different pre-pass: it expands bullets to cover trailing/leading
//! filler audio that sits in the gap, capped at 1500 ms per side. This
//! module fires only when the bullet is severely under-budgeted
//! relative to its word count, and expands much more aggressively
//! (up to the trailing gap minus a small buffer) because the original
//! bullet conveys no useful information about where the speech
//! actually lives in the audio.

use talkbank_model::model::{BulletSource, ChatFile, Line};

#[cfg(test)]
use talkbank_transform::decisions::DecisionModule;
use talkbank_transform::decisions::DecisionRecord;

// ---------------------------------------------------------------------------
// Tunables
// ---------------------------------------------------------------------------

/// Word density above which a bullet is considered physically impossible
/// — the **catastrophic** trigger for the rescue.
///
/// Normal English speech is 2-7 words per second; rapid news-anchor delivery
/// reaches ~9 wps; auctioneer or rap is in the 12-15 wps range. Anything
/// above 15 wps is not natural speech and reliably indicates a broken
/// transcribe-time bullet, not a fast speaker. When triggered by this
/// threshold, the rescue expands aggressively all the way to the next
/// utterance's start (minus a safety buffer) because the original bullet
/// conveys no useful information about where the speech actually lives.
const CATASTROPHIC_DENSITY_THRESHOLD_WPS: f64 = 15.0;

/// Minimum acceptable per-word duration (ms) — the **tight-but-not-broken**
/// trigger for the rescue.
///
/// At normal conversational pace, words take 250-500 ms each (after
/// accounting for inter-word silences and consonant clusters). A bullet
/// that gives less than 250 ms per word on average is tight enough that
/// the FA's per-word DP allocation routinely collapses the tail of the
/// utterance into 40-60 ms slivers. A 21-word utterance in a 4740 ms
/// bullet (≈ 225 ms/word) is the canonical example: the closing words
/// collapse to `to 40 go 40 to 40 the 60 ball 140`.
///
/// Tighter than this threshold but not catastrophic enough for the
/// `CATASTROPHIC_DENSITY_THRESHOLD_WPS` rule, we apply a milder
/// expansion that gives the utterance enough room for
/// `TIGHT_BULLET_TARGET_MS_PER_WORD` per word — capped, of course, at
/// the next utterance's start minus the safety buffer.
const TIGHT_BULLET_THRESHOLD_MS_PER_WORD: f64 = 250.0;

/// When the tight-bullet trigger fires, expand the bullet so that there
/// is at least this many milliseconds of audio per word. 350 ms/word is
/// comfortable for normal-pace English narrative and gives Wave2Vec
/// enough headroom to align the closing words without collapse, while
/// staying well short of overrunning the next utterance.
const TIGHT_BULLET_TARGET_MS_PER_WORD: u64 = 350;

/// Minimum gap (ms) to leave between the rescued utterance's expanded end
/// and the next utterance's start. Prevents the FA audio window from
/// running into the next utterance's speech and confusing the aligner.
const SAFETY_BUFFER_MS: u64 = 200;

// ---------------------------------------------------------------------------
// Trigger classification
// ---------------------------------------------------------------------------

/// Why the rescue fired for a particular utterance.
///
/// We carry this through the pipeline so the decision tier and the
/// audit log can record which threshold tripped, and so the expansion
/// strategy can pick the right new end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RescueTrigger {
    /// `density_wps > CATASTROPHIC_DENSITY_THRESHOLD_WPS`. The bullet is
    /// physically impossible for the word count and conveys no useful
    /// information about where the speech is in the audio. Expand all
    /// the way to `next_start - SAFETY_BUFFER_MS`.
    Catastrophic,
    /// `ms_per_word < TIGHT_BULLET_THRESHOLD_MS_PER_WORD`. The bullet is
    /// believable but tight, and the FA's per-word DP routinely
    /// collapses the tail. Expand to `start + word_count *
    /// TIGHT_BULLET_TARGET_MS_PER_WORD`, capped at `next_start -
    /// SAFETY_BUFFER_MS`.
    Tight,
}

impl RescueTrigger {
    fn as_decision_tag(self) -> &'static str {
        match self {
            Self::Catastrophic => "catastrophic_density",
            Self::Tight => "tight_per_word_budget",
        }
    }
}

/// Classify a bullet's word density and pick the right rescue trigger
/// (if any). Returns `None` when the bullet is in the normal range and
/// no rescue is needed.
fn classify_bullet(word_count: usize, start_ms: u64, end_ms: u64) -> Option<RescueTrigger> {
    if word_count == 0 {
        return None;
    }
    if end_ms <= start_ms {
        // Zero or negative duration is always catastrophic — there is
        // no audio at all for these words.
        return Some(RescueTrigger::Catastrophic);
    }
    let duration_ms = end_ms - start_ms;
    let duration_s = duration_ms as f64 / 1000.0;
    let words_per_second = word_count as f64 / duration_s;
    if words_per_second > CATASTROPHIC_DENSITY_THRESHOLD_WPS {
        return Some(RescueTrigger::Catastrophic);
    }
    // Float division because the threshold (250.0) is a soft physical
    // limit. Integer division at 4740 / 21 = 225 vs the more precise
    // 225.7 does not change the answer here, but using float
    // consistently avoids surprise rounding at boundary cases.
    let ms_per_word = duration_ms as f64 / word_count as f64;
    if ms_per_word < TIGHT_BULLET_THRESHOLD_MS_PER_WORD {
        return Some(RescueTrigger::Tight);
    }
    None
}

/// Compute the rescued bullet end given the trigger, current bullet,
/// word count, and next utterance start. Returns `None` when no
/// expansion is possible (e.g., next utterance is too close, or the
/// computed expansion would not actually widen the bullet).
fn rescued_end_ms(
    trigger: RescueTrigger,
    start_ms: u64,
    end_ms: u64,
    word_count: usize,
    next_start_ms: u64,
) -> Option<u64> {
    // The hard upper bound: we never expand into the next utterance's
    // safety buffer.
    if next_start_ms <= end_ms + SAFETY_BUFFER_MS {
        return None;
    }
    let upper_bound = next_start_ms - SAFETY_BUFFER_MS;

    let proposed_end = match trigger {
        RescueTrigger::Catastrophic => upper_bound,
        RescueTrigger::Tight => {
            // Want at least TIGHT_BULLET_TARGET_MS_PER_WORD per word.
            let target_duration = (word_count as u64) * TIGHT_BULLET_TARGET_MS_PER_WORD;
            let target_end = start_ms + target_duration;
            // The proposed end is the larger of the current end and the
            // target end (we never shrink), capped at the upper bound.
            target_end.max(end_ms).min(upper_bound)
        }
    };

    // Only return Some when this actually widens the bullet.
    if proposed_end > end_ms {
        Some(proposed_end)
    } else {
        None
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Expand utterance bullets that are too narrow for their word count.
///
/// For each timed utterance whose word density exceeds
/// `NARROW_BULLET_THRESHOLD_WPS`, this function extends the bullet's
/// `end_ms` toward the next utterance's start (capped at
/// `next_start - SAFETY_BUFFER_MS`). The bullet's `start_ms` is left
/// unchanged because the start time of the utterance is the more
/// trustworthy of the two boundaries from `transcribe`.
///
/// Returns one `DecisionRecord` per rescued utterance so the
/// `%xalign`/`%xrev` decision tiers reflect the rescue.
///
/// ## Why this is safe
///
/// 1. The rescue is self-healing: after FA succeeds, the FA word span
///    overwrites the rescued bullet via `update_utterance_bullet`, so the
///    final bullet is the actual speech range (tighter than the rescue).
/// 2. Untimed utterances are not touched; only utterances whose bullet
///    already exists but is under-budgeted.
/// 3. The trailing gap is the audio that sits between this utterance and
///    the next, with a `SAFETY_BUFFER_MS` of margin so the FA window
///    cannot bleed into the next utterance's speech.
/// 4. When there is no next utterance (last utterance in the file), the
///    rescue does nothing — there is no upper bound to safely extend to.
pub fn rescue_narrow_bullets(chat_file: &mut ChatFile) -> Vec<DecisionRecord> {
    let mut decisions = Vec::new();

    // First pass: collect (line_idx, current_bullet, word_count, next_start) tuples.
    // Done in a separate pass so the second pass can mutate bullets without
    // double-borrowing the lines vec.
    let observations = collect_rescue_candidates(chat_file);

    for obs in observations {
        // Classify the bullet density and pick a trigger.
        let Some(trigger) = classify_bullet(obs.word_count, obs.bullet_start_ms, obs.bullet_end_ms)
        else {
            continue;
        };

        // We need a next-utterance anchor to know how far we can safely
        // expand. Without one (last utterance in the file), the rescue
        // skips rather than guess.
        let Some(next_start) = obs.next_start_ms else {
            continue;
        };

        // Compute the expansion target. Returns `None` when there is no
        // room to expand (next utterance too close) or when the computed
        // target would not widen the bullet at all.
        let Some(new_end) = rescued_end_ms(
            trigger,
            obs.bullet_start_ms,
            obs.bullet_end_ms,
            obs.word_count,
            next_start,
        ) else {
            continue;
        };

        // Apply the expansion.
        let Some(Line::Utterance(utt)) = chat_file.lines.get_mut(obs.line_idx) else {
            continue;
        };
        let Some(bullet) = utt.main.content.bullet.as_mut() else {
            continue;
        };
        let original_end = bullet.timing.end_ms;
        bullet.timing.end_ms = new_end;
        bullet.source = BulletSource::Utr;

        decisions.push(DecisionRecord::new_and_trace(
            obs.line_idx,
            utt.main.speaker.as_str().to_string(),
            talkbank_transform::decisions::DecisionStrategy::Fa(
                talkbank_transform::decisions::FaStrategy::NarrowBulletRescued,
            ),
            format!(
                "word_count={wc} original_end={oe} expanded_end={ne} \
                 next_start={ns} trigger={tag} \
                 cause=transcribe_bullet_too_narrow_for_word_count",
                wc = obs.word_count,
                oe = original_end,
                ne = new_end,
                ns = next_start,
                tag = trigger.as_decision_tag(),
            ),
            // The rescue is a routine pre-pass correction, not a sign that
            // something is broken in the user's input. Surface it via
            // `%xalign` (audit trail) but do not trigger `%xrev` (human
            // review) — the FA result is still correct after the rescue.
            false,
        ));
    }

    decisions
}

// ---------------------------------------------------------------------------
// Internals
// ---------------------------------------------------------------------------

/// One observation collected during the read-only first pass.
struct RescueObservation {
    /// Line index in `chat_file.lines` (matches `Line::Utterance` position).
    line_idx: usize,
    /// Current bullet start in ms.
    bullet_start_ms: u64,
    /// Current bullet end in ms.
    bullet_end_ms: u64,
    /// Number of FA-alignable words on the main tier.
    word_count: usize,
    /// Start time of the next timed utterance, if any.
    next_start_ms: Option<u64>,
}

/// Walk the file once and collect rescue candidates with their next-utterance
/// anchors. Done as a read-only pass so the mutation pass in the caller can
/// borrow `lines` mutably without re-borrow conflicts.
///
/// Word counting uses `super::count_alignable_main_words` so the rescue's
/// density threshold operates on the same word population FA will receive
/// downstream. This is one of the four sites listed in `CLAUDE.md`'s
/// "ReplacedWord Extraction/Injection Policy" table that must stay in sync;
/// do not duplicate the counting logic here.
fn collect_rescue_candidates(chat_file: &ChatFile) -> Vec<RescueObservation> {
    // First pass: build candidates with `next_start_ms` unset.
    let mut observations: Vec<RescueObservation> = Vec::new();
    for (line_idx, line) in chat_file.lines.iter().enumerate() {
        let Line::Utterance(utt) = line else {
            continue;
        };
        let Some(bullet) = utt.main.content.bullet.as_ref() else {
            continue;
        };
        let word_count = super::count_alignable_main_words(utt);
        if word_count == 0 {
            continue;
        }
        observations.push(RescueObservation {
            line_idx,
            bullet_start_ms: bullet.timing.start_ms,
            bullet_end_ms: bullet.timing.end_ms,
            word_count,
            next_start_ms: None,
        });
    }
    // Second pass: fill each candidate's `next_start_ms` from the next
    // candidate's start. Kept separate from the first pass because the
    // read-only `chat_file.lines` borrow would otherwise conflict with
    // mutating `observations` in the same loop.
    for i in 0..observations.len().saturating_sub(1) {
        observations[i].next_start_ms = Some(observations[i + 1].bullet_start_ms);
    }
    observations
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_transform::parse::{TreeSitterParser, parse_lenient};

    /// Build a small CHAT file with three utterances. The middle one has a
    /// 22-word main tier compressed into a 380 ms bullet — exactly the
    /// pathological pattern the rescue is designed to catch.
    fn synthetic_narrow_bullet_chat() -> String {
        // The neighbor utterances have normal-density bullets so the rescue
        // can compute a "next start" anchor for the under-budgeted middle
        // utterance.
        "@UTF8\n\
         @Begin\n\
         @Languages:\teng\n\
         @Participants:\tCHI Target_Child\n\
         @ID:\teng|test|CHI|||||Target_Child|||\n\
         *CHI:\thello there friends . \u{15}1000_3000\u{15}\n\
         *CHI:\tone two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo . \u{15}3500_3880\u{15}\n\
         *CHI:\tafterward they went home . \u{15}20000_22000\u{15}\n\
         @End\n"
            .to_string()
    }

    fn parse_chat(source: &str) -> ChatFile {
        let parser = TreeSitterParser::new().expect("parser construct");
        let (file, errors) = parse_lenient(&parser, source);
        assert!(errors.is_empty(), "parse errors: {errors:?}");
        file
    }

    fn nth_utterance_bullet(file: &ChatFile, n: usize) -> (u64, u64) {
        let utt = file
            .utterances()
            .nth(n)
            .unwrap_or_else(|| panic!("no utterance #{n}"));
        let bullet = utt
            .main
            .content
            .bullet
            .as_ref()
            .unwrap_or_else(|| panic!("utterance #{n} has no bullet"));
        (bullet.timing.start_ms, bullet.timing.end_ms)
    }

    #[test]
    fn rescue_catastrophic_expands_to_next_utterance_minus_buffer() {
        // The middle utterance has 22 words in 380 ms (~58 wps), which is
        // far above the catastrophic-density threshold (15 wps). The
        // catastrophic trigger fires and expands the bullet all the way
        // to next_start - SAFETY_BUFFER_MS = 20000 - 200 = 19800.
        let mut file = parse_chat(&synthetic_narrow_bullet_chat());
        assert_eq!(nth_utterance_bullet(&file, 1), (3500, 3880));

        let decisions = rescue_narrow_bullets(&mut file);

        assert_eq!(nth_utterance_bullet(&file, 1), (3500, 19800));

        assert_eq!(decisions.len(), 1);
        let d = &decisions[0];
        assert_eq!(d.strategy.module(), DecisionModule::Fa);
        assert_eq!(d.strategy.strategy_name(), "narrow_bullet_rescued");
        assert!(d.reason.contains("word_count=22"));
        assert!(d.reason.contains("original_end=3880"));
        assert!(d.reason.contains("expanded_end=19800"));
        assert!(d.reason.contains("trigger=catastrophic_density"));
    }

    #[test]
    fn rescue_tight_expands_to_target_ms_per_word() {
        // 20 words in 3500 ms = 5.7 wps (well under catastrophic 15 wps)
        // and 175 ms/word (below the 200 ms/word tight floor). Tight
        // trigger fires and expands to start + 20 * 350 = 7000 ms
        // duration, i.e. end = 1000 + 7000 = 8000, well below the next
        // utterance start - safety buffer (19800).
        let chat = "@UTF8\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI|||||Target_Child|||\n\
                    *CHI:\twe walked along the path until we reached the meadow with the tall green grass and the bright yellow flowers . \u{15}1000_4500\u{15}\n\
                    *CHI:\tbye . \u{15}20000_22000\u{15}\n\
                    @End\n";
        let mut file = parse_chat(chat);
        assert_eq!(nth_utterance_bullet(&file, 0), (1000, 4500));

        let decisions = rescue_narrow_bullets(&mut file);

        // 20 words * 350 ms = 7000 ms target → end = 1000 + 7000 = 8000.
        // Below the upper bound 19800, so the target wins.
        assert_eq!(nth_utterance_bullet(&file, 0), (1000, 8000));

        assert_eq!(decisions.len(), 1);
        assert!(
            decisions[0]
                .reason
                .contains("trigger=tight_per_word_budget")
        );
        assert!(decisions[0].reason.contains("word_count=20"));
    }

    #[test]
    fn rescue_tight_does_not_shrink_already_wide_bullet() {
        // The tight trigger should never shrink a bullet — only widen.
        // The 22-word utterance below has a 12000 ms bullet, giving
        // 545 ms/word: well above the 250 ms/word tight floor and well
        // below the 15 wps catastrophic floor. The rescue must be a
        // no-op.
        let chat = "@UTF8\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI|||||Target_Child|||\n\
                    *CHI:\twe walked along the path until we reached the meadow with the tall green grass and the bright yellow flowers and butterflies . \u{15}1000_13000\u{15}\n\
                    *CHI:\tbye . \u{15}20000_22000\u{15}\n\
                    @End\n";
        let mut file = parse_chat(chat);
        let before = nth_utterance_bullet(&file, 0);

        let _ = rescue_narrow_bullets(&mut file);

        assert_eq!(nth_utterance_bullet(&file, 0), before);
    }

    #[test]
    fn rescue_does_not_touch_normal_density_bullets() {
        // First and third utterances are normal density (~1-2 wps); they
        // must stay untouched by the rescue.
        let mut file = parse_chat(&synthetic_narrow_bullet_chat());
        let before_first = nth_utterance_bullet(&file, 0);
        let before_third = nth_utterance_bullet(&file, 2);

        let _ = rescue_narrow_bullets(&mut file);

        assert_eq!(nth_utterance_bullet(&file, 0), before_first);
        assert_eq!(nth_utterance_bullet(&file, 2), before_third);
    }

    #[test]
    fn rescue_skips_when_next_utterance_is_too_close() {
        // Synthetic file where the under-budgeted utterance has only a 100 ms
        // gap before the next utterance — less than SAFETY_BUFFER_MS, so the
        // rescue must do nothing rather than introduce overlap.
        let chat = "@UTF8\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI|||||Target_Child|||\n\
                    *CHI:\tone two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo . \u{15}1000_1380\u{15}\n\
                    *CHI:\tnext . \u{15}1480_1700\u{15}\n\
                    @End\n";
        let mut file = parse_chat(chat);
        let before = nth_utterance_bullet(&file, 0);

        let decisions = rescue_narrow_bullets(&mut file);

        assert_eq!(nth_utterance_bullet(&file, 0), before);
        assert!(decisions.is_empty());
    }

    #[test]
    fn rescue_skips_last_utterance_with_no_next_anchor() {
        // The last under-budgeted utterance has no next-utterance anchor.
        // Without an anchor we cannot safely choose an expansion target,
        // so the rescue must skip rather than guess.
        let chat = "@UTF8\n\
                    @Begin\n\
                    @Languages:\teng\n\
                    @Participants:\tCHI Target_Child\n\
                    @ID:\teng|test|CHI|||||Target_Child|||\n\
                    *CHI:\thello . \u{15}0_1000\u{15}\n\
                    *CHI:\tone two three four five six seven eight nine ten eleven twelve thirteen fourteen fifteen sixteen seventeen eighteen nineteen twenty twentyone twentytwo . \u{15}1500_1880\u{15}\n\
                    @End\n";
        let mut file = parse_chat(chat);
        let before = nth_utterance_bullet(&file, 1);

        let decisions = rescue_narrow_bullets(&mut file);

        assert_eq!(nth_utterance_bullet(&file, 1), before);
        assert!(decisions.is_empty());
    }

    #[test]
    fn classify_bullet_picks_the_right_trigger() {
        // Zero or negative duration is always catastrophic.
        assert_eq!(
            classify_bullet(1, 1000, 1000),
            Some(RescueTrigger::Catastrophic)
        );
        // 22 words in 380 ms = 57.9 wps → catastrophic.
        assert_eq!(
            classify_bullet(22, 1000, 1380),
            Some(RescueTrigger::Catastrophic)
        );
        // 5 words in 220 ms = 22.7 wps → catastrophic.
        assert_eq!(
            classify_bullet(5, 1000, 1220),
            Some(RescueTrigger::Catastrophic)
        );
        // 21 words in 4740 ms = 4.4 wps but 225.7 ms/word → tight.
        // This is the canonical "tight" pathology: enough words per
        // second to look like natural speech at a glance, but not
        // enough audio budget per word for the per-word DP to spread
        // them out without crushing the tail.
        assert_eq!(
            classify_bullet(21, 10160, 14900),
            Some(RescueTrigger::Tight)
        );
        // 20 words in 3500 ms = 5.7 wps and 175 ms/word → tight.
        assert_eq!(classify_bullet(20, 1000, 4500), Some(RescueTrigger::Tight));
        // 22 words in 6500 ms = ~295 ms/word → above the tight floor.
        assert_eq!(classify_bullet(22, 1000, 7500), None);
        // 1 word in 1 second is fine.
        assert_eq!(classify_bullet(1, 1000, 2000), None);
        // Empty utterance: classified as no rescue (no words to rescue).
        assert_eq!(classify_bullet(0, 1000, 1000), None);
    }
}
