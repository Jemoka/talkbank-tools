//! CHAT serialization helpers for `Utterance`.
//!
//! Serialization preserves parsed tier order exactly; this is required for
//! stable roundtrips because CHAT does not enforce a canonical dependent-tier
//! ordering across corpora.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Line>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::Utterance;
use crate::WriteChat;

impl Utterance {
    /// Serialize to an owned CHAT string.
    pub fn to_chat(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}

impl WriteChat for Utterance {
    /// Write this utterance as CHAT text.
    ///
    /// Writes preceding headers, then main tier, then dependent tiers in
    /// preserved source order.
    ///
    /// **IMPORTANT**: The CHAT format does NOT mandate any particular tier ordering.
    /// We preserve input order to avoid introducing diffs during roundtrip.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        // Write any preceding headers (e.g., @Comment, @Bg, @Eg)
        for header in &self.preceding_headers {
            header.write_chat(w)?;
            w.write_char('\n')?;
        }

        // Write main tier
        self.main.write_chat(w)?;
        w.write_char('\n')?;

        // Write dependent tiers in parsed order.
        for tier in &self.dependent_tiers {
            tier.write_chat(w)?;
            w.write_char('\n')?;
        }

        Ok(())
    }
}

impl std::fmt::Display for Utterance {
    /// Formats one utterance as CHAT text.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.write_chat(f)
    }
}
