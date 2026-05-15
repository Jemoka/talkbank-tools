//! Timestamp injection into the CHAT AST.

use talkbank_model::alignment::helpers::{
    TierDomain, WordItemMut, counts_for_tier, walk_words_mut,
};
use talkbank_model::model::{Bullet, Utterance, Word};

use super::WordTiming;

/// Read cursor into a flat array of word timings for an FA group.
///
/// Advances by one for each Wor-alignable word encountered.
pub struct TimingCursor<'a> {
    timings: &'a [Option<WordTiming>],
    pos: usize,
}

#[allow(dead_code)]
impl<'a> TimingCursor<'a> {
    /// Create a new cursor at position 0.
    pub fn new(timings: &'a [Option<WordTiming>]) -> Self {
        Self { timings, pos: 0 }
    }

    /// Create a new cursor starting at the given offset.
    pub fn with_offset(timings: &'a [Option<WordTiming>], offset: usize) -> Self {
        Self {
            timings,
            pos: offset,
        }
    }

    /// Advance the position and return the timing at the previous position.
    ///
    /// Always advances by one, even past the end — this matches the FA injection
    /// invariant that every alignable word must advance the cursor.
    pub fn take(&mut self) -> Option<&WordTiming> {
        let slot = self.timings.get(self.pos);
        self.pos += 1;
        slot.and_then(|o| o.as_ref())
    }

    /// Current read position.
    pub fn position(&self) -> usize {
        self.pos
    }
}

/// Inject word-level timings into the AST for a specific utterance.
///
/// `timings` is indexed by the flat word position within the group.
/// Only words that are Wor-alignable get timing (matching the extraction order).
///
/// * `utterance` - The utterance whose words will receive inline timing bullets.
/// * `timings` - Flat array of optional timings for the entire FA group. Each
///   element corresponds to one Wor-alignable word across all utterances in the
///   group.
/// * `timing_offset` - Current read position into `timings`. Advanced by one for
///   each Wor-alignable word encountered in this utterance. The caller should
///   initialize this to 0 for the first utterance in a group and pass the same
///   mutable reference through consecutive utterances.
pub fn inject_timings_for_utterance(
    utterance: &mut Utterance,
    timings: &[Option<WordTiming>],
    timing_offset: &mut usize,
) {
    let mut cursor = TimingCursor::with_offset(timings, *timing_offset);
    // domain=None: recurse into all groups unconditionally (FA needs all words)
    walk_words_mut(
        &mut utterance.main.content.content,
        None,
        &mut |leaf| match leaf {
            WordItemMut::Word(word) => {
                inject_timing_on_word(word, &mut cursor);
            }
            WordItemMut::ReplacedWord(replaced) => {
                // Extraction always sends the original word to FA (not the
                // replacement words).  Injection must mirror that policy:
                // consume exactly one cursor position for the original word
                // and set its inline bullet.  Replacement words are never
                // FA-aligned — they are corrections that the speaker did not
                // actually say.  Using the original word here keeps the
                // cursor in sync with extraction across utterance boundaries.
                inject_timing_on_word(&mut replaced.word, &mut cursor);
            }
            WordItemMut::Separator(_) => {}
        },
    );
    *timing_offset = cursor.position();
}

/// Inject timing onto a single CHAT word from the FA timing cursor.
///
/// For compound fillers (`&-you_know`), extraction split the word into N
/// parts for FA. We must consume N timings and merge them into one span.
fn inject_timing_on_word(word: &mut Word, cursor: &mut TimingCursor<'_>) {
    if !counts_for_tier(word, TierDomain::Wor) {
        return;
    }

    let parts = compound_filler_part_count(word);
    if parts <= 1 {
        // Normal word: consume one timing.
        if let Some(t) = cursor.take() {
            word.inline_bullet = Some(Bullet::new(t.start_ms, t.end_ms));
        }
    } else {
        // Compound filler: consume N timings and merge into one span.
        let mut min_start: Option<u64> = None;
        let mut max_end: Option<u64> = None;
        for _ in 0..parts {
            if let Some(t) = cursor.take() {
                min_start = Some(min_start.map_or(t.start_ms, |s: u64| s.min(t.start_ms)));
                max_end = Some(max_end.map_or(t.end_ms, |e: u64| e.max(t.end_ms)));
            }
        }
        if let (Some(start), Some(end)) = (min_start, max_end) {
            word.inline_bullet = Some(Bullet::new(start, end));
        }
    }
}

/// Return the number of FA words this CHAT word was split into during extraction.
///
/// Delegates to `split_compound_filler` — the single source of truth for the
/// splitting rule shared between extraction and injection.
fn compound_filler_part_count(word: &Word) -> usize {
    super::split_compound_filler(word).len()
}
