//! Formatting layer for alignment hover text.
//!
//! Re-exports per-tier formatters (`content`, `mor`, `pho`, `sin`, `pos`) and
//! the top-level [`format_alignment_info`] that assembles an
//! [`AlignmentHoverInfo`](super::types::AlignmentHoverInfo) into Markdown.

mod alignment_info;
mod content;
mod mor;
mod pho;
mod pos;
mod sin;

// Re-export all public functions
pub use alignment_info::format_alignment_info;
pub use content::format_content_item;
pub use mor::format_mor_item;
pub use pho::format_pho_item;
pub use sin::{format_sin_item, format_sin_item_details};
