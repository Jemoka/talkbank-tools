//! Error types and conversions for this subsystem.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>

use std::ffi::NulError;
use std::os::raw::c_int;

/// Error type for send2clan operations.
///
/// All error variants correspond to specific error codes returned by the
/// underlying C library. Error codes are stable across versions (ABI guarantee).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Platform is not supported (Linux or other non-macOS/Windows OS).
    ///
    /// **Error Code**: 1
    /// **Recovery**: Check platform with [`is_platform_supported`] before calling.
    UnsupportedPlatform,

    /// Failed to launch CLAN application.
    ///
    /// **Error Code**: 2
    /// **Recovery**: Check CLAN installation, file permissions, or system resources.
    LaunchFailed,

    /// CLAN application not found on system.
    ///
    /// **Error Code**: 3
    /// **Recovery**: Install CLAN from <https://dali.talkbank.org/clan/>
    AppNotFound,

    /// Failed to send message to CLAN.
    ///
    /// **Error Code**: 4
    /// **Recovery**: Retry operation; check CLAN is responsive; verify disk space.
    SendFailed,

    /// Operation timed out.
    ///
    /// **Error Code**: 5
    /// **Recovery**: Increase timeout value; check CLAN responsiveness; restart CLAN.
    Timeout,

    /// Invalid parameter(s) provided.
    ///
    /// **Error Code**: 6
    /// **Recovery**: Fix calling code - this indicates a programming error.
    InvalidParameter(String),

    /// Unknown or unexpected error.
    ///
    /// **Error Code**: 99
    /// **Recovery**: Report as bug with system details and parameters.
    Unknown,

    /// String contains null byte (Rust-specific error).
    ///
    /// This error occurs when converting Rust strings to C strings fails
    /// due to interior null bytes.
    NulByteInString,
}

impl std::fmt::Display for Error {
    /// Format an end-user-facing error message.
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::UnsupportedPlatform => {
                write!(
                    f,
                    "Platform not supported (only macOS and Windows are supported)"
                )
            }
            Error::LaunchFailed => write!(f, "Failed to launch CLAN application"),
            Error::AppNotFound => {
                write!(
                    f,
                    "CLAN application not found - install from https://dali.talkbank.org/clan/"
                )
            }
            Error::SendFailed => write!(f, "Failed to send message to CLAN"),
            Error::Timeout => write!(f, "Operation timed out"),
            Error::InvalidParameter(msg) => write!(f, "Invalid parameter: {}", msg),
            Error::Unknown => write!(f, "Unknown error occurred"),
            Error::NulByteInString => write!(f, "String contains null byte"),
        }
    }
}

impl std::error::Error for Error {}

impl From<NulError> for Error {
    /// Convert `CString` construction failures into invalid-parameter errors.
    fn from(_: NulError) -> Self {
        Error::NulByteInString
    }
}

impl Error {
    /// Returns the numeric error code corresponding to this error.
    ///
    /// Error codes match the C library return values:
    /// - `UnsupportedPlatform` → 1
    /// - `LaunchFailed` → 2
    /// - `AppNotFound` → 3
    /// - `SendFailed` → 4
    /// - `Timeout` → 5
    /// - `InvalidParameter` → 6
    /// - `Unknown` → 99
    /// - `NulByteInString` → 6 (treated as invalid parameter)
    ///
    /// # Examples
    ///
    /// ```
    /// use send2clan::Error;
    ///
    /// let err = Error::AppNotFound;
    /// assert_eq!(err.error_code(), 3);
    /// ```
    pub fn error_code(&self) -> i32 {
        match self {
            Error::UnsupportedPlatform => 1,
            Error::LaunchFailed => 2,
            Error::AppNotFound => 3,
            Error::SendFailed => 4,
            Error::Timeout => 5,
            Error::InvalidParameter(_) | Error::NulByteInString => 6,
            Error::Unknown => 99,
        }
    }

    /// Creates an Error from a C library error code.
    ///
    /// # Safety
    ///
    /// This function assumes the error code is a valid send2clan error code.
    pub(crate) fn from_code(code: c_int) -> Self {
        match code {
            1 => Error::UnsupportedPlatform,
            2 => Error::LaunchFailed,
            3 => Error::AppNotFound,
            4 => Error::SendFailed,
            5 => Error::Timeout,
            6 => Error::InvalidParameter("C library reported invalid parameter".to_string()),
            99 => Error::Unknown,
            _ => Error::Unknown,
        }
    }

    /// Checks if this error might be recoverable with retry logic.
    ///
    /// Some errors (like timeouts or temporary launch failures) might be
    /// recoverable with retry logic, while others (like unsupported platform)
    /// are permanent.
    ///
    /// # Examples
    ///
    /// ```
    /// use send2clan::Error;
    ///
    /// let recoverable = Error::Timeout;
    /// assert!(recoverable.is_recoverable());
    ///
    /// let permanent = Error::UnsupportedPlatform;
    /// assert!(!permanent.is_recoverable());
    /// ```
    pub fn is_recoverable(&self) -> bool {
        matches!(
            self,
            Error::LaunchFailed | Error::SendFailed | Error::Timeout
        )
    }
}

/// Result type alias for send2clan operations.
pub type Result<T> = std::result::Result<T, Error>;
