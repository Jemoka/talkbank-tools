//! Golden-XML parity harness: Rust CHAT → XML vs Java Chatter output.
//!
//! For each `.xml` file under `resources/corpus/reference-xml/` we expect a sibling
//! `.cha` file at the matching path under `resources/corpus/reference/`. The test:
//!
//! 1. Loads the CHAT source.
//! 2. Parses + validates it with the production pipeline.
//! 3. Emits XML via `talkbank_transform::xml::write_chat_xml`.
//! 4. Structurally compares the emission to the golden XML.
//!
//! Comparison is **structural**, not byte-for-byte: whitespace between
//! elements is normalized, attribute ordering is normalized, and
//! CDATA/text content is compared after trimming surrounding whitespace.
//! That matches what external XML consumers (NLTK's ElementTree,
//! childes-db's `xml.etree.ElementTree`, Phon's StAX reader) actually
//! observe; byte-exact comparison would over-constrain the emitter for
//! no downstream benefit.
//!
//! All tests are expected to be RED while the emitter is being built up
//! stage by stage. Each stage flips a subset GREEN. This is deliberate
//! per the TDD policy (`CLAUDE.md §12`).
//!
//! # Running
//!
//! ```bash
//! cargo nextest run -p talkbank-parser-tests --test xml_golden
//! ```

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use rstest::rstest;
use talkbank_model::ParseValidateOptions;
use talkbank_transform::parse_and_validate;
use talkbank_transform::xml::write_chat_xml;
use walkdir::WalkDir;

mod xml_support;
use xml_support::{XmlStructuralDiff, assert_xml_structurally_equal};

const EXPECTED_MISSING_XML_GOLDENS: &[&str] = &[
    "core/headers-comments",
    "edge-cases/unicode-ipa-content",
    "tiers/pho",
];

const EXPECTED_PHONETIC_TIER_UNSUPPORTED: &[&str] = &[
    "annotation/groups-phonological.cha",
    "annotation/groups-sign.cha",
    "edge-cases/multi-tier-alignment.cha",
    "edge-cases/unicode-ipa-content.cha",
    "tiers/pho-groupings.cha",
    "tiers/pho-syllable-dots.cha",
    "tiers/pho.cha",
    "tiers/phon-syllabification.cha",
];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("talkbank-parser-tests should live under <repo>/crates/")
        .to_path_buf()
}

fn reference_root() -> PathBuf {
    repo_root().join("resources/corpus/reference")
}

fn reference_xml_root() -> PathBuf {
    repo_root().join("resources/corpus/reference-xml")
}

fn rel_path(base: &Path, path: &Path) -> String {
    path.strip_prefix(base)
        .expect("path should live under expected base")
        .to_string_lossy()
        .replace('\\', "/")
}

fn rel_stem(base: &Path, path: &Path) -> String {
    let mut rel = PathBuf::from(rel_path(base, path));
    rel.set_extension("");
    rel.to_string_lossy().replace('\\', "/")
}

fn allowlist(entries: &[&str]) -> BTreeSet<String> {
    entries.iter().map(|entry| (*entry).to_string()).collect()
}

fn reference_cha_paths() -> Vec<PathBuf> {
    let root = reference_root();
    WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("cha"))
        .collect()
}

fn reference_cha_stems() -> BTreeSet<String> {
    let root = reference_root();
    reference_cha_paths()
        .into_iter()
        .map(|path| rel_stem(&root, &path))
        .collect()
}

fn reference_xml_stems() -> BTreeSet<String> {
    let root = reference_xml_root();
    WalkDir::new(&root)
        .into_iter()
        .filter_map(Result::ok)
        .map(|entry| entry.into_path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("xml"))
        .map(|path| rel_stem(&root, &path))
        .collect()
}

#[test]
fn xml_golden_inventory_is_explicit() {
    let cha = reference_cha_stems();
    let xml = reference_xml_stems();

    let missing: BTreeSet<_> = cha.difference(&xml).cloned().collect();
    let stale: BTreeSet<_> = xml.difference(&cha).cloned().collect();

    assert!(
        stale.is_empty(),
        "stale XML goldens present without matching CHAT source: {stale:#?}\n\
         Remove the stale golden or restore the paired reference file."
    );
    assert_eq!(
        missing,
        allowlist(EXPECTED_MISSING_XML_GOLDENS),
        "reference files without XML goldens changed.\n\
         Update EXPECTED_MISSING_XML_GOLDENS in xml_golden.rs only after \
         deliberately adjudicating the coverage change."
    );
}

