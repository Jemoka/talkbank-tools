//! CHAT serialization for bullet-capable dependent-tier text content.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Bullets>
//!
//! Serializers here are the single source of truth for inline bullets, so any
//! tier that needs to emit `\u0015…\u0015` markers (including `%act`/`%cod`)
//! should route through this module.

use super::{BulletContent, BulletContentSegment};

impl BulletContent {
    /// Serializes bullet-capable dependent-tier text in CHAT form.
    ///
    /// Segment order is preserved exactly so inline timing/picture markers keep
    /// their original textual placement. Control delimiters (`U+0015`) and
    /// continuation markers (`\n\t`) are emitted verbatim for roundtrip fidelity.
    /// Callers should treat this as the canonical writer for `%act/%cod/%com`
    /// style payloads rather than rebuilding bullet strings manually.
    pub fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        for segment in &self.segments {
            match segment {
                BulletContentSegment::Text(text) => {
                    w.write_str(text.text.as_str())?;
                }
                BulletContentSegment::Bullet(bullet) => {
                    w.write_char('\u{0015}')?;
                    write!(w, "{}_{}", bullet.start_ms, bullet.end_ms)?;
                    w.write_char('\u{0015}')?;
                }
                BulletContentSegment::Picture(picture) => {
                    w.write_char('\u{0015}')?;
                    write!(w, "%pic:\"{}\"", picture.filename)?;
                    w.write_char('\u{0015}')?;
                }
                BulletContentSegment::Continuation => {
                    w.write_str("\n\t")?;
                }
            }
        }
        Ok(())
    }

    /// Allocating convenience wrapper over [`Self::write_chat`].
    ///
    /// Prefer [`Self::write_chat`] when writing into existing buffers to avoid
    /// transient allocation in hot paths.
    pub fn to_chat_string(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}
