//! CLAN argument pre-processor.
//!
//! Rewrites legacy CLAN `+flag`/`-flag` syntax into modern `--flag` equivalents
//! so that clap can parse them. This allows users to write either:
//!
//! ```text
//! clan analyze freq +t*CHI +s"want" +z25-125 file.cha
//! ```
//!
//! or the modern equivalent:
//!
//! ```text
//! clan analyze freq --speaker CHI --include-word want --range 25-125 file.cha
//! ```
//!
//! The rewriter is a pure function that operates on the raw argument list before
//! clap sees it. It only touches arguments that look like CLAN flags (`+` or `-`
//! prefix followed by a known flag letter); everything else passes through unchanged.

/// Rewrite CLAN-style `+flag`/`-flag` arguments into modern `--flag` equivalents.
///
/// The function scans `args` for patterns like `+t*CHI`, `+s"word"`, `+z25-125`,
/// etc., and replaces them with `--speaker CHI`, `--include-word word`,
/// `--range 25-125`, etc. Unrecognised arguments pass through unchanged.
///
/// This is intentionally applied to the full argument list (including the binary
/// name and subcommand tokens). Subcommand names like `analyze`, `freq`, etc.
/// never start with `+` or `-` followed by a CLAN flag letter, so they are
/// never matched.
///
/// The rewriter is context-aware for the `check` subcommand: `+g1`–`+g5` are
/// CHECK generic options (not gem labels), so they are rewritten to
/// `--check-target`, `--check-id`, `--check-unused` etc. For all other
/// subcommands, `+g` is gem filtering as usual.
pub fn rewrite_clan_args(args: &[String]) -> Vec<String> {
    // Detect if the subcommand is "check" by scanning for it in the args.
    let is_check = args.iter().any(|a| a == "check");

    let mut out = Vec::with_capacity(args.len());
    let mut i = 0;

    while i < args.len() {
        let arg = &args[i];

        // Only attempt rewriting on args starting with + or - that look like
        // CLAN flags (second char is a known flag letter, not a digit or '-').
        if let Some(rewritten) = try_rewrite_clan_flag(arg, is_check) {
            out.extend(rewritten);
            i += 1;
            continue;
        }

        // Pass through unchanged.
        out.push(arg.clone());
        i += 1;
    }

    out
}

/// Attempt to rewrite a single CLAN-style argument.
///
/// Returns `Some(vec![...])` with the replacement tokens, or `None` if the
/// argument is not a recognised CLAN flag.
fn try_rewrite_clan_flag(arg: &str, is_check: bool) -> Option<Vec<String>> {
    let bytes = arg.as_bytes();
    if bytes.len() < 2 {
        return None;
    }

    let polarity = bytes[0];
    if polarity != b'+' && polarity != b'-' {
        return None;
    }

    let flag_char = bytes[1];
    let rest = &arg[2..];

    match (polarity, flag_char) {
        // +t*CHI / -t*CHI — speaker include/exclude
        (b'+', b't') | (b'-', b't') => rewrite_tier_speaker(polarity, rest),

        // +s"word" / +sword / -s"word" / -sword — word include/exclude
        (b'+', b's') | (b'-', b's') => rewrite_search_word(polarity, rest),

        // +g: For CHECK, +g1–+g5 are generic options; otherwise gem filtering
        (b'+', b'g') if is_check => rewrite_check_generic(rest),
        (b'+', b'g') | (b'-', b'g') => rewrite_gem(polarity, rest),

        // +z25-125 — utterance range
        (b'+', b'z') => rewrite_range(rest),

        // +r6 — include retracings
        (b'+', b'r') if rest == "6" => Some(vec!["--include-retracings".into()]),

        // +u: For CHECK, +u means validate UD features; for other commands, merge speakers (no-op)
        (b'+', b'u') if rest.is_empty() && is_check => Some(vec!["--check-ud".into()]),
        (b'+', b'u') if rest.is_empty() => Some(vec![]),

        // +dN — display mode
        (b'+', b'd') => rewrite_display_mode(rest),

        // +k — case sensitive
        (b'+', b'k') if rest.is_empty() => Some(vec!["--case-sensitive".into()]),

        // +fEXT — output extension
        (b'+', b'f') if !rest.is_empty() => Some(vec!["--output-ext".into(), rest.to_string()]),

        // +wN / -wN — context window
        (b'+', b'w') => rewrite_context_window("+w", rest),
        (b'-', b'w') => rewrite_context_window("-w", rest),

        // CHECK-specific flags
        // +cN — bullet check level
        (b'+', b'c') if !rest.is_empty() => Some(vec!["--bullets".into(), rest.to_string()]),
        // +eN — include error / +e — list errors
        (b'+', b'e') => rewrite_check_error(rest),
        // -eN — exclude error
        (b'-', b'e') if !rest.is_empty() => Some(vec!["--exclude-error".into(), rest.to_string()]),

        _ => None,
    }
}

