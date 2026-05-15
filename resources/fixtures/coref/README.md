# Coref Regression Fixtures

This directory will hold real-world `batchalign3` coreference resolution
regression fixtures. The convention matches `align/` — see the
top-level `resources/fixtures/README.md` for the directory layout and the
`source.json` schema.

No fixtures yet. Add the first one when a user reports a coref
failure that should be tracked permanently. Use the official trim tool
(see the "CRITICAL RULES" at the top of `CLAUDE.md`); never hand-roll
a clip.

Coref fixtures need `input.cha` with multi-utterance discourse and an
`expected.cha` with `%xcoref` tiers showing the expected chains. No
audio is needed.
