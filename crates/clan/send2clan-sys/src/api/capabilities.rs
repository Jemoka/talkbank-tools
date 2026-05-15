//! Runtime capability queries for the `send2clan` backend.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::error::{Error, Result};
use crate::ffi;

/// Get library capabilities as bit flags.
///
/// Returns a 32-bit capability mask indicating which features are available
/// in the current runtime environment.
///
/// # Capability Bits
///
/// - **Bit 0 (0x01)**: Platform supported (macOS or Windows)
/// - **Bit 1 (0x02)**: CLAN application available on system
/// - **Bit 2 (0x04)**: Unicode path support (always enabled)
/// - **Bit 3 (0x08)**: Timeout support (always enabled)
/// - **Bits 4-31**: Reserved for future use (currently 0)
///
/// # Typical Values
///
/// - **macOS with CLAN**: `0x0F` (all bits set)
/// - **macOS without CLAN**: `0x0D` (bit 1 clear)
/// - **Windows with CLAN**: `0x0F` (all bits set)
/// - **Windows without CLAN**: `0x0D` (bit 1 clear)
/// - **Linux**: `0x0C` (only Unicode and timeout bits)
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::get_capabilities;
///
/// let caps = get_capabilities()?;
///
/// if caps & 0x01 != 0 {
///     println!("Platform is supported");
/// }
///
/// if caps & 0x02 != 0 {
///     println!("CLAN is installed");
/// } else {
///     println!("Please install CLAN from https://dali.talkbank.org/clan/");
/// }
/// # Ok::<(), send2clan::Error>(())
/// ```
pub fn get_capabilities() -> Result<u32> {
    let mut caps: u32 = 0;

    let result = unsafe { ffi::send2clan_get_capabilities(&mut caps) };

    if result == 0 {
        Ok(caps)
    } else {
        Err(Error::from_code(result))
    }
}