#[test]
fn xml_emission_skip_policy_is_explicit() {
    let reference_root = reference_root();
    let mut actual_unsupported = BTreeSet::new();

    for cha_path in reference_cha_paths() {
        let rel = rel_path(&reference_root, &cha_path);
        let cha_source = std::fs::read_to_string(&cha_path)
            .unwrap_or_else(|err| panic!("failed to read {}: {err}", cha_path.display()));
        let chat_file = parse_and_validate(
            &cha_source,
            ParseValidateOptions::default().with_validation(),
        )
        .unwrap_or_else(|err| {
            panic!(
                "parse_and_validate failed for {} while auditing XML skip policy: {err}",
                cha_path.display()
            )
        });

        match write_chat_xml(&chat_file) {
            Ok(_) => {}
            Err(talkbank_transform::xml::XmlWriteError::PhoneticTierUnsupported { .. }) => {
                actual_unsupported.insert(rel);
            }
            Err(err) => panic!(
                "unexpected XML emission error for {} while auditing skip policy: {err}",
                cha_path.display()
            ),
        }
    }

    assert_eq!(
        actual_unsupported,
        allowlist(EXPECTED_PHONETIC_TIER_UNSUPPORTED),
        "files skipped by XML emission changed.\n\
         Update EXPECTED_PHONETIC_TIER_UNSUPPORTED in xml_golden.rs only after \
         deliberately approving the policy change."
    );
}

/// Parameterised golden test, one invocation per `*.xml` in `resources/corpus/reference-xml/`.
///
/// Intentionally named `xml_golden_parity` so the test output surfaces
/// the parameter file path next to the intent.
#[rstest]
fn xml_golden_parity(#[files("../../resources/corpus/reference-xml/**/*.xml")] golden_path: PathBuf) {
    let cha_path = paired_chat_path(&golden_path);
    let reference_root = reference_root();

    let cha_source = match std::fs::read_to_string(&cha_path) {
        Ok(s) => s,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => panic!(
            "stale golden: {} has no matching source at {}\n\
             xml_golden_inventory_is_explicit should be updated or the stale golden removed.",
            golden_path.display(),
            cha_path.display()
        ),
        Err(e) => panic!(
            "failed to read paired CHAT source {}: {e}",
            cha_path.display()
        ),
    };
    let golden_xml = match std::fs::read_to_string(&golden_path) {
        Ok(s) => s,
        Err(e) => panic!("failed to read golden XML {}: {e}", golden_path.display()),
    };

    // Parse with validation. A parse/validation failure on a reference
    // corpus file is itself a regression — surface it directly rather
    // than muddling it with XML-emitter diagnostics.
    let chat_file = match parse_and_validate(
        &cha_source,
        ParseValidateOptions::default().with_validation(),
    ) {
        Ok(file) => file,
        Err(e) => panic!(
            "parse_and_validate failed for paired CHAT {}: {e}",
            cha_path.display()
        ),
    };

    // Emit XML. While the port is in progress this will commonly error
    // with `FeatureNotImplemented`. We fail the test with that error
    // text visible so the harness doubles as a coverage dashboard.
    //
    // Phonetic-tier material (`%pho`, `%mod`, `%phosyl`, `%modsyl`,
    // `%phoaln`, `<pg>`, `<sg>`) is intentionally skipped for a
    // reviewed allowlist only. `xml_emission_skip_policy_is_explicit`
    // audits that allowlist so a new unsupported class cannot silently
    // shrink XML coverage.
    let emitted = match write_chat_xml(&chat_file) {
        Ok(xml) => xml,
        Err(talkbank_transform::xml::XmlWriteError::PhoneticTierUnsupported { .. }) => {
            let rel = rel_path(&reference_root, &cha_path);
            assert!(
                allowlist(EXPECTED_PHONETIC_TIER_UNSUPPORTED).contains(&rel),
                "unexpected phonetic-tier XML skip for {rel}; update policy audit first"
            );
            println!(
                "phonetic tier skip: {} (Phon XML emission permanently unsupported)",
                cha_path.display()
            );
            return;
        }
        Err(e) => panic!(
            "write_chat_xml refused to emit for {}: {e}",
            cha_path.display()
        ),
    };

    if let Err(diff) = assert_xml_structurally_equal(&emitted, &golden_xml) {
        panic!(
            "XML structural mismatch for {}\n--- diff ---\n{diff}",
            golden_path.display(),
        );
    }
    // Silence dead-code warning if the diff type becomes unused.
    let _ = std::marker::PhantomData::<XmlStructuralDiff>;
}

/// Map `resources/corpus/reference-xml/<rel>.xml` → `resources/corpus/reference/<rel>.cha`.
///
/// The two trees mirror each other: the XML tree was generated by
/// running Java Chatter over the CHAT reference tree, preserving the
/// relative paths.
fn paired_chat_path(golden_xml_path: &Path) -> PathBuf {
    // Replace the last "reference-xml" path component with "reference".
    // Swap the extension.
    let mut out = PathBuf::new();
    let mut swapped = false;
    for component in golden_xml_path.components() {
        let os = component.as_os_str();
        if !swapped && os == "reference-xml" {
            out.push("reference");
            swapped = true;
        } else {
            out.push(os);
        }
    }
    if !swapped {
        // Defensive: if the layout ever changes, loud failure beats
        // silently pointing at the wrong file.
        panic!(
            "golden XML path does not contain a 'reference-xml' component: {}",
            golden_xml_path.display()
        );
    }
    out.set_extension("cha");
    out
}
