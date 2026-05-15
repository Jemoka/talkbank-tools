//! Version-query API for the `send2clan` backend.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::ffi::CStr;

use crate::ffi;

/// Get send2clan library version string.
///
/// Returns the semantic version in "MAJOR.MINOR.PATCH" format (e.g., "1.0.0").
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::version;
///
/// let ver = version();
/// println!("Using send2clan version {}", ver);
/// // Output: "Using send2clan version 1.0.0"
/// ```
pub fn version() -> &'static str {
    // Safety:
    // - The C function always returns a pointer to a static string
    // - The pointer is never null
    // - The string is valid UTF-8 (version strings are ASCII)
    // - The string remains valid for the program lifetime
    unsafe {
        let version_ptr = ffi::send2clan_version();
        match CStr::from_ptr(version_ptr).to_str() {
            Ok(version) => version,
            Err(_err) => "invalid-utf8",
        }
    }
}
