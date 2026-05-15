//! Test-only lookup helper for `@Birth of <CODE>` headers.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Birth_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>

#[cfg(test)]
use talkbank_model::model::Header;

/// Find @Birth of <CODE> header for a speaker
///
/// Searches headers for a @Birth header matching the given speaker code.
///
/// # Arguments
///
/// - `speaker_code`: The speaker code to search for (e.g., "CHI", "MOT")
/// - `headers`: All headers from the CHAT file
///
/// # Returns
///
/// `Some(date)` if @Birth of <CODE> header found, `None` otherwise
///
/// # Example
///
/// ```rust,ignore
/// let headers = vec![
///     Header::Birth { participant: SpeakerCode::new("CHI"), date: ChatDate::new("28-JUN-2001") },
/// ];
///
/// let birth_date = find_birth_header("CHI", &headers);
/// assert_eq!(birth_date.map(|d| d.as_str()), Some("28-JUN-2001"));
/// ```
#[cfg(test)]
pub(super) fn find_birth_header(
    speaker_code: &str,
    headers: &[Header],
) -> Option<talkbank_model::model::ChatDate> {
    for header in headers {
        if let Header::Birth { participant, date } = header
            && participant.as_str() == speaker_code
        {
            return Some(date.clone());
        }
    }
    None
}
