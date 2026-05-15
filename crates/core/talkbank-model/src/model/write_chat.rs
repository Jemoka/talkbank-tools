//! `WriteChat` trait for serializing data models to CHAT format
//!
//! This trait ensures all CHAT data model types can be serialized consistently,
//! similar to how serde's `Serialize` trait works for JSON/etc.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Design Principles
//!
//! 1. **Zero-allocation**: Use `std::fmt::Write` trait for efficient streaming
//! 2. **Self-contained**: Each type serializes itself completely (including prefixes)
//! 3. **Composable**: Types can call write_chat on their children
//! 4. **Error handling**: Returns `std::fmt::Result` for write errors
//!
//! # Examples
//!
//! ```
//! use talkbank_model::model::{MorWord, PosCategory, WriteChat};
//!
//! let word = MorWord::new(PosCategory::new("pro"), "I");
//!
//! // Zero-allocation: write to any std::fmt::Write
//! let mut output = String::new();
//! let _ = word.write_chat(&mut output);
//! assert_eq!(output, "pro|I");
//!
//! // Convenience: allocate a String
//! let output = word.to_chat_string();
//! assert_eq!(output, "pro|I");
//! ```

use std::fmt::Write;

/// Trait for types that can be serialized to CHAT format
///
/// All CHAT data model types should implement this trait to ensure
/// they can be serialized consistently.
pub trait WriteChat {
    /// Write this object to CHAT format (zero-allocation)
    ///
    /// This method streams the CHAT representation directly to the writer,
    /// avoiding intermediate String allocations for better performance.
    ///
    /// # Arguments
    ///
    /// * `w` - A mutable reference to any type implementing `std::fmt::Write`
    ///
    /// # Returns
    ///
    /// * `Ok(())` if writing succeeded
    /// * `Err(std::fmt::Error)` if writing failed
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::{MorWord, PosCategory, WriteChat};
    ///
    /// let word = MorWord::new(PosCategory::new("n"), "dog");
    ///
    /// let mut output = String::new();
    /// let _ = word.write_chat(&mut output);
    /// assert_eq!(output, "n|dog");
    /// ```
    fn write_chat<W: Write>(&self, w: &mut W) -> std::fmt::Result;

    /// Convert to CHAT format string (convenience method that allocates)
    ///
    /// This is a convenience method that allocates a String. For better
    /// performance with large data structures, use `write_chat` directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use talkbank_model::model::{MorWord, PosCategory, WriteChat};
    ///
    /// let word = MorWord::new(PosCategory::new("v"), "run");
    /// assert_eq!(word.to_chat_string(), "v|run");
    /// ```
    fn to_chat_string(&self) -> String {
        let mut s = String::new();
        let _ = self.write_chat(&mut s);
        s
    }
}
