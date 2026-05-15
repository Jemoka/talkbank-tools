//! DATES -- age computation from `@Birth` and `@Date` headers.
//!
//! Reimplements CLAN's `dates` command, which computes the age of each
//! participant at the time of transcription by subtracting `@Birth` dates
//! from the file-level `@Date` header. Computed ages are inserted as
//! `@Comment: Age of CHI is Y;M.D` headers after the `@ID`/`@Birth` block.
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409311)
//! for the original command documentation.
//!
//! The computed age uses CHAT format: `years;months.days` (e.g., `2;6.15`
//! for two years, six months, and fifteen days).
//!
//! # Differences from CLAN
//!
//! - Operates on the typed AST rather than raw text line scanning.
//! - Uses the framework transform pipeline (parse → transform → serialize).
//! - Reads `@Date` and `@Birth` from structured header variants instead of
//!   parsing raw `@` lines with string splitting.

use talkbank_model::{ChatFile, Header, Line};

use crate::framework::{TransformCommand, TransformError};

/// DATES transform: compute and insert @Age headers.
pub struct DatesCommand;

impl TransformCommand for DatesCommand {
    type Config = ();

    /// Compute participant ages from `@Birth`/`@Date` and inject comment headers.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        // Collect @Date and @Birth headers
        let mut file_date: Option<ParsedDate> = None;
        let mut births: Vec<(String, ParsedDate)> = Vec::new();

        for line in file.lines.iter() {
            if let Line::Header { header, .. } = line {
                match header.as_ref() {
                    Header::Date { date, .. } => {
                        if let Some(parsed) = ParsedDate::parse(date.as_str()) {
                            file_date = Some(parsed);
                        }
                    }
                    Header::Birth {
                        participant, date, ..
                    } => {
                        if let Some(parsed) = ParsedDate::parse(date.as_str()) {
                            births.push((participant.as_str().to_string(), parsed));
                        }
                    }
                    _ => {}
                }
            }
        }

        let file_date = match file_date {
            Some(d) => d,
            None => return Ok(()), // No @Date header, nothing to compute
        };

        // Compute ages and build @Comment headers
        let mut age_comments = Vec::new();
        for (participant, birth) in &births {
            if let Some(age) = compute_age(birth, &file_date) {
                age_comments.push((participant.clone(), age));
            }
        }

        if age_comments.is_empty() {
            return Ok(());
        }

        // Find insert position: after @ID headers or @Participants
        let insert_idx = find_age_insert_position(&file.lines);

        for (i, (participant, age)) in age_comments.into_iter().enumerate() {
            let header_text = format!("@Comment:\tAge of {participant} is {age}");
            let header = Header::Unknown {
                text: talkbank_model::WarningText::new(header_text),
                parse_reason: None,
                suggested_fix: None,
            };
            file.lines.insert(insert_idx + i, Line::header(header));
        }

        Ok(())
    }
}

/// Find the position to insert @Age comments (after @ID headers).
fn find_age_insert_position(lines: &[Line]) -> usize {
    let mut last_id_or_birth = 0;
    for (i, line) in lines.iter().enumerate() {
        if let Line::Header { header, .. } = line
            && matches!(header.as_ref(), Header::ID(_) | Header::Birth { .. })
        {
            last_id_or_birth = i + 1;
        }
    }
    last_id_or_birth
}

/// A parsed date from DD-MMM-YYYY format.
#[derive(Debug, Clone)]
struct ParsedDate {
    day: u32,
    month: u32, // 1-based
    year: i32,
}

impl ParsedDate {
    /// Parse a CHAT date string (DD-MMM-YYYY).
    fn parse(s: &str) -> Option<Self> {
        let parts: Vec<&str> = s.split('-').collect();
        if parts.len() != 3 {
            return None;
        }

        let day = parts[0].parse::<u32>().ok()?;
        let month = month_to_number(parts[1])?;
        let year = parts[2].parse::<i32>().ok()?;

        Some(Self { day, month, year })
    }
}

/// Convert month abbreviation to 1-based number.
fn month_to_number(month: &str) -> Option<u32> {
    match month.to_uppercase().as_str() {
        "JAN" => Some(1),
        "FEB" => Some(2),
        "MAR" => Some(3),
        "APR" => Some(4),
        "MAY" => Some(5),
        "JUN" => Some(6),
        "JUL" => Some(7),
        "AUG" => Some(8),
        "SEP" => Some(9),
        "OCT" => Some(10),
        "NOV" => Some(11),
        "DEC" => Some(12),
        _ => None,
    }
}

/// Days in a given month (accounting for leap years).
fn days_in_month(month: u32, year: i32) -> u32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) {
                29
            } else {
                28
            }
        }
        _ => 30,
    }
}

/// Compute age difference as "years;months.days" string.
fn compute_age(birth: &ParsedDate, date: &ParsedDate) -> Option<String> {
    let mut years = date.year - birth.year;
    let mut months = date.month as i32 - birth.month as i32;
    let mut days = date.day as i32 - birth.day as i32;

    if days < 0 {
        months -= 1;
        // Borrow days from the previous month
        let prev_month = if date.month == 1 { 12 } else { date.month - 1 };
        let prev_year = if date.month == 1 {
            date.year - 1
        } else {
            date.year
        };
        days += days_in_month(prev_month, prev_year) as i32;
    }

    if months < 0 {
        years -= 1;
        months += 12;
    }

    if years < 0 {
        return None; // Birth date is after file date
    }

    Some(format!("{years};{months}.{days}"))
}
