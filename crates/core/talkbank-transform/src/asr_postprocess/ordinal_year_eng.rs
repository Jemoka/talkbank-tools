//! English ordinal + year + decade expansion. Round 2 of the
//! number-expansion rework documented in
//! `book/src/architecture/number-expansion.md`.
//!
//! Replaces the Python `num2words(N, to="ordinal" | "year")` IPC
//! roundtrip for the only language that realistically hits these
//! modes: `detect_expansion` only flags ordinal/decade tokens via
//! English-style suffixes (`"3rd"`, `"1950s"`), so non-English
//! ASR output never reaches this module.
//!
//! Cross-validated against `num2words` output for ordinals 0-1234
//! and years 1900-2100 (see fixture
//! `data/eng_ordinal_year_fixtures.json`).

/// Cardinal forms 0-19 for composing ordinals 20+ ("twenty-first").
const CARDINAL_TENS: [&str; 10] = [
    "", "ten", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];

/// Irregular ordinal forms for 0-19; index N is the ordinal of N.
const ORDINAL_0_19: [&str; 20] = [
    "zeroth",
    "first",
    "second",
    "third",
    "fourth",
    "fifth",
    "sixth",
    "seventh",
    "eighth",
    "ninth",
    "tenth",
    "eleventh",
    "twelfth",
    "thirteenth",
    "fourteenth",
    "fifteenth",
    "sixteenth",
    "seventeenth",
    "eighteenth",
    "nineteenth",
];

/// Tens-only ordinals (20, 30, ..., 90). Index N is the ordinal of N*10.
const ORDINAL_TENS_MULTIPLE: [&str; 10] = [
    "",
    "tenth",
    "twentieth",
    "thirtieth",
    "fortieth",
    "fiftieth",
    "sixtieth",
    "seventieth",
    "eightieth",
    "ninetieth",
];

const CARDINAL_0_19: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];

/// English ordinal: 0..=9999. Mirrors `num2words(n, to="ordinal", lang="en")`.
///
/// Composition rule:
/// - 0-19: irregular table lookup
/// - 20-99: tens-multiple ordinal OR `cardinal-tens-name + "-" + units-ordinal`
/// - 100-999: `cardinal-N-hundred` then either `"th"` (for `N00`) or
///   `" and "` + sub-100 ordinal
/// - 1000-9999: same shape with thousands
pub fn expand_ordinal_eng(n: u64) -> String {
    if n < 20 {
        return ORDINAL_0_19[n as usize].to_string();
    }
    if n < 100 {
        let tens = (n / 10) as usize;
        let ones = (n % 10) as usize;
        if ones == 0 {
            return ORDINAL_TENS_MULTIPLE[tens].to_string();
        }
        return format!("{}-{}", CARDINAL_TENS[tens], ORDINAL_0_19[ones]);
    }
    if n < 1000 {
        let hundreds = n / 100;
        let remainder = n % 100;
        let head = format!("{} hundred", CARDINAL_0_19[hundreds as usize]);
        if remainder == 0 {
            return format!("{head}th");
        }
        return format!("{head} and {}", expand_ordinal_eng(remainder));
    }
    if n < 10_000 {
        let thousands = n / 1000;
        let remainder = n % 1000;
        let head = format!("{} thousand", CARDINAL_0_19[thousands as usize]);
        if remainder == 0 {
            return format!("{head}th");
        }
        let connector = if remainder < 100 { ", and " } else { ", " };
        return format!("{head}{connector}{}", expand_ordinal_eng(remainder));
    }
    // Beyond 9999, fall back to the cardinal+th pattern; ASR rarely
    // produces ordinals that high, and this avoids a recursive
    // explosion on truly large values.
    format!("{}th", n)
}

/// English year-form: `1950 → "nineteen fifty"`, `2007 → "two
/// thousand and seven"`, `2010 → "twenty ten"`. Mirrors
/// `num2words(n, to="year", lang="en")` for 4-digit years
/// 1100-2999. Years outside that range fall back to the
/// raw integer string.
pub fn expand_year_eng(n: u64) -> String {
    // 2-digit "decade" shorthand ("the 80s") — caller usually
    // routes these via `expand_decade_eng`; keep the year function
    // honest for whatever it gets.
    if n < 100 {
        return cardinal_under_100(n as u32);
    }
    if !(1100..=2999).contains(&n) {
        return n.to_string();
    }
    let century = n / 100;
    let two_low = n % 100;

    // Special cases for 2000-2009: "two thousand", "two thousand and one"
    if century == 20 && two_low < 10 {
        if two_low == 0 {
            return "two thousand".to_string();
        }
        return format!("two thousand and {}", CARDINAL_0_19[two_low as usize]);
    }

    // 1900, 2000, 2100: "<century> hundred"
    if two_low == 0 {
        return format!("{} hundred", cardinal_under_100(century as u32));
    }

    // Default: "<century-pair> <decade-pair>"
    format!(
        "{} {}",
        cardinal_under_100(century as u32),
        cardinal_under_100(two_low as u32)
    )
}

