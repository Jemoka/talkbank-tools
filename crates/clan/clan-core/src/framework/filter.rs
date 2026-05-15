//! Shared filtering criteria for CLAN analysis commands.
//!
//! This module provides the Rust equivalent of CUTT's speaker selection (`+t`/`-t`),
//! word search (`+s`/`-s`), gem filtering (`+g`/`-g`), and utterance range
//! (`+z`). The [`AnalysisRunner`](super::AnalysisRunner) applies filters before
//! passing utterances to commands, so each command only sees relevant data --
//! exactly like CUTT's `checktier()` + `getwholeutter()`.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for the
//! original filter flag semantics.
//!
//! # Filter evaluation order
//!
//! 1. Utterance range (cheapest check, `+z`)
//! 2. Speaker inclusion/exclusion (`+t`/`-t`)
//! 3. Gem segment boundaries (`+g`/`-g`)
//! 4. Word pattern matching (`+s`/`-s`)

use std::fmt;
use std::str::FromStr;

use talkbank_model::{Header, SpeakerCode, Utterance};
use thiserror::Error;

use super::word_filter::{countable_words_in_utterance, word_pattern_matches};

/// Shared filtering criteria applied before utterances reach a command.
///
/// Replaces CUTT's global filtering flags. The runner evaluates these
/// against each utterance and only passes matching utterances to the
/// command's `process_utterance`.
#[derive(Debug, Clone, Default)]
pub struct FilterConfig {
    /// Include/exclude speakers (CUTT: +t/-t @ID)
    pub speakers: SpeakerFilter,
    /// Include/exclude dependent tiers (CUTT: +t/-t %tier)
    pub tiers: TierFilter,
    /// Word/morpheme search patterns (CUTT: +s/-s)
    pub words: WordFilter,
    /// Gem segment filtering (CUTT: +g/-g)
    pub gems: GemFilter,
    /// Restrict to a 1-based utterance range within each file (CUTT: +z)
    /// inclusive — e.g., `25-125` processes utterances 25–125
    pub utterance_range: Option<UtteranceRange>,
    /// Filter by `@ID` header pattern (CUTT: `+t@ID="lang|*|CHI|*"`).
    ///
    /// When `Some`, the analysis runner uses it twice:
    ///  - **file prefilter:** skip any file whose `@ID` headers all fail
    ///    the match;
    ///  - **utterance filter:** drop utterances whose speaker's `@ID` row
    ///    fails the match.
    ///
    /// `FilterConfig::matches` does not consult this field directly; the
    /// runner is responsible for both passes because it owns the parsed
    /// `@ID` headers.
    pub id_filter: Option<super::IdFilter>,
}

/// Inclusive 1-based utterance range within a file.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UtteranceRange {
    start: usize,
    end: usize,
}

/// Error returned when parsing an utterance range.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ParseUtteranceRangeError {
    /// The input was not in `start-end` form.
    #[error("invalid range format '{input}' — expected 'start-end' (e.g., '25-125')")]
    InvalidFormat {
        /// Original input string.
        input: String,
    },
    /// The bounds were syntactically valid but semantically invalid.
    #[error("invalid range '{input}' — start must be >= 1 and end >= start")]
    InvalidBounds {
        /// Original input string.
        input: String,
    },
}

impl UtteranceRange {
    /// Create a validated utterance range.
    pub fn new(start: usize, end: usize) -> Result<Self, ParseUtteranceRangeError> {
        if start == 0 || end < start {
            return Err(ParseUtteranceRangeError::InvalidBounds {
                input: format!("{start}-{end}"),
            });
        }

        Ok(Self { start, end })
    }

    /// Inclusive lower bound.
    pub const fn start(self) -> usize {
        self.start
    }

    /// Inclusive upper bound.
    pub const fn end(self) -> usize {
        self.end
    }

    /// Check whether a 1-based utterance index falls within the range.
    pub const fn contains(self, utterance_index: usize) -> bool {
        utterance_index >= self.start && utterance_index <= self.end
    }
}

impl fmt::Display for UtteranceRange {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}-{}", self.start, self.end)
    }
}

impl FromStr for UtteranceRange {
    type Err = ParseUtteranceRangeError;

    fn from_str(input: &str) -> Result<Self, Self::Err> {
        let parts: Vec<&str> = input.splitn(2, '-').collect();
        if parts.len() != 2 {
            return Err(ParseUtteranceRangeError::InvalidFormat {
                input: input.to_owned(),
            });
        }

        let start =
            parts[0]
                .parse::<usize>()
                .map_err(|_| ParseUtteranceRangeError::InvalidFormat {
                    input: input.to_owned(),
                })?;
        let end =
            parts[1]
                .parse::<usize>()
                .map_err(|_| ParseUtteranceRangeError::InvalidFormat {
                    input: input.to_owned(),
                })?;

        Self::new(start, end).map_err(|_| ParseUtteranceRangeError::InvalidBounds {
            input: input.to_owned(),
        })
    }
}

