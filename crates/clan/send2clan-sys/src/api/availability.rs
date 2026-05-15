//! Platform and CLAN-installation availability checks.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use crate::ffi;

/// Check if current platform is supported by send2clan.
///
/// Checks at compile time whether the library was built for
/// a supported platform (macOS or Windows). Always returns the same value
/// for a given binary.
///
/// Call this function early in your application to provide clear error messages
/// on unsupported platforms, rather than receiving errors at runtime.
///
/// # Performance
///
/// Extremely fast (< 1µs) - just checks compile-time constants.
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::is_platform_supported;
///
/// if !is_platform_supported() {
///     eprintln!("Error: send2clan only supports macOS and Windows");
///     std::process::exit(1);
/// }
///
/// // Safe to proceed with send2clan operations
/// ```
pub fn is_platform_supported() -> bool {
    // Safety:
    // - This function takes no parameters and always succeeds
    // - It just returns a compile-time constant
    unsafe { ffi::ffi_is_platform_supported() }
}

/// Check if CLAN application is installed and available.
///
/// Checks at runtime whether the CLAN application can be found
/// on the system. This may involve filesystem or registry access and can take
/// 10-100ms on first call.
///
/// # Platform-Specific Detection
///
/// * **macOS**: Searches for bundle ID "org.talkbank.clanc" via Launch Services
/// * **Windows**: Checks registry key and default installation paths
///
/// # Performance
///
/// * Typical: 10-100ms (filesystem or registry access)
/// * Consider caching the result for repeated calls
///
/// # Examples
///
/// ```rust,no_run
/// use send2clan::is_clan_available;
///
/// if !is_clan_available() {
///     eprintln!("CLAN is not installed");
///     eprintln!("Download from: https://dali.talkbank.org/clan/");
///     std::process::exit(1);
/// }
///
/// // CLAN is available, safe to call send_to_clan()
/// ```
///
/// # Example with caching
///
/// ```rust,no_run
/// use send2clan::is_clan_available;
/// use std::sync::OnceLock;
///
/// static CLAN_AVAILABLE: OnceLock<bool> = OnceLock::new();
///
/// fn check_clan() -> bool {
///     *CLAN_AVAILABLE.get_or_init(|| is_clan_available())
/// }
/// ```
pub fn is_clan_available() -> bool {
    // Safety:
    // - This function takes no parameters and always succeeds
    // - It performs platform-specific checks to locate CLAN
    unsafe { ffi::ffi_is_clan_available() }
}