/// English decade: `1950s → "nineteen fifties"`, `80s → "eighties"`.
/// Composes year-form for 4-digit, then pluralizes the trailing
/// decade word per English rule (`-y` → `-ies`, otherwise `-s`).
pub fn expand_decade_eng(n: u64) -> String {
    if n < 100 {
        // 2-digit shorthand: "80s" → "eighties"
        let cardinal = cardinal_under_100(n as u32);
        return pluralize_last_word(&cardinal);
    }
    let year = expand_year_eng(n);
    pluralize_last_word(&year)
}

fn cardinal_under_100(n: u32) -> String {
    if n < 20 {
        return CARDINAL_0_19[n as usize].to_string();
    }
    let tens = (n / 10) as usize;
    let ones = (n % 10) as usize;
    if ones == 0 {
        return CARDINAL_TENS[tens].to_string();
    }
    format!("{}-{}", CARDINAL_TENS[tens], CARDINAL_0_19[ones])
}

/// Pluralize the last whitespace-separated word per English decade
/// rule: trailing `-y` → `-ies`; trailing `hundred` → `hundreds`;
/// otherwise append `s`. Mirrors `_pluralize_decade` in the
/// soon-to-be-deleted `_number_expansion.py`.
fn pluralize_last_word(s: &str) -> String {
    let mut parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return s.to_string();
    }
    // is_empty guard above ensures pop() returns Some.
    #[allow(clippy::unwrap_used)]
    let last = parts.pop().unwrap();
    let pluralized = if let Some(stem) = last.strip_suffix('y') {
        format!("{stem}ies")
    } else {
        format!("{last}s")
    };
    if parts.is_empty() {
        pluralized
    } else {
        format!("{} {}", parts.join(" "), pluralized)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde::Deserialize;

    #[derive(Deserialize)]
    struct Fixtures {
        ordinal: std::collections::BTreeMap<String, String>,
        year: std::collections::BTreeMap<String, String>,
    }

    fn load_fixtures() -> Fixtures {
        let raw = include_str!("../../data/eng_ordinal_year_fixtures.json");
        serde_json::from_str(raw).expect("parse fixture")
    }

    /// Cross-check every ordinal in the fixture file against the
    /// Rust implementation. Any divergence from `num2words` output
    /// is a bug — we promise behavioural parity for the values
    /// `detect_expansion` realistically routes through this path.
    #[test]
    fn ordinals_match_num2words_fixture() {
        let f = load_fixtures();
        for (k, expected) in &f.ordinal {
            let n: u64 = k.parse().expect("numeric key");
            let actual = expand_ordinal_eng(n);
            assert_eq!(
                &actual, expected,
                "ordinal({n}) — Rust: {actual:?}, num2words: {expected:?}"
            );
        }
    }

    /// Same cross-check for year forms over 1900-2100.
    #[test]
    fn years_match_num2words_fixture() {
        let f = load_fixtures();
        for (k, expected) in &f.year {
            let n: u64 = k.parse().expect("numeric key");
            let actual = expand_year_eng(n);
            assert_eq!(
                &actual, expected,
                "year({n}) — Rust: {actual:?}, num2words: {expected:?}"
            );
        }
    }

    /// Decade composition: year + pluralize. Spot-check the cases
    /// users will actually see (Whisper "1950s" / "80s" output).
    #[test]
    fn decade_pluralizes_year_form() {
        assert_eq!(expand_decade_eng(1950), "nineteen fifties");
        assert_eq!(expand_decade_eng(1920), "nineteen twenties");
        assert_eq!(expand_decade_eng(1900), "nineteen hundreds");
        assert_eq!(expand_decade_eng(2010), "twenty tens");
        // 2-digit shorthand
        assert_eq!(expand_decade_eng(80), "eighties");
        assert_eq!(expand_decade_eng(20), "twenties");
        assert_eq!(expand_decade_eng(10), "tens");
    }

    /// Boundary-condition tests beyond the fixture range. Larger
    /// values should still produce sensible output, even if
    /// num2words would format them differently — we promise no
    /// crash, no allocation explosion.
    #[test]
    fn ordinal_large_values_dont_crash() {
        let _ = expand_ordinal_eng(10_000);
        let _ = expand_ordinal_eng(1_000_000);
        let _ = expand_ordinal_eng(u64::MAX);
    }
}
