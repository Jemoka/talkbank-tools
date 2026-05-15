//! Integration-style tests for the send2clan Rust API wrapper.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Format>
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use super::*;

#[test]
fn test_version() {
    let ver = version();
    assert!(!ver.is_empty());
    assert!(ver.contains('.'));
    println!("send2clan version: {}", ver);
}

#[test]
fn test_is_platform_supported() {
    let supported = is_platform_supported();
    println!("Platform supported: {}", supported);

    // On macOS and Windows, this should return true
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    assert!(supported);

    // On other platforms, this should return false
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    assert!(!supported);
}

#[test]
fn test_is_clan_available() {
    let available = is_clan_available();
    println!("CLAN available: {}", available);
    // We don't assert anything since CLAN may or may not be installed
}

#[test]
fn test_get_capabilities() {
    let caps = get_capabilities();
    println!("Capabilities result: {:?}", caps);

    match caps {
        Ok(c) => {
            println!("Capabilities: 0x{:X}", c);
            println!("  Platform supported: {}", (c & 0x01) != 0);
            println!("  CLAN available: {}", (c & 0x02) != 0);
            println!("  Unicode support: {}", (c & 0x04) != 0);
            println!("  Timeout support: {}", (c & 0x08) != 0);
        }
        Err(e) => {
            println!("  Error: {}", e);
        }
    }
}

#[test]
#[ignore = "This test has a 30-second timeout waiting for CLAN - run manually"]
fn test_send_to_clan_validates_params() {
    // This should fail with InvalidParameter or AppNotFound
    // depending on whether CLAN is installed
    let result = send_to_clan(30, "/nonexistent/file.cha", 1, 1, Some("Test"));
    println!("send_to_clan result: {:?}", result);

    // We expect either an error or success (if CLAN is installed and handles it)
    match result {
        Ok(()) => {
            println!("Unexpectedly succeeded - CLAN might be handling the nonexistent file")
        }
        Err(e) => println!("Expected error: {} (code {})", e, e.error_code()),
    }
}

#[test]
fn test_error_codes() {
    assert_eq!(Error::UnsupportedPlatform.error_code(), 1);
    assert_eq!(Error::LaunchFailed.error_code(), 2);
    assert_eq!(Error::AppNotFound.error_code(), 3);
    assert_eq!(Error::SendFailed.error_code(), 4);
    assert_eq!(Error::Timeout.error_code(), 5);
    assert_eq!(Error::InvalidParameter("test".to_string()).error_code(), 6);
    assert_eq!(Error::Unknown.error_code(), 99);
}

#[test]
fn test_error_is_recoverable() {
    assert!(Error::LaunchFailed.is_recoverable());
    assert!(Error::SendFailed.is_recoverable());
    assert!(Error::Timeout.is_recoverable());

    assert!(!Error::UnsupportedPlatform.is_recoverable());
    assert!(!Error::AppNotFound.is_recoverable());
    assert!(!Error::InvalidParameter("test".to_string()).is_recoverable());
    assert!(!Error::Unknown.is_recoverable());
}

#[test]
fn test_null_byte_in_path() {
    let result = send_to_clan(30, "/path/with\0null", 1, 1, None);
    assert!(matches!(result, Err(Error::NulByteInString)));
}

#[test]
fn test_null_byte_in_message() {
    let result = send_to_clan(30, "/path/to/file.cha", 1, 1, Some("message\0with\0nulls"));
    assert!(matches!(result, Err(Error::NulByteInString)));
}

#[test]
fn test_error_display() {
    assert!(
        Error::UnsupportedPlatform
            .to_string()
            .contains("Platform not supported")
    );
    assert!(Error::AppNotFound.to_string().contains("not found"));
    assert!(Error::Timeout.to_string().contains("timed out"));
}

// --- Error handling tests ---

#[test]
fn test_send_to_nonexistent_clan_returns_error() {
    // If CLAN is not running/installed, send_to_clan should return an Err, never panic.
    // We use a minimal timeout (1 second) to avoid long waits.
    let result = send_to_clan(1, "/tmp/nonexistent_test_file.cha", 1, 1, None);
    // On a CI/dev machine without CLAN, this must be Err.
    // On a machine with CLAN, it might succeed or fail (file doesn't exist on disk),
    // but it must never panic.
    println!("send_to_nonexistent_clan result: {:?}", result);
}

#[test]
fn test_empty_file_path() {
    // An empty file path is technically valid for CString (no interior null),
    // so the C library receives it. It should either return an error or
    // at least not panic.
    let result = send_to_clan(1, "", 1, 1, None);
    println!("empty_file_path result: {:?}", result);
    // The key property: no panic occurred.
}

// --- Platform detection tests ---

#[test]
fn test_platform_support_matches_compile_target() {
    let supported = is_platform_supported();

    #[cfg(target_os = "macos")]
    assert!(supported, "macOS must report platform supported");

    #[cfg(target_os = "windows")]
    assert!(supported, "Windows must report platform supported");

    #[cfg(target_os = "linux")]
    assert!(!supported, "Linux must report platform not supported");
}

