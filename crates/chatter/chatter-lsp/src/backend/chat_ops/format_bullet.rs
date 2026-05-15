//! Timing bullet formatting for `talkbank/formatBulletLine`.

use serde::{Deserialize, Serialize};

use crate::backend::execute_commands::FormatBulletLineRequest;

/// JSON payload returned to the extension when a timing bullet is inserted.
#[derive(Serialize, Deserialize)]
struct FormatBulletOutput {
    /// Bullet marker inserted at the end of the current utterance line.
    bullet: String,
    /// Fresh utterance line scaffold inserted after the bullet.
    new_line: String,
}

/// Handle `talkbank/formatBulletLine`.
pub(crate) fn handle_format_bullet_line(
    request: &FormatBulletLineRequest,
) -> Result<serde_json::Value, crate::backend::LspBackendError> {
    let output = FormatBulletOutput {
        bullet: format!("\u{2022}{}_{}\u{2022}", request.prev_ms, request.current_ms),
        new_line: format!("*{}:\t", request.speaker),
    };

    serde_json::to_value(&output).map_err(Into::into)
}
