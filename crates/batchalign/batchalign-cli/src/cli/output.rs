//! Write server job results to the local filesystem.
//!
//! After the CLI polls a completed job, each [`FileResult`] must be written to
//! the correct output path. This module handles:
//!
//! - **Path resolution**: a pre-built `result_map` (server filename to local
//!   output path) provides exact lookup; a fallback joins the filename with the
//!   output directory and, for transcribe jobs that rename extensions, ensures
//!   the output gets a `.cha` suffix.
//! - **Path traversal protection**: the resolved output path is checked against
//!   the canonicalized output directory so a malicious server cannot write
//!   outside the intended tree (e.g. `../../../etc/passwd`).
//! - **Parent directory creation**: intermediate directories are created
//!   automatically so callers do not need to pre-create nested output trees.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::api::{ContentType, FileResult};

use crate::cli::error::CliError;

/// Write a single file result to the output directory.
///
/// Uses `result_map` (server_name -> output_path) for exact lookup.
/// Falls back to joining with `out_dir` if not in the map.
///
/// Returns `Ok(true)` on success, `Ok(false)` if the result had an error,
/// or `Err` on I/O failure.
pub fn write_result(
    result: &FileResult,
    result_map: &HashMap<String, PathBuf>,
    out_dir: &Path,
) -> Result<bool, CliError> {
    // Server-side error → skip
    if result.error.is_some() {
        return Ok(false);
    }

    let content_type = &result.content_type;
    let out_path = if *content_type != ContentType::Chat {
        // Non-CHAT output (e.g. CSV from opensmile) — use server filename directly
        out_dir.join(&*result.filename)
    } else {
        resolve_output_path(&result.filename, result_map, out_dir)
    };

    // Path traversal protection: result path must be under out_dir
    let out_resolved = std::fs::canonicalize(out_dir).unwrap_or_else(|_| out_dir.to_path_buf());
    // We can't canonicalize the output path yet (it may not exist), so normalize manually
    let out_path_abs = if out_path.is_absolute() {
        out_path.clone()
    } else {
        out_dir.join(&out_path)
    };

    // Check that the output path doesn't escape the output directory
    // by checking that canonicalizing the parent stays under out_dir
    if let Some(parent) = out_path_abs.parent() {
        std::fs::create_dir_all(parent)?;
        let parent_resolved = std::fs::canonicalize(parent)?;
        if !parent_resolved.starts_with(&out_resolved) {
            return Err(CliError::PathTraversal(result.filename.to_string()));
        }
    }

    std::fs::write(&out_path_abs, &result.content)?;
    Ok(true)
}

