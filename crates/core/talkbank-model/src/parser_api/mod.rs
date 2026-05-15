//! Parser API types for CHAT format parsing.

mod chat_parser;
mod context;
mod outcome;

pub use chat_parser::ChatParser;
pub use context::FragmentSemanticContext;
pub use outcome::ParseOutcome;
