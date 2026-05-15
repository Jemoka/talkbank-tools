//! Predicates that decide whether a scanned CHAT file is reported by
//! `chatter debug find`.
//!
//! Filters are ANDed: a file is reported only if every configured
//! predicate admits it. Conceptually:
//!
//! - `LanguageSetMatch::Exact(set)` — `@Languages` contains exactly
//!   these codes (order-insensitive, duplicates treated as sets).
//! - `LanguageSetMatch::ContainsAll(set)` — every code in `set` appears
//!   in `@Languages`.
//! - `LanguageSetMatch::MinCount(n)` — `@Languages` has ≥`n` codes.
//!
//! The public [`FindFilter`] struct bundles all three plus the token
//! count threshold. Tests exercise each predicate in isolation.

use std::collections::BTreeSet;

use talkbank_model::LanguageCodes;

use super::scanner::{ChatFileScan, TokenPatternCount};

/// Set of canonical lowercase language codes, used for order-insensitive
/// header comparison. We hold `String` rather than `LanguageCode` because
/// `LanguageCode` does not implement `Ord` (it wraps `Arc<str>` with
/// custom semantics) and the comparison only needs the surface form.
pub type LanguageCodeSet = BTreeSet<String>;

/// Language-set predicates applied to `@Languages` header payloads.
///
/// Multiple variants may be combined on the same [`FindFilter`]: all must
/// pass for a file to be reported.
#[derive(Clone, Debug, Default)]
pub struct LanguageSetMatch {
    /// Exact match against `@Languages` (order-insensitive).
    pub exact: Option<LanguageCodeSet>,
    /// Every code in this set must appear in `@Languages`.
    pub contains_all: LanguageCodeSet,
    /// `@Languages` must have at least this many codes.
    pub min_count: Option<u32>,
}

impl LanguageSetMatch {
    /// Returns true when `codes` satisfies every configured predicate.
    pub fn matches(&self, codes: &LanguageCodes) -> bool {
        let present: LanguageCodeSet = codes.iter().map(|c| c.as_str().to_string()).collect();

        if let Some(exact) = &self.exact
            && present != *exact
        {
            return false;
        }
        if !self.contains_all.is_empty() && !self.contains_all.is_subset(&present) {
            return false;
        }
        if let Some(min) = self.min_count
            && (present.len() as u32) < min
        {
            return false;
        }
        true
    }
}

/// Full filter composed of language and body-token predicates.
#[derive(Clone, Debug, Default)]
pub struct FindFilter {
    /// Language-header predicates.
    pub languages: LanguageSetMatch,
    /// Minimum body token count required. `0` means "no threshold"
    /// (any count, including zero, passes).
    pub min_token_count: u32,
}

impl FindFilter {
    /// Returns true when `scan` satisfies every predicate.
    pub fn accepts(&self, scan: &ChatFileScan) -> bool {
        self.languages.matches(&scan.languages) && self.tokens_pass(scan.token_count)
    }

    fn tokens_pass(&self, count: TokenPatternCount) -> bool {
        count.get() >= self.min_token_count
    }
}

#[cfg(test)]
mod filter_tests {
    use super::*;
    use std::path::PathBuf;

    fn lc(s: &str) -> talkbank_model::model::LanguageCode {
        talkbank_model::model::LanguageCode::new(s)
    }

    fn codes(items: &[&str]) -> LanguageCodes {
        LanguageCodes::new(items.iter().map(|s| lc(s)).collect())
    }

    fn scan_with(languages: LanguageCodes, token_count: u32) -> ChatFileScan {
        ChatFileScan {
            path: PathBuf::from("test.cha"),
            languages,
            utterance_count: Default::default(),
            token_count: TokenPatternCount(token_count),
            file_bytes: 0,
        }
    }

    #[test]
    fn exact_match_is_order_insensitive() {
        let matcher = LanguageSetMatch {
            exact: Some(["spa".into(), "eng".into()].into_iter().collect()),
            ..Default::default()
        };
        assert!(matcher.matches(&codes(&["spa", "eng"])));
        assert!(matcher.matches(&codes(&["eng", "spa"])));
        assert!(!matcher.matches(&codes(&["spa"])));
        assert!(!matcher.matches(&codes(&["spa", "eng", "fra"])));
    }

    #[test]
    fn contains_all_requires_subset_presence() {
        let matcher = LanguageSetMatch {
            contains_all: ["eng".into()].into_iter().collect(),
            ..Default::default()
        };
        assert!(matcher.matches(&codes(&["eng"])));
        assert!(matcher.matches(&codes(&["eng", "spa"])));
        assert!(matcher.matches(&codes(&["spa", "eng", "fra"])));
        assert!(!matcher.matches(&codes(&["spa"])));
    }

    #[test]
    fn min_count_rejects_monolingual() {
        let matcher = LanguageSetMatch {
            min_count: Some(2),
            ..Default::default()
        };
        assert!(!matcher.matches(&codes(&["eng"])));
        assert!(matcher.matches(&codes(&["eng", "spa"])));
        assert!(matcher.matches(&codes(&["zho", "eng", "yue"])));
        assert!(!matcher.matches(&codes(&[])));
    }

    #[test]
    fn combined_predicates_all_must_pass() {
        let filter = FindFilter {
            languages: LanguageSetMatch {
                min_count: Some(2),
                contains_all: ["eng".into()].into_iter().collect(),
                ..Default::default()
            },
            min_token_count: 5,
        };
        // Bilingual with eng and enough tokens — accepted.
        assert!(filter.accepts(&scan_with(codes(&["spa", "eng"]), 10)));
        // Bilingual without eng — rejected.
        assert!(!filter.accepts(&scan_with(codes(&["spa", "fra"]), 10)));
        // Monolingual — rejected.
        assert!(!filter.accepts(&scan_with(codes(&["eng"]), 10)));
        // Not enough tokens — rejected.
        assert!(!filter.accepts(&scan_with(codes(&["spa", "eng"]), 3)));
    }

    #[test]
    fn default_filter_accepts_everything() {
        let filter = FindFilter::default();
        assert!(filter.accepts(&scan_with(codes(&[]), 0)));
        assert!(filter.accepts(&scan_with(codes(&["eng"]), 0)));
    }
}
