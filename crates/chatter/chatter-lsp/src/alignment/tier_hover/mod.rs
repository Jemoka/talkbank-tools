//! Tier-specific hover resolvers that build [`AlignmentHoverInfo`](super::types::AlignmentHoverInfo).
//!
//! Each sub-module handles one tier type: [`main_tier`] resolves main-tier words
//! and looks up aligned `%mor`/`%pho`/`%sin` items; [`mor_tier`] and [`gra_tier`]
//! resolve dependent-tier items and look back to the main-tier word;
//! [`pho_tier`] and [`sin_tier`] do the same for their respective tiers.
//! Shared CST traversal helpers live in [`helpers`].

mod gra_tier;
mod helpers;
mod main_tier;
mod mor_tier;
mod pho_tier;
mod phon_tiers;
mod sin_tier;

// Re-export public resolver entry points.
pub use gra_tier::find_gra_tier_hover_info;
pub use main_tier::find_main_tier_hover_info;
pub use mor_tier::find_mor_tier_hover_info;
pub use pho_tier::find_pho_tier_hover_info;
pub use phon_tiers::{
    find_modsyl_tier_hover_info, find_phoaln_tier_hover_info, find_phosyl_tier_hover_info,
};
pub use sin_tier::find_sin_tier_hover_info;
