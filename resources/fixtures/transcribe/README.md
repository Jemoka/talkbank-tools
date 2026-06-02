# Transcribe Regression Fixtures

> **Note (runner not yet public):** The
> `crates/batchalign/tests/regression_fixtures.rs` runner referenced below
> lives in an internal-only test crate that has not been open-sourced yet.
> Until it lands publicly, these fixtures document expected behavior;
> running them requires the private workspace.

This directory will hold real-world `batchalign3 transcribe` regression
fixtures. The convention matches `align/` — see the top-level
`resources/fixtures/README.md` for the directory layout and the
`source.json` schema.

No fixtures yet. Add the first one when a user reports a transcribe
failure that should be tracked permanently. Use the official trim tool
(see the "CRITICAL RULES" at the top of `CLAUDE.md`); never hand-roll
a clip.

For transcribe, `input.cha` is empty / minimal (transcribe takes audio
input only). The fixture is mainly the audio clip, the
`expected.cha`, and the manifest. The Rust integration test
`crates/batchalign/tests/regression_fixtures.rs` will pick it up
automatically once `source.json` is in place.
