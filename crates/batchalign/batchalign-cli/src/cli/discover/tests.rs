use super::*;
use std::fs;
use std::path::PathBuf;

#[test]
fn discover_filters_by_extension() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.cha"), "@Begin\n*CHI:\thello .\n@End").unwrap();
    fs::write(dir.path().join("b.txt"), "text").unwrap();
    fs::write(dir.path().join("c.cha"), "@Begin\n*CHI:\tworld .\n@End").unwrap();

    let (files, outputs) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(outputs.len(), 2);
    // Both should be .cha files
    for f in &files {
        assert_eq!(f.extension().unwrap(), "cha");
    }
}

#[test]
fn discover_skips_dummy() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("real.cha"), "@Begin\n*CHI:\thello .\n@End").unwrap();
    fs::write(
        dir.path().join("dummy.cha"),
        "@Options:\tdummy\nThis is a dummy file to permit playback from the TalkBank browser",
    )
    .unwrap();

    let (files, _) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();
    assert_eq!(files.len(), 1);
    assert!(
        files[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("real")
    );

    // Dummy should have been copied to output
    assert!(out.path().join("dummy.cha").exists());
}

#[test]
fn discover_sorted_largest_first() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("small.cha"), "x").unwrap();
    fs::write(dir.path().join("large.cha"), "x".repeat(1000)).unwrap();

    let (files, _) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();
    assert_eq!(files.len(), 2);
    // Largest first
    assert!(
        files[0]
            .file_name()
            .unwrap()
            .to_str()
            .unwrap()
            .contains("large")
    );
}

#[test]
fn copy_nonmatching_preserves_structure() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    fs::write(dir.path().join("a.cha"), "cha").unwrap();
    fs::write(sub.join("b.txt"), "txt").unwrap();

    copy_nonmatching(dir.path(), out.path(), &["cha"], ReleasedCommand::Morphotag).unwrap();
    assert!(out.path().join("sub").join("b.txt").exists());
    // .cha should NOT have been copied
    assert!(!out.path().join("a.cha").exists());
}

#[test]
fn copy_nonmatching_skips_generation() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "txt").unwrap();

    copy_nonmatching(
        dir.path(),
        out.path(),
        &["mp3"],
        ReleasedCommand::Transcribe,
    )
    .unwrap();
    assert!(!out.path().join("a.txt").exists());
}

#[test]
fn infer_base_dir_single_dir() {
    let dir = tempfile::tempdir().unwrap();
    let inputs = vec![dir.path().to_path_buf()];
    let base = infer_base_dir(&inputs).unwrap();
    // macOS: /var is a symlink to /private/var, so canonicalize both sides
    assert_eq!(
        std::fs::canonicalize(&base).unwrap(),
        std::fs::canonicalize(dir.path()).unwrap()
    );
}

#[test]
fn is_dummy_chat_options_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dummy.cha");
    fs::write(&path, "@Options:\tdummy\n@Begin\n@End").unwrap();
    assert!(is_dummy_chat(&path));
}

#[test]
fn is_dummy_chat_text_only() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("dummy.cha");
    fs::write(
        &path,
        "This is a dummy file to permit playback from the TalkBank browser",
    )
    .unwrap();
    assert!(is_dummy_chat(&path));
}

#[test]
fn is_dummy_chat_normal_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("real.cha");
    fs::write(&path, "@Begin\n*CHI:\thello .\n@End").unwrap();
    assert!(!is_dummy_chat(&path));
}

#[test]
fn is_dummy_chat_missing_file() {
    assert!(!is_dummy_chat(Path::new("/nonexistent/file.cha")));
}

#[test]
fn is_dummy_chat_empty_file() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("empty.cha");
    fs::write(&path, "").unwrap();
    assert!(!is_dummy_chat(&path));
}

#[test]
fn discover_server_inputs_dir_and_file() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("corpus");
    fs::create_dir(&sub).unwrap();
    fs::write(sub.join("a.cha"), "@Begin\n@End").unwrap();

    let loose = dir.path().join("loose.cha");
    fs::write(&loose, "@Begin\n@End").unwrap();

    let out = tempfile::tempdir().unwrap();
    let inputs = vec![sub.to_path_buf(), loose.to_path_buf()];
    let (files, outputs) = discover_server_inputs(&inputs, Some(out.path()), &["cha"]).unwrap();
    assert_eq!(files.len(), 2);
    assert_eq!(outputs.len(), 2);
}

