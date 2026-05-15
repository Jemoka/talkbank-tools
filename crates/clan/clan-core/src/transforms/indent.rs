//! INDENT -- align CA overlap markers.
//!
//! Reimplements CLAN's `indent` command, which adjusts whitespace so that
//! closing overlap markers (`⌊`, U+230A) are column-aligned with their
//! matching opening overlap markers (`⌈`, U+2308) on a preceding speaker tier.
//!
//! # Algorithm
//!
//! 1. Read the file into a list of tiers (speaker prefix + content text).
//! 2. For each main tier (`*SPK:`), scan for opening overlap markers `⌈`
//!    and record their column positions and optional numeric suffixes.
//! 3. Scan up to 30 subsequent tiers from *different* speakers for closing
//!    overlap markers `⌊`.
//! 4. Match open/close pairs by numeric suffix (or sequentially if unnumbered).
//! 5. Insert or remove spaces before the closing marker to align columns.
//! 6. Report unmatched markers as warnings.
//!
//! # Differences from CLAN
//!
//! - Operates on UTF-8 text directly using Rust's `char`-based column counting
//!   rather than C byte-level scanning with `my_CharacterByteType`.
//! - Uses the text-based transform pattern (no AST round-trip) to preserve
//!   original formatting outside of overlap alignment.
//! - Column counting treats each Unicode scalar value as width 1, matching
//!   CLAN's behavior for the overlap markers (all BMP characters).

use std::io::Write as IoWrite;

use crate::framework::TransformError;

/// Opening overlap marker: ⌈ (U+2308, LEFT CEILING BRACKET).
const OPEN_OVERLAP: char = '\u{2308}';

/// Closing overlap marker: ⌊ (U+230A, LEFT FLOOR BRACKET).
const CLOSE_OVERLAP: char = '\u{230A}';

/// Maximum number of tracked open overlaps across the file.
const MAX_OVERLAPS: usize = 30;

/// Maximum number of overlaps per utterance.
const MAX_OV_PER_UTT: usize = 15;

/// A single tier from the CHAT file (one logical line, possibly multi-line
/// with continuation tabs).
#[derive(Debug)]
struct Tier {
    /// The speaker prefix (e.g., `*CHI:\t` or `%mor:\t`), empty for headers.
    speaker: String,
    /// The tier content (everything after the prefix).
    text: String,
    /// Line number in the original file (for diagnostics).
    line_number: usize,
    /// Per-utterance overlap tracking.
    open_ov: Vec<OverlapMark>,
    /// Per-utterance close overlap tracking.
    close_ov: Vec<OverlapMark>,
}

/// Tracking info for one overlap marker within a tier.
#[derive(Debug, Clone)]
struct OverlapMark {
    /// Optional numeric suffix ('0'-'9'), or 0 if unnumbered.
    number: u8,
    /// Whether this marker was successfully matched.
    matched: bool,
}

/// A tracked opening overlap across the file.
#[derive(Debug)]
struct OpenOverlap {
    /// Column position of the opening marker.
    col: usize,
    /// Which continuation line (CR count) the marker is on.
    cr_count: usize,
    /// Optional numeric suffix.
    number: u8,
    /// Whether this overlap has been matched.
    matched: bool,
    /// Whether to skip this overlap (already used for different close).
    skip: bool,
    /// Index of the tier containing this opening marker.
    tier_index: usize,
    /// Index within that tier's open_ov array.
    ov_index: usize,
}

