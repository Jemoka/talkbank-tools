// Integration test target: Cargo compiles this as a separate crate,
// so the lib's `cfg_attr(test, ...)` allow does not apply. Test code
// uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

//! Focused tests for the workflow helper seams.

use batchalign::compare::{gold_path_for, is_gold_file, template_gold_path_for};

#[test]
fn compare_gold_path_helpers_match_expected_convention() {
    assert_eq!(gold_path_for("test.cha"), "test.gold.cha");
    assert_eq!(
        gold_path_for("/data/corpus/01DM.cha"),
        "/data/corpus/01DM.gold.cha"
    );
    assert_eq!(gold_path_for("dir/sub/file.cha"), "dir/sub/file.gold.cha");
    assert_eq!(template_gold_path_for("test.cha"), "template.gold.cha");
    assert_eq!(
        template_gold_path_for("/data/corpus/01DM.cha"),
        "/data/resources/corpus/template.gold.cha"
    );
    assert_eq!(
        template_gold_path_for("dir/sub/file.cha"),
        "dir/sub/template.gold.cha"
    );
}

#[test]
fn compare_gold_file_detection_is_exact() {
    assert!(is_gold_file("test.gold.cha"));
    assert!(is_gold_file("/data/01DM.gold.cha"));
    assert!(!is_gold_file("test.cha"));
    assert!(!is_gold_file("test.gold.txt"));
}
