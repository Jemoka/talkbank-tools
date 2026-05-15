//! Validation policy for remapping or suppressing diagnostics.
//!
//! `ValidationConfig` is applied by [`ConfigurableErrorSink`](crate::ConfigurableErrorSink)
//! before diagnostics are forwarded to downstream consumers.
//!
//! ## Precedence
//!
//! 1. Explicit per-code override from `set_severity`/`upgrade`/`downgrade`/`disable`.
//! 2. Global strict-mode escalation (`strict`) for diagnostics still marked as warnings.
//! 3. Original parser/validator severity.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use crate::{ErrorCode, Severity};
use std::collections::HashMap;

/// Configuration for validation severity behavior.
///
/// Allows downgrading errors to warnings, disabling specific checks,
/// or upgrading warnings to errors.
///
/// # Example
///
/// ```
/// use talkbank_model::ValidationConfig;
/// use talkbank_model::{ErrorCode, Severity};
///
/// let config = ValidationConfig::new()
///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
///     .disable(ErrorCode::InvalidOverlapIndex)
///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
/// ```
#[derive(Clone, Debug, Default)]
pub struct ValidationConfig {
    /// Map from error code to overridden severity.
    ///
    /// `None` means the diagnostic is disabled.
    severity_overrides: HashMap<ErrorCode, Option<Severity>>,
    /// If true, warnings without explicit per-code overrides are escalated to errors.
    upgrade_unmapped_warnings: bool,
    /// Enable strict cross-utterance linker validation (E351-E355).
    ///
    /// When true, self-completion (`+,`) and other-completion (`++`) linkers
    /// are checked for correct pairing with preceding terminators (`+/.` and
    /// `+...` respectively). Disabled by default because many existing corpora
    /// do not follow these strict conventions.
    enable_quotation_validation: bool,
}

impl ValidationConfig {
    /// Create a new validation configuration with default behavior.
    pub fn new() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: false,
            enable_quotation_validation: false,
        }
    }

    /// Downgrade an error code to a lower severity
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);
    /// ```
    pub fn downgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Disable a specific error code entirely
    ///
    /// Errors with this code will not be reported.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::ErrorCode;
    ///
    /// let config = ValidationConfig::new()
    ///     .disable(ErrorCode::InvalidOverlapIndex);
    /// ```
    pub fn disable(mut self, code: ErrorCode) -> Self {
        self.severity_overrides.insert(code, None);
        self
    }

    /// Upgrade a warning to an error
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .upgrade(ErrorCode::UnknownAnnotation, Severity::Error);
    /// ```
    pub fn upgrade(mut self, code: ErrorCode, severity: Severity) -> Self {
        self.severity_overrides.insert(code, Some(severity));
        self
    }

    /// Set a custom severity for an error code.
    ///
    /// Pass `None` to disable the error.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    /// use talkbank_model::{ErrorCode, Severity};
    ///
    /// let config = ValidationConfig::new()
    ///     .set_severity(ErrorCode::IllegalUntranscribed, Some(Severity::Warning))
    ///     .set_severity(ErrorCode::InvalidOverlapIndex, None);  // Disable
    /// ```
    pub fn set_severity(mut self, code: ErrorCode, severity: Option<Severity>) -> Self {
        self.severity_overrides.insert(code, severity);
        self
    }

    /// Resolve the severity that should be emitted for a diagnostic.
    ///
    /// Returns `None` when the code is disabled.
    pub fn effective_severity(&self, code: ErrorCode, original: Severity) -> Option<Severity> {
        match self.severity_overrides.get(&code) {
            Some(override_severity) => *override_severity,
            None if self.upgrade_unmapped_warnings && original == Severity::Warning => {
                Some(Severity::Error)
            }
            None => Some(original),
        }
    }

    /// Check if an error code is disabled
    pub fn is_disabled(&self, code: ErrorCode) -> bool {
        matches!(self.severity_overrides.get(&code), Some(None))
    }

    /// Get all severity overrides
    pub fn overrides(&self) -> &HashMap<ErrorCode, Option<Severity>> {
        &self.severity_overrides
    }

    /// Create a strict configuration that escalates unmapped warnings to errors.
    ///
    /// Explicit per-code overrides still take precedence, so callers can opt out
    /// for specific codes by setting them back to `Severity::Warning`.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    ///
    /// let config = ValidationConfig::strict();
    /// // All warnings will be treated as errors
    /// ```
    pub fn strict() -> Self {
        Self {
            severity_overrides: HashMap::new(),
            upgrade_unmapped_warnings: true,
            enable_quotation_validation: false,
        }
    }

    /// Enable strict cross-utterance linker validation (E351-E355).
    ///
    /// When enabled, self-completion (`+,`) and other-completion (`++`)
    /// linkers are validated against their required preceding terminators.
    /// This is off by default because many real corpora do not follow
    /// strict sequential linker pairing conventions.
    pub fn with_strict_linkers(mut self) -> Self {
        self.enable_quotation_validation = true;
        self
    }

    /// Returns whether strict cross-utterance linker validation is enabled.
    pub fn strict_linkers_enabled(&self) -> bool {
        self.enable_quotation_validation
    }

    /// Create a lenient configuration for legacy corpora.
    ///
    /// Downgrades common strict errors to warnings for gradual migration.
    ///
    /// # Example
    ///
    /// ```
    /// use talkbank_model::ValidationConfig;
    ///
    /// let config = ValidationConfig::lenient();
    /// // E241 (illegal untranscribed) becomes a warning instead of error
    /// ```
    pub fn lenient() -> Self {
        Self::new()
            .downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning)
            .downgrade(ErrorCode::InvalidOverlapIndex, Severity::Warning)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests downgrade error.
    #[test]
    fn test_downgrade_error() {
        let config =
            ValidationConfig::new().downgrade(ErrorCode::IllegalUntranscribed, Severity::Warning);

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Warning)
        );
    }

    /// Tests disable error.
    #[test]
    fn test_disable_error() {
        let config = ValidationConfig::new().disable(ErrorCode::InvalidOverlapIndex);

        assert_eq!(
            config.effective_severity(ErrorCode::InvalidOverlapIndex, Severity::Error),
            None
        );
        assert!(config.is_disabled(ErrorCode::InvalidOverlapIndex));
    }

    /// Tests upgrade warning.
    #[test]
    fn test_upgrade_warning() {
        let config = ValidationConfig::new().upgrade(ErrorCode::UnknownAnnotation, Severity::Error);

        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Error)
        );
    }

    /// Tests no override uses original.
    #[test]
    fn test_no_override_uses_original() {
        let config = ValidationConfig::new();

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Error)
        );
    }

    /// Tests lenient config.
    #[test]
    fn test_lenient_config() {
        let config = ValidationConfig::lenient();

        assert_eq!(
            config.effective_severity(ErrorCode::IllegalUntranscribed, Severity::Error),
            Some(Severity::Warning)
        );
    }

    /// Strict mode escalates warnings that do not have explicit overrides.
    #[test]
    fn test_strict_config_upgrades_warnings() {
        let config = ValidationConfig::strict();
        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Error)
        );
    }

    /// Explicit per-code overrides take precedence over strict-mode escalation.
    #[test]
    fn test_strict_with_explicit_warning_override() {
        let config = ValidationConfig::strict()
            .set_severity(ErrorCode::UnknownAnnotation, Some(Severity::Warning));
        assert_eq!(
            config.effective_severity(ErrorCode::UnknownAnnotation, Severity::Warning),
            Some(Severity::Warning)
        );
    }
}