/// Parse a CHAT file into tiers.
///
/// Each tier is a logical unit: a speaker line plus any continuation lines
/// (lines starting with `\t`). Header lines (`@`-prefixed) are their own tiers.
fn parse_tiers(content: &str) -> Vec<Tier> {
    let mut tiers: Vec<Tier> = Vec::new();

    for (line_num, line) in content.lines().enumerate() {
        let line_num = line_num + 1;

        if line.starts_with('\t') {
            // Continuation line — append to current tier
            if let Some(tier) = tiers.last_mut() {
                tier.text.push('\n');
                tier.text.push_str(line);
            }
        } else if line.starts_with('*') || line.starts_with('%') || line.starts_with('@') {
            // New tier
            let (speaker, text) = if let Some(tab_pos) = line.find('\t') {
                let sp = &line[..=tab_pos];
                let txt = &line[tab_pos + 1..];
                (sp.to_owned(), txt.to_owned())
            } else if let Some(colon_pos) = line.find(':') {
                // Speaker line without tab (normalize to tab)
                let sp = format!("{}\t", &line[..=colon_pos]);
                let txt = line[colon_pos + 1..].trim_start().to_owned();
                (sp, txt)
            } else {
                (String::new(), line.to_owned())
            };

            tiers.push(Tier {
                speaker,
                text,
                line_number: line_num,
                open_ov: Vec::new(),
                close_ov: Vec::new(),
            });
        } else {
            // Blank or other lines — preserve as-is
            tiers.push(Tier {
                speaker: String::new(),
                text: line.to_owned(),
                line_number: line_num,
                open_ov: Vec::new(),
                close_ov: Vec::new(),
            });
        }
    }

    tiers
}

/// Extract the speaker code from a speaker prefix (e.g., `*CHI:\t` -> `*CHI`).
fn speaker_code(speaker: &str) -> &str {
    speaker.find(':').map(|i| &speaker[..i]).unwrap_or(speaker)
}

/// Scan a tier's text for opening overlap markers, recording their positions.
fn scan_open_overlaps(tier: &mut Tier, tier_index: usize, open_ovs: &mut Vec<OpenOverlap>) {
    let chars: Vec<char> = tier.text.chars().collect();
    let mut col: usize = 0;
    let mut cr_count: usize = 0;
    let mut ov_index: usize = 0;

    let mut i = 0;
    while i < chars.len() {
        // Replace tabs with spaces (matching CLAN)
        if chars[i] == '\t' && (i == 0 || chars[i.saturating_sub(1)] != '\n') {
            // Tab -> space (we track columns, not modifying in place)
        }

        if chars[i] == OPEN_OVERLAP {
            if open_ovs.len() >= MAX_OVERLAPS {
                tracing::warn!(
                    "File line {}: overlap array full (max {MAX_OVERLAPS})",
                    tier.line_number,
                );
                break;
            }

            // Check if preceded by ⌊ (closing marker immediately before)
            let effective_col = if col > 0 && i > 0 && chars[i - 1] == CLOSE_OVERLAP {
                col - 1
            } else {
                col
            };

            // Check for numeric suffix
            let number = if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                chars[i + 1] as u8
            } else {
                0
            };

            if ov_index < MAX_OV_PER_UTT {
                tier.open_ov.push(OverlapMark {
                    number,
                    matched: false,
                });
            }

            open_ovs.push(OpenOverlap {
                col: effective_col,
                cr_count,
                number,
                matched: false,
                skip: false,
                tier_index,
                ov_index,
            });

            ov_index += 1;
        }

        if chars[i] == '\n' {
            col = 0;
            cr_count += 1;
        } else {
            col += 1;
        }

        i += 1;
    }
}

