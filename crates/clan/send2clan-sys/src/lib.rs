#![warn(missing_docs)]
//! # Send2Clan Rust Bindings
//!
//! Idiomatic Rust bindings for the send2clan library, which enables sending file
//! open messages with error information to the CLAN (Computerized Language Analysis) application.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//!
//! ## Features
//!
//! - **Ultra-simple API**: One function does everything
//! - **Cross-platform support**: Works on macOS and Windows
//! - **Stateless operation**: No context or configuration management needed
//! - **Idiomatic Rust**: Uses Result types and proper error handling
//! - **Thread-safe**: Can be called from multiple threads simultaneously
//! - **Zero-cost abstractions**: Thin wrapper over C API
//!
//! ## Quick Start
//!
//! ```rust,no_run
//! use send2clan::send_to_clan;
//!
//! fn main() -> Result<(), send2clan::Error> {
//!     // Send file to CLAN with 30-second timeout
//!     send_to_clan(30, "/path/to/file.cha", 42, 10, Some("Syntax error"))?;
//!     println!("Successfully sent file to CLAN!");
//!     Ok(())
//! }
//! ```
//!
//! ## API Overview
//!
//! The primary function is [`send_to_clan`], which performs the complete workflow:
//! 1. Validates parameters
//! 2. Launches CLAN if not already running
//! 3. Sends file path and cursor position to CLAN
//! 4. Displays optional error message
//!
//! Helper functions:
//! - [`is_platform_supported`]: Check if current platform is supported
//! - [`is_clan_available`]: Check if CLAN is installed
//! - [`version`]: Get library version string
//! - [`get_capabilities`]: Query runtime capabilities

mod api;
mod error;
mod ffi;

#[cfg(test)]
mod tests;

pub use api::{get_capabilities, is_clan_available, is_platform_supported, send_to_clan, version};
pub use error::{Error, Result};
