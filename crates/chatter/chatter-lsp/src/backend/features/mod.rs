//! LSP feature implementations.
//!
//! These modules implement individual LSP features (hover, completion, diagnostics,
//! highlights, inlay hints, etc.). They keep feature-specific logic out of the core
//! backend router so testing and documentation can focus on one feature at a time.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod code_action;
pub mod code_lens;
mod completion;
mod document_link;
mod document_symbol;
mod folding_range;
mod highlights;
mod hover;
mod inlay_hints;
pub mod linked_editing;
mod on_type_formatting;
pub mod references;
pub mod rename;
mod selection_range;
mod workspace_symbol;

pub use code_action::code_action;
pub use completion::completion;
pub use document_link::document_links;
pub use document_symbol::document_symbol;
pub use folding_range::folding_range;
pub use highlights::document_highlights;
pub use hover::hover;
pub use inlay_hints::generate_alignment_hints;
pub use linked_editing::linked_editing_ranges;
pub use on_type_formatting::on_type_formatting;
pub use selection_range::selection_range;
pub use workspace_symbol::workspace_symbols_for_document;
