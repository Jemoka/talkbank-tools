# Utseg Regression Fixtures

> **Note (runner not yet public):** The regression-fixture runner referenced
> from the top-level `resources/fixtures/README.md` lives in an internal-only
> test crate that has not been open-sourced yet. Until it lands publicly,
> these fixtures document expected behavior; running them requires the
> private workspace.

This directory will hold real-world `batchalign3 utseg` (utterance
segmentation) regression fixtures. The convention matches `align/` —
see the top-level `resources/fixtures/README.md` for the directory layout
and the `source.json` schema.

No fixtures yet. Add the first one when a user reports an utseg
failure that should be tracked permanently. Use the official trim tool
(see the "CRITICAL RULES" at the top of `CLAUDE.md`); never hand-roll
a clip.

Utseg fixtures need `input.cha` with un-segmented main-tier text and
an `expected.cha` with the segmented version. No audio is needed.
