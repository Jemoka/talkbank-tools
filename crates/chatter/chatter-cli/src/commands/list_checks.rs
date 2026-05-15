//! Implementation of `chatter validate --list-checks`.
//!
//! Prints every known error code together with its implementation status
//! (Active vs Planned). This gives users and successors a machine-readable
//! view of which validation checks the running binary enforces and which
//! are only documented in `resources/spec/errors/`.
//!
//! The list of Planned (not_implemented) codes is hard-coded in
//! [`PLANNED_CODES`] below. It was derived from
//! `grep -l "Status.*not_implemented" resources/spec/errors/*.md` and must be kept
//! in sync when a spec changes from `not_implemented` to `implemented`.

use talkbank_model::ErrorCode;

/// Implementation status of an error check.
///
/// This is a closed, two-state enum because every spec in `resources/spec/errors/`
/// is either `implemented` (we call it [`Active`]) or `not_implemented`
/// (we call it [`Planned`]). Any other status is a spec bug.
///
/// [`Active`]: CheckStatus::Active
/// [`Planned`]: CheckStatus::Planned
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CheckStatus {
    /// The check is active and fires when the error condition is detected.
    Active,
    /// The check is documented but not yet enforced by the validator.
    Planned,
}

/// Error codes whose spec is currently marked `Status: not_implemented`.
///
/// Derived from `resources/spec/errors/*.md`. Keep sorted for easy auditing.
///
/// Source of truth: `grep -l "Status.*not_implemented" resources/spec/errors/*.md`.
const PLANNED_CODES: &[&str] = &[
    "E003", "E101", "E208", "E212", "E214", "E245", "E246", "E251", "E252", "E302", "E303", "E309",
    "E310", "E311", "E312", "E319", "E320", "E321", "E322", "E323", "E325", "E331", "E341", "E342",
    "E344", "E346", "E348", "E351", "E352", "E353", "E354", "E355", "E360", "E364", "E365", "E370",
    "E404", "E531", "E702", "E708", "E709", "E711", "E720",
];

/// Returns the implementation status of a given error code.
///
/// Performs a linear scan over [`PLANNED_CODES`]. The list is small
/// (~50 entries) so a hashmap would be overkill.
pub fn check_status(code: ErrorCode) -> CheckStatus {
    if PLANNED_CODES.contains(&code.as_str()) {
        CheckStatus::Planned
    } else {
        CheckStatus::Active
    }
}

/// Every error code the binary knows about, in declaration order.
///
/// This intentionally relies on `ErrorCode::iter()` from the
/// `#[error_code_enum]` macro, which guarantees one entry per enum
/// variant. `PLANNED_CODES` remains the small manual overlay for specs
/// that are documented but not yet enforced.
pub fn all_error_codes() -> Vec<ErrorCode> {
    ErrorCode::iter().copied().collect()
}

/// Print the list of all error checks with their status.
///
/// Output format is stable-ish but intended for human consumption. It is
/// deliberately NOT machine-parseable JSON — downstream tooling should read
/// the spec files directly instead.
pub fn print_check_list() {
    let mut codes = all_error_codes();
    codes.sort_by_key(|c| c.as_str());

    let active_count = codes
        .iter()
        .filter(|c| check_status(**c) == CheckStatus::Active)
        .count();
    let planned_count = codes.len() - active_count;

    println!("Validation checks (Active / Planned):");
    println!();
    for code in &codes {
        let (badge, label) = match check_status(*code) {
            CheckStatus::Active => ("[Active] ", "Active"),
            CheckStatus::Planned => ("[Planned]", "Planned"),
        };
        // Debug print of the variant gives the canonical Rust name
        // (e.g., `UnclosedBracket`) which is more informative than the
        // raw code alone.
        println!("  {}  {}  {:?}  ({})", badge, code.as_str(), code, label);
    }
    println!();
    println!(
        "Total: {} checks ({} Active, {} Planned)",
        codes.len(),
        active_count,
        planned_count
    );
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_error_codes_is_nonempty() {
        assert!(!all_error_codes().is_empty());
    }

    #[test]
    fn planned_codes_are_known_variants() {
        // Every string in PLANNED_CODES must correspond to a real variant.
        // ErrorCode::new() falls back to UnknownError for unknown codes,
        // so anything that round-trips through as_str() is genuine.
        for raw in PLANNED_CODES {
            let code = ErrorCode::new(raw);
            assert_ne!(
                code,
                ErrorCode::UnknownError,
                "PLANNED_CODES contains unknown code {:?}",
                raw
            );
            assert_eq!(code.as_str(), *raw);
        }
    }

    #[test]
    fn status_lookup_matches_planned_list() {
        assert_eq!(
            check_status(ErrorCode::UnparsableUtterance),
            CheckStatus::Planned
        );
        assert_eq!(
            check_status(ErrorCode::MissingColonAfterSpeaker),
            CheckStatus::Planned
        );
        // E201 is not in PLANNED_CODES -> should be Active (and the variant
        // itself is UnknownError since E201 isn't defined; use a known
        // Active one instead).
        assert_eq!(
            check_status(ErrorCode::MorCountMismatchTooFew),
            CheckStatus::Active
        );
    }
}
