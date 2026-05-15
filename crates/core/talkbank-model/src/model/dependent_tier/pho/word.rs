//! Token type for one `%pho`/`%mod` phonological word.
//!
//! This newtype intentionally keeps the payload as an open string because
//! corpora use multiple transcription conventions (IPA, UNIBET, and project-
//! specific variants) that should roundtrip without normalization.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Model_Tier>

use crate::string_newtype;

string_newtype!(
    /// One phonological token from a `%pho` or `%mod` tier.
    ///
    /// Newtype wrapper around `String` to prevent primitive obsession and keep
    /// phonological-tier APIs distinct from orthographic word types.
    ///
    /// The model deliberately does not enforce a symbol inventory here; strict
    /// constraints belong in corpus- or project-specific validators.
    ///
    /// # Notation Systems
    ///
    /// - **IPA (International Phonetic Alphabet)**: Unicode phonetic symbols
    /// - **UNIBET**: ASCII-based phonetic notation for computational processing
    /// - **X-SAMPA**: Extended SAMPA ASCII notation
    /// - **Custom**: Project-specific notation systems for %upho
    ///
    /// # CHAT Format Examples
    ///
    /// IPA transcription:
    /// ```text
    /// həˈloʊ    (hello)
    /// wʌn       (one)
    /// θri       (three)
    /// ```
    ///
    /// UNIBET transcription:
    /// ```text
    /// h@'lVU    (hello)
    /// wVn       (one)
    /// Tri       (three)
    /// ```
    ///
    /// Child speech error:
    /// ```text
    /// fwi       (child says "fwi" for "three")
    /// wɛd       (child says "wed" for "red")
    /// ```
    ///
    /// # References
    ///
    /// - [Phonology Tier](https://talkbank.org/0info/manuals/CHAT.html#Phonology_Tier)
    /// - [IPA Usage](https://talkbank.org/0info/manuals/CHAT.html#IPA)
    /// - [Model Phonology](https://talkbank.org/0info/manuals/CHAT.html#Model_Tier)
    #[serde(transparent)]
    pub struct PhoWord;
);