#[test]
fn discover_server_inputs_file_only() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("test.cha");
    fs::write(&f, "@Begin\n@End").unwrap();

    let out = tempfile::tempdir().unwrap();
    let inputs = vec![f.to_path_buf()];
    let (files, outputs) = discover_server_inputs(&inputs, Some(out.path()), &["cha"]).unwrap();
    assert_eq!(files.len(), 1);
    assert_eq!(outputs.len(), 1);
    assert_eq!(
        outputs[0].file_name().unwrap().to_str().unwrap(),
        "test.cha"
    );
}

#[test]
fn discover_server_inputs_nonexistent_is_error() {
    let inputs = vec![PathBuf::from("/nonexistent/path")];
    let result = discover_server_inputs(&inputs, None, &["cha"]);
    assert!(matches!(
        result,
        Err(crate::cli::error::CliError::InputMissing(_))
    ));
}

#[test]
fn discover_server_inputs_in_place() {
    let dir = tempfile::tempdir().unwrap();
    let f = dir.path().join("test.cha");
    fs::write(&f, "@Begin\n@End").unwrap();

    let inputs = vec![f.to_path_buf()];
    let (files, outputs) = discover_server_inputs(&inputs, None, &["cha"]).unwrap();
    assert_eq!(files.len(), 1);
    // No out_dir → output path = input path (in-place)
    assert_eq!(outputs[0], files[0]);
}

#[test]
fn discover_client_files_wildcard() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.cha"), "@Begin\n@End").unwrap();
    fs::write(dir.path().join("b.txt"), "text").unwrap();
    fs::write(dir.path().join("c.mp3"), "audio").unwrap();

    let (files, _) = discover_client_files(dir.path(), out.path(), &["*"]).unwrap();
    assert_eq!(files.len(), 3);
}

#[test]
fn discover_client_files_in_place_dummy() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("real.cha"), "@Begin\n@End").unwrap();
    fs::write(dir.path().join("dummy.cha"), "@Options:\tdummy\n").unwrap();

    // in-place: in_dir == out_dir → dummy should NOT be copied
    let (files, _) = discover_client_files(dir.path(), dir.path(), &["cha"]).unwrap();
    assert_eq!(files.len(), 1);
    assert!(files[0].to_str().unwrap().contains("real"));
}

#[test]
fn infer_base_dir_multiple_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.cha");
    let b = dir.path().join("b.cha");
    fs::write(&a, "x").unwrap();
    fs::write(&b, "y").unwrap();

    let inputs = vec![a.to_path_buf(), b.to_path_buf()];
    let base = infer_base_dir(&inputs).unwrap();
    // Common ancestor of two files in the same dir → that dir
    let canonical_dir = fs::canonicalize(dir.path()).unwrap();
    assert_eq!(base, canonical_dir);
}

#[test]
fn infer_base_dir_no_inputs() {
    let base = infer_base_dir(&[]).unwrap();
    assert_eq!(base, PathBuf::from("."));
}

#[test]
fn build_server_names_dir_input() {
    let dir = tempfile::tempdir().unwrap();
    let sub = dir.path().join("sub");
    fs::create_dir(&sub).unwrap();
    let f = sub.join("test.cha");
    fs::write(&f, "x").unwrap();

    let out = dir.path().join("out");
    fs::create_dir(&out).unwrap();
    let o = out.join("sub").join("test.cha");

    let inputs = vec![dir.path().to_path_buf()];
    let outputs = [o.clone()];
    let (names, result_map) = build_server_names(&[f], &outputs, &inputs).unwrap();
    assert_eq!(names.len(), 1);
    assert!(names[0].contains("test.cha"));
    assert_eq!(result_map[&names[0]], o);
}

