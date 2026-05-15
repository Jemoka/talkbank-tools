//! Raw FFI declarations for the `send2clan` C API.
//!
//! This module keeps the `unsafe extern "C"` bindings in one place and documents
//! the per-function safety contracts required by Rust. Higher-level helpers
//! in `crate::api` wrap these calls and expose safe abstractions to the rest of
//! the workspace.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::os::raw::{c_char, c_int, c_long};

// External C functions from the send2clan library.
//
// These declarations must match the C API exactly.
// See: include/send2clan/send2clan.h
//
// Safety note: In Rust 2024 edition, extern blocks must be marked unsafe.
// Calling these functions is unsafe because they cross the FFI boundary.
unsafe extern "C" {
    /// Send file information to CLAN application.
    ///
    /// # Safety
    ///
    /// - `file_path` must be a valid null-terminated C string
    /// - `file_path` must remain valid for the duration of the call
    /// - `message` must be either null or a valid null-terminated C string
    /// - `message` must remain valid for the duration of the call if not null
    pub(crate) fn send2clan(
        timeout: c_long,
        file_path: *const c_char,
        line_number: c_int,
        column_number: c_int,
        message: *const c_char,
    ) -> c_int;

    /// Get library version string.
    ///
    /// # Safety
    ///
    /// - Returns a pointer to a statically allocated string
    /// - The pointer is always valid and never null
    /// - The string remains valid for the program lifetime
    pub(crate) fn send2clan_version() -> *const c_char;

    /// Get library capabilities as bit flags.
    ///
    /// # Safety
    ///
    /// - `capabilities` must be a valid pointer to a uint32_t
    /// - The pointed-to value will be overwritten
    pub(crate) fn send2clan_get_capabilities(capabilities: *mut u32) -> c_int;

    /// Check if current platform is supported.
    ///
    /// # Safety
    ///
    /// This function is always safe to call (no parameters).
    #[link_name = "is_platform_supported"]
    pub(crate) fn ffi_is_platform_supported() -> bool;

    /// Check if CLAN application is installed and available.
    ///
    /// # Safety
    ///
    /// This function is always safe to call (no parameters).
    #[link_name = "is_clan_available"]
    pub(crate) fn ffi_is_clan_available() -> bool;
}
