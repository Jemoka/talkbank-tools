# French

**Status:** Current
**Last updated:** 2026-05-01 09:47 EDT

## Scope

French is a Romance language with productive MWT and clitic-elision
phenomena that Stanza's neural tokenizer and MWT processor handle
natively:

* Preposition+article contractions: `au → à + le`, `aux → à + les`,
  `du → de + le`, `des → de + les`
* Clitic-article elisions: `l'ami → l' + ami`, `l'eau → l' + eau`
* Clitic-pronoun and complementizer elisions: `c'est`, `qu'il`,
  `d'un`, `n'avait`, `l'on`, `j'ai`
* Elision-prefix words (single CHAT token, apostrophe-internal):
  `jusqu'à`, `puisqu'il`, `quelqu'un`, `aujourd'hui`
* Multi-clitic stacks: `d'l'attraper`, `qu'l'on`

French carries no per-language BA3 MWT-override rules. All earlier
overrides ported from BA2 (`ud.py:662-695`) were audited and removed
— see [History](#history).

## What Stanza handles natively

Paired probes (free-tokenize vs our postprocessor) on 50+ French
constructions with Stanza 1.11.1. All produce identical output on
both paths and satisfy the morphotag 1-to-1 invariant:

| Pattern | Example | Stanza output |
|---------|---------|---------------|
| au / aux / du / des | `va au cinéma` | `à + le + cinéma` (MWT expansion, 1 CHAT word → 2 UD words, Range preserved) |
| c'est / n'est / qu'il | `c'est vrai` | MWT split to `c' + est` (Range preserved) |
| l'X (noun, vowel-initial) | `l'ami`, `l'eau`, `l'Ecosse` | MWT split `l' + ami`, 1 CHAT word → 2 UD words |
| Elision prefix | `jusqu'à`, `puisqu'il`, `quelqu'un`, `aujourd'hui` | 1 CHAT word → 2 UD words; the DP realigner in `align_tokens` merges any over-split back |
| Multi-clitic | `d'l'attraper`, `qu'l'on` | Stanza emits correct Range expansion; DP realigner handles any residual splits |

Probes in
[`batchalign/tests/investigations/_cases/french.py`](https://github.com/TalkBank/batchalign/blob/main/batchalign/tests/investigations/_cases/french.py)
(typed `ProbeCase` fixtures consumed by the matrix harness at
`test_stanza_mwt_probe_matrix.py`).

## Known Stanza limitations

No French-specific Stanza defects are tracked. Earlier `missing_mor`
flags on the `eng,fra` pair were produced by the
`FRENCH_ELISION_PREFIXES` splitting hack (see
[History](#history)); these are expected to go to zero once the
removal is verified end-to-end.

## History

### Rules that existed and were removed

| Rule | What it did | Audit finding |
|------|-------------|---------------|
| `Exact("au") → ForceMwt` | Force MWT expansion on `au` | **Redundant.** Modern Stanza French expands `au → à + le` natively; the hint was dead weight. |
| `Exact("aujourd'hui") → PlainText("aujourd'hui")` | Replace with plain text (no MWT hint) | **Dormant.** Stanza emits `aujourd'hui` as 1 token in all tested positions (standalone, sentence-initial, sentence-medial, sentence-final). The override never fired on a real input. |
| `FRENCH_ELISION_PREFIXES` split | For tokens matching `jusqu'`, `puisqu'`, `quelqu'`, `aujourd'`, split into `(prefix, False) + (suffix, False)` with no MWT hint | **Actively broke the 1-to-1 invariant.** The split produced N Stanza-facing tokens for 1 CHAT word without MWT Range metadata, causing the morphotag 1-to-1 gate to reject the utterance. Root cause of `eng,fra` `missing_mor` residuals and Wave 4 `PipelineAbsorbedFailure` anomalies. |
| `is_french_multi_clitic` split | Same split logic for multi-apostrophe clitic stacks like `d'l'attraper` | **Same invariant break.** Removed alongside `FRENCH_ELISION_PREFIXES`. |

### Why the DP alignment is the load-bearing piece

The character-level Hirschberg realignment in `align_tokens` merges
Stanza's native 2-token expansion of `jusqu'à` (which Stanza emits
as `jusqu' + à`) back to 1 Stanza-facing token per 1 CHAT word,
preserving the MWT Range metadata. That rescue is unconditional and
language-agnostic, which is why the BA2 split hack was unnecessary
to begin with.

## Tests

* **Probe matrix cases:**
  `batchalign/tests/investigations/_cases/french.py` — typed
  `ProbeCase` fixtures (elision prefixes, clitic contractions,
  MWT natives, `aujourd'hui`, plus the seed 040802:1620 utterance).
* **Matrix harness:**
  `batchalign/tests/investigations/test_stanza_mwt_probe_matrix.py`
  runs every case through paired (free-tokenize vs postprocessor)
  pipelines. Invoke with `uv run pytest
  batchalign/tests/investigations/ -m golden`.
* **Behavior-table renderer:**
  `scripts/analysis/render_probe_matrix_table.py --lang fra` —
  regenerates the per-language behavior table from the probe
  matrix for comparison against the hand-curated native-handling
  table above.

## References

* [Morphotag Invariants](../../architecture/morphotag-invariants.md)
* Wave 4 eval harness: `batchalign3 eval l2-morphotag`
