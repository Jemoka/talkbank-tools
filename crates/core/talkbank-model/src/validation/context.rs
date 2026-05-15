//! Shared validation context and language-policy helpers.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Options_Header>

use crate::model::{LanguageCode, SpeakerCode};
use crate::validation::ValidationConfig;
use crate::{ErrorCode, Span};
use std::collections::HashSet;
use std::sync::Arc;

/// Languages that allow numeric digits (`0-9`) inside word tokens.
///
/// This list mirrors the legacy Java parser's `validNumberLanguages` policy and
/// is used by `E220` digit checks.
const LANGUAGES_ALLOWING_NUMBERS: &[&str] = &[
    "zho", // Chinese
    "cym", // Welsh
    "vie", // Vietnamese
    "tha", // Thai
    "nan", // Min Nan
    "yue", // Cantonese
    "min", // Min Chinese
    "hak", // Hakka
];

/// Returns whether a language code permits numeric digits in words.
///
/// This is a policy helper used by word-level digit checks after language
/// resolution and mirrors legacy CHAT parser behavior.
pub fn language_allows_numbers(lang: &str) -> bool {
    LANGUAGES_ALLOWING_NUMBERS.contains(&lang)
}

/// Returns whether the text contains at least one ASCII digit.
///
/// Digit policy intentionally targets ASCII `0-9` because CHAT language code
/// rules and historical checks are defined over those characters.
pub fn contains_digits(text: &str) -> bool {
    text.chars().any(|c| c.is_ascii_digit())
}

/// File-level constant data shared across all validation contexts via `Arc`.
///
/// These fields are set once per file (from headers) and never change during
/// validation. Wrapping them in `Arc` makes `ValidationContext::clone()` cheap
/// — only the `Arc` pointer is copied instead of the `HashSet`, `Vec`, etc.
#[derive(Clone, Debug)]
pub struct SharedValidationData {
    /// Valid participant IDs from @Participants header
    pub participant_ids: HashSet<SpeakerCode>,

    /// Default language code (affects character validation)
    /// This is typically the primary language from @Languages header
    pub default_language: Option<LanguageCode>,

    /// All declared languages from @Languages header (in order)
    /// First is primary, second is secondary, rest are tertiary
    pub declared_languages: Vec<LanguageCode>,

    /// Whether this file is in Conversation Analysis mode (from @Options: CA)
    /// In CA mode, terminators are optional
    pub ca_mode: bool,

    /// Whether to perform strict quotation marker cross-utterance validation
    /// Disabled by default - legacy CHAT system never performed these checks
    /// and real-world corpora don't follow strict sequential patterns.
    /// Includes: E341 (quotation follows), E344 (quotation precedes),
    /// E346 (quoted linker), E352 (self-completion linker)
    pub enable_quotation_validation: bool,

    /// Whether this file is in bullets mode (from @Options: bullets)
    /// In bullets mode, timestamp monotonicity validation is disabled.
    /// This allows overlapping speech, out-of-sequence editing, and reference timestamps.
    pub bullets_mode: bool,

    /// Validation configuration (severity overrides, disabled errors)
    /// Defaults to standard validation (no overrides)
    pub config: ValidationConfig,
}

impl Default for SharedValidationData {
    /// Builds empty file-level defaults before headers are applied.
    fn default() -> Self {
        Self {
            participant_ids: HashSet::new(),
            default_language: None,
            declared_languages: Vec::new(),
            ca_mode: false,
            enable_quotation_validation: false,
            bullets_mode: false,
            config: ValidationConfig::new(),
        }
    }
}

/// Validation context propagated through file, tier, and token validators.
///
/// This struct represents **file-level knowledge** (participants, languages, options)
/// that flows down the validation hierarchy from ChatFile → Utterance → MainTier → Word.
///
/// # Design Notes
///
/// File-level constants live in `shared` (`Arc<SharedValidationData>`), so cloning a
/// context only copies the `Arc` pointer plus 5 small overlay fields.
///
/// The overlay fields (`tier_language`, `field_span`, `field_text`, `field_label`,
/// `field_error_code`) vary per-tier or per-word and are set via builder methods
/// before passing the context down.
#[derive(Clone, Debug)]
pub struct ValidationContext {
    /// File-level constants (participants, languages, options, config).
    /// Shared across clones via `Arc` — cloning copies only the pointer.
    pub shared: Arc<SharedValidationData>,

    /// Optional tier language override for the active tier.
    /// Set by the main tier when a language marker is present (for example `*CHI@s:spa`).
    pub tier_language: Option<LanguageCode>,

    /// Optional span for field-level validation (set by parent when needed)
    pub field_span: Option<Span>,

    /// Optional field text for validation context
    pub field_text: Option<String>,

    /// Optional label for field-level errors
    pub field_label: Option<&'static str>,

    /// Optional error code override for field-level validation
    pub field_error_code: Option<ErrorCode>,
}

impl Default for ValidationContext {
    /// Builds a context with empty shared data and no field-level overlays.
    fn default() -> Self {
        Self {
            shared: Arc::new(SharedValidationData::default()),
            tier_language: None,
            field_span: None,
            field_text: None,
            field_label: None,
            field_error_code: None,
        }
    }
}

impl ValidationContext {
    /// Creates a new validation context with empty file-level defaults.
    ///
    /// Header parsing normally populates participants/languages immediately
    /// after construction via the builder helpers below.
    pub fn new() -> Self {
        Self::default()
    }

    /// Creates a context from pre-built shared data.
    ///
    /// Use this when upstream code already assembled file-level metadata and
    /// wants to avoid re-copying large participant/language collections.
    pub fn from_shared(shared: Arc<SharedValidationData>) -> Self {
        Self {
            shared,
            tier_language: None,
            field_span: None,
            field_text: None,
            field_label: None,
            field_error_code: None,
        }
    }

