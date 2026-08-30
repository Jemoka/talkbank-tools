//! Chinese number-to-word conversion.
//!
//! Converts integers to Chinese character representations (simplified or
//! traditional). Supports values up to 10^48. Ported from the Python
//! `num2chinese` function in `batchalign/pipelines/asr/num2chinese.py`.

/// Selects Chinese character variant for number-to-text conversion.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChineseScript {
    /// Simplified characters (简体字). Used for cmn (Mandarin).
    Simplified,
    /// Traditional characters (繁體字). Used for yue (Cantonese).
    Traditional,
}

/// Convert a non-negative integer to Chinese characters.
///
/// # Arguments
/// * `num` - The number to convert (must be non-negative, < 10^48).
/// * `script` - Character variant (simplified for Mandarin, traditional for Cantonese).
///
/// # Panics
/// Panics if `num` is >= 10^48.
pub fn num2chinese(num: u64, script: ChineseScript) -> String {
    if num == 0 {
        return "零".to_string();
    }

    let c_basic: &[char] = &['零', '一', '二', '三', '四', '五', '六', '七', '八', '九'];
    let c_unit1: &[&str] = &["十", "百", "千"];
    let c_unit2: &[&str] = if script == ChineseScript::Simplified {
        &[
            "万", "亿", "兆", "京", "垓", "秭", "穰", "沟", "涧", "正", "载",
        ]
    } else {
        &[
            "萬", "億", "兆", "京", "垓", "秭", "穰", "溝", "澗", "正", "載",
        ]
    };

    // Split into groups of 4 digits from the right
    let s = num.to_string();
    let digits: Vec<u8> = s.bytes().map(|b| b - b'0').collect();

    // Pad so length is multiple of 4
    let mut groups: Vec<&[u8]> = Vec::new();
    let remainder = digits.len() % 4;
    let mut start = 0;
    if remainder > 0 {
        groups.push(&digits[0..remainder]);
        start = remainder;
    }
    while start < digits.len() {
        groups.push(&digits[start..start + 4]);
        start += 4;
    }

    let total_groups = groups.len();
    let mut parts: Vec<String> = Vec::new();
    let mut prev_group_was_zero = false;

    for (gi, group) in groups.iter().enumerate() {
        let group_val: u64 = group.iter().fold(0u64, |acc, &d| acc * 10 + d as u64);
        let unit_idx = total_groups - 1 - gi; // index into c_unit2 (0 = no unit)

        if group_val == 0 {
            if !parts.is_empty() {
                prev_group_was_zero = true;
            }
            continue;
        }

        // Add a leading zero if the previous group was zero (or this group
        // has leading zeros and there are preceding groups)
        if prev_group_was_zero {
            parts.push("零".to_string());
            prev_group_was_zero = false;
        }

        // Convert the 4-digit group
        let group_str = convert_group(group, c_basic, c_unit1, !parts.is_empty());

        parts.push(group_str);

        // Add the large unit (万, 亿, etc.)
        if unit_idx > 0 {
            parts.push(c_unit2[unit_idx - 1].to_string());
        }
    }

    parts.join("")
}