/// Rewrite `+t*CHI` → `--speaker CHI`, `-t*MOT` → `--exclude-speaker MOT`,
/// `+t%mor` → `--tier mor`, `-t%gra` → `--exclude-tier gra`.
fn rewrite_tier_speaker(polarity: u8, rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }

    match rest.as_bytes()[0] {
        b'*' => {
            let speaker = &rest[1..];
            if speaker.is_empty() {
                return None;
            }
            let flag = if polarity == b'+' {
                "--speaker"
            } else {
                "--exclude-speaker"
            };
            Some(vec![flag.into(), speaker.to_string()])
        }
        b'%' => {
            let tier = &rest[1..];
            if tier.is_empty() {
                return None;
            }
            let flag = if polarity == b'+' {
                "--tier"
            } else {
                "--exclude-tier"
            };
            Some(vec![flag.into(), tier.to_string()])
        }
        b'@' => {
            // +t@ID="eng|*|CHI|*" → --id-filter "eng|*|CHI|*"
            if rest.len() >= 4 && rest[1..].starts_with("ID=") {
                let value = strip_quotes(&rest[4..]);
                if value.is_empty() {
                    return None;
                }
                Some(vec!["--id-filter".into(), value])
            } else {
                None
            }
        }
        _ => None,
    }
}

/// Rewrite `+s"word"` or `+sword` → `--include-word word`,
/// `-s"word"` or `-sword` → `--exclude-word word`.
fn rewrite_search_word(polarity: u8, rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }
    let word = strip_quotes(rest);
    if word.is_empty() {
        return None;
    }
    let flag = if polarity == b'+' {
        "--include-word"
    } else {
        "--exclude-word"
    };
    Some(vec![flag.into(), word])
}

/// Rewrite `+glabel` → `--gem label`, `-glabel` → `--exclude-gem label`.
fn rewrite_gem(polarity: u8, rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }
    let label = strip_quotes(rest);
    if label.is_empty() {
        return None;
    }
    let flag = if polarity == b'+' {
        "--gem"
    } else {
        "--exclude-gem"
    };
    Some(vec![flag.into(), label])
}

/// Rewrite `+z25-125` → `--range 25-125`.
fn rewrite_range(rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }
    Some(vec!["--range".into(), rest.to_string()])
}

/// Rewrite `+dN` → `--display-mode N`.
fn rewrite_display_mode(rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }
    // Validate that rest is a number
    if rest.chars().all(|c| c.is_ascii_digit()) {
        Some(vec!["--display-mode".into(), rest.to_string()])
    } else {
        None
    }
}

/// Rewrite `+wN` → `--context-after N`, `-wN` → `--context-before N`.
fn rewrite_context_window(prefix: &str, rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        return None;
    }
    if !rest.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let flag = if prefix == "+w" {
        "--context-after"
    } else {
        "--context-before"
    };
    Some(vec![flag.into(), rest.to_string()])
}

/// Strip surrounding double quotes from a string value.
fn strip_quotes(s: &str) -> String {
    let s = s.trim();
    if s.len() >= 2 && s.starts_with('"') && s.ends_with('"') {
        s[1..s.len() - 1].to_string()
    } else {
        s.to_string()
    }
}