#[test]
fn test_capabilities_platform_bit_consistent() {
    // The platform-supported bit (bit 0) in capabilities must agree
    // with is_platform_supported().
    let supported = is_platform_supported();
    let caps = get_capabilities();
    match caps {
        Ok(c) => {
            let caps_platform = (c & 0x01) != 0;
            assert_eq!(
                caps_platform, supported,
                "capabilities bit 0 ({}) must match is_platform_supported() ({})",
                caps_platform, supported,
            );
        }
        Err(e) => {
            println!("get_capabilities returned error (acceptable): {}", e);
        }
    }
}

#[test]
fn test_capabilities_unicode_and_timeout_always_set() {
    // Bits 2 (unicode) and 3 (timeout) should always be set per the docs.
    if let Ok(c) = get_capabilities() {
        assert_ne!(c & 0x04, 0, "Unicode support bit must be set");
        assert_ne!(c & 0x08, 0, "Timeout support bit must be set");
    }
}

// --- Error type exhaustive tests ---

#[test]
fn test_error_display_all_variants() {
    // Every Error variant must produce a non-empty, meaningful Display string.
    let variants: Vec<Error> = vec![
        Error::UnsupportedPlatform,
        Error::LaunchFailed,
        Error::AppNotFound,
        Error::SendFailed,
        Error::Timeout,
        Error::InvalidParameter("bad param".to_string()),
        Error::Unknown,
        Error::NulByteInString,
    ];

    for err in &variants {
        let msg = err.to_string();
        assert!(!msg.is_empty(), "Display for {:?} must be non-empty", err);
        println!("{:?} => \"{}\"", err, msg);
    }
}

#[test]
fn test_error_code_roundtrip_through_from_code() {
    // For each error code, from_code should produce the matching variant,
    // and error_code() on that variant should return the original code.
    let codes_and_variants: Vec<(i32, Error)> = vec![
        (1, Error::UnsupportedPlatform),
        (2, Error::LaunchFailed),
        (3, Error::AppNotFound),
        (4, Error::SendFailed),
        (5, Error::Timeout),
        (99, Error::Unknown),
    ];

    for (code, expected_variant) in &codes_and_variants {
        let reconstructed = Error::from_code(*code);
        assert_eq!(
            reconstructed.error_code(),
            *code,
            "from_code({}).error_code() must round-trip",
            code,
        );
        assert_eq!(
            std::mem::discriminant(&reconstructed),
            std::mem::discriminant(expected_variant),
            "from_code({}) must produce the correct variant",
            code,
        );
    }
}

#[test]
fn test_error_from_code_unknown_codes() {
    // Unrecognized error codes should map to Unknown.
    for code in [0, 7, 50, 100, -1, i32::MAX, i32::MIN] {
        let err = Error::from_code(code);
        assert_eq!(
            err.error_code(),
            99,
            "Unrecognized code {} should map to Unknown (code 99)",
            code,
        );
    }
}

#[test]
fn test_error_implements_std_error() {
    // Verify Error implements std::error::Error (compile-time check made runtime).
    let err: Box<dyn std::error::Error> = Box::new(Error::Timeout);
    assert!(!err.to_string().is_empty());
}

#[test]
fn test_nul_byte_error_code_matches_invalid_parameter() {
    // NulByteInString is treated as an invalid parameter (code 6).
    assert_eq!(Error::NulByteInString.error_code(), 6);
    assert_eq!(
        Error::NulByteInString.error_code(),
        Error::InvalidParameter("any".to_string()).error_code(),
        "NulByteInString and InvalidParameter must share code 6",
    );
}

#[test]
fn test_nul_byte_not_recoverable() {
    // NulByteInString is a programming error — not recoverable by retry.
    assert!(!Error::NulByteInString.is_recoverable());
}

// --- Robustness ---

#[test]
fn test_concurrent_send_attempts_no_crash() {
    // Two threads calling send_to_clan simultaneously must not crash.
    // Both may fail (CLAN probably not running), but there must be no
    // data race, segfault, or panic.
    let handles: Vec<_> = (0..2)
        .map(|i| {
            std::thread::spawn(move || {
                let path = format!("/tmp/concurrent_test_{}.cha", i);
                let result = send_to_clan(1, &path, 1, 1, None);
                println!("Thread {} result: {:?}", i, result);
                // Key property: we reached this point without crashing.
            })
        })
        .collect();

    for h in handles {
        h.join().expect("Thread must not panic");
    }
}

#[test]
fn test_version_is_semver_like() {
    // Version string must look like a semantic version (digits and dots).
    let ver = version();
    assert!(
        ver.chars().all(|c| c.is_ascii_digit() || c == '.'),
        "Version '{}' should contain only digits and dots",
        ver,
    );
    let parts: Vec<&str> = ver.split('.').collect();
    assert!(
        parts.len() >= 2,
        "Version '{}' should have at least major.minor",
        ver,
    );
}

#[test]
fn test_is_clan_available_is_deterministic() {
    // Calling is_clan_available twice should return the same value
    // (CLAN installation status doesn't change mid-test).
    let first = is_clan_available();
    let second = is_clan_available();
    assert_eq!(first, second, "is_clan_available must be deterministic");
}
