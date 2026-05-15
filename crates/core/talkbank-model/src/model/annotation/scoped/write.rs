//! CHAT serialization for scoped annotations (`[*]`, `[=]`, `[/]`, ...).
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Error_Coding>
//!
//! This module keeps display semantics centralized so annotation labels can be
//! rendered consistently in validation diagnostics, caret reconstruction, and
//! batchalign exports.

use super::ContentAnnotation;

impl ContentAnnotation {
    /// Serializes one scoped annotation in canonical CHAT bracket syntax.
    ///
    /// Known variants emit normalized spellings (`[*]`, `[/]`, `[= ...]`, ...),
    /// while unknown markers are preserved in a lossy-free fallback form so
    /// roundtrip tooling can keep corpus-specific extensions intact. Keeping
    /// formatting logic centralized here guarantees consistent rendering across
    /// main-tier, bracketed, and replacement contexts.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            ContentAnnotation::Error(error) => match error.code.as_ref() {
                None => w.write_str("[*]"),
                Some(code) => {
                    w.write_str("[* ")?;
                    w.write_str(code)?;
                    w.write_char(']')
                }
            },
            ContentAnnotation::Explanation(explanation) => {
                w.write_str("[= ")?;
                w.write_str(&explanation.text)?;
                w.write_char(']')
            }
            ContentAnnotation::OverlapBegin(overlap) => {
                if let Some(idx) = overlap.index.as_ref() {
                    write!(w, "[<{}]", idx)
                } else {
                    w.write_str("[<]")
                }
            }
            ContentAnnotation::OverlapEnd(overlap) => {
                if let Some(idx) = overlap.index.as_ref() {
                    write!(w, "[>{}]", idx)
                } else {
                    w.write_str("[>]")
                }
            }
            ContentAnnotation::Stressing => w.write_str("[!]"),
            ContentAnnotation::ContrastiveStressing => w.write_str("[!!]"),
            ContentAnnotation::Uncertain => w.write_str("[?]"),
            ContentAnnotation::Paralinguistic(paralinguistic) => {
                w.write_str("[=! ")?;
                w.write_str(&paralinguistic.text)?;
                w.write_char(']')
            }
            ContentAnnotation::Alternative(alternative) => {
                w.write_str("[=? ")?;
                w.write_str(&alternative.text)?;
                w.write_char(']')
            }
            ContentAnnotation::PercentComment(comment) => {
                w.write_str("[% ")?;
                w.write_str(&comment.text)?;
                w.write_char(']')
            }
            ContentAnnotation::Exclude => w.write_str("[e]"),
            ContentAnnotation::Unknown(unknown) => {
                w.write_char('[')?;
                w.write_str(&unknown.marker)?;
                if !unknown.text.is_empty() {
                    w.write_char(' ')?;
                    w.write_str(&unknown.text)?;
                }
                w.write_char(']')
            }
        }
    }
}

impl std::fmt::Display for ContentAnnotation {
    /// Formats one scoped annotation using CHAT serialization.
    ///
    /// This wraps [`Self::write_chat`] so consumers writing to `Display` receive
    /// identical output without dealing with format buffers directly.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
