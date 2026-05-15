//! Scoped annotation parsing
//!
//! Handles parsing of base annotations like [*], [=], [+], etc.
//! Retrace markers (`[/]`, `[//]`, `[///]`, `[/-]`, `[/?]`) are parsed
//! separately from content annotations and returned as `RetraceKind` in
//! `ParsedAnnotations`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Scoped_Symbols>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Retracing_and_Repetition>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Scope>

mod list;
mod single;
mod symbols;

pub(crate) use list::parse_scoped_annotations;
