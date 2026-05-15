# Test Fixture Provenance

**Status:** Current
**Last updated:** 2026-03-18

Every test fixture's origin, extraction method, and licensing context.
All source material is from TalkBank corpora under CC-BY-NC-SA or
institutional agreement. Test fixtures are minimal excerpts for
automated regression testing only.

## Audio Clips

| Fixture | Source corpus | Source file | Language | Extraction | Duration | Size |
|---------|-------------|------------|----------|-----------|----------|------|
| `test.mp3` | (original recording) | — | eng | Pre-existing, committed by Chen | 27s | 441KB |
| `eng_multi_speaker.mp3` | CHILDES Eng-NA MacWhinney | `data/childes-data/Eng-NA/MacWhinney/010411a.mp3` | eng | `ffmpeg -ss 20 -t 18` from corpus mp3 | 18s | 99KB |
| `spa_marrero_clip.mp3` | CHILDES Spanish Marrero | `data/childes-data/Spanish/Marrero/Idaira/040707.mp3` | spa | `prepare_corpus_media_fixture.py --lines 1-5` via net | 8s | 127KB |
| `fra_geneva_clip.mp3` | CHILDES French Geneva | `data/childes-data/French/Geneva/020303.mp3` | fra | `prepare_corpus_media_fixture.py --lines 4-10` via net | 4s | 34KB |
| `jpn_tyo_clip.mp3` | Aphasia Japanese TYO | `data/aphasia-data/Japanese/Protocol/TYO/PWA/TYO_a1.mp4` | jpn | `prepare_corpus_media_fixture.py --lines 1-8` via net, converted mp4→mp3 | 21s | 219KB |
| `yue_hku_clip.mp3` | Aphasia Cantonese HKU | `data/aphasia-data/Cantonese/Protocol/HKU/PWA/A023.mp4` | yue | `prepare_corpus_media_fixture.py --lines 11-16 --context 2` via net, converted mp4→mp3 | 26s | 353KB |
| `biling_vec_hrv_clip.mp3` | Bilingual C-ORAL-IC | `data/biling-data/C-ORAL-IC/2020_01.mp3` | vec, hrv | `prepare_corpus_media_fixture.py --lines 1-8` via net | 23s | 374KB |
| `biling_cat_spa_clip.mp3` | CHILDES Romance Catalan Jordina | `data/childes-data/Romance/Catalan/Jordina/010911.mp4` | cat, spa | `ffmpeg -t 15 -vn` piped from net | 13s | 225KB |

Media on net is accessed via `ssh operator@server` from the volumes listed in
`~/.batchalign3/server.yaml` media_mappings. The `prepare_corpus_media_fixture.py`
script (in `scripts/analysis/`) handles media resolution, download, and trimming
automatically.

## CHAT Fixtures (`parity/`)

### Constructed (not from corpus)