    // ── Shared-field builder methods (construct the Arc) ──

    /// Sets the file-level validation policy (`severity`/`disabled` overrides).
    ///
    /// This uses `Arc::make_mut`, so clones that still share the old `Arc`
    /// keep their previous config.
    pub fn with_config(mut self, config: ValidationConfig) -> Self {
        Arc::make_mut(&mut self.shared).config = config;
        self
    }

    /// Sets the allowed speaker IDs from `@Participants`.
    ///
    /// Any `*SPK:` line not in this set is eligible for speaker-id diagnostics.
    pub fn with_participant_ids(mut self, ids: HashSet<SpeakerCode>) -> Self {
        Arc::make_mut(&mut self.shared).participant_ids = ids;
        self
    }

    /// Sets the default language from the `@Languages` header.
    ///
    /// This baseline is used when tiers/words do not provide narrower language markers.
    pub fn with_default_language(mut self, lang: impl Into<LanguageCode>) -> Self {
        Arc::make_mut(&mut self.shared).default_language = Some(lang.into());
        self
    }

    /// Sets all declared languages from `@Languages`, in source order.
    ///
    /// Ordering matters because shortcut markers (for example `@s`) depend on
    /// primary-vs-secondary position.
    pub fn with_declared_languages(mut self, langs: Vec<LanguageCode>) -> Self {
        Arc::make_mut(&mut self.shared).declared_languages = langs;
        self
    }

    /// Sets whether Conversation Analysis mode is active.
    ///
    /// CA mode relaxes selected punctuation/terminator expectations.
    pub fn with_ca_mode(mut self, ca_mode: bool) -> Self {
        Arc::make_mut(&mut self.shared).ca_mode = ca_mode;
        self
    }

    /// Enables or disables strict quotation cross-utterance checks.
    ///
    /// This is off by default because many real corpora do not follow strict
    /// sequential quotation-linker patterns.
    pub fn with_quotation_validation(mut self, enable: bool) -> Self {
        Arc::make_mut(&mut self.shared).enable_quotation_validation = enable;
        self
    }

    /// Sets whether bullets mode is active.
    ///
    /// Bullets mode disables strict timestamp monotonicity checks so edited or
    /// overlapping timelines can still roundtrip.
    pub fn with_bullets_mode(mut self, bullets_mode: bool) -> Self {
        Arc::make_mut(&mut self.shared).bullets_mode = bullets_mode;
        self
    }

    // ── Overlay-field builder methods (mutate inline, cheap) ──

    /// Sets a tier-scoped language override for downstream word checks.
    ///
    /// This does not mutate shared file-level metadata; it only changes this
    /// context instance's overlay field.
    pub fn with_tier_language(mut self, lang: Option<LanguageCode>) -> Self {
        self.tier_language = lang;
        self
    }

    /// Sets the active field span used when emitting diagnostics.
    ///
    /// Parents typically call this before validating one child token.
    pub fn with_field_span(mut self, span: Span) -> Self {
        self.field_span = Some(span);
        self
    }

    /// Sets the active field text used in diagnostic context blocks.
    ///
    /// This allows errors to quote the exact offending token even when nested.
    pub fn with_field_text(mut self, text: impl Into<String>) -> Self {
        self.field_text = Some(text.into());
        self
    }

    /// Sets the diagnostic label for field-level errors.
    ///
    /// Typical values are domain nouns such as `"word"` or `"speaker"`.
    pub fn with_field_label(mut self, label: &'static str) -> Self {
        self.field_label = Some(label);
        self
    }

    /// Sets an explicit error code override for field-level checks.
    ///
    /// Useful for shared validators that are reused under multiple rule IDs.
    pub fn with_field_error_code(mut self, code: ErrorCode) -> Self {
        self.field_error_code = Some(code);
        self
    }

    // ── Convenience accessors for shared fields ──

    /// Get the "other" language for @s shortcut resolution.
    ///
    /// Given the current language, returns the alternate language:
    /// - If in primary language → returns secondary (may be None)
    /// - If in secondary language → returns primary
    /// - If in tertiary language → returns None (error - @s not allowed)
    ///
    /// This implements the Java Languages.getOtherLanguage() logic.
    pub fn get_other_language(&self, current_lang: &LanguageCode) -> Option<LanguageCode> {
        if self.shared.declared_languages.is_empty() {
            return None;
        }

        let primary = self.shared.declared_languages[0].as_str();
        let secondary = self
            .shared
            .declared_languages
            .get(1)
            .map(|code| code.as_str());

        if current_lang.as_str() == primary {
            // In primary → switch to secondary (may be None if only one language)
            secondary.map(LanguageCode::from)
        } else if let Some(sec) = secondary {
            if current_lang.as_str() == sec {
                // In secondary → switch to primary
                Some(LanguageCode::from(primary))
            } else {
                // In tertiary language or unlisted language → can't use @s
                None
            }
        } else {
            // In tertiary language or unlisted language → can't use @s
            None
        }
    }

    /// Returns whether a language is tertiary in the declared-language order.
    ///
    /// Tertiary languages cannot use the bare `@s` shortcut and must be named
    /// explicitly (`@s:code`) to avoid ambiguous fallback behavior.
    pub fn is_tertiary_language(&self, lang: &LanguageCode) -> bool {
        match self
            .shared
            .declared_languages
            .iter()
            .position(|l| l.as_str() == lang.as_str())
        {
            Some(pos) => pos >= 2,
            None => false,
        }
    }
}
