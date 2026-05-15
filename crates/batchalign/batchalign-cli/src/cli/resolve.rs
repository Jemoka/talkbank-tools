//! Input resolution — mirrors `_resolve_inputs()` in `batchalign/cli/dispatch.py`.

use std::path::{Path, PathBuf};

use crate::cli::error::CliError;

/// Resolve CLI arguments into `(input_paths, output_dir)`.
///
/// - `--file-list FILE`: read paths from text file (skip `#` comments, blank lines).
/// - `--in-place` or `-o`: all paths are inputs.
/// - Legacy: 2 args where first is dir, second is not a file → `IN_DIR OUT_DIR`.
/// - Single/multiple paths → in-place processing.
pub fn resolve_inputs(
    paths: &[PathBuf],
    output: Option<&Path>,
    file_list: Option<&Path>,
    in_place: bool,
) -> Result<(Vec<PathBuf>, Option<PathBuf>), CliError> {
    // --file-list mode
    if let Some(fl_path) = file_list {
        if !fl_path.exists() {
            return Err(CliError::FileListMissing(fl_path.to_path_buf()));
        }
        let text = std::fs::read_to_string(fl_path)?;
        let items: Vec<PathBuf> = text
            .lines()
            .map(|l| l.trim())
            .filter(|l| !l.is_empty() && !l.starts_with('#'))
            .map(PathBuf::from)
            .collect();
        if items.is_empty() {
            return Err(CliError::FileListEmpty);
        }
        for p in &items {
            if !p.exists() {
                return Err(CliError::InputMissing(p.clone()));
            }
        }
        return Ok((items, output.map(Path::to_path_buf)));
    }

    if paths.is_empty() {
        return Err(CliError::NoInputPaths);
    }

    // --in-place or -o: all paths are inputs
    if in_place || output.is_some() {
        for p in paths {
            if !p.exists() {
                return Err(CliError::InputMissing(p.clone()));
            }
        }
        return Ok((paths.to_vec(), output.map(Path::to_path_buf)));
    }

    // Legacy: exactly 2 paths, first is dir, second is not a file → IN_DIR OUT_DIR
    if paths.len() == 2 && paths[0].is_dir() && !paths[1].is_file() {
        return Ok((vec![paths[0].clone()], Some(paths[1].clone())));
    }

    // Single or multiple paths → in-place
    for p in paths {
        if !p.exists() {
            return Err(CliError::InputMissing(p.clone()));
        }
    }

    Ok((paths.to_vec(), None))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn no_paths_is_error() {
        let result = resolve_inputs(&[], None, None, false);
        assert!(matches!(result, Err(CliError::NoInputPaths)));
    }

    #[test]
    fn file_list_mode() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.cha");
        let f2 = dir.path().join("b.cha");
        fs::write(&f1, "content").unwrap();
        fs::write(&f2, "content").unwrap();

        let list_file = dir.path().join("files.txt");
        fs::write(
            &list_file,
            format!("# comment\n{}\n\n{}\n", f1.display(), f2.display()),
        )
        .unwrap();

        let (inputs, out) = resolve_inputs(&[], None, Some(&list_file), false).unwrap();
        assert_eq!(inputs.len(), 2);
        assert!(out.is_none());
    }

    #[test]
    fn file_list_missing_path() {
        let dir = tempfile::tempdir().unwrap();
        let list_file = dir.path().join("files.txt");
        fs::write(&list_file, "/nonexistent/path\n").unwrap();
        let result = resolve_inputs(&[], None, Some(&list_file), false);
        assert!(matches!(result, Err(CliError::InputMissing(_))));
    }

    #[test]
    fn in_place_mode() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.cha");
        fs::write(&f1, "content").unwrap();

        let (inputs, out) = resolve_inputs(std::slice::from_ref(&f1), None, None, true).unwrap();
        assert_eq!(inputs.len(), 1);
        assert!(out.is_none());
    }

    #[test]
    fn legacy_two_dir_mode() {
        let dir = tempfile::tempdir().unwrap();
        let in_dir = dir.path().join("input");
        fs::create_dir(&in_dir).unwrap();

        let (inputs, out) = resolve_inputs(
            &[in_dir.clone(), PathBuf::from("/tmp/nonexistent_output_dir")],
            None,
            None,
            false,
        )
        .unwrap();

        assert_eq!(inputs.len(), 1);
        assert!(out.is_some());
    }

    #[test]
    fn explicit_output() {
        let dir = tempfile::tempdir().unwrap();
        let f1 = dir.path().join("a.cha");
        fs::write(&f1, "content").unwrap();

        let (inputs, out) = resolve_inputs(
            std::slice::from_ref(&f1),
            Some(Path::new("/tmp/out")),
            None,
            false,
        )
        .unwrap();
        assert_eq!(inputs.len(), 1);
        assert_eq!(out.as_deref(), Some(Path::new("/tmp/out")));
    }
}