#[test]
fn build_server_names_individual_files() {
    let dir = tempfile::tempdir().unwrap();
    let a = dir.path().join("a.cha");
    let b = dir.path().join("b.cha");
    fs::write(&a, "x").unwrap();
    fs::write(&b, "y").unwrap();

    let oa = dir.path().join("out_a.cha");
    let ob = dir.path().join("out_b.cha");

    let inputs = vec![a.to_path_buf(), b.to_path_buf()];
    let (names, result_map) =
        build_server_names(&[a, b], &[oa.clone(), ob.clone()], &inputs).unwrap();
    assert_eq!(names.len(), 2);
    assert_eq!(result_map.len(), 2);
    // Both should be simple filenames (common ancestor stripped)
    assert!(names[0].ends_with(".cha"));
    assert!(names[1].ends_with(".cha"));
}

#[test]
fn copy_nonmatching_in_place_skips() {
    let dir = tempfile::tempdir().unwrap();
    fs::write(dir.path().join("a.txt"), "text").unwrap();

    // in_dir == out_dir → nothing copied (early return)
    copy_nonmatching(dir.path(), dir.path(), &["cha"], ReleasedCommand::Morphotag).unwrap();
    // No error, no crash — just a no-op
}

#[test]
fn copy_nonmatching_all_generation_commands() {
    for &cmd in GENERATION_COMMANDS {
        let dir = tempfile::tempdir().unwrap();
        let out = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("extra.txt"), "text").unwrap();

        copy_nonmatching(dir.path(), out.path(), &["cha"], cmd).unwrap();
        // Generation commands skip all copying
        assert!(
            !out.path().join("extra.txt").exists(),
            "command {cmd} should skip copy"
        );
    }
}

// ---- Recursive directory discovery ----

/// Helper: create a 3-level nested directory tree with files at every level.
///
/// Structure:
///     root/
///     ├── top.cha
///     ├── session1/
///     │   ├── child1.cha
///     │   └── notes.txt
///     ├── session2/
///     │   ├── child2.cha
///     │   └── deep/
///     │       └── nested.cha
///     └── empty_sub/
fn make_nested_tree(root: &Path) {
    fs::create_dir_all(root.join("session1")).unwrap();
    fs::create_dir_all(root.join("session2/deep")).unwrap();
    fs::create_dir_all(root.join("empty_sub")).unwrap();

    let chat = "@Begin\n*CHI:\thello .\n@End";
    fs::write(root.join("top.cha"), chat).unwrap();
    fs::write(root.join("session1/child1.cha"), chat).unwrap();
    fs::write(root.join("session1/notes.txt"), "some notes").unwrap();
    fs::write(root.join("session2/child2.cha"), chat).unwrap();
    fs::write(root.join("session2/deep/nested.cha"), chat).unwrap();
}

#[test]
fn discover_recursive_nested_three_levels() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let (files, outputs) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();

    // 4 .cha files at various depths
    assert_eq!(files.len(), 4);
    assert_eq!(outputs.len(), 4);
    let mut basenames: Vec<_> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    basenames.sort();
    assert_eq!(
        basenames,
        vec!["child1.cha", "child2.cha", "nested.cha", "top.cha"]
    );

    // Output paths preserve the relative directory structure
    let mut in_rels: Vec<_> = files
        .iter()
        .map(|f| f.strip_prefix(dir.path()).unwrap().to_path_buf())
        .collect();
    let mut out_rels: Vec<_> = outputs
        .iter()
        .map(|o| o.strip_prefix(out.path()).unwrap().to_path_buf())
        .collect();
    in_rels.sort();
    out_rels.sort();
    assert_eq!(in_rels, out_rels);
}

#[test]
fn discover_recursive_mixed_extensions() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let (files, _) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();

    // Only .cha files
    assert_eq!(files.len(), 4);
    for f in &files {
        assert_eq!(f.extension().unwrap(), "cha");
    }

    // Non-matching notes.txt should be copied preserving structure
    copy_nonmatching(dir.path(), out.path(), &["cha"], ReleasedCommand::Morphotag).unwrap();
    assert!(out.path().join("session1/notes.txt").exists());
    assert_eq!(
        fs::read_to_string(out.path().join("session1/notes.txt")).unwrap(),
        "some notes"
    );
}

