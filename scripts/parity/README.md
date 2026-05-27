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
| transcribe| **rev**: en, zh, es                              | **byte-identical** speaker + utterance segmentation + text |
| align     | **wav2vec (MMS_FA)**: en                         | **byte-identical** `%wor` + utterance bullets |
| compare   | single `template.gold.cha` lookup (parity.md)    | structure matches; content note below |

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

### transcribe — rev: exact; whisper-family: BA2-side blocked

All BA2 ASR engines are selectable via `transcribe --engine`
(`rev | whisperx | whisper | chatwhisper | openai | vllm`), `--language`
propagated. The BA2 *pairing* (ASR → CHATUtterance BERT segmentation →
disfluency `&-uh` → retrace `[/]`) is implemented as the recipe's UtSeg stage,
applied uniformly to every engine's word stream; non-BERT languages (es) use
Rev's punctuation to sentence-split and keep commas. RevAI uploads the original
media (byte-identical to BA2) so the transcript matches.

**rev is byte-identical to BA2** on en / zh / es. The **whisper-family engines
(whisper / whisperx / openai / chatwhisper) cannot be verified here**: BA2's own
local Whisper inference crashes in this environment ("process pool terminated
abruptly" — a torch/transformers native crash in BA2's py3.11 venv, for both
CHATWhisper-en and the Cantonese model), so there is no BA2 reference to diff
against. BA3's ChatWhisperBackend (CHATWhisper-en + bfloat16 + the CHATUtterance
pairing) is implemented and runs; it just can't be compared until BA2's whisper
crash is fixed. **Cantonese** transcribe is blocked for the same reason (BA2
uses Whisper for `yue`). A torchcodec import incompatibility in the Bazel
hermetic env (transformers 5.x) is worked around in `backends/asr/_torch_audio.py`.

### align — wav2vec (MMS_FA): exact

`Wav2Vec2FaBackend` is a faithful port of BA2's `--wav2vec` aligner
(torchaudio `MMS_FA` + `forced_align`/`merge_tokens`, 15 s chunking, char-DP
word mapping, post-correction). `align --engine wav2vec` produces `%wor` word
timings + the utterance media bullets byte-identical to BA2 (verified on the
English clip; torchaudio 2.6 vs 2.11 did not perturb the alignment). The FA
runner resolves a sibling audio file from the `.cha` and feeds each utterance's
bullet span as the FA window. Standalone `.cha` inputs need utterance bullets
(BA2 derives them with UTR; here they come from the transcribe step).
