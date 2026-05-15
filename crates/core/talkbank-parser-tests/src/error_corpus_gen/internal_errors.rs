//! E0-E1xx: Internal/Structural Errors
//!
//! Generates parse error corpus files for internal and structural errors
//! (E0xx and E1xx ranges).

use std::fs;
use std::path::Path;

use super::{GenResult, write_file};

//
// E0-E1xx: Internal/Structural Errors (4 missing)
//

/// Generates e0 e1xx internal errors.
pub fn generate_e0_e1xx_internal_errors(root: &Path) -> GenResult {
    let dir = root.join("parse_errors");
    fs::create_dir_all(&dir)?;

    let mut count = 0;

    // E003: EmptyString
    write_file(&dir.join("E003_empty_string.cha"), "".to_string())?;
    count += 1;

    // E101: InvalidLineFormat
    write_file(
        &dir.join("E101_invalid_line_format.cha"),
        "@Begin\n@Languages:\teng\n\
         InvalidLine\n\
         @Comment:\tERROR: Line format invalid\n\
         @End\n"
            .to_string(),
    )?;
    count += 1;

    // E001, E002 are not testable with CHAT files (internal system errors)

    Ok(count)
}