#[test]
fn discover_recursive_audio_extensions() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("session1")).unwrap();
    fs::create_dir_all(dir.path().join("session2/deep")).unwrap();

    fs::write(dir.path().join("top.mp3"), b"fake mp3").unwrap();
    fs::write(dir.path().join("session1/child.wav"), b"fake wav").unwrap();
    fs::write(dir.path().join("session2/deep/nested.mp4"), b"fake mp4").unwrap();
    fs::write(dir.path().join("session2/notes.txt"), "notes").unwrap();

    let (files, _) = discover_client_files(dir.path(), out.path(), &["mp3", "mp4", "wav"]).unwrap();

    assert_eq!(files.len(), 3);
    let mut basenames: Vec<_> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    basenames.sort();
    assert_eq!(basenames, vec!["child.wav", "nested.mp4", "top.mp3"]);
}

#[test]
fn discover_recursive_skips_nested_dummy() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    fs::create_dir_all(dir.path().join("sub")).unwrap();

    fs::write(dir.path().join("real.cha"), "@Begin\n*CHI:\thello .\n@End").unwrap();
    fs::write(dir.path().join("sub/dummy.cha"), "@Options:\tdummy\n").unwrap();

    let (files, _) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();

    assert_eq!(files.len(), 1);
    assert!(files[0].to_str().unwrap().contains("real"));
    // Dummy should be copied preserving subdirectory
    assert!(out.path().join("sub/dummy.cha").exists());
}

#[test]
fn discover_recursive_in_place() {
    let dir = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let (files, outputs) = discover_client_files(dir.path(), dir.path(), &["cha"]).unwrap();

    assert_eq!(files.len(), 4);
    // Output paths should equal input paths (in-place)
    let mut sorted_files = files.clone();
    let mut sorted_outputs = outputs.clone();
    sorted_files.sort();
    sorted_outputs.sort();
    assert_eq!(sorted_files, sorted_outputs);

    // Original files should still be intact
    let chat = "@Begin\n*CHI:\thello .\n@End";
    assert_eq!(
        fs::read_to_string(dir.path().join("top.cha")).unwrap(),
        chat
    );
    assert_eq!(
        fs::read_to_string(dir.path().join("session2/deep/nested.cha")).unwrap(),
        chat
    );
}

#[test]
fn discover_recursive_output_preserves_structure() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let (files, outputs) = discover_client_files(dir.path(), out.path(), &["cha"]).unwrap();

    // Every output path's relative structure matches input
    for (f, o) in files.iter().zip(outputs.iter()) {
        let in_rel = f.strip_prefix(dir.path()).unwrap();
        let out_rel = o.strip_prefix(out.path()).unwrap();
        assert_eq!(in_rel, out_rel);
    }
}

#[test]
fn discover_server_inputs_recursive_directory() {
    let dir = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let inputs = vec![dir.path().to_path_buf()];
    let (files, _) = discover_server_inputs(&inputs, None, &["cha"]).unwrap();

    assert_eq!(files.len(), 4);
    let mut basenames: Vec<_> = files
        .iter()
        .map(|f| f.file_name().unwrap().to_str().unwrap().to_string())
        .collect();
    basenames.sort();
    assert_eq!(
        basenames,
        vec!["child1.cha", "child2.cha", "nested.cha", "top.cha"]
    );
}

#[test]
fn discover_server_inputs_recursive_with_output() {
    let dir = tempfile::tempdir().unwrap();
    let out = tempfile::tempdir().unwrap();
    make_nested_tree(dir.path());

    let inputs = vec![dir.path().to_path_buf()];
    let (files, outputs) = discover_server_inputs(&inputs, Some(out.path()), &["cha"]).unwrap();

    assert_eq!(files.len(), 4);
    assert_eq!(outputs.len(), 4);
    // Relative paths should match
    for (f, o) in files.iter().zip(outputs.iter()) {
        let in_rel = f.strip_prefix(dir.path()).unwrap();
        let out_rel = o.strip_prefix(out.path()).unwrap();
        assert_eq!(in_rel, out_rel);
    }
}
