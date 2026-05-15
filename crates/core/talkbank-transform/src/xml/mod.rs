//! CHAT → TalkBank XML emission.
//!
//! Serializes a [`ChatFile`] into TalkBank XML conforming to `talkbank.xsd`
//! (namespace `http://www.talkbank.org/ns/talkbank`). This module is the
//! Rust-side replacement for the legacy Java Chatter tool's CHAT-to-XML
//! output path.
//!
//! # Scope
//!
//! - **Emission only.** XML ingest (XML → CHAT) is explicitly out of scope;
//!   known external consumers (NLTK `CHILDESCorpusReader`, `childes-db`,
//!   TEICORPO) only read the XML. Phon historically consumed the rich
//!   `%pho` XML but has since pivoted to CHAT-only interchange.
//! - **Phonetic tiers are permanently out of scope.** `%pho`, `%mod`,
//!   `%phosyl`, `%modsyl`, and `%phoaln` report
//!   [`XmlWriteError::PhoneticTierUnsupported`]. These tiers round-trip
//!   through CHAT unchanged — only the XML projection is declined.
//!   Porting the `phon-ipa` IPA parser into Rust solely to
//!   reproduce `<pg>/<pw>/<ph>/<cmph>/<ss>` is not worth the cost when
//!   no downstream consumer reads it.
//!
//! # Parity target
//!
//! The authoritative parity oracle is the Java Chatter output stored in
//! `resources/corpus/reference-xml/`, generated against `resources/corpus/reference/`. The
//! golden-XML test harness in `talkbank-parser-tests` compares Rust output
//! against those goldens after XML-structural normalization (whitespace
//! and attribute order).
//!
//! # Example
//!
//! ```no_run
//! use talkbank_transform::xml::write_chat_xml;
//! use talkbank_transform::parse_and_validate;
//! use talkbank_model::ParseValidateOptions;
//!
//! let chat = "@UTF8\n@Begin\n@Languages:\teng\n\
//!     @Participants:\tCHI Child\n\
//!     @ID:\teng|corpus|CHI|||||Child|||\n\
//!     *CHI:\thello .\n@End\n";
//! let file = parse_and_validate(chat, ParseValidateOptions::default().with_validation())
//!     .expect("valid CHAT");
//! let xml = write_chat_xml(&file).expect("emit XML");
//! assert!(xml.contains("<CHAT"));
//! ```

mod deptier;
mod error;
mod mor;
mod root;
mod wor;
mod word;
mod writer;

pub use error::XmlWriteError;
pub use writer::write_chat_xml;
