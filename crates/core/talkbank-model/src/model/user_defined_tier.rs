//! Model types for `%x...` user-defined dependent tiers.
//!
//! These types preserve custom tier labels and payloads verbatim so projects
//! can layer domain-specific annotations without modifying core CHAT schemas.
//!
//! CHAT reference anchor:
//! - [User-defined tiers](https://talkbank.org/0info/manuals/CHAT.html#User_Defined)

use crate::string_newtype;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

string_newtype!(
    /// User-defined tier label (the LABEL part of %xLABEL).
    ///
    /// Captures the custom label portion of user-defined tiers in CHAT format.
    /// User-defined tiers allow researchers to add project-specific coding that
    /// extends beyond the standard CHAT tier types.
    ///
    /// # Format
    ///
    /// The label is the text after `%x` in a user-defined tier:
    /// - `%xfoo` → label = "foo"
    /// - `%xpho` → label = "pho"
    /// - `%xmor` → label = "mor"
    /// - `%xgesture` → label = "gesture"
    ///
    /// # Standard vs. User-Defined
    ///
    /// Some tier types use `%x` prefix even though they're standard:
    /// - `%xpho` - Extended phonology (standard, but uses %x prefix)
    /// - `%xmod` - Model phonology (standard, but uses %x prefix)
    /// - `%xmor` - Would be a user-defined tier that mimics %mor
    ///
    /// See the CHAT manual's discussion of tier ordering for details on
    /// how %xpho and %xmod get special positioning in canonical tier order.
    ///
    /// # CHAT Format Examples
    ///
    /// **Example 1: Custom coding tier**
    /// ```text
    /// *CHI: I want cookie .
    /// %xgesture: points_to cookie jar
    /// ```
    /// Label: "gesture"
    ///
    /// **Example 2: Project-specific phonology**
    /// ```text
    /// *CHI: want cookie .
    /// %xpho: wɑnt kʊki
    /// ```
    /// Label: "pho" (extended phonology, a standard tier type)
    ///
    /// **Example 3: Custom research coding**
    /// ```text
    /// *CHI: no go away .
    /// %xemotion: frustrated
    /// %xaction: pushes_toy
    /// ```
    /// Labels: "emotion", "action"
    ///
    /// # References
    ///
    /// - [User-Defined Tiers](https://talkbank.org/0info/manuals/CHAT.html#User_Defined)
    /// - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
    pub struct UserDefinedTierLabel;
);

impl PartialEq<&str> for UserDefinedTierLabel {
    /// Allows direct comparisons with borrowed `&str` literals in tests and filters.
    fn eq(&self, other: &&str) -> bool {
        self.0 == *other
    }
}

impl PartialEq<str> for UserDefinedTierLabel {
    /// Allows direct comparisons with dynamically borrowed string slices.
    fn eq(&self, other: &str) -> bool {
        self.0 == other
    }
}

/// User-defined custom tier (%xLABEL).
///
/// Represents arbitrary custom tiers defined by researchers for project-specific coding.
/// User-defined tiers extend CHAT format to support domain-specific annotations
/// without requiring changes to the core specification.
///
/// # Structure
///
/// A user-defined tier has two parts:
/// - **label**: The custom identifier after `%x` (e.g., "gesture", "emotion")
/// - **content**: Raw text content of the tier (unparsed)
///
/// # Format
///
/// ```text
/// %xLABEL: content goes here
/// ```
///
/// The content is stored as-is without parsing. This allows maximum flexibility
/// for researchers to define their own coding schemes.
///
/// # CHAT Format Examples
///
/// **Example 1: Gesture coding**
/// ```text
/// *CHI: I want that .
/// %xgesture: points_to cookie jar
/// %xgaze: looking_at MOT
/// ```
///
/// Two user-defined tiers:
/// - `UserDefinedTier { label: "gesture", content: "points_to cookie jar" }`
/// - `UserDefinedTier { label: "gaze", content: "looking_at MOT" }`
///
/// **Example 2: Emotional state coding**
/// ```text
/// *CHI: no go away .
/// %xemotion: frustrated
/// %xaction: pushes_toy
/// %xintensity: high
/// ```
///
/// Three user-defined tiers coding emotional state, action, and intensity.
///
/// **Example 3: Extended phonology (%xpho)**
/// ```text
/// *CHI: want cookie .
/// %xpho: wɑnt kʊki
/// ```
///
/// The `%xpho` tier is a standard extended phonology tier, but uses the `%x` prefix.
/// It gets special positioning in canonical tier order (alongside other phonology tiers).
///
/// **Example 4: Model phonology (%xmod)**
/// ```text
/// *CHI: doggie running .
/// %mod: dɔgi rʌniŋ
/// ```
///
/// The `%mod` tier (also written as `%xmod`) represents model phonological form.
///
/// **Example 5: Custom morphology coding**
/// ```text
/// *CHI: quiero galletas .
/// %xmor: v|quer-1S n|galleta-PL .
/// ```
///
/// A user-defined tier that mimics the structure of standard %mor tier,
/// but uses custom coding conventions for a specific project.
///
/// # Use Cases
///
/// User-defined tiers are commonly used for:
/// - **Gesture/action coding**: Physical actions, gaze, pointing
/// - **Emotional state**: Affect, intensity, engagement
/// - **Context coding**: Situational factors, environmental conditions
/// - **Custom linguistic coding**: Project-specific grammatical annotations
/// - **Video/audio notes**: Timestamps, quality markers, technical notes
///
/// # Parsing Strategy
///
/// User-defined tier content is stored as raw text (unparsed). This is intentional:
/// - Each project defines its own coding conventions
/// - No universal structure to parse
/// - Flexible for evolving research needs
///
/// If structured parsing is needed for specific user-defined tiers, that can be
/// implemented in application code using the raw `content` field.
///
/// # References
///
/// - [User-Defined Tiers](https://talkbank.org/0info/manuals/CHAT.html#User_Defined)
/// - [Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
/// - [%pho tier (extended)](https://talkbank.org/0info/manuals/CHAT.html#pho)
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct UserDefinedTier {
    /// The custom label (e.g., "foo" from %xfoo, "pho" from %xpho)
    pub label: UserDefinedTierLabel,
    /// Raw text content of the tier
    pub content: smol_str::SmolStr,
}

impl UserDefinedTier {
    /// Create a user-defined tier with explicit label and raw content.
    ///
    /// Content is intentionally unparsed at this layer; projects that need
    /// structure should parse `content` in downstream application code.
    pub fn new(
        label: impl Into<UserDefinedTierLabel>,
        content: impl Into<smol_str::SmolStr>,
    ) -> Self {
        Self {
            label: label.into(),
            content: content.into(),
        }
    }
}