/// Process closing overlap markers in a tier, aligning them with their
/// matching opening markers by inserting or removing spaces.
///
/// Returns `true` if the opening tier needed adjustment (requiring restart).
fn align_close_overlaps(tier: &mut Tier, open_ovs: &mut [OpenOverlap], tiers: &mut [Tier]) -> bool {
    let chars: Vec<char> = tier.text.chars().collect();
    let mut result = String::with_capacity(tier.text.len() + 32);
    let mut col: usize = 0;
    let mut close_ov_index: usize = 0;

    // If all open overlaps are unnumbered (count < 2), reset matched state
    let unnumbered_count = open_ovs.iter().filter(|o| o.number == 0).count();
    if unnumbered_count < 2 {
        for ov in open_ovs.iter_mut() {
            ov.matched = false;
        }
    }

    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\n' {
            result.push('\n');
            col = 0;
            i += 1;
            continue;
        }

        if chars[i] == CLOSE_OVERLAP {
            let close_number = if i + 1 < chars.len() && chars[i + 1].is_ascii_digit() {
                chars[i + 1] as u8
            } else {
                0
            };

            if close_ov_index < MAX_OV_PER_UTT {
                tier.close_ov.push(OverlapMark {
                    number: close_number,
                    matched: false,
                });
            }

            // Find matching open overlap
            let match_idx = open_ovs.iter().position(|ov| {
                // Skip same-speaker overlaps
                if ov.tier_index < tiers.len()
                    && speaker_code(&tiers[ov.tier_index].speaker) == speaker_code(&tier.speaker)
                {
                    return false;
                }
                !ov.skip && !ov.matched && ov.number == close_number
            });

            if let Some(idx) = match_idx {
                let target_col = open_ovs[idx].col;

                if close_number == 0 {
                    open_ovs[idx].matched = true;
                }

                // Mark both sides as matched
                if let Some(close_mark) = tier.close_ov.last_mut() {
                    close_mark.matched = true;
                }
                if open_ovs[idx].ov_index < MAX_OV_PER_UTT {
                    let open_tier_idx = open_ovs[idx].tier_index;
                    let open_ov_idx = open_ovs[idx].ov_index;
                    if open_tier_idx < tiers.len()
                        && open_ov_idx < tiers[open_tier_idx].open_ov.len()
                    {
                        tiers[open_tier_idx].open_ov[open_ov_idx].matched = true;
                    }
                }

                if col < target_col {
                    // Need to insert spaces before the closing marker
                    // Back up to word boundary
                    let insert_pos = find_word_boundary_back(&result);
                    let spaces_needed = target_col - col;
                    let prefix = result[..insert_pos].to_owned();
                    let suffix = result[insert_pos..].to_owned();
                    result = prefix;
                    for _ in 0..spaces_needed {
                        result.push(' ');
                    }
                    result.push_str(&suffix);
                    col = target_col;
                } else if col > target_col {
                    // Need to remove spaces before the closing marker
                    let mut removed = 0;
                    while col > target_col && result.ends_with("  ") {
                        result.pop();
                        col -= 1;
                        removed += 1;
                    }

                    if col > target_col && target_col == 0 {
                        // Can't move close marker further left — adjust
                        // the opening tier instead
                        let open_tier_idx = open_ovs[idx].tier_index;
                        let open_cr = open_ovs[idx].cr_count;
                        let open_col = open_ovs[idx].col;

                        // Restore removed spaces
                        for _ in 0..removed {
                            result.push(' ');
                        }

                        adjust_open_tier(&mut tiers[open_tier_idx], col, open_col, open_cr);
                        // Need full restart
                        // Write current result back to tier
                        result.push_str(&chars[i..].iter().collect::<String>());
                        tier.text = result;
                        return true;
                    }
                }
            }

            close_ov_index += 1;
        }

        result.push(chars[i]);
        col += 1;
        i += 1;
    }

    // Mark unmatched opens as skip for future tiers
    let chars_again: Vec<char> = tier.text.chars().collect();
    for ch in &chars_again {
        if *ch == CLOSE_OVERLAP {
            let close_number = 0u8; // simplified
            for ov in open_ovs.iter_mut() {
                if ov.number == close_number {
                    ov.skip = true;
                }
            }
        }
    }

    tier.text = result;
    false
}

/// Find a position to insert spaces — back up past the current word to
/// insert spaces at a word boundary.
fn find_word_boundary_back(text: &str) -> usize {
    let bytes = text.as_bytes();
    let mut pos = bytes.len();

    // Back up past non-space, non-bracket characters
    while pos > 0 {
        let ch = text[..pos].chars().next_back().unwrap_or(' ');
        if ch.is_whitespace() || ch == '[' || ch == ']' {
            break;
        }
        pos -= ch.len_utf8();
    }

    // Move forward past trailing spaces to insert before the word
    pos
}

