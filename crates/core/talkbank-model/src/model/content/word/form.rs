//! Word-form `@` suffix markers (`gumma@c`, `younz@d`, ...).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Special_Form_Markers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>

use crate::model::WriteChat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use talkbank_derive::{SemanticEq, SpanShift};

/// Special-form suffix marker attached to a word token.
///
/// Most variants correspond to fixed CHAT marker codes. [`FormType::UserDefined`]
/// keeps project-specific `@z:...` values available without schema changes.
///
/// # CHAT Format Examples
///
/// ```text
/// gumma@c           Child-invented form (@c)
/// younz@d           Dialect form (@d)
/// abame@b           Babbling (@b)
/// bunko@f           Family-specific form (@f)
/// woofwoof@o        Onomatopoeia (@o)
/// um@fp             Filled pause (@fp) - deprecated, use &-um
/// b@l               Letter (@l)
/// abc@k             Letter sequence (@k)
/// if@q              Metalinguistic reference (@q)
/// breaked@n         Neologism (@n)
/// lalala@si         Singing (@si)
/// wug@t             Test word (@t)
/// custom@z:label    User-defined (@z:label)
/// ```
///
/// # Standard Markers
///
/// - `@a` - Approximate/phonologically consistent form
/// - `@b` - Babbling
/// - `@c` - Child-invented form
/// - `@d` - Dialect form
/// - `@f` - Family-specific form
/// - `@fp` - Filled pause (deprecated)
/// - `@g` - Gemination/general special form
/// - `@i` - Interjection
/// - `@k` - Letter sequence (kinship)
/// - `@l` - Single letter
/// - `@ls` - Letter plural
/// - `@n` - Neologism
/// - `@o` - Onomatopoeia
/// - `@p` - Proper name
/// - `@q` - Metalinguistic reference
/// - `@sas` - Second attempt success
/// - `@si` - Singing
/// - `@sl` - Slang
/// - `@t` - Test word
/// - `@u` - Unibet transcription
/// - `@wp` - Word play
/// - `@x` - Complex/excluded
/// - `@z:xxx` - User-defined custom code
///
/// # References
///
/// - [Special Form Markers](https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker)
/// - [Babbling Marker](https://talkbank.org/0info/manuals/CHAT.html#Babbling_Marker)
/// - [Child-Invented Marker](https://talkbank.org/0info/manuals/CHAT.html#ChildInvented_Marker)
/// - [Dialect Form Marker](https://talkbank.org/0info/manuals/CHAT.html#DialectForm_Marker)
/// - [Family-Specific Form Marker](https://talkbank.org/0info/manuals/CHAT.html#FamilySpecificForm_Marker)
/// - [Neologism Marker](https://talkbank.org/0info/manuals/CHAT.html#Neologism_Marker)
/// - [Metalinguistic Reference Marker](https://talkbank.org/0info/manuals/CHAT.html#MetalinguisticReference_Marker)
#[derive(
    Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema, SemanticEq, SpanShift,
)]
pub enum FormType {
    /// `@a` - Approximate/phonologically consistent form
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "a")]
    A,
    /// `@b` - Babbling
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Babbling_Marker>
    #[serde(rename = "b")]
    B,
    /// `@c` - Child-invented form
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#ChildInvented_Marker>
    #[serde(rename = "c")]
    C,
    /// `@d` - Dialect form
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#DialectForm_Marker>
    #[serde(rename = "d")]
    D,
    /// `@f` - Family-specific form
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#FamilySpecificForm_Marker>
    #[serde(rename = "f")]
    F,
    /// `@fp` - Filled pause (deprecated, use &-um instead)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "fp")]
    FP,
    /// `@g` - Gemination/general special form
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "g")]
    G,
    /// `@i` - Interjection
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "i")]
    I,
    /// `@k` - Letter sequence (kinship)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "k")]
    K,
    /// `@l` - Single letter
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "l")]
    L,
    /// `@ls` - Letter plural
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "ls")]
    LS,
    /// `@n` - Neologism
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Neologism_Marker>
    #[serde(rename = "n")]
    N,
    /// `@o` - Onomatopoeia
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "o")]
    O,
    /// `@p` - Proper name
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "p")]
    P,
    /// `@q` - Metalinguistic reference
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#MetalinguisticReference_Marker>
    #[serde(rename = "q")]
    Q,
    /// `@sas` - Second attempt success
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "sas")]
    SAS,
    /// `@si` - Singing
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "si")]
    SI,
    /// `@sl` - Slang
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "sl")]
    SL,
    /// `@t` - Test word
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "t")]
    T,
    /// `@u` - Unibet transcription
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "u")]
    U,
    /// `@wp` - Word play
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "wp")]
    WP,
    /// `@x` - Complex/excluded
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "x")]
    X,

    /// User-defined special form (@z:label)
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#SpecialForm_Marker>
    #[serde(rename = "z")]
    UserDefined(String),
}