/// Rewrite CHECK's `+g1`–`+g5` generic options.
///
/// | Flag | Meaning |
/// |------|---------|
/// | `+g1` | Check prosodic delimiters (no-op — always on) |
/// | `+g2` | Check CHI has Target_Child role |
/// | `+g3` | Word detail checks (partially implemented via parser) |
/// | `+g4` | Check for missing @ID tiers (on by default) |
/// | `+g5` | Check for unused speakers |
///
/// Falls back to gem rewriting if the rest is not a single digit 1–5.
fn rewrite_check_generic(rest: &str) -> Option<Vec<String>> {
    match rest {
        "1" => Some(vec![]), // no-op: prosodic delimiters always recognized
        "2" => Some(vec!["--check-target".into()]),
        "3" => Some(vec![]), // no-op: word checks via parser
        "4" => Some(vec!["--check-id".into(), "true".into()]),
        "5" => Some(vec!["--check-unused".into()]),
        // Not a CHECK generic option — fall back to gem
        _ => rewrite_gem(b'+', rest),
    }
}

/// Rewrite `+eN` → `--error N`, `+e` → `--list-errors`.
fn rewrite_check_error(rest: &str) -> Option<Vec<String>> {
    if rest.is_empty() {
        Some(vec!["--list-errors".into()])
    } else {
        Some(vec!["--error".into(), rest.to_string()])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(s: &str) -> Vec<String> {
        s.split_whitespace().map(String::from).collect()
    }

    #[test]
    fn speaker_include() {
        let input = args("clan analyze freq +t*CHI file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --speaker CHI file.cha"));
    }

    #[test]
    fn speaker_exclude() {
        let input = args("clan analyze freq -t*MOT file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --exclude-speaker MOT file.cha")
        );
    }

    #[test]
    fn multiple_speakers() {
        let input = args("clan analyze freq +t*CHI +t*MOT file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --speaker CHI --speaker MOT file.cha")
        );
    }

    #[test]
    fn tier_include() {
        let input = args("clan analyze freq +t%mor file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --tier mor file.cha"));
    }

    #[test]
    fn tier_exclude() {
        let input = args("clan analyze freq -t%gra file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --exclude-tier gra file.cha")
        );
    }

    #[test]
    fn search_word_quoted() {
        let input: Vec<String> = vec![
            "clan".into(),
            "analyze".into(),
            "freq".into(),
            "+s\"want\"".into(),
            "file.cha".into(),
        ];
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --include-word want file.cha")
        );
    }

    #[test]
    fn search_word_unquoted() {
        let input = args("clan analyze freq +swant file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --include-word want file.cha")
        );
    }

    #[test]
    fn exclude_word() {
        let input = args("clan analyze freq -swant file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --exclude-word want file.cha")
        );
    }

    #[test]
    fn gem_include() {
        let input = args("clan analyze freq +gstory file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --gem story file.cha"));
    }

    #[test]
    fn gem_exclude() {
        let input = args("clan analyze freq -gstory file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --exclude-gem story file.cha")
        );
    }

    #[test]
    fn utterance_range() {
        let input = args("clan analyze freq +z25-125 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --range 25-125 file.cha"));
    }

    #[test]
    fn include_retracings() {
        let input = args("clan analyze mlu +r6 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze mlu --include-retracings file.cha")
        );
    }

    #[test]
    fn merge_noop() {
        let input = args("clan analyze freq +u file.cha");
        let result = rewrite_clan_args(&input);
        // +u is a no-op (merge is default), so it's dropped
        assert_eq!(result, args("clan analyze freq file.cha"));
    }

    #[test]
    fn display_mode() {
        let input = args("clan analyze freq +d2 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --display-mode 2 file.cha"));
    }

    #[test]
    fn case_sensitive() {
        let input = args("clan analyze freq +k file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --case-sensitive file.cha"));
    }

    #[test]
    fn output_extension() {
        let input = args("clan analyze freq +fcex file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze freq --output-ext cex file.cha"));
    }

    #[test]
    fn context_after() {
        let input = args("clan analyze kwal +w3 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("clan analyze kwal --context-after 3 file.cha"));
    }

    #[test]
    fn context_before() {
        let input = args("clan analyze kwal -w2 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze kwal --context-before 2 file.cha")
        );
    }

    #[test]
    fn id_filter() {
        let input: Vec<String> = vec![
            "clan".into(),
            "analyze".into(),
            "freq".into(),
            "+t@ID=\"eng|*|CHI|*\"".into(),
            "file.cha".into(),
        ];
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            vec![
                "clan".to_string(),
                "analyze".to_string(),
                "freq".to_string(),
                "--id-filter".to_string(),
                "eng|*|CHI|*".to_string(),
                "file.cha".to_string(),
            ]
        );
    }

    #[test]
    fn mixed_clan_and_modern_flags() {
        let input = args("clan analyze freq +t*CHI --format json file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --speaker CHI --format json file.cha")
        );
    }

    #[test]
    fn combined_flags() {
        let input = args("clan analyze freq +t*CHI +swant +z1-50 +r6 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args(
                "clan analyze freq --speaker CHI --include-word want --range 1-50 --include-retracings file.cha"
            )
        );
    }

    #[test]
    fn unknown_flag_passes_through() {
        let input = args("clan analyze freq +x123 file.cha");
        let result = rewrite_clan_args(&input);
        // Unknown +x flag is not rewritten
        assert_eq!(result, args("clan analyze freq +x123 file.cha"));
    }

    #[test]
    fn modern_flags_pass_through() {
        let input = args("clan analyze freq --speaker CHI --per-file file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(
            result,
            args("clan analyze freq --speaker CHI --per-file file.cha")
        );
    }

    #[test]
    fn empty_args() {
        let result = rewrite_clan_args(&[]);
        assert!(result.is_empty());
    }

    #[test]
    fn bare_plus_minus_pass_through() {
        let input = args("+ -");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("+ -"));
    }

    #[test]
    fn r_without_6_passes_through() {
        let input = args("clan analyze freq +r3 file.cha");
        let result = rewrite_clan_args(&input);
        // +r3 is not +r6, so it passes through
        assert_eq!(result, args("clan analyze freq +r3 file.cha"));
    }

    #[test]
    fn display_mode_non_numeric_passes_through() {
        let input = args("clan analyze freq +dabc file.cha");
        let result = rewrite_clan_args(&input);
        // +dabc is not a valid display mode
        assert_eq!(result, args("clan analyze freq +dabc file.cha"));
    }

    // CHECK-specific flag tests

    #[test]
    fn check_bullets() {
        let input = args("check +c0 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --bullets 0 file.cha"));
    }

    #[test]
    fn check_list_errors() {
        let input = args("check +e file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --list-errors file.cha"));
    }

    #[test]
    fn check_include_error() {
        let input = args("check +e6 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --error 6 file.cha"));
    }

    #[test]
    fn check_exclude_error() {
        let input = args("check -e6 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --exclude-error 6 file.cha"));
    }

    #[test]
    fn check_g2_target_child() {
        let input = args("check +g2 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --check-target file.cha"));
    }

    #[test]
    fn check_g5_unused_speakers() {
        let input = args("check +g5 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --check-unused file.cha"));
    }

    #[test]
    fn check_g4_check_id() {
        let input = args("check +g4 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --check-id true file.cha"));
    }

    #[test]
    fn check_g1_noop() {
        let input = args("check +g1 file.cha");
        let result = rewrite_clan_args(&input);
        // +g1 is a no-op (prosodic delimiters always recognized)
        assert_eq!(result, args("check file.cha"));
    }

    #[test]
    fn check_u_maps_to_check_ud() {
        let input = args("check +u file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --check-ud file.cha"));
    }

    #[test]
    fn non_check_u_is_noop() {
        let input = args("freq +u file.cha");
        let result = rewrite_clan_args(&input);
        // +u is a no-op (merge is default) for non-CHECK commands
        assert_eq!(result, args("freq file.cha"));
    }

    #[test]
    fn non_check_g_is_gem() {
        // For non-check commands, +g is always gem filtering
        let input = args("freq +g2 file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("freq --gem 2 file.cha"));
    }

    #[test]
    fn check_g_with_label_falls_back_to_gem() {
        // +g with a non-digit label (even in check context) falls back to gem
        let input = args("check +gstory file.cha");
        let result = rewrite_clan_args(&input);
        assert_eq!(result, args("check --gem story file.cha"));
    }
}
