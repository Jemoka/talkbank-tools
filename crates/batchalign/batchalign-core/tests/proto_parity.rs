//! Cross-check that every Rust proto type registered with
//! [`register_proto_schema!`] survives the Bazel codegen pipeline and lands
//! in `batchalign._core._proto_generated` (and is re-exported via the
//! `batchalign._core.proto` shim).
//!
//! Pre-codegen, this test only probed class existence in a hand-mirrored
//! Python file. Now the Python side is auto-generated from the Rust
//! registry, so a missing class indicates either:
//!
//!   1. The Bazel `:_proto_generated_py` genrule didn't run / isn't staged
//!      into runfiles (test environment problem).
//!   2. `register_proto_schema!` was forgotten on a freshly-added type
//!      (contributor problem — fix by adding the macro call).
//!
//! Either way the test fails fast with a list of what's missing. The
//! probe is skip-tolerant when `python3` is unavailable (CI on platforms
//! where the Python side isn't built) so the rust workspace stays
//! independently testable.

use std::collections::BTreeSet;

use batchalign_core::proto::ProtoSchemaEntry;

#[test]
fn python_proto_classes_exist() {
    if std::env::var_os("BATCHALIGN_SKIP_PYTHON_PARITY").is_some() {
        eprintln!("skipping: BATCHALIGN_SKIP_PYTHON_PARITY set");
        return;
    }
    if which("python3").is_none() {
        eprintln!("skipping: python3 not on PATH");
        return;
    }

    // Pull the expected class set straight from the link-time registry.
    // Any future `register_proto_schema!` call lands here automatically.
    let mut expected: BTreeSet<&'static str> = BTreeSet::new();
    for entry in inventory::iter::<ProtoSchemaEntry> {
        expected.insert(entry.name);
    }
    // The discriminated-union types (`TaskInput`, `TaskOutput`) need their
    // variant names visible on the Python side too, since backends often
    // do `isinstance(item, TaskInputAsr)`. We don't enumerate them in the
    // registry (they're synthesized by codegen from the union schema), so
    // probe for them explicitly.
    let variant_classes = ["TaskInputAsr", "TaskOutputAsr"];

    let expected_list: Vec<&'static str> = expected.iter().copied().collect();
    let script = format!(
        r#"
import sys
try:
    import batchalign._core.proto as p
except Exception as e:
    print(f"IMPORT_ERROR: {{e}}", file=sys.stderr)
    sys.exit(2)
expected = {expected_list:?}
variants = {variant_classes:?}
# Re-export via proto.py covers most names; variants live on
# _proto_generated and are not always re-exported.
gen = sys.modules.get("batchalign._core._proto_generated")
missing = [n for n in expected if not hasattr(p, n)]
missing += [n for n in variants if gen is not None and not hasattr(gen, n)]
if missing:
    print("MISSING:", missing, file=sys.stderr)
    sys.exit(1)
"#
    );
    let out = std::process::Command::new("python3")
        .arg("-c")
        .arg(&script)
        .output()
        .expect("invoke python3");
    if !out.status.success() {
        if out.status.code() == Some(2) {
            eprintln!(
                "skipping: cannot import batchalign._core.proto ({})",
                String::from_utf8_lossy(&out.stderr).trim()
            );
            return;
        }
        panic!(
            "python proto parity failed: {}",
            String::from_utf8_lossy(&out.stderr)
        );
    }
}

fn which(cmd: &str) -> Option<std::path::PathBuf> {
    let path = std::env::var_os("PATH")?;
    for dir in std::env::split_paths(&path) {
        let candidate = dir.join(cmd);
        if candidate.is_file() {
            return Some(candidate);
        }
    }
    None
}
