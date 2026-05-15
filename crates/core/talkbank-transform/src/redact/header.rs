//! Header-level sanitization.

use talkbank_model::{BulletContent, Header, IDHeader};

use super::REDACTED_TEXT;

/// Sanitizes a single header in place.
pub(crate) fn sanitize_header(header: &mut Header) {
    match header {
        Header::Participants { entries } => {
            for entry in entries.iter_mut() {
                entry.name = None;
            }
        }
        Header::ID(id) => {
            anonymize_id(id);
        }
        Header::Comment { content } => {
            *content = BulletContent::from_text(REDACTED_TEXT);
        }
        _ => {}
    }
}

fn anonymize_id(id: &mut IDHeader) {
    id.custom_field = None;
    id.education = None;
}
