# Batchalign3 ↔ Batchalign2 parity proof

`prove_parity.py` is the acceptance harness for the BA2→BA3 parity effort
(`parity.md`). For each cell in its matrix it runs the **same input** through
both engines and diffs the command's *important lines* — the lines `parity.md`
says must match (speaker/utterance segmentation, `%mor`, `%gra`, `%wor`,
compare/translate tiers). Headers and other cosmetics are ignored.

```bash
python scripts/parity/prove_parity.py                 # run the whole matrix
python scripts/parity/prove_parity.py --command morphotag
python scripts/parity/prove_parity.py --command translate -v
python scripts/parity/prove_parity.py --list
```

- **BA2** is invoked from its own virtualenv
  (`/Users/houjun/Documents/Projects/batchalign2/.venv`,
  `execute.py <cmd> <IN_DIR> <OUT_DIR>`).
- **BA3** is invoked via `just batchalign::cli <cmd> …`.
- Fixtures under `fixtures/` are committed into the repo so the proof does not
  depend on any external corpus folder.

## Status

| Command   | Cells                                            | Result |
|-----------|--------------------------------------------------|--------|
| morphotag | en, en (?/!), en (contractions), es, ja, zh      | **byte-identical** `%mor` + `%gra` |
| translate | es→eng (Google free tier)                        | **byte-identical** `%xtra` |
| compare   | single `template.gold.cha` lookup (parity.md)    | structure matches; see note |
| transcribe| rev / whisperx / whisper / openai / vllm wired   | engines selectable; see note |
| align     | wav2vec / whisperx wired                         | engines selectable; see note |

### morphotag — exact

A faithful port of BA2's UD→CHAT handler layer
(`python/batchalign/backends/morphosyntax/ud/`) reproduces BA2's lowercase CHAT
POS, per-POS ordered + combined features (`pron|I-Prs-Nom-S1`), the `%gra`
tier, and the tokenize postprocessor (so contractions / CJK align to the
upstream word split). Verified byte-identical to BA2 across Stanza 1.12 (BA3)
vs 1.10.1 (BA2) on six fixtures.

### translate — exact

`GoogleTranslateBackend`'s free (`googletrans`) path mirrors BA2's
`GoogleTranslateEngine` exactly (fresh client per call, auto-detect source,
English default, CJK + punctuation post-processing).

### compare — structure done, content differs

BA3 now looks for a single `template.gold.cha` (or per-file `FILE.gold.cha`)
in the **input folder**, replacing BA2's parallel-gold-folder structure — the
change `parity.md` explicitly asks for. The compare *tier content*
(`%xsrep`/`%xsmor`) still diverges from BA2 on two BA2-specific behaviours:
BA2 replaces the main line with the gold text, and its bag-of-words windowing
leaks boundary words across utterance edges (`+he`/`-he`). BA3's aligner is
cleaner; matching BA2's windowing quirk byte-for-byte is tracked but not done.

### transcribe / align — engines wired, segmentation parity is model-bound

All BA2 ASR engines (rev, whisperx, whisper, openai-whisper) plus a vLLM
Whisper are selectable via `--engine`, and `--language` is propagated to the
backend. **Byte-identical** transcribe output is not attainable from wiring
alone: BA2 segments ASR output with a trained BERT utterance model and uses a
custom CHATWhisper English model; reproducing BA2's per-utterance segmentation
requires using those same models. The engine plumbing is in place; closing the
segmentation gap means vendoring BA2's segmentation models.
