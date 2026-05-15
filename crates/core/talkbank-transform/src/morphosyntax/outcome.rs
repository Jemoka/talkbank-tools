//! Per-utterance morphotag outcome: the typed statement of what
//! happened on each utterance the pipeline visited
//! ([`MorOutcome`] / [`MorOutcomeKind`]), the structured "no Mor
//! content" reason ([`NotApplicableReason`]), the classifier that picks
//! that reason from CHAT content ([`classify_not_applicable`]), and the
//! adapter that converts an outcome into a [`DecisionRecord`] for
//! `%xalign` tier emission.
//!
//! The split exists because these types form a cohesive
//! "outcome reporting" boundary that callers consume independently of
//! the payload-collection or UD-typing layers.

use talkbank_model::alignment::helpers::{TierDomain, WordItem, walk_words};
use talkbank_model::model::{SpeakerCode, Utterance, WordCategory};

use crate::decisions::{DecisionRecord, DecisionStrategy, MorphosyntaxStrategy};
use crate::inject::MisalignmentDiagnostic;

/// One morphotag outcome for one utterance.
///
/// Carries enough information (line index, speaker, kind) to be converted
/// into a [`DecisionRecord`] for `%xalign` tier emission without further
/// context from the caller.
#[derive(Debug, Clone)]
pub struct MorOutcome {
    /// Index into `ChatFile.lines` identifying the utterance.
    pub line_idx: usize,
    /// Speaker code for the affected utterance.
    pub speaker: SpeakerCode,
    /// What happened on this utterance.
    pub kind: MorOutcomeKind,
}

/// The three possible morphotag outcomes per utterance.
#[derive(Debug, Clone)]
pub enum MorOutcomeKind {
    /// The utterance had zero Mor-alignable words under CHAT policy.
    /// No `%mor`/`%gra` was produced, and that is correct behavior —
    /// there is no morphological content to analyze.
    NotApplicable {
        /// Which class of non-linguistic content the utterance held.
        reason: NotApplicableReason,
    },

    /// Stanza returned N tokens for N CHAT words after MWT reassembly;
    /// `%mor`/`%gra` were injected successfully.
    Aligned {
        /// The alignable-word count that was matched on both sides.
        n_words: usize,
    },

    /// The `|stanza_tokens| = |chat_words|` invariant was violated.
    /// Always a bug in extraction, realignment, MWT reassembly, or the
    /// terminator filter. Never silently absorbed.
    MisalignmentBug(MisalignmentDiagnostic),
}

/// Why an utterance had no Mor-alignable content.
///
/// These reasons are mutually exclusive at the point of classification:
/// when an utterance yields zero Mor-alignable words, exactly one of
/// these describes what was there instead. The classifier walks the
/// utterance content once and picks the most specific variant that
/// matches every non-separator word in the utterance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotApplicableReason {
    /// The utterance body was empty after parsing (no words, no
    /// separators, no annotations that carry content).
    Empty,
    /// Every word in the utterance is a filler (`&-um`, `&-hmm`, …).
    /// `%mor` does not annotate paralinguistic fillers.
    FillerOnly,
    /// Every word in the utterance is a phonological fragment (`&+le`).
    FragmentOnly,
    /// Every word in the utterance is a nonword (`&~ach`, `&~uh`).
    NonwordOnly,
    /// Every word in the utterance is untranscribed (`xxx`, `yyy`, `www`).
    UntranscribedOnly,
    /// The utterance has words, but all of them are inside retrace
    /// groups (`<...> [/]`, `<...> [//]`), which Mor excludes.
    AllRetraced,
    /// The utterance contains a mix of non-linguistic categories
    /// (e.g. fillers + fragments + untranscribed) where no single
    /// narrower reason above fully describes it.
    MixedNonLinguistic,
}

impl NotApplicableReason {
    /// Short label for `%xalign` tier output: `not_applicable:<label>`.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Empty => "empty",
            Self::FillerOnly => "filler_only",
            Self::FragmentOnly => "fragment_only",
            Self::NonwordOnly => "nonword_only",
            Self::UntranscribedOnly => "untranscribed_only",
            Self::AllRetraced => "all_retraced",
            Self::MixedNonLinguistic => "mixed_nonlinguistic",
        }
    }
}

/// Inspect an utterance whose Mor-domain extraction yielded zero words,
/// and classify the [`NotApplicableReason`] that best describes why.
///
/// The classifier walks the content once, recording which word
/// categories it sees. If every content-bearing element is of one
/// category, that specific variant is returned; otherwise
/// [`NotApplicableReason::MixedNonLinguistic`] is used.
///
/// Callers must only invoke this for utterances where
/// `extract::collect_utterance_content(…, TierDomain::Mor, …)` returned
/// an empty vector; otherwise the classification is meaningless.
pub fn classify_not_applicable(utterance: &Utterance) -> NotApplicableReason {
    // Two walks: domain-free sees all content (including retrace);
    // Mor-domain sees everything except retrace. If Mor sees nothing
    // but domain-free saw words, retrace is the reason. Otherwise
    // classify by category from the Mor walk. `walk_words` doesn't
    // expose retrace context to the closure, so a single-walk fusion
    // would require a custom walker; the per-call cost is tiny (invoked
    // only on empty-payload utterances, ~3K across a 54-file corpus).
    let mut total = ContentCategories::default();
    walk_words(&utterance.main.content.content, None, &mut |item| {
        accumulate(&mut total, item)
    });

    let mut mor_only = ContentCategories::default();
    walk_words(
        &utterance.main.content.content,
        Some(TierDomain::Mor),
        &mut |item| accumulate(&mut mor_only, item),
    );

    if total.total_words == 0 {
        return NotApplicableReason::Empty;
    }
    if mor_only.total_words == 0 {
        // domain-free saw content but Mor saw none → all retrace
        return NotApplicableReason::AllRetraced;
    }
    mor_only.reason_when_nothing_alignable()
}

