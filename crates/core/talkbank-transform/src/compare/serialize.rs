use talkbank_model::model::{DependentTier, NonEmptyString, UserDefinedDependentTier};
use talkbank_model::{Span, WriteChat};

use super::model::{CompareStatus, UtteranceComparison};

/// Structured serialization errors for compare-owned output artifacts.
#[derive(Debug, thiserror::Error)]
pub enum CompareSerializationError {
    /// `%xsrep` cannot contain an empty token payload.
    #[error(
        "compare serialization produced empty content for xsrep at utterance {utterance_index} token {token_index}"
    )]
    EmptyXsrepToken {
        /// Zero-based utterance index in the source comparison.
        utterance_index: usize,
        /// Zero-based token index within the utterance comparison.
        token_index: usize,
    },
    /// `%xsmor` cannot contain an empty POS payload.
    #[error(
        "compare serialization produced empty content for xsmor at utterance {utterance_index} token {token_index}"
    )]
    EmptyXsmorToken {
        /// Zero-based utterance index in the source comparison.
        utterance_index: usize,
        /// Zero-based token index within the utterance comparison.
        token_index: usize,
    },
    /// Per-POS CSV rows must have a non-empty label.
    #[error("compare serialization produced empty content for compare metrics POS label")]
    EmptyMetricsPosLabel,
    /// A serialized compare tier payload must not collapse to empty text.
    #[error("compare serialization produced empty content for %{label}")]
    EmptyTierContent {
        /// Compare tier label without the leading `%`.
        label: CompareTierLabel,
    },
    /// CSV writer failed while rendering compare metrics.
    #[error("compare CSV serialization failed: {0}")]
    Csv(#[from] csv::Error),
    /// Structured CSV output should always be UTF-8, but convert explicitly.
    #[error("compare CSV output was not valid UTF-8: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),
}

/// Newtype for compare user-defined tier labels.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareTierLabel(NonEmptyString);

impl CompareTierLabel {
    /// `%xsrep`
    pub fn xsrep() -> Self {
        Self(NonEmptyString::new_unchecked("xsrep"))
    }

    /// `%xsmor`
    pub fn xsmor() -> Self {
        Self(NonEmptyString::new_unchecked("xsmor"))
    }

    pub(in crate::compare) fn as_str(&self) -> &str {
        self.0.as_ref()
    }

    fn into_inner(self) -> NonEmptyString {
        self.0
    }
}

impl std::fmt::Display for CompareTierLabel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Newtype for one `%xsrep` token payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareSurfaceToken(NonEmptyString);

impl CompareSurfaceToken {
    fn new(
        text: &str,
        utterance_index: usize,
        token_index: usize,
    ) -> Result<Self, CompareSerializationError> {
        NonEmptyString::new(text)
            .map(Self)
            .ok_or(CompareSerializationError::EmptyXsrepToken {
                utterance_index,
                token_index,
            })
    }
}

impl WriteChat for CompareSurfaceToken {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.0.as_ref())
    }
}

/// Newtype for one `%xsmor` POS payload or per-POS metric key fragment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparePosLabel(NonEmptyString);

impl ComparePosLabel {
    fn for_tier(
        pos: Option<&str>,
        utterance_index: usize,
        token_index: usize,
    ) -> Result<Self, CompareSerializationError> {
        let raw = pos.unwrap_or("?");
        NonEmptyString::new(raw)
            .map(Self)
            .ok_or(CompareSerializationError::EmptyXsmorToken {
                utterance_index,
                token_index,
            })
    }

    pub(in crate::compare) fn for_metrics(raw: &str) -> Result<Self, CompareSerializationError> {
        NonEmptyString::new(raw)
            .map(Self)
            .ok_or(CompareSerializationError::EmptyMetricsPosLabel)
    }

    pub(in crate::compare) fn as_str(&self) -> &str {
        self.0.as_ref()
    }
}

impl WriteChat for ComparePosLabel {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.0.as_ref())
    }
}

/// Structural prefix marker used in compare user-defined tiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompareTierMarker {
    /// Match: no prefix
    Match,
    /// Main-side extra: `+`
    ExtraMain,
    /// Gold-side extra: `-`
    ExtraGold,
}