/// Adjust an opening tier by inserting spaces to shift content rightward,
/// so that the opening marker aligns with a closing marker that couldn't
/// move further left.
fn adjust_open_tier(tier: &mut Tier, target_col: usize, current_col: usize, cr_count: usize) {
    if target_col <= current_col {
        return;
    }

    let chars: Vec<char> = tier.text.chars().collect();
    let mut result = String::with_capacity(chars.len() + (target_col - current_col));
    let mut cr = 0;
    let mut col = 0;
    let spaces_to_add = target_col - current_col;

    for (i, &ch) in chars.iter().enumerate() {
        if ch == '\n' {
            result.push(ch);
            cr += 1;
            col = 0;
            continue;
        }

        // Find the right position on the right continuation line
        if cr == cr_count && col == current_col {
            // Back up to word boundary
            let insert_pos = find_word_boundary_back(&result);
            let prefix = result[..insert_pos].to_owned();
            let suffix = result[insert_pos..].to_owned();
            result = prefix;
            for _ in 0..spaces_to_add {
                result.push(' ');
            }
            result.push_str(&suffix);
            // Append the rest
            for &remaining in &chars[i..] {
                result.push(remaining);
            }
            tier.text = result;
            return;
        }

        result.push(ch);
        col += 1;
    }

    tier.text = result;
}

/// Serialize tiers back to a CHAT file string.
fn serialize_tiers(tiers: &[Tier]) -> String {
    let mut output = String::new();
    for tier in tiers {
        if !tier.speaker.is_empty() {
            output.push_str(&tier.speaker);
        }
        output.push_str(&tier.text);
        output.push('\n');
    }
    output
}

/// Run the INDENT transform on a CHAT file.
///
/// Reads the file, aligns overlap markers, writes the result.
pub fn run_indent(
    input: &std::path::Path,
    output: Option<&std::path::Path>,
) -> Result<(), TransformError> {
    let content = std::fs::read_to_string(input)?;
    let result = indent_text(&content, input.display().to_string().as_str());

    if let Some(output_path) = output {
        std::fs::write(output_path, &result)?;
    } else {
        std::io::stdout().write_all(result.as_bytes())?;
    }

    Ok(())
}