| Fixture | Language | Key features | Rationale |
|---------|----------|-------------|-----------|
| `eng_disfluency.cha` | eng | Filled pauses (um, uh), retraces `[/]`, reformulations `[//]`, false starts `[///]`, replacements (cuz, mm-hmm, 'em) | Tests D1/D1b disfluency/retrace pipeline stages |
| `eng_retokenize.cha` | eng | MWT candidates: gonna, wanna, 'em, cuz, mm-hmm | Tests retokenization DP alignment |
| `eng_overlap_ca.cha` | eng | `+<` overlaps, `[>]`/`[<]` scope markers, `www`, multi-speaker | Tests CA overlap handling |
| `eng_clinical_aphasia.cha` | eng | Error codes `[*]`, `+...` trailing, reformulations `[//]` | Tests clinical notation passthrough |
| `eng_bilingual.cha` | eng, spa | `@Languages: eng, spa`, `@s:spa` code-switch, `[- spa]` | Tests bilingual language routing |
| `eng_complex_tiers.cha` | eng | Pre-existing %mor/%gra, timing bullets, retraces, filled pauses | Tests tier preservation and re-annotation |
| `spa_simple.cha` | spa | Basic Spanish multi-utterance | Tests Spanish Stanza model routing |
| `spa_clinical.cha` | spa | Numbered overlaps `[>1]`/`[<1]`, action codes `[=! ...]`, `&=carraspea` | Tests Spanish clinical notation |
| `fra_simple.cha` | fra | Basic French multi-utterance | Tests French Stanza model routing |
| `deu_clinical.cha` | deu | Error codes `[*]`, `+...`, German morphology | Tests German model routing |
| `jpn_clinical.cha` | jpn | `+/.` unfinished, Japanese script | Tests Japanese model routing |
| `yue_timed.cha` | yue | `xxx` unintelligible, Cantonese script | Tests Cantonese model routing |

### Excerpted from corpus (via `prepare_corpus_media_fixture.py`)

| Fixture | Source | Lines extracted | Language | Key features |
|---------|--------|----------------|----------|-------------|
| `eng_multi_speaker.cha` | CHILDES Eng-NA MacWhinney `010411a.cha` | 1-12 (re-timed from 20s offset) | eng | 3 speakers (FAT/CHI/MOT), overlaps `[>]`/`[<]`, interruptions `+/.`/`+,`, filled pause `&-uh`, language switch `@s:hun`, child speech age 1;04 |
| `spa_marrero_timed.cha` | CHILDES Spanish Marrero `040707.cha` | 1-5 | spa | 4 participants, `(es)cucha` phonological reduction, `xxx` unintelligible, `%com` tier |
| `fra_geneva_timed.cha` | CHILDES French Geneva `020303.cha` | 4-10 | fra | Mother-child, `d@u` dialect form, `xxx`, child age 2;03 |
| `jpn_tyo_timed.cha` | Aphasia Japanese TYO `TYO_a1.cha` | 1-8 | jpn | Clinical interview, `+/.` interruption, `+//.` reformulation, Broca's aphasia participant age 61;03 |
| `yue_hku_timed.cha` | Aphasia Cantonese HKU `A023.cha` | 11-16 (+2 context) | yue | Anomic aphasia, `xxx` unintelligible, romanized Cantonese (gam2, zau6), `+...` trailing, `%wor` timing |
| `biling_vec_hrv_timed.cha` | Bilingual C-ORAL-IC `2020_01.cha` | 1-8 | vec, hrv | 3 speakers, `[/]` retrace, code-switching between Venetian and Croatian, `%xmor` extended tier |

## BA2 Golden Reference Outputs (`golden/ba2_reference/`)

Generated by `scripts/generate_ba2_golden.sh` using `batchalignjan9`
(BA2 Jan 9 baseline, commit `84ad500b`). See `golden/ba2_reference/KNOWN_DIFFS.md`
for documented divergences.

| Command | Files generated | Notes |
|---------|----------------|-------|
| `morphotag` | 13 (all fixtures) | No `--lang` flag (BA2 reads `@Languages`) |
| `morphotag_retok` | 7 (English only) | With `--retokenize` flag |
| `utseg` | 11 of 13 | French and Cantonese constituency models unavailable |
| `translate` | 13 (all fixtures) | No `--lang` flag |
| `coref` | 0 | Stanza version incompatibility (`plateau_epochs` error) |

## Regeneration

```bash
# Regenerate BA2 golden outputs:
bash scripts/generate_ba2_golden.sh

# Create a new trimmed fixture from a corpus file:
python3 scripts/analysis/prepare_corpus_media_fixture.py \
    data/REPO/path/to/file.cha \
    --lines START-END \
    --output /tmp/fixture_name \
    --padding-ms 1000

# Then copy results to test support:
cp /tmp/fixture_name/trimmed/*.mp3 python/batchalign/tests/support/NAME_clip.mp3
cp /tmp/fixture_name/trimmed/*.cha python/batchalign/tests/support/parity/NAME_timed.cha
# Update this PROVENANCE.md with the new entry.
```