impl From<CompareStatus> for CompareTierMarker {
    fn from(value: CompareStatus) -> Self {
        match value {
            CompareStatus::Match => Self::Match,
            CompareStatus::ExtraMain => Self::ExtraMain,
            CompareStatus::ExtraGold => Self::ExtraGold,
        }
    }
}

impl WriteChat for CompareTierMarker {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            Self::Match => Ok(()),
            Self::ExtraMain => w.write_char('+'),
            Self::ExtraGold => w.write_char('-'),
        }
    }
}

/// One structured compare-tier item.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareTierItem<T> {
    /// Whether the token is bare, prefixed with `+`, or prefixed with `-`.
    pub marker: CompareTierMarker,
    /// Typed payload for the token body.
    pub value: T,
}

impl<T: WriteChat> WriteChat for CompareTierItem<T> {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        self.marker.write_chat(w)?;
        self.value.write_chat(w)
    }
}

/// Structured payload for `%xsrep`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsrepTierContent {
    /// One entry per compared token.
    pub items: Vec<CompareTierItem<CompareSurfaceToken>>,
}

impl TryFrom<&UtteranceComparison> for XsrepTierContent {
    type Error = CompareSerializationError;

    fn try_from(comparison: &UtteranceComparison) -> Result<Self, Self::Error> {
        let items = comparison
            .tokens
            .iter()
            .enumerate()
            .map(|(token_index, token)| {
                Ok(CompareTierItem {
                    marker: token.status.into(),
                    value: CompareSurfaceToken::new(
                        &token.text,
                        comparison.utterance_index,
                        token_index,
                    )?,
                })
            })
            .collect::<Result<Vec<_>, CompareSerializationError>>()?;
        Ok(Self { items })
    }
}

impl WriteChat for XsrepTierContent {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (idx, item) in self.items.iter().enumerate() {
            if idx > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }
}

/// Structured payload for `%xsmor`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XsmorTierContent {
    /// One entry per compared token.
    pub items: Vec<CompareTierItem<ComparePosLabel>>,
}

impl TryFrom<&UtteranceComparison> for XsmorTierContent {
    type Error = CompareSerializationError;

    fn try_from(comparison: &UtteranceComparison) -> Result<Self, Self::Error> {
        let last_index = comparison.tokens.len().saturating_sub(1);
        let items = comparison
            .tokens
            .iter()
            .enumerate()
            .map(|(token_index, token)| {
                let pos = if token_index == last_index && token.pos.as_deref() == Some("PUNCT") {
                    Some(token.text.as_str())
                } else {
                    token.pos.as_deref()
                };
                Ok(CompareTierItem {
                    marker: token.status.into(),
                    value: ComparePosLabel::for_tier(pos, comparison.utterance_index, token_index)?,
                })
            })
            .collect::<Result<Vec<_>, CompareSerializationError>>()?;
        Ok(Self { items })
    }
}

impl WriteChat for XsmorTierContent {
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for (idx, item) in self.items.iter().enumerate() {
            if idx > 0 {
                w.write_char(' ')?;
            }
            item.write_chat(w)?;
        }
        Ok(())
    }
}

/// Structured compare tier ready to cross into an untyped `%x...` CHAT tier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompareUserDefinedTier<T> {
    /// Tier label without `%`.
    pub label: CompareTierLabel,
    /// Structured compare-tier payload.
    pub content: T,
}

impl<T: WriteChat> CompareUserDefinedTier<T> {
    pub(in crate::compare) fn into_dependent_tier(
        self,
    ) -> Result<DependentTier, CompareSerializationError> {
        let content_text = self.content.to_chat_string();
        let Some(content) = NonEmptyString::new(&content_text) else {
            return Err(CompareSerializationError::EmptyTierContent { label: self.label });
        };

        Ok(DependentTier::UserDefined(UserDefinedDependentTier {
            label: self.label.into_inner(),
            content,
            span: Span::DUMMY,
        }))
    }
}

/// Serialize comparison results as a `%xsrep` tier payload.
pub fn format_xsrep(comparison: &UtteranceComparison) -> Result<String, CompareSerializationError> {
    Ok(XsrepTierContent::try_from(comparison)?.to_chat_string())
}

/// Serialize comparison results as a `%xsmor` tier payload.
pub fn format_xsmor(comparison: &UtteranceComparison) -> Result<String, CompareSerializationError> {
    Ok(XsmorTierContent::try_from(comparison)?.to_chat_string())
}