/// Parse a clap-friendly utterance range argument.
pub fn parse_utterance_range(input: &str) -> Result<UtteranceRange, String> {
    input
        .parse::<UtteranceRange>()
        .map_err(|error| error.to_string())
}

/// Speaker inclusion/exclusion filter.
///
/// When `include` is non-empty, only those speakers are processed.
/// When `exclude` is non-empty, those speakers are skipped.
/// When both are empty, all speakers pass (default behavior).
#[derive(Debug, Clone, Default)]
pub struct SpeakerFilter {
    /// Speakers to include (empty = include all)
    pub include: Vec<SpeakerCode>,
    /// Speakers to exclude
    pub exclude: Vec<SpeakerCode>,
}

/// Dependent tier inclusion/exclusion filter.
///
/// Controls which dependent tiers are visible to commands.
/// By default all tiers are visible.
#[derive(Debug, Clone, Default)]
pub struct TierFilter {
    /// Tier kinds to include (empty = include all)
    pub include: Vec<super::TierKind>,
    /// Tier kinds to exclude
    pub exclude: Vec<super::TierKind>,
}

/// Word/morpheme pattern filter (CUTT: +s/-s).
///
/// When `include` is non-empty, only utterances containing at least
/// one matching word are processed. When `exclude` is non-empty,
/// utterances containing any matching word are skipped.
#[derive(Debug, Clone, Default)]
pub struct WordFilter {
    /// Word patterns to include (empty = include all)
    pub include: Vec<super::WordPattern>,
    /// Word patterns to exclude
    pub exclude: Vec<super::WordPattern>,
}

/// Gem segment filter (CUTT: +g/-g).
///
/// When `include` is non-empty, only utterances within matching
/// @BG/@EG segments are processed. Gem labels use case-insensitive matching.
#[derive(Debug, Clone, Default)]
pub struct GemFilter {
    /// Gem labels to include (empty = include all)
    pub include: Vec<super::GemLabel>,
    /// Gem labels to exclude
    pub exclude: Vec<super::GemLabel>,
}

impl FilterConfig {
    /// Check whether an utterance passes all filter criteria.
    ///
    /// # Preconditions
    /// - `utterance` is a valid parsed utterance from a ChatFile
    /// - `active_gems` is the set of currently open @BG labels
    /// - `utterance_index` is the 1-based index of this utterance within its file
    ///
    /// # Returns
    /// `true` if the utterance should be passed to the command
    pub fn matches(
        &self,
        utterance: &Utterance,
        active_gems: &[String],
        utterance_index: usize,
    ) -> bool {
        // Check utterance range first (cheapest check)
        if let Some(range) = self.utterance_range
            && !range.contains(utterance_index)
        {
            return false;
        }

        self.speakers.matches(&utterance.main.speaker)
            && self.gems.matches(active_gems)
            && self.words.matches(utterance)
    }
}

impl SpeakerFilter {
    /// Check whether a speaker passes this filter.
    ///
    /// - If `include` is non-empty, speaker must be in the include list.
    /// - If `exclude` is non-empty, speaker must NOT be in the exclude list.
    /// - If both are empty, all speakers pass.
    pub fn matches(&self, speaker: &SpeakerCode) -> bool {
        if !self.include.is_empty() && !self.include.contains(speaker) {
            return false;
        }
        if self.exclude.contains(speaker) {
            return false;
        }
        true
    }
}

impl WordFilter {
    /// Check whether an utterance's words pass this filter.
    ///
    /// - If `include` is non-empty, the utterance must contain at least one
    ///   countable word matching any include pattern (case-insensitive exact match,
    ///   `*` wildcards supported).
    /// - If `exclude` is non-empty, the utterance must NOT contain any countable
    ///   word matching any exclude pattern.
    /// - If both are empty, all utterances pass.
    pub fn matches(&self, utterance: &Utterance) -> bool {
        if self.include.is_empty() && self.exclude.is_empty() {
            return true;
        }

        // Pre-lowercase include/exclude patterns for matching
        let include_lower: Vec<String> = self.include.iter().map(|p| p.to_lowercase()).collect();
        let exclude_lower: Vec<String> = self.exclude.iter().map(|p| p.to_lowercase()).collect();

        // Collect lowercased word texts once for both checks
        let word_texts: Vec<String> = countable_words_in_utterance(utterance)
            .map(|w| w.cleaned_text().to_lowercase())
            .collect();

        // If include patterns specified, at least one word must match
        if !include_lower.is_empty() {
            let has_match = word_texts.iter().any(|text| {
                include_lower
                    .iter()
                    .any(|pattern| word_pattern_matches(text, pattern))
            });
            if !has_match {
                return false;
            }
        }

        // If exclude patterns specified, no word may match
        if !exclude_lower.is_empty() {
            let has_excluded = word_texts.iter().any(|text| {
                exclude_lower
                    .iter()
                    .any(|pattern| word_pattern_matches(text, pattern))
            });
            if has_excluded {
                return false;
            }
        }

        true
    }
}

