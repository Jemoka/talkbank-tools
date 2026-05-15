//! Public API surface for `send2clan` operations.
//!
//! This thin wrapper exposes the useful helpers (`availability`, `capabilities`,
//! `send`, `version`) while keeping the unsafe FFI declarations confined to
//! `crate::ffi`. Each re-export represents one programmable action facing
//! higher-level Rust callers inside `talkbank-chatter`.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

mod availability;
mod capabilities;
mod send;
mod version;

pub use availability::{is_clan_available, is_platform_supported};
pub use capabilities::get_capabilities;
pub use send::send_to_clan;
pub use version::version;