/// Convert a group of up to 4 digits to Chinese characters.
///
/// `has_preceding` is true when there are higher-order groups before this one,
/// meaning leading zeros in this group should emit a "零" placeholder.
fn convert_group(digits: &[u8], c_basic: &[char], c_unit1: &[&str], has_preceding: bool) -> String {
    let mut parts: Vec<String> = Vec::new();
    let len = digits.len();
    let mut has_zero_run = false;

    for (i, &d) in digits.iter().enumerate() {
        let pos_from_right = len - 1 - i; // 0=ones, 1=tens, 2=hundreds, 3=thousands

        if d == 0 {
            // Track zero runs — emit one zero placeholder if followed by non-zero
            if has_preceding || !parts.is_empty() {
                has_zero_run = true;
            }
            continue;
        }

        // Emit zero placeholder if we skipped zeros
        if has_zero_run {
            parts.push("零".to_string());
            has_zero_run = false;
        }

        if pos_from_right == 0 {
            // Ones position — just the digit
            parts.push(c_basic[d as usize].to_string());
        } else if pos_from_right == 1 && d == 1 && parts.is_empty() && !has_preceding {
            // Tens position with "1" at the start — just "十" (not "一十")
            // Only for standalone numbers (not when preceded by higher groups)
            parts.push(c_unit1[0].to_string());
        } else {
            // Normal digit + unit
            parts.push(format!(
                "{}{}",
                c_basic[d as usize],
                c_unit1[pos_from_right - 1]
            ));
        }
    }

    parts.join("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_zero() {
        assert_eq!(num2chinese(0, ChineseScript::Simplified), "零");
    }

    #[test]
    fn test_single_digits() {
        assert_eq!(num2chinese(1, ChineseScript::Simplified), "一");
        assert_eq!(num2chinese(5, ChineseScript::Simplified), "五");
        assert_eq!(num2chinese(9, ChineseScript::Simplified), "九");
    }

    #[test]
    fn test_teens() {
        assert_eq!(num2chinese(10, ChineseScript::Simplified), "十");
        assert_eq!(num2chinese(11, ChineseScript::Simplified), "十一");
        assert_eq!(num2chinese(12, ChineseScript::Simplified), "十二");
    }

    #[test]
    fn test_tens() {
        assert_eq!(num2chinese(20, ChineseScript::Simplified), "二十");
        assert_eq!(num2chinese(21, ChineseScript::Simplified), "二十一");
        assert_eq!(num2chinese(42, ChineseScript::Simplified), "四十二");
    }

    #[test]
    fn test_hundreds() {
        assert_eq!(num2chinese(100, ChineseScript::Simplified), "一百");
        assert_eq!(num2chinese(123, ChineseScript::Simplified), "一百二十三");
    }

    #[test]
    fn test_thousands() {
        assert_eq!(num2chinese(1000, ChineseScript::Simplified), "一千");
        assert_eq!(
            num2chinese(1234, ChineseScript::Simplified),
            "一千二百三十四"
        );
    }

    /// Golden test: matches Python `num2chinese` output.
    #[test]
    fn test_golden_simplified() {
        assert_eq!(num2chinese(10000, ChineseScript::Simplified), "一万");
        assert_eq!(
            num2chinese(99999, ChineseScript::Simplified),
            "九万九千九百九十九"
        );
    }

    /// Golden test: traditional Chinese.
    #[test]
    fn test_golden_traditional() {
        assert_eq!(num2chinese(10000, ChineseScript::Traditional), "一萬");
        assert_eq!(
            num2chinese(99999, ChineseScript::Traditional),
            "九萬九千九百九十九"
        );
    }

    #[test]
    fn test_with_zeros() {
        assert_eq!(num2chinese(101, ChineseScript::Simplified), "一百零一");
        assert_eq!(num2chinese(1001, ChineseScript::Simplified), "一千零一");
        assert_eq!(num2chinese(10001, ChineseScript::Simplified), "一万零一");
    }

    // --- property tests ---

    use proptest::prelude::*;

    fn script_strategy() -> impl Strategy<Value = ChineseScript> {
        prop_oneof![
            Just(ChineseScript::Simplified),
            Just(ChineseScript::Traditional),
        ]
    }

    proptest! {
        /// Output never contains ASCII digits.
        #[test]
        fn no_ascii_digits_in_output(n in 0..1_000_000u64, script in script_strategy()) {
            let result = num2chinese(n, script);
            prop_assert!(
                !result.chars().any(|c| c.is_ascii_digit()),
                "ASCII digit in output for {}: '{}'", n, result
            );
        }

        /// Output is never empty.
        #[test]
        fn output_never_empty(n in 0..1_000_000u64, script in script_strategy()) {
            let result = num2chinese(n, script);
            prop_assert!(!result.is_empty(), "Empty output for {}", n);
        }

        /// Simplified and Traditional agree for small numbers (0-9999).
        /// They only differ at ≥10000 (万/萬, 亿/億, etc.).
        #[test]
        fn scripts_agree_below_10000(n in 0..10_000u64) {
            let s = num2chinese(n, ChineseScript::Simplified);
            let t = num2chinese(n, ChineseScript::Traditional);
            prop_assert_eq!(
                &s, &t,
                "Scripts differ below 10000 for {}: '{}' vs '{}'", n, s, t
            );
        }

        /// Simplified and Traditional differ for numbers ≥10000.
        #[test]
        fn scripts_differ_at_10000_plus(n in 10_000..100_000u64) {
            let s = num2chinese(n, ChineseScript::Simplified);
            let t = num2chinese(n, ChineseScript::Traditional);
            prop_assert_ne!(
                &s, &t,
                "Scripts should differ at {} but both produced '{}'", n, s
            );
        }
    }
}