impl GemFilter {
    /// Check whether the current gem context passes this filter.
    ///
    /// - If `include` is non-empty, at least one active gem must match.
    /// - If `exclude` is non-empty, no active gem must match.
    /// - If both are empty, all contexts pass.
    pub fn matches(&self, active_gems: &[String]) -> bool {
        if !self.include.is_empty() {
            let has_match = active_gems.iter().any(|gem| {
                self.include
                    .iter()
                    .any(|pattern| gem.eq_ignore_ascii_case(pattern))
            });
            if !has_match {
                return false;
            }
        }
        if !self.exclude.is_empty() {
            let has_excluded = active_gems.iter().any(|gem| {
                self.exclude
                    .iter()
                    .any(|pattern| gem.eq_ignore_ascii_case(pattern))
            });
            if has_excluded {
                return false;
            }
        }
        true
    }
}

/// Track @BG/@EG gem boundaries across utterances.
///
/// Call `update` for each utterance's preceding headers to maintain
/// the set of currently active gem labels.
pub fn update_active_gems(headers: &[Header], active_gems: &mut Vec<String>) {
    for header in headers {
        match header {
            Header::BeginGem { label: Some(label) } => {
                active_gems.push(label.as_str().to_owned());
            }
            Header::EndGem { label: Some(label) } => {
                // Remove the most recent matching @BG
                if let Some(pos) = active_gems
                    .iter()
                    .rposition(|g| g.eq_ignore_ascii_case(label.as_str()))
                {
                    active_gems.remove(pos);
                }
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::domain_types::{GemLabel, WordPattern};
    use super::*;

    /// Empty speaker filters should allow any speaker code.
    #[test]
    fn speaker_filter_empty_matches_all() {
        let filter = SpeakerFilter::default();
        let speaker: SpeakerCode = "CHI".into();
        assert!(filter.matches(&speaker));
    }

    /// An include list should restrict matches to listed speakers only.
    #[test]
    fn speaker_filter_include_restricts() {
        let filter = SpeakerFilter {
            include: vec!["CHI".into(), "MOT".into()],
            exclude: vec![],
        };
        assert!(filter.matches(&"CHI".into()));
        assert!(filter.matches(&"MOT".into()));
        assert!(!filter.matches(&"FAT".into()));
    }

    /// Excluded speakers should fail even when include is empty.
    #[test]
    fn speaker_filter_exclude_removes() {
        let filter = SpeakerFilter {
            include: vec![],
            exclude: vec!["INV".into()],
        };
        assert!(filter.matches(&"CHI".into()));
        assert!(!filter.matches(&"INV".into()));
    }

    /// Empty gem filters should match whether or not a gem is active.
    #[test]
    fn gem_filter_empty_matches_all() {
        let filter = GemFilter::default();
        assert!(filter.matches(&[]));
        assert!(filter.matches(&["Story".to_owned()]));
    }

    /// Gem include filters require at least one active matching label.
    #[test]
    fn gem_filter_include_requires_match() {
        let filter = GemFilter {
            include: vec![GemLabel::from("Story")],
            exclude: vec![],
        };
        assert!(!filter.matches(&[]));
        assert!(filter.matches(&["Story".to_owned()]));
        assert!(filter.matches(&["story".to_owned()])); // case-insensitive
        assert!(!filter.matches(&["Narrative".to_owned()]));
    }

    /// Gem exclude filters should reject matching labels case-insensitively.
    #[test]
    fn gem_filter_exclude_blocks_match() {
        let filter = GemFilter {
            include: vec![],
            exclude: vec![GemLabel::from("Warmup")],
        };
        assert!(filter.matches(&[]));
        assert!(filter.matches(&["Story".to_owned()]));
        assert!(!filter.matches(&["Warmup".to_owned()]));
        assert!(!filter.matches(&["warmup".to_owned()]));
    }

    /// Empty word filters should not gate utterances.
    #[test]
    fn word_filter_empty_matches_all() {
        let filter = WordFilter::default();
        let utterance = make_test_utterance(&["hello", "world"]);
        assert!(filter.matches(&utterance));
    }

    /// Include patterns should require at least one lexical match.
    #[test]
    fn word_filter_include_requires_match() {
        let filter = WordFilter {
            include: vec![WordPattern::from("hello")],
            exclude: vec![],
        };
        let matching = make_test_utterance(&["hello", "world"]);
        assert!(filter.matches(&matching));

        let non_matching = make_test_utterance(&["goodbye", "world"]);
        assert!(!filter.matches(&non_matching));
    }

    /// Word include matching is case-insensitive.
    #[test]
    fn word_filter_include_case_insensitive() {
        let filter = WordFilter {
            include: vec![WordPattern::from("Hello")],
            exclude: vec![],
        };
        let utterance = make_test_utterance(&["hello", "world"]);
        assert!(filter.matches(&utterance));
    }

    /// Word includes use exact match semantics (CLAN parity).
    #[test]
    fn word_filter_include_exact() {
        let filter = WordFilter {
            include: vec![WordPattern::from("hello")],
            exclude: vec![],
        };
        let utterance = make_test_utterance(&["hello", "world"]);
        assert!(filter.matches(&utterance));

        // Substring should NOT match
        let filter_sub = WordFilter {
            include: vec![WordPattern::from("ell")],
            exclude: vec![],
        };
        assert!(!filter_sub.matches(&utterance));
    }

    /// Wildcard `*` enables partial matching in word filters.
    #[test]
    fn word_filter_include_wildcard() {
        let filter = WordFilter {
            include: vec![WordPattern::from("hel*")],
            exclude: vec![],
        };
        let utterance = make_test_utterance(&["hello", "world"]);
        assert!(filter.matches(&utterance));
    }

    /// Exclude patterns block utterances containing any matching word.
    #[test]
    fn word_filter_exclude_blocks() {
        let filter = WordFilter {
            include: vec![],
            exclude: vec![WordPattern::from("world")],
        };
        let blocked = make_test_utterance(&["hello", "world"]);
        assert!(!filter.matches(&blocked));

        let allowed = make_test_utterance(&["hello", "there"]);
        assert!(filter.matches(&allowed));
    }

    /// Exclude matches win when both include and exclude patterns match.
    #[test]
    fn word_filter_include_and_exclude() {
        let filter = WordFilter {
            include: vec![WordPattern::from("hello")],
            exclude: vec![WordPattern::from("world")],
        };
        // Has include match but also has exclude match → blocked
        let blocked = make_test_utterance(&["hello", "world"]);
        assert!(!filter.matches(&blocked));

        // Has include match, no exclude match → pass
        let allowed = make_test_utterance(&["hello", "there"]);
        assert!(filter.matches(&allowed));
    }

    /// The utterance range filter uses inclusive 1-based bounds.
    #[test]
    fn utterance_range_filters() {
        let config = FilterConfig {
            utterance_range: Some(UtteranceRange::new(2, 4).expect("valid test range")),
            ..FilterConfig::default()
        };
        let utterance = make_test_utterance(&["hello"]);
        let gems: Vec<String> = vec![];

        assert!(!config.matches(&utterance, &gems, 1));
        assert!(config.matches(&utterance, &gems, 2));
        assert!(config.matches(&utterance, &gems, 3));
        assert!(config.matches(&utterance, &gems, 4));
        assert!(!config.matches(&utterance, &gems, 5));
    }

    /// Utterance ranges should parse from CLAN-style `start-end` strings.
    #[test]
    fn utterance_range_parses() {
        let range = "25-125"
            .parse::<UtteranceRange>()
            .expect("range should parse");
        assert_eq!(range.start(), 25);
        assert_eq!(range.end(), 125);
        assert_eq!(range.to_string(), "25-125");
    }

    /// Invalid utterance ranges should report whether syntax or bounds failed.
    #[test]
    fn utterance_range_rejects_invalid_input() {
        assert!(matches!(
            "oops".parse::<UtteranceRange>(),
            Err(ParseUtteranceRangeError::InvalidFormat { .. })
        ));
        assert!(matches!(
            "0-5".parse::<UtteranceRange>(),
            Err(ParseUtteranceRangeError::InvalidBounds { .. })
        ));
        assert!(matches!(
            "9-3".parse::<UtteranceRange>(),
            Err(ParseUtteranceRangeError::InvalidBounds { .. })
        ));
    }

    /// Build a minimal Utterance with the given words for filter testing.
    fn make_test_utterance(words: &[&str]) -> talkbank_model::Utterance {
        use talkbank_model::Span;
        use talkbank_model::{MainTier, Terminator, UtteranceContent, Word};

        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        talkbank_model::Utterance::new(main)
    }
}
