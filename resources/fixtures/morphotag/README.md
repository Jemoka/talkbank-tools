# Morphotag Regression Fixtures

> **Note (runner not yet public):** The regression-fixture runner referenced
> from the top-level `resources/fixtures/README.md` lives in an internal-only
> test crate that has not been open-sourced yet. Until it lands publicly,
> these fixtures document expected behavior; running them requires the
> private workspace.

This directory will hold real-world `batchalign3 morphotag` regression
fixtures. The convention matches `align/` — see the top-level
`resources/fixtures/README.md` for the directory layout and the
`source.json` schema.

No fixtures yet. Add the first one when a user reports a morphotag
failure that should be tracked permanently. Use the official trim tool
(see the "CRITICAL RULES" at the top of `CLAUDE.md`); never hand-roll
a clip.

Morphotag fixtures need `input.cha` with main-tier utterances and an
`expected.cha` with `%mor` and `%gra` tiers. No audio is needed.
