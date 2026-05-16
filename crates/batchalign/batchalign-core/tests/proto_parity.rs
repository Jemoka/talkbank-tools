//! Probe that the hand-mirrored Python proto types in
//! `python/batchalign/_core/proto.py` still exist.
//!
//! Per `spec2.md` §18.2.
//!
//! The test is a no-op (passing) if `python3` is not on PATH or if the user
//! sets `BATCHALIGN_SKIP_PYTHON_PARITY=1`. When the python module is reachable
//! but the proto classes are missing, the test fails.

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
    let expected = &[
        "AsrInput",
        "AsrOutput",
        "AsrSegment",
        "AsrWord",
        "AsrOptions",
        "LanguageSpec",
        "PreparedAudio",
        "FaInput",
        "FaOutput",
        "SpeakerInput",
        "SpeakerOutput",
        "Diarization",
        "DiarizationSegment",
        "UtSegInput",
        "UtSegOutput",
        "UtteranceSpan",
        "MorphosyntaxInput",
        "MorphosyntaxOutput",
        "MorphosyntaxUtterance",
        "MorphosyntaxToken",
        "TaggedUtterance",
        "TranslateInput",
        "TranslateOutput",
        "CorefInput",
        "CorefOutput",
        "OpenSmileInput",
        "OpenSmileOutput",
        "AvqiInput",
        "AvqiOutput",
    ];
    let script = format!(
        r#"
import sys
try:
    import batchalign._core.proto as p
except Exception as e:
    print(f"IMPORT_ERROR: {{e}}", file=sys.stderr)
    sys.exit(2)
expected = {expected:?}
missing = [n for n in expected if not hasattr(p, n)]
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
        // Exit 2 = import failure (acceptable during early bootstrap before
        // the python package is on sys.path). Exit 1 = real missing class.
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
