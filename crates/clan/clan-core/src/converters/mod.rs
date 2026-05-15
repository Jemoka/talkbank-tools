//! Format converters between CHAT and other transcription/annotation formats.
//!
//! Each converter is a standalone module with functions for converting between
//! CHAT format and a foreign format. Converters produce or consume
//! [`ChatFile`](talkbank_model::ChatFile) instances, building proper AST
//! structures rather than emitting raw text.
//!
//! # Available converters
//!
//! | Module | Direction | Description |
//! |--------|-----------|-------------|
//! | [`chat2text`] | CHAT --> Text | Plain text export (strip all annotations) |
//! | [`chat2srt`] | CHAT --> SRT/WebVTT | Subtitle export using timing bullets |
//! | [`elan2chat`] | ELAN --> CHAT | ELAN annotation XML (`.eaf`) import |
//! | [`lab2chat`] | LAB --> CHAT | Timing label files (speech research) |
//! | [`lena2chat`] | LENA --> CHAT | LENA device XML (`.its`) import |
//! | [`lipp2chat`] | LIPP --> CHAT | LIPP phonetic profile with `%pho` tiers |
//! | [`play2chat`] | PLAY --> CHAT | PLAY annotation format import |
//! | [`praat2chat`] | Praat <--> CHAT | Praat TextGrid bidirectional conversion |
//! | [`rtf2chat`] | RTF --> CHAT | Rich Text Format import |
//! | [`salt2chat`] | SALT --> CHAT | SALT transcription format import |
//! | [`srt2chat`] | SRT --> CHAT | SRT subtitle import with timing |
//! | [`text2chat`] | Text --> CHAT | Plain text import (sentence splitting) |

pub mod chat2elan;
pub mod chat2srt;
pub mod chat2text;
pub mod elan2chat;
pub mod lab2chat;
pub mod lena2chat;
pub mod lipp2chat;
pub mod play2chat;
pub mod praat2chat;
pub mod rtf2chat;
pub mod salt2chat;
pub mod srt2chat;
pub mod text2chat;
