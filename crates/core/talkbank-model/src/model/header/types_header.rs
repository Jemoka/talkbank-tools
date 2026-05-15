//! @Types header components.
//!
//! The @Types header marks classes of groups, activities, and experimental
//! design for child language corpora. It has three mandatory fields.
//! There is no fixed vocabulary — any alphanumeric string is valid.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>

use super::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use talkbank_derive::{SemanticEq, SpanShift};

// ---------------------------------------------------------------------------
// DesignType
// ---------------------------------------------------------------------------

/// Design type for @Types header (string newtype — no fixed vocabulary).
///
/// Common values: `cross`, `long`, `observ`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct DesignType(#[span_shift(skip)] SmolStr);

impl DesignType {
    /// Construct from text.
    pub fn from_text(value: &str) -> Self {
        Self(SmolStr::from(value))
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for DesignType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for DesignType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for DesignType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "DesignType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl std::fmt::Display for DesignType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for DesignType {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for DesignType {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// ActivityType
// ---------------------------------------------------------------------------

/// Activity type for @Types header (string newtype — no fixed vocabulary).
///
/// Common values: `toyplay`, `narrative`, `meal`, `pictures`, `book`,
/// `interview`, `tests`, `preverbal`, `group`, `classroom`, `reading`, `everyday`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct ActivityType(#[span_shift(skip)] SmolStr);

impl ActivityType {
    /// Construct from text.
    pub fn from_text(value: &str) -> Self {
        Self(SmolStr::from(value))
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for ActivityType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for ActivityType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for ActivityType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "ActivityType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl std::fmt::Display for ActivityType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for ActivityType {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for ActivityType {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// GroupType
// ---------------------------------------------------------------------------

/// Group type for @Types header (string newtype — no fixed vocabulary).
///
/// Common values: `TD`, `biling`, `AAE`, `L2`, `SLI`, `HL`, `CI`, `PD`,
/// `ASD`, `LT`, `DS`, `MR`, `ADHD`, `CWS`, `AWS`.
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
#[derive(Debug, Clone, PartialEq, Eq, Hash, SemanticEq, SpanShift)]
pub struct GroupType(#[span_shift(skip)] SmolStr);

impl GroupType {
    /// Construct from text.
    pub fn from_text(value: &str) -> Self {
        Self(SmolStr::from(value))
    }

    /// Returns the value as a string.
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}

impl Serialize for GroupType {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for GroupType {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        Ok(Self::from_text(&s))
    }
}

impl JsonSchema for GroupType {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        "GroupType".into()
    }

    fn json_schema(_generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        schemars::json_schema!({ "type": "string" })
    }
}

impl std::fmt::Display for GroupType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl From<String> for GroupType {
    fn from(value: String) -> Self {
        Self::from_text(&value)
    }
}

impl From<&str> for GroupType {
    fn from(value: &str) -> Self {
        Self::from_text(value)
    }
}

// ---------------------------------------------------------------------------
// TypesHeader
// ---------------------------------------------------------------------------

/// Parsed representation of the mandatory `@Types` header triple.
///
/// The @Types header marks classes of groups, activities, and experimental
/// design. It has three mandatory fields in order: design, activity, and group.
///
/// **Format:** `@Types:\tdesign, activity, group`
/// **Example:** `@Types:\tlong, toyplay, TD`
///
/// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Types_Header>
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub struct TypesHeader {
    /// Design type (cross-sectional, longitudinal, observational, etc.)
    pub design: DesignType,

    /// Activity type (toyplay, narrative, meal, etc.)
    pub activity: ActivityType,

    /// Group type (TD, SLI, ASD, etc.)
    pub group: GroupType,
}

impl TypesHeader {
    /// Constructs a `@Types` header value in canonical field order.
    ///
    /// Callers should preserve this order (`design, activity, group`) to stay
    /// compatible with CHAT validators and downstream tooling expectations.
    pub fn new(
        design: impl Into<DesignType>,
        activity: impl Into<ActivityType>,
        group: impl Into<GroupType>,
    ) -> Self {
        Self {
            design: design.into(),
            activity: activity.into(),
            group: group.into(),
        }
    }
}

impl WriteChat for TypesHeader {
    /// Serializes as `@Types:\tdesign, activity, group`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        write!(
            w,
            "@Types:\t{}, {}, {}",
            self.design.as_str(),
            self.activity.as_str(),
            self.group.as_str()
        )
    }
}