/// Map a result filename back to the correct output path.
fn resolve_output_path(
    result_filename: &str,
    result_map: &HashMap<String, PathBuf>,
    out_dir: &Path,
) -> PathBuf {
    if let Some(path) = result_map.get(result_filename) {
        return path.clone();
    }
    // Fallback: join with out_dir
    let out_path = out_dir.join(result_filename);
    // For transcribe which renames extensions: ensure .cha
    let ext = out_path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_lowercase();
    if ext != "cha" && !result_filename.ends_with(".csv") {
        out_path.with_extension("cha")
    } else {
        out_path
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_output_uses_map() {
        let mut map = HashMap::new();
        map.insert("test.cha".to_string(), PathBuf::from("/out/test.cha"));
        let result = resolve_output_path("test.cha", &map, Path::new("/out"));
        assert_eq!(result, PathBuf::from("/out/test.cha"));
    }

    #[test]
    fn resolve_output_fallback() {
        let map = HashMap::new();
        let result = resolve_output_path("test.cha", &map, Path::new("/out"));
        assert_eq!(result, PathBuf::from("/out/test.cha"));
    }

    #[test]
    fn write_result_success() {
        let dir = tempfile::tempdir().unwrap();
        let mut map = HashMap::new();
        let out_path = dir.path().join("test.cha");
        map.insert("test.cha".to_string(), out_path.clone());

        let result = FileResult {
            filename: "test.cha".into(),
            content: "@Begin\n*CHI:\thello .\n@End\n".to_string(),
            content_type: ContentType::Chat,
            error: None,
            provenance: Vec::new(),
        };

        let ok = write_result(&result, &map, dir.path()).unwrap();
        assert!(ok);
        assert!(out_path.exists());
    }

    #[test]
    fn write_result_skips_error() {
        let dir = tempfile::tempdir().unwrap();
        let map = HashMap::new();
        let result = FileResult {
            filename: "test.cha".into(),
            content: String::new(),
            content_type: ContentType::Chat,
            error: Some("processing failed".to_string()),
            provenance: Vec::new(),
        };

        let ok = write_result(&result, &map, dir.path()).unwrap();
        assert!(!ok);
    }

    #[test]
    fn resolve_output_fallback_adds_cha_for_media() {
        let map = HashMap::new();
        let result = resolve_output_path("audio.mp3", &map, Path::new("/out"));
        assert_eq!(result, PathBuf::from("/out/audio.cha"));
    }

    #[test]
    fn resolve_output_fallback_keeps_csv() {
        let map = HashMap::new();
        let result = resolve_output_path("features.csv", &map, Path::new("/out"));
        assert_eq!(result, PathBuf::from("/out/features.csv"));
    }

    #[test]
    fn resolve_output_fallback_no_extension() {
        let map = HashMap::new();
        let result = resolve_output_path("filename", &map, Path::new("/out"));
        assert_eq!(result, PathBuf::from("/out/filename.cha"));
    }

    #[test]
    fn write_result_path_traversal_blocked() {
        let dir = tempfile::tempdir().unwrap();
        let map = HashMap::new();
        let result = FileResult {
            filename: "../../../escaped.cha".into(),
            content: "bad".to_string(),
            content_type: ContentType::Chat,
            error: None,
            provenance: Vec::new(),
        };

        let err = write_result(&result, &map, dir.path()).unwrap_err();
        assert!(
            format!("{err}").contains("path escapes output directory"),
            "expected PathTraversal, got: {err}"
        );
    }

    #[test]
    fn write_result_path_traversal_nested() {
        let dir = tempfile::tempdir().unwrap();
        let map = HashMap::new();
        let result = FileResult {
            filename: "sub/../../escaped.cha".into(),
            content: "bad".to_string(),
            content_type: ContentType::Chat,
            error: None,
            provenance: Vec::new(),
        };

        let err = write_result(&result, &map, dir.path()).unwrap_err();
        assert!(
            format!("{err}").contains("path escapes output directory"),
            "expected PathTraversal, got: {err}"
        );
    }

    #[test]
    fn write_result_path_traversal_absolute() {
        let dir = tempfile::tempdir().unwrap();
        let map = HashMap::new();
        let result = FileResult {
            filename: "/etc/stuff".into(),
            content: "bad".to_string(),
            content_type: ContentType::Csv,
            error: None,
            provenance: Vec::new(),
        };

        let err = write_result(&result, &map, dir.path()).unwrap_err();
        assert!(
            format!("{err}").contains("path escapes output directory"),
            "expected PathTraversal, got: {err}"
        );
    }

    #[test]
    fn write_result_creates_nested_parent() {
        let dir = tempfile::tempdir().unwrap();
        let out_path = dir.path().join("sub").join("deep").join("file.cha");
        let mut map = HashMap::new();
        map.insert("file.cha".to_string(), out_path.clone());

        let result = FileResult {
            filename: "file.cha".into(),
            content: "@Begin\n@End\n".to_string(),
            content_type: ContentType::Chat,
            error: None,
            provenance: Vec::new(),
        };

        let ok = write_result(&result, &map, dir.path()).unwrap();
        assert!(ok);
        assert!(out_path.exists());
        assert_eq!(
            std::fs::read_to_string(&out_path).unwrap(),
            "@Begin\n@End\n"
        );
    }

    #[test]
    fn write_result_non_chat_content_type() {
        let dir = tempfile::tempdir().unwrap();
        let map = HashMap::new();
        let result = FileResult {
            filename: "features.csv".into(),
            content: "col1,col2\n1,2\n".to_string(),
            content_type: ContentType::Csv,
            error: None,
            provenance: Vec::new(),
        };

        let ok = write_result(&result, &map, dir.path()).unwrap();
        assert!(ok);
        let written = dir.path().join("features.csv");
        assert!(written.exists());
        assert_eq!(
            std::fs::read_to_string(written).unwrap(),
            "col1,col2\n1,2\n"
        );
    }

    #[test]
    fn write_result_empty_content() {
        let dir = tempfile::tempdir().unwrap();
        let mut map = HashMap::new();
        let out_path = dir.path().join("empty.cha");
        map.insert("empty.cha".to_string(), out_path.clone());

        let result = FileResult {
            filename: "empty.cha".into(),
            content: String::new(),
            content_type: ContentType::Chat,
            error: None,
            provenance: Vec::new(),
        };

        let ok = write_result(&result, &map, dir.path()).unwrap();
        assert!(ok);
        assert!(out_path.exists());
        assert_eq!(std::fs::read_to_string(&out_path).unwrap(), "");
    }
}
