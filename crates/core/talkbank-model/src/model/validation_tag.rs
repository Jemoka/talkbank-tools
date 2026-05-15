//! Lightweight validation-severity tags used by model enums.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// Validation significance of a model state variant.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ValidationTag {
    /// State is valid/neutral for validation.
    Clean,
    /// State indicates degraded but non-fatal validation quality.
    Warning,
    /// State indicates an error condition relevant to validation.
    Error,
}

/// Trait for model enums that can self-describe validation significance.
pub trait ValidationTagged {
    /// Returns the validation tag for this variant.
    fn validation_tag(&self) -> ValidationTag;

    /// Returns `true` when this variant maps to `ValidationTag::Error`.
    fn is_validation_error(&self) -> bool {
        matches!(self.validation_tag(), ValidationTag::Error)
    }

    /// Returns `true` when this variant maps to `ValidationTag::Warning`.
    fn is_validation_warning(&self) -> bool {
        matches!(self.validation_tag(), ValidationTag::Warning)
    }

    /// Returns `true` when this variant is warning/error (not clean).
    fn has_validation_issue(&self) -> bool {
        !matches!(self.validation_tag(), ValidationTag::Clean)
    }
}
