//! Domain newtypes for CLAN analysis parameters.
//!
//! These types replace raw `String`, `usize`, `u64`, and `f64` values at
//! public API boundaries so that function signatures are self-documenting
//! and type-incompatible values cannot be silently swapped.
//!
//! # Design
//!
//! - String selectors from a closed set → enum (`TierKind`)
//! - Open-ended string patterns → thin newtype (`KeywordPattern`, `WordPattern`, `GemLabel`)
//! - Numeric limits and thresholds → newtype with documented default
//!
//! All types implement `Display`, `From`/`Into` the underlying primitive,
//! `Clone`, `Debug`, `PartialEq`, and `Eq` (where meaningful).

use std::fmt;
use std::str::FromStr;

// ---------------------------------------------------------------------------
// TierKind — closed-set dependent tier selector
// ---------------------------------------------------------------------------

/// CHAT dependent tier kind used to select which tier a CLAN command operates on.
///
/// This is a **selector**, not a data carrier — it identifies a tier by label
/// (e.g., `"mor"`, `"cod"`) for filtering, configuration, and command dispatch.
/// The actual parsed tier data lives in `talkbank_model::DependentTier`.
///
/// Known tier labels are variants; unrecognized labels use `Other(String)` so
/// that user-defined tiers (e.g., `%xfoo`) can pass through without panicking.
///
/// # Aliases
///
/// `FromStr` normalizes common aliases: `"grt"` → `Gra`, `"trn"` → `Mor`.
///
/// # References
///
/// - [CHAT Manual: Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum TierKind {
    /// Morphological analysis tier (`%mor`).
    Mor,
    /// Grammatical relations tier (`%gra`, alias `%grt`).
    Gra,
    /// Phonological tier (`%pho`).
    Pho,
    /// Sign/gesture tier (`%sin`).
    Sin,
    /// Word-timing tier (`%wor`).
    Wor,
    /// Coding tier (`%cod`).
    Cod,
    /// Model phonology tier (`%mod`).
    Mod,
    /// Action tier (`%act`).
    Act,
    /// Addressee tier (`%add`).
    Add,
    /// Comment tier (`%com`).
    Com,
    /// Explanation tier (`%exp`).
    Exp,
    /// Situation tier (`%sit`).
    Sit,
    /// Speech act tier (`%spa`).
    Spa,
    /// Internal tier (`%int`).
    Int,
    /// Gesture-point tier (`%gpx`).
    Gpx,
    /// Alternative transcription tier (`%alt`).
    Alt,
    /// English translation tier (`%eng`).
    Eng,
    /// Error tier (`%err`).
    Err,
    /// Fluency tier (`%flo`).
    Flo,
    /// Orthography tier (`%ort`).
    Ort,
    /// Paralinguistic tier (`%par`).
    Par,
    /// Unrecognized or user-defined tier label.
    Other(String),
}

impl TierKind {
    /// The wire-format label for this tier kind (e.g., `"mor"`, `"gra"`).
    pub fn as_str(&self) -> &str {
        match self {
            Self::Mor => "mor",
            Self::Gra => "gra",
            Self::Pho => "pho",
            Self::Sin => "sin",
            Self::Wor => "wor",
            Self::Cod => "cod",
            Self::Mod => "mod",
            Self::Act => "act",
            Self::Add => "add",
            Self::Com => "com",
            Self::Exp => "exp",
            Self::Sit => "sit",
            Self::Spa => "spa",
            Self::Int => "int",
            Self::Gpx => "gpx",
            Self::Alt => "alt",
            Self::Eng => "eng",
            Self::Err => "err",
            Self::Flo => "flo",
            Self::Ort => "ort",
            Self::Par => "par",
            Self::Other(s) => s.as_str(),
        }
    }
}

impl fmt::Display for TierKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for TierKind {
    type Err = std::convert::Infallible;

    fn from_str(s: &str) -> Result<Self, <Self as FromStr>::Err> {
        Ok(Self::from(s))
    }
}

impl From<&str> for TierKind {
    fn from(s: &str) -> Self {
        match s {
            "mor" => Self::Mor,
            "gra" | "grt" => Self::Gra,
            "pho" => Self::Pho,
            "sin" => Self::Sin,
            "wor" => Self::Wor,
            "cod" => Self::Cod,
            "mod" => Self::Mod,
            "act" => Self::Act,
            "add" => Self::Add,
            "com" => Self::Com,
            "exp" => Self::Exp,
            "sit" => Self::Sit,
            "spa" => Self::Spa,
            "int" => Self::Int,
            "gpx" => Self::Gpx,
            "alt" => Self::Alt,
            "eng" => Self::Eng,
            "err" => Self::Err,
            "flo" => Self::Flo,
            "ort" => Self::Ort,
            "par" => Self::Par,
            "trn" => Self::Mor, // alias: trn maps to mor
            other => Self::Other(other.to_owned()),
        }
    }
}

impl From<String> for TierKind {
    fn from(s: String) -> Self {
        // Parse first, avoid allocation if it's a known tier
        Self::from(s.as_str())
    }
}

// Enable `== "mor"` comparisons
impl PartialEq<str> for TierKind {
    fn eq(&self, other: &str) -> bool {
        self.as_str() == other
    }
}

impl PartialEq<&str> for TierKind {
    fn eq(&self, other: &&str) -> bool {
        self.as_str() == *other
    }
}

// ---------------------------------------------------------------------------
// String pattern newtypes
// ---------------------------------------------------------------------------

/// Keyword search pattern for KWAL/KEYMAP commands.
///
/// Supports case-insensitive matching and optional wildcards (`cook*`).
/// Parsed from CLI `--keyword` arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct KeywordPattern(pub String);

