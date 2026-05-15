//! Print the embedded CHAT JSON Schema or its canonical URL.
//!
//! The schema (`chat-file.schema.json`) is auto-generated from `talkbank-model` types
//! via schemars and compiled into the binary at build time. Running `chatter schema`
//! emits the full JSON Schema to stdout so downstream tools can validate their own
//! JSON output against the exact version this binary was built with. `--url` prints
//! just the canonical `talkbank.org` URL for external consumers who don't bundle
//! the binary.
use talkbank_transform::SCHEMA_JSON;

/// Canonical URL for the current schema version.
const SCHEMA_URL: &str = "https://talkbank.org/schemas/v0.1/chat-file.json";

/// Print either the embedded CHAT JSON schema or the canonical schema URL.
///
/// The schema encodes all the rules discussed in the File Format, Headers, and Tier sections of the
/// CHAT manual. When invoked without `--url-only`, the command emits the stored JSON schema so
/// downstream tooling can validate against the exact version currently deployed by talkbank-chat. When
/// `--url-only` is set, it prints the official `talkbank.org` URL so external consumers can rely on the
/// same canonical schema without bundling the binary.
pub fn run_schema(url_only: bool) {
    if url_only {
        println!("{SCHEMA_URL}");
    } else {
        print!("{SCHEMA_JSON}");
    }
}
