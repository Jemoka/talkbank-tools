//! Serialization and display implementations for [`Word`].
//!
//! Contains the custom [`Serialize`] impl (with computed fields), the
//! [`JsonSchema`] proxy struct, [`WriteChat`] (CHAT surface form), and
//! [`Display`].

use schemars::JsonSchema;
use serde::Serialize;

use crate::model::WriteChat;

use super::category::WordCategory;
use super::form::FormType;
use super::language::WordLanguageMarker;
use super::untranscribed::UntranscribedStatus;
use super::word_contents::WordContents;
use super::word_type::Word;
use crate::model::Bullet;

impl Serialize for Word {
    /// Serializes Word to JSON with computed fields (`cleaned_text`, `untranscribed`).
    ///
    /// This hand-written impl replaces `#[derive(Serialize)]` so that:
    /// - `span` is always omitted (byte offsets are useless to JSON consumers)
    /// - `cleaned_text` is emitted as a computed string (NLP-ready text)
    /// - `untranscribed` is emitted when the word is xxx/yyy/www
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeStruct;

        // Count fields: raw_text + cleaned_text + content are always present
        let mut field_count = 3;
        if self.word_id.is_some() {
            field_count += 1;
        }
        if self.category.is_some() {
            field_count += 1;
        }
        if self.form_type.is_some() {
            field_count += 1;
        }
        if self.lang.is_some() {
            field_count += 1;
        }
        if self.part_of_speech.is_some() {
            field_count += 1;
        }
        if self.inline_bullet.is_some() {
            field_count += 1;
        }
        let untranscribed = self.untranscribed();
        if untranscribed.is_some() {
            field_count += 1;
        }

        let mut state = serializer.serialize_struct("Word", field_count)?;

        if let Some(ref id) = self.word_id {
            state.serialize_field("word_id", id)?;
        }
        state.serialize_field("raw_text", &self.raw_text)?;
        state.serialize_field("cleaned_text", self.cleaned_text())?;
        state.serialize_field("content", &self.content)?;
        if let Some(ref cat) = self.category {
            state.serialize_field("category", cat)?;
        }
        if let Some(ref ft) = self.form_type {
            state.serialize_field("form_type", ft)?;
        }
        if let Some(ref lang) = self.lang {
            state.serialize_field("lang", lang)?;
        }
        if let Some(ref pos) = self.part_of_speech {
            state.serialize_field("part_of_speech", pos)?;
        }
        if let Some(ref bullet) = self.inline_bullet {
            state.serialize_field("inline_bullet", bullet)?;
        }
        if let Some(ref ut) = untranscribed {
            state.serialize_field("untranscribed", ut)?;
        }

        state.end()
    }
}

/// A word in a CHAT transcript with optional markers and internal structure.
///
/// Words are the fundamental units of CHAT transcripts. They can be simple dictionary
/// forms, compounds, cliticizations, or include special markers indicating dialectal
/// variations, child-invented forms, and other linguistic phenomena.
// NOTE: This struct mirrors Word's Serialize output for JsonSchema generation.
// It must be kept in sync with the manual `impl Serialize for Word` above.
#[derive(JsonSchema)]
#[schemars(rename = "Word")]
#[allow(dead_code)] // Fields are only read by the JsonSchema derive macro
struct WordJsonSchema {
    /// Unique identifier for tier alignment.
    #[schemars(skip_serializing_if = "Option::is_none")]
    word_id: Option<smol_str::SmolStr>,

    /// Raw text exactly as it appeared in the input, including all markers.
    raw_text: smol_str::SmolStr,

    /// Cleaned text suitable for downstream NLP.
    ///
    /// Computed from structured content by concatenating `Text` and `Shortening`
    /// elements (e.g., `sit(ting)` → `sitting`). Excludes prosodic markers,
    /// lengthening, stress, CA elements, and overlap points.
    cleaned_text: String,

    /// Structured content breakdown.
    content: WordContents,

    /// Word category prefix.
    #[schemars(skip_serializing_if = "Option::is_none")]
    category: Option<WordCategory>,

    /// Form type marker (@a, @b, @c, @z:custom, etc.).
    #[schemars(skip_serializing_if = "Option::is_none")]
    form_type: Option<FormType>,

    /// Language-specific marker (@s or @s:code).
    #[schemars(skip_serializing_if = "Option::is_none")]
    lang: Option<WordLanguageMarker>,

    /// Part-of-speech tag ($adj, $n, $v, etc.).
    #[schemars(skip_serializing_if = "Option::is_none")]
    part_of_speech: Option<smol_str::SmolStr>,

    /// Inline timing bullet parsed directly from %wor tier.
    #[schemars(skip_serializing_if = "Option::is_none")]
    inline_bullet: Option<Bullet>,

    /// Untranscribed status classification (xxx → unintelligible, yyy → phonetic, www → untranscribed).
    ///
    /// Present only when the word is one of the three canonical untranscribed markers.
    #[schemars(skip_serializing_if = "Option::is_none")]
    untranscribed: Option<UntranscribedStatus>,
}

impl JsonSchema for Word {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        WordJsonSchema::schema_name()
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        WordJsonSchema::json_schema(generator)
    }
}

impl WriteChat for Word {
    /// Serializes a word token with category/form/language/POS markers in CHAT order.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        if let Some(ref cat) = self.category {
            cat.write_chat(w)?;
        }

        let wrap_ca_omission = matches!(self.category, Some(WordCategory::CAOmission));

        if wrap_ca_omission {
            w.write_char('(')?;
        }

        for item in &self.content {
            item.write_chat(w)?;
        }

        if wrap_ca_omission {
            w.write_char(')')?;
        }

        if let Some(ref ft) = self.form_type {
            w.write_char('@')?;
            ft.write_chat(w)?;
        }

        if let Some(ref marker) = self.lang {
            match marker {
                WordLanguageMarker::Shortcut => w.write_str("@s")?,
                WordLanguageMarker::Explicit(code) => {
                    w.write_str("@s:")?;
                    code.write_chat(w)?;
                }
                WordLanguageMarker::Multiple(codes) => {
                    w.write_str("@s:")?;
                    for (i, code) in codes.iter().enumerate() {
                        if i > 0 {
                            w.write_char('+')?;
                        }
                        code.write_chat(w)?;
                    }
                }
                WordLanguageMarker::Ambiguous(codes) => {
                    w.write_str("@s:")?;
                    for (i, code) in codes.iter().enumerate() {
                        if i > 0 {
                            w.write_char('&')?;
                        }
                        code.write_chat(w)?;
                    }
                }
            }
        }

        if let Some(ref pos) = self.part_of_speech {
            w.write_char('$')?;
            w.write_str(pos)?;
        }

        Ok(())
    }
}

impl std::fmt::Display for Word {
    /// Formats this word using CHAT serialization.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