#[derive(Default, Debug)]
struct ContentCategories {
    /// Total word-like leaves seen (excluding separators).
    total_words: usize,
    filler_count: usize,
    fragment_count: usize,
    nonword_count: usize,
    untranscribed_count: usize,
    /// Linguistic words that would have been Mor-alignable — their
    /// presence means extraction would have returned non-empty, so
    /// the caller's precondition (zero alignable words) was violated.
    linguistic_count: usize,
}

impl ContentCategories {
    /// Pick the narrowest reason that explains "nothing alignable".
    fn reason_when_nothing_alignable(&self) -> NotApplicableReason {
        // If there are any linguistic words, the caller's precondition
        // is violated: extraction would have returned non-empty. Fall
        // through to MixedNonLinguistic conservatively.
        if self.linguistic_count > 0 {
            return NotApplicableReason::MixedNonLinguistic;
        }

        // Mutually-exclusive single-category cases.
        let nonzero_cats = [
            (self.filler_count > 0, NotApplicableReason::FillerOnly),
            (self.fragment_count > 0, NotApplicableReason::FragmentOnly),
            (self.nonword_count > 0, NotApplicableReason::NonwordOnly),
            (
                self.untranscribed_count > 0,
                NotApplicableReason::UntranscribedOnly,
            ),
        ];
        let active: Vec<NotApplicableReason> = nonzero_cats
            .iter()
            .filter_map(|(b, r)| b.then_some(*r))
            .collect();

        match active.as_slice() {
            [] => NotApplicableReason::Empty,
            [single] => *single,
            _ => NotApplicableReason::MixedNonLinguistic,
        }
    }
}

fn accumulate(cats: &mut ContentCategories, item: WordItem<'_>) {
    let word: &talkbank_model::model::Word = match item {
        WordItem::Word(w) => w,
        WordItem::ReplacedWord(rw) => {
            // For Mor, the replacement words are what would be aligned;
            // but for NotApplicable classification we care about what
            // the transcriber actually wrote, so use the original word.
            &rw.word
        }
        WordItem::Separator(_) => {
            // Tag-marker separators (`,`, `„`, `‡`) contribute to the
            // alignable count in the Mor domain. If they're present
            // alongside non-linguistic content, the utterance would
            // extract non-empty, so classify would not run. Ignore here.
            return;
        }
    };

    if word.cleaned_text().is_empty() {
        return;
    }
    cats.total_words += 1;

    if word.untranscribed().is_some() {
        cats.untranscribed_count += 1;
        return;
    }

    match &word.category {
        Some(WordCategory::Filler) => cats.filler_count += 1,
        Some(WordCategory::PhonologicalFragment) => cats.fragment_count += 1,
        Some(WordCategory::Nonword) => cats.nonword_count += 1,
        _ => cats.linguistic_count += 1,
    }
}

impl MorOutcome {
    /// Convert this outcome into a [`DecisionRecord`] for `%xalign`
    /// tier emission.
    ///
    /// [`MorOutcomeKind::Aligned`] outcomes return `None` because the
    /// happy path is not review-worthy and would produce a tier entry
    /// per successfully-morphotagged utterance — noise, not signal.
    /// Callers that want to surface aligned counts should aggregate
    /// separately.
    pub fn to_decision_record(&self) -> Option<DecisionRecord> {
        match &self.kind {
            MorOutcomeKind::Aligned { .. } => None,
            MorOutcomeKind::NotApplicable { reason } => Some(DecisionRecord {
                line_idx: self.line_idx,
                speaker: self.speaker.as_str().to_string(),
                strategy: DecisionStrategy::Morphosyntax(MorphosyntaxStrategy::NotApplicable),
                reason: format!("reason={}", reason.as_str()),
                // NotApplicable is correct behavior, not a failure,
                // so it does not require review.
                needs_review: false,
            }),
            MorOutcomeKind::MisalignmentBug(diag) => Some(DecisionRecord {
                line_idx: self.line_idx,
                speaker: self.speaker.as_str().to_string(),
                strategy: DecisionStrategy::Morphosyntax(MorphosyntaxStrategy::MisalignmentBug),
                reason: format!(
                    "class={} expected={} actual={} chat_words={:?} stanza_tokens={:?}",
                    diag.suspected_class.as_str(),
                    diag.expected,
                    diag.actual,
                    diag.chat_words,
                    diag.stanza_tokens_after_mapping,
                ),
                // Misalignment bugs always want human attention —
                // they indicate something the pipeline got wrong.
                needs_review: true,
            }),
        }
    }
}