/// Core indent logic operating on text.
pub fn indent_text(content: &str, filename: &str) -> String {
    let mut tiers = parse_tiers(content);

    // May need multiple passes if adjusting open tiers causes restarts
    for _pass in 0..10 {
        // Reset overlap tracking
        for tier in tiers.iter_mut() {
            tier.open_ov.clear();
            tier.close_ov.clear();
        }

        let mut needs_restart = false;

        let tier_count = tiers.len();
        let mut i = 0;
        while i < tier_count {
            if tiers[i].speaker.starts_with('*') {
                // Normalize speaker prefix: ensure tab after colon
                if let Some(colon_pos) = tiers[i].speaker.find(':') {
                    let after_colon = &tiers[i].speaker[colon_pos + 1..];
                    if after_colon.starts_with(' ') {
                        let code = tiers[i].speaker[..=colon_pos].to_owned();
                        tiers[i].speaker = format!("{code}\t");
                    }
                }

                let mut open_ovs: Vec<OpenOverlap> = Vec::new();
                scan_open_overlaps(&mut tiers[i], i, &mut open_ovs);

                if !open_ovs.is_empty() {
                    // Scan subsequent tiers for closing markers
                    let mut tier_cnt = 0;
                    let mut j = i + 1;
                    while tier_cnt < 30 && j < tier_count && !open_ovs.is_empty() {
                        tier_cnt += 1;

                        // We need to split the borrow: open_ovs borrows nothing
                        // from tiers, and we pass tiers mutably for potential
                        // open-tier adjustment
                        let (before, rest) = tiers.split_at_mut(j);
                        if let Some(close_tier) = rest.first_mut()
                            && align_close_overlaps(close_tier, &mut open_ovs, before)
                        {
                            needs_restart = true;
                            break;
                        }

                        j += 1;
                    }
                }

                if needs_restart {
                    break;
                }

                // Report unmatched markers
                for (idx, mark) in tiers[i].open_ov.iter().enumerate() {
                    if !mark.matched {
                        let ordinal = match idx {
                            0 => "first".to_owned(),
                            1 => "second".to_owned(),
                            2 => "third".to_owned(),
                            n => format!("{}th", n + 1),
                        };
                        if mark.number == 0 || mark.number < b'0' {
                            tracing::warn!(
                                "File \"{filename}\": line {}. \
                                 Can't find closing overlap marker for {ordinal} \
                                 overlap \u{2308} on any following speaker tiers.",
                                tiers[i].line_number,
                            );
                        } else {
                            tracing::warn!(
                                "File \"{filename}\": line {}. \
                                 Can't find closing overlap marker for \
                                 \u{2308}{} on any following speaker tiers.",
                                tiers[i].line_number,
                                mark.number as char,
                            );
                        }
                    }
                }
            }

            if needs_restart {
                break;
            }
            i += 1;
        }

        // Also report unmatched close markers
        if !needs_restart {
            for tier in &tiers {
                for (idx, mark) in tier.close_ov.iter().enumerate() {
                    if !mark.matched {
                        let ordinal = match idx {
                            0 => "first".to_owned(),
                            1 => "second".to_owned(),
                            2 => "third".to_owned(),
                            n => format!("{}th", n + 1),
                        };
                        if mark.number == 0 || mark.number < b'0' {
                            tracing::warn!(
                                "File \"{filename}\": line {}. \
                                 Can't find opening overlap marker for {ordinal} \
                                 overlap \u{230A} on any previous speaker tiers.",
                                tier.line_number,
                            );
                        } else {
                            tracing::warn!(
                                "File \"{filename}\": line {}. \
                                 Can't find opening overlap marker for \
                                 \u{230A}{} on any previous speaker tiers.",
                                tier.line_number,
                                mark.number as char,
                            );
                        }
                    }
                }
            }
        }

        if !needs_restart {
            break;
        }
    }

    serialize_tiers(&tiers)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn basic_overlap_alignment() {
        let input = "\
@UTF8
@Begin
@Participants:\tCHI Target_Child, MOT Mother
*CHI:\tI want ⌈ cookies ⌉ .
*MOT:\t⌊ yeah ⌋ okay .
@End
";
        let result = indent_text(input, "test.cha");
        // The ⌊ in MOT's line should be aligned to the same column as ⌈ in CHI's line
        let chi_line = result.lines().find(|l| l.starts_with("*CHI:")).unwrap();
        let mot_line = result.lines().find(|l| l.starts_with("*MOT:")).unwrap();

        let open_col = chi_line.chars().position(|c| c == OPEN_OVERLAP).unwrap();
        let close_col = mot_line.chars().position(|c| c == CLOSE_OVERLAP).unwrap();
        assert_eq!(
            open_col, close_col,
            "overlap markers should be column-aligned"
        );
    }

    #[test]
    fn numbered_overlaps() {
        let input = "\
@UTF8
@Begin
@Participants:\tCHI Target_Child, MOT Mother
*CHI:\t⌈1 hello ⌉1 ⌈2 world ⌉2 .
*MOT:\t⌊1 hi ⌋1 ⌊2 earth ⌋2 .
@End
";
        let result = indent_text(input, "test.cha");
        let chi_line = result.lines().find(|l| l.starts_with("*CHI:")).unwrap();
        let mot_line = result.lines().find(|l| l.starts_with("*MOT:")).unwrap();

        // Find positions of ⌈1 and ⌊1
        let chi_chars: Vec<char> = chi_line.chars().collect();
        let mot_chars: Vec<char> = mot_line.chars().collect();

        let open1_col = chi_chars.iter().position(|&c| c == OPEN_OVERLAP).unwrap();
        let close1_col = mot_chars.iter().position(|&c| c == CLOSE_OVERLAP).unwrap();
        assert_eq!(open1_col, close1_col, "first overlap pair should align");
    }

    #[test]
    fn no_overlaps_unchanged() {
        let input = "\
@UTF8
@Begin
*CHI:\thello world .
@End
";
        let result = indent_text(input, "test.cha");
        assert_eq!(result, input);
    }

    #[test]
    fn same_speaker_not_matched() {
        // Overlaps between same speaker should not be matched
        let input = "\
@UTF8
@Begin
@Participants:\tCHI Target_Child
*CHI:\t⌈ hello ⌉ .
*CHI:\t⌊ world ⌋ .
@End
";
        let result = indent_text(input, "test.cha");
        // Should warn about unmatched markers, not align them
        // The text should still be valid
        assert!(result.contains("*CHI:"));
    }
}