impl FormType {
    /// All standard form markers with their short descriptions.
    pub const ALL_MARKERS: &'static [(&'static str, &'static str)] = &[
        ("a", "approximate"),
        ("b", "babbling"),
        ("c", "child-invented"),
        ("d", "dialect"),
        ("f", "family-specific"),
        ("fp", "filled pause"),
        ("g", "gemination"),
        ("i", "interjection"),
        ("k", "kinship"),
        ("l", "letter"),
        ("ls", "letter sequence"),
        ("n", "neologism"),
        ("o", "onomatopoeia"),
        ("p", "proper name"),
        ("q", "meta-linguistic"),
        ("sas", "second attempt success"),
        ("si", "sing"),
        ("sl", "slang"),
        ("t", "test word"),
        ("u", "unibet"),
        ("wp", "word play"),
        ("x", "complex"),
    ];

    /// Parse a standard marker code in case-insensitive form, with or without `@`.
    ///
    /// This parser intentionally covers only fixed built-in markers. `@z:...`
    /// user-defined markers should be constructed through `UserDefined` paths.
    pub fn parse(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "a" | "@a" => Some(FormType::A),
            "b" | "@b" => Some(FormType::B),
            "c" | "@c" => Some(FormType::C),
            "d" | "@d" => Some(FormType::D),
            "f" | "@f" => Some(FormType::F),
            "fp" | "@fp" => Some(FormType::FP),
            "g" | "@g" => Some(FormType::G),
            "i" | "@i" => Some(FormType::I),
            "k" | "@k" => Some(FormType::K),
            "l" | "@l" => Some(FormType::L),
            "ls" | "@ls" => Some(FormType::LS),
            "n" | "@n" => Some(FormType::N),
            "o" | "@o" => Some(FormType::O),
            "p" | "@p" => Some(FormType::P),
            "q" | "@q" => Some(FormType::Q),
            "sas" | "@sas" => Some(FormType::SAS),
            "si" | "@si" => Some(FormType::SI),
            "sl" | "@sl" => Some(FormType::SL),
            "t" | "@t" => Some(FormType::T),
            "u" | "@u" => Some(FormType::U),
            "wp" | "@wp" => Some(FormType::WP),
            "x" | "@x" => Some(FormType::X),
            _ => None,
        }
    }

    /// Return all standard markers as a comma-separated `@`-prefixed list.
    ///
    /// Useful for diagnostics/help text that needs to enumerate accepted
    /// built-in codes in a stable presentation order.
    pub fn all_markers_string() -> String {
        FormType::ALL_MARKERS
            .iter()
            .map(|(marker, _)| format!("@{}", marker))
            .collect::<Vec<_>>()
            .join(", ")
    }

    /// Return a human-readable label for UI/help surfaces.
    ///
    /// The returned label is intentionally concise and not localized; it is
    /// meant for diagnostics and debug displays rather than end-user prose.
    pub fn description(&self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow;
        match self {
            FormType::A => Cow::Borrowed("approximate"),
            FormType::B => Cow::Borrowed("babbling"),
            FormType::C => Cow::Borrowed("child-invented"),
            FormType::D => Cow::Borrowed("dialect"),
            FormType::F => Cow::Borrowed("family-specific"),
            FormType::FP => Cow::Borrowed("filled pause"),
            FormType::G => Cow::Borrowed("gemination"),
            FormType::I => Cow::Borrowed("interjection"),
            FormType::K => Cow::Borrowed("kinship"),
            FormType::L => Cow::Borrowed("letter"),
            FormType::LS => Cow::Borrowed("letter sequence"),
            FormType::N => Cow::Borrowed("neologism"),
            FormType::O => Cow::Borrowed("onomatopoeia"),
            FormType::P => Cow::Borrowed("proper name"),
            FormType::Q => Cow::Borrowed("meta-linguistic"),
            FormType::SAS => Cow::Borrowed("second attempt success"),
            FormType::SI => Cow::Borrowed("sing"),
            FormType::SL => Cow::Borrowed("slang"),
            FormType::T => Cow::Borrowed("test word"),
            FormType::U => Cow::Borrowed("unibet"),
            FormType::WP => Cow::Borrowed("word play"),
            FormType::X => Cow::Borrowed("complex"),
            FormType::UserDefined(label) => Cow::Owned(format!("user-defined: {}", label)),
        }
    }

    /// Return marker payload text written after `@` in CHAT output.
    ///
    /// Callers writing full CHAT tokens should add the `@` separator
    /// themselves, then append this payload.
    pub fn to_chat_marker(&self) -> std::borrow::Cow<'static, str> {
        use std::borrow::Cow;
        match self {
            FormType::A => Cow::Borrowed("a"),
            FormType::B => Cow::Borrowed("b"),
            FormType::C => Cow::Borrowed("c"),
            FormType::D => Cow::Borrowed("d"),
            FormType::F => Cow::Borrowed("f"),
            FormType::FP => Cow::Borrowed("fp"),
            FormType::G => Cow::Borrowed("g"),
            FormType::I => Cow::Borrowed("i"),
            FormType::K => Cow::Borrowed("k"),
            FormType::L => Cow::Borrowed("l"),
            FormType::LS => Cow::Borrowed("ls"),
            FormType::N => Cow::Borrowed("n"),
            FormType::O => Cow::Borrowed("o"),
            FormType::P => Cow::Borrowed("p"),
            FormType::Q => Cow::Borrowed("q"),
            FormType::SAS => Cow::Borrowed("sas"),
            FormType::SI => Cow::Borrowed("si"),
            FormType::SL => Cow::Borrowed("sl"),
            FormType::T => Cow::Borrowed("t"),
            FormType::U => Cow::Borrowed("u"),
            FormType::WP => Cow::Borrowed("wp"),
            FormType::X => Cow::Borrowed("x"),
            FormType::UserDefined(label) => Cow::Owned(format!("z:{}", label)),
        }
    }
}

impl WriteChat for FormType {
    /// Serializes marker payload without leading `@`.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        w.write_str(self.to_chat_marker().as_ref())
    }
}