impl KeywordPattern {
    /// Return the keyword pattern as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for KeywordPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for KeywordPattern {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for KeywordPattern {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::ops::Deref for KeywordPattern {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Word inclusion/exclusion pattern for utterance filtering (CUTT `+s`/`-s`).
///
/// Supports case-insensitive substring matching against main-tier words.
/// Parsed from CLI `--include-word` / `--exclude-word` arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WordPattern(pub String);

impl WordPattern {
    /// Return the word pattern as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for WordPattern {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for WordPattern {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for WordPattern {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::ops::Deref for WordPattern {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

/// Gem segment boundary label for `@BG`/`@EG` filtering (CUTT `+g`/`-g`).
///
/// Labels are matched case-insensitively against `@Bg` and `@Eg` headers.
/// Parsed from CLI `--gem` / `--exclude-gem` arguments.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct GemLabel(pub String);

impl GemLabel {
    /// Return the gem label as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for GemLabel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for GemLabel {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for GemLabel {
    fn from(s: &str) -> Self {
        Self(s.to_owned())
    }
}

impl std::ops::Deref for GemLabel {
    type Target = str;
    fn deref(&self) -> &str {
        &self.0
    }
}

// ---------------------------------------------------------------------------
// Numeric newtypes
// ---------------------------------------------------------------------------

/// Maximum number of utterances to analyze per speaker.
///
/// Used by DSS (default 50), IPSyn (default 100), KidEval, and Sugar commands.
/// A value of 0 means "no limit."
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UtteranceLimit(pub usize);

impl UtteranceLimit {
    /// Create a new utterance limit (0 means no limit).
    pub const fn new(n: usize) -> Self {
        Self(n)
    }
    /// Return the raw limit value.
    pub const fn get(self) -> usize {
        self.0
    }
}

impl fmt::Display for UtteranceLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for UtteranceLimit {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl FromStr for UtteranceLimit {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// Minimum word frequency threshold for inclusion in analysis results.
///
/// Used by CORELEX (default 3). Words below this frequency are excluded.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrequencyThreshold(pub u64);

impl FrequencyThreshold {
    /// Create a new frequency threshold.
    pub const fn new(n: u64) -> Self {
        Self(n)
    }
    /// Return the raw threshold value.
    pub const fn get(self) -> u64 {
        self.0
    }
}

impl fmt::Display for FrequencyThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<u64> for FrequencyThreshold {
    fn from(n: u64) -> Self {
        Self(n)
    }
}

impl FromStr for FrequencyThreshold {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// Maximum recursion depth for hierarchical `%cod` tier parsing.
///
/// Used by the CODES command (default 0 = all levels).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct CodeDepth(pub usize);

impl CodeDepth {
    /// Create a new code depth (0 means all levels).
    pub const fn new(n: usize) -> Self {
        Self(n)
    }
    /// Return the raw depth value.
    pub const fn get(self) -> usize {
        self.0
    }
    /// Whether all levels should be included (depth = 0).
    pub const fn is_unlimited(self) -> bool {
        self.0 == 0
    }
}

impl fmt::Display for CodeDepth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for CodeDepth {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl FromStr for CodeDepth {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// Maximum number of words to report in frequency-based output.
///
/// Used by MAXWD (default 20).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WordLimit(pub usize);

impl WordLimit {
    /// Create a new word limit.
    pub const fn new(n: usize) -> Self {
        Self(n)
    }
    /// Return the raw limit value.
    pub const fn get(self) -> usize {
        self.0
    }
}

impl fmt::Display for WordLimit {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<usize> for WordLimit {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl FromStr for WordLimit {
    type Err = std::num::ParseIntError;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(s.parse()?))
    }
}

/// Overlap ratio threshold (0.0–1.0) for CHIP interaction classification.
///
/// Two consecutive utterances with shared-word ratio ≥ this threshold
/// are classified as overlapping. Default: 0.5 (50%).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlapThreshold(pub f64);

impl OverlapThreshold {
    /// Create a new overlap threshold (0.0–1.0).
    pub const fn new(ratio: f64) -> Self {
        Self(ratio)
    }
    /// Return the raw ratio value.
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl Default for OverlapThreshold {
    fn default() -> Self {
        Self(0.5)
    }
}

impl fmt::Display for OverlapThreshold {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:.1}%", self.0 * 100.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_kind_from_str_known() {
        assert_eq!(TierKind::from("mor"), TierKind::Mor);
        assert_eq!(TierKind::from("gra"), TierKind::Gra);
        assert_eq!(TierKind::from("grt"), TierKind::Gra); // alias
        assert_eq!(TierKind::from("trn"), TierKind::Mor); // alias
        assert_eq!(TierKind::from("cod"), TierKind::Cod);
    }

    #[test]
    fn tier_kind_from_str_unknown() {
        assert_eq!(TierKind::from("xfoo"), TierKind::Other("xfoo".to_owned()));
    }

    #[test]
    fn tier_kind_display_roundtrip() {
        assert_eq!(TierKind::Mor.to_string(), "mor");
        assert_eq!(TierKind::Gra.to_string(), "gra");
    }

    #[test]
    fn tier_kind_partial_eq_str() {
        assert!(TierKind::Mor == "mor");
        assert!(TierKind::Gra == "gra");
        assert!(TierKind::Other("xfoo".to_owned()) == "xfoo");
    }

    #[test]
    fn utterance_limit_basics() {
        let limit = UtteranceLimit::new(50);
        assert_eq!(limit.get(), 50);
        assert_eq!(limit.to_string(), "50");
    }

    #[test]
    fn overlap_threshold_default() {
        let threshold = OverlapThreshold::default();
        assert_eq!(threshold.get(), 0.5);
    }
}
