//! Label newtype shared by nonvocal event markers.
//!
//! A shared label type avoids accidental mixing with unrelated string payloads
//! and keeps begin/end/simple nonvocal markers interoperable by construction.
//! This type helps parsers enforce label equality when matching begin/end spans.
//! Namespace collisions are avoided by keeping these labels narrow to long
//! nonverbal events, unlike more general `@Comment` strings.
//!
//! Reference: <https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent>

use crate::string_newtype;

string_newtype!(
    /// Nonvocal event label payload.
    ///
    /// Kept as an open string newtype because corpora commonly define custom
    /// nonvocal labels beyond any fixed vocabulary. Labels are preserved
    /// verbatim for roundtrip fidelity and begin/end pairing checks.
    ///
    /// # CHAT Format Examples
    ///
    /// ```text
    /// &{n=laugh}         Simple laugh (point event)
    /// &{n=crying         Begin crying (scoped event start)
    /// &}n=crying         End crying (scoped event end)
    /// &{n=cough}         Simple cough
    /// &{n=gesture        Begin gesture
    /// &}n=gesture        End gesture
    /// ```
    ///
    /// # Common Labels
    ///
    /// - **Vocalizations**: laugh, cry, cough, yawn, sneeze, grunt, sigh, gasp, hiccup
    /// - **Gestures**: point, wave, clap, nod, shake, gesture
    /// - **Actions**: eat, drink, play, touch, throw, drop
    /// - **Other**: breath, sniff, burp, whistle, hum
    ///
    /// # References
    ///
    /// - [Long Nonverbal Event](https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent)
    ///
    /// These labels stay verbatim because researchers often invent corpus-specific terms;
    /// the type exists so validators can require label parity between begin/end markers.
    #[serde(transparent)]
    pub struct NonvocalLabel;
);
