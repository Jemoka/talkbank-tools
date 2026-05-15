//! XSD validation harness: every emitted XML must validate against
//! `talkbank.xsd`.
//!
//! Complements the golden-XML parity harness
//! ([`xml_golden`](./xml_golden.rs)) — parity proves we match Java
//! Chatter's shape, XSD validation proves the shape is schema-legal.
//! Both gates together guarantee downstream consumers can parse our
//! output.
//!
//! The test shells out to `xmllint --schema`. This keeps the test
//! crate dependency graph small — `xmllint` ships with macOS and is
//! packaged by every major Linux distro. When `xmllint` is absent,
//! the test skips with a `println!` diagnostic rather than failing.
//!
//! # Running
//!
//! ```bash
//! cargo nextest run -p talkbank-parser-tests --test xml_schema_validate
//! ```

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use rstest::rstest;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate;
use talkbank_transform::xml::write_chat_xml;

/// Path to the committed XSD inside the repo. Resolved relative to
/// `CARGO_MANIFEST_DIR` so the test runs from any working directory.
fn xsd_path() -> PathBuf {
    let manifest = env!("CARGO_MANIFEST_DIR");
    Path::new(manifest)
        .parent()
        .and_then(Path::parent)
        .unwrap()
        .join("schemas/chat-file/xml/talkbank.xsd")
}

/// Parametrised schema-validation test: emit XML for every `.cha`
/// under `resources/corpus/reference/` and pipe it through
/// `xmllint --schema talkbank.xsd --noout`. Files that Java Chatter
/// rejected (no paired `.xml` golden) still get validated — if our
/// output is schema-conforming, the parity gap is safely Java-side
/// only.
#[rstest]
fn xml_schema_validates(#[files("../../resources/corpus/reference/**/*.cha")] cha_path: PathBuf) {
    // Skip gracefully when xmllint isn't installed so CI doesn't
    // hard-fail on minimal images.
    if Command::new("xmllint")
        .arg("--version")
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .is_err()
    {
        println!(
            "xmllint not available, skipping schema validation for {}",
            cha_path.display()
        );
        return;
    }

    let xsd = xsd_path();
    if !xsd.exists() {
        panic!(
            "talkbank.xsd missing at expected path {} — did the schemas/chat-file/xml/ \
             layout change?",
            xsd.display()
        );
    }

    let cha_source = match std::fs::read_to_string(&cha_path) {
        Ok(s) => s,
        Err(e) => panic!("failed to read {}: {e}", cha_path.display()),
    };

    let chat_file = match parse_and_validate(
        &cha_source,
        ParseValidateOptions::default().with_validation(),
    ) {
        Ok(f) => f,
        Err(e) => panic!(
            "parse_and_validate failed for {} (not a schema issue, but a \
             regression elsewhere): {e}",
            cha_path.display()
        ),
    };

    let emitted = match write_chat_xml(&chat_file) {
        Ok(xml) => xml,
        Err(talkbank_transform::xml::XmlWriteError::PhoneticTierUnsupported { .. }) => {
            // Phonetic / syllabification tiers are permanently
            // unsupported by policy (Phon has pivoted to CHAT-only
            // interchange). Files carrying `%pho` / `%mod` /
            // `%phosyl` / `%modsyl` / `%phoaln` / `<pg>` / `<sg>`
            // legitimately can't validate against the XSD through
            // the Rust emitter. Skip with a diagnostic.
            println!(
                "phonetic tier skip: {} (permanently unsupported)",
                cha_path.display()
            );
            return;
        }
        Err(e) => panic!(
            "write_chat_xml refused to emit for {}: {e}\n\
             (all staged features should be wired up by now — this is a gap, \
             not an expected failure)",
            cha_path.display()
        ),
    };

    // xmllint reads XML from stdin when passed `-`. `--noout`
    // suppresses the echoed document; only diagnostics reach stderr.
    let mut child = Command::new("xmllint")
        .arg("--schema")
        .arg(&xsd)
        .arg("--noout")
        .arg("-")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn xmllint");

    use std::io::Write;
    child
        .stdin
        .as_mut()
        .expect("xmllint stdin")
        .write_all(emitted.as_bytes())
        .expect("write xml to xmllint");

    let output = child.wait_with_output().expect("xmllint wait");
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "XSD validation failed for {}\n\
             --- xmllint stderr ---\n{stderr}\n\
             --- emitted XML (head) ---\n{head}",
            cha_path.display(),
            head = emitted.chars().take(2000).collect::<String>(),
        );
    }
}
