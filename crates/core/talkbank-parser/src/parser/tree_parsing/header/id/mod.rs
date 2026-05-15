//! @ID header parsing
//!
//! **Grammar Rule** (tree-sitter-talkbank/grammar.js):
//! ```javascript
//! id_header: $ => seq(
//!     token('@ID:\t'),
//!     $.id_contents,
//!     $.newline
//! )
//!
//! id_contents: $ => seq(
//!     $.id_languages,        // Position 0
//!     '|',                   // Position 1
//!     optional($.id_corpus), // Position 2
//!     '|',                   // Position 3
//!     $.id_speaker,          // Position 4
//!     '|',                   // Position 5
//!     optional($.id_age),    // Position 6
//!     '|',                   // Position 7
//!     optional($.id_sex),    // Position 8
//!     '|',                   // Position 9
//!     optional($.id_group),  // Position 10
//!     '|',                   // Position 11
//!     optional($.id_ses),    // Position 12
//!     '|',                   // Position 13
//!     $.id_role,             // Position 14
//!     '|',                   // Position 15
//!     optional($.id_education), // Position 16
//!     '|',                   // Position 17
//!     optional($.id_custom_field), // Position 18
//!     '|'                    // Position 19
//! )
//! ```
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>

mod fields;
mod helpers;
mod parse;

pub use parse::parse_id_header;
