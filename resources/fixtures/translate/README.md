# Translate Regression Fixtures

This directory will hold real-world `batchalign3 translate` regression
fixtures. The convention matches `align/` — see the top-level
`resources/fixtures/README.md` for the directory layout and the
`source.json` schema.

No fixtures yet. Add the first one when a user reports a translate
failure that should be tracked permanently. Use the official trim tool
(see the "CRITICAL RULES" at the top of `CLAUDE.md`); never hand-roll
a clip.

Translate fixtures need `input.cha` with main-tier source-language
text and an `expected.cha` with `%xtra` translation tiers. No audio
is needed.
