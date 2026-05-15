# NLP Engine Text Input Expectations

**Status:** Current
**Last updated:** 2026-05-02 09:12 EDT

Comprehensive reference for what text format each NLP engine/tool in batchalign3
expects as input, what preprocessing is applied, and what would break if raw
CHAT structural markers leaked through.

## Architecture Summary

Batchalign3 enforces a strict ownership boundary:

- **Rust** owns all CHAT parsing, text extraction, normalization, cache keying,
  and result injection.
- **Python** is a stateless ML inference server -- it receives structured data
  (word lists, audio paths, joined text) and returns raw model output.

NLP engines never see raw CHAT text. Every engine receives text that has
already passed through Rust's `Word::cleaned_text()` extraction, which strips
all CHAT markers at parse time.

### The `cleaned_text()` Contract

`Word::cleaned_text()` (defined in `talkbank-model/.../word_type.rs:285`)
concatenates only two `WordContent` variants:

- `WordContent::Text` -- base graphemes
- `WordContent::Shortening` -- elided material restored (e.g., `(be)cause` becomes `because`)

All other variants are excluded:

| Excluded variant | CHAT surface form | Example |
|---|---|---|
| `OverlapPoint` | overlap markers | `word` (with embedded overlap points removed) |
| `CAElement` | prosodic markers | removed |
| `CADelimiter` | paired prosodic markers | removed |
| `StressMarker` | primary/secondary stress | removed |
| `Lengthening` | syllable lengthening `:` | removed |
| `SyllablePause` | syllable pause `^` | removed |
| `UnderlineBegin/End` | underline control chars | removed |
| `CompoundMarker` | compound `+` | removed |

Additionally, the extraction layer (`extract.rs`) uses `ChatCleanedText` -- a
provenance newtype wrapping the output of `cleaned_text()`. The parallel
`ChatRawText` newtype preserves the original CHAT surface form for roundtrip
serialization but is never sent to NLP engines.

CHAT category prefixes (`&-`, `&~`, `&+`, `0`) and form-type markers (`@c`,
`@b`, `@s`, etc.) are also stripped by the parser before `cleaned_text()`
returns.

---

## Engine-by-Engine Reference

### 1. Whisper (OpenAI) -- ASR

**Task:** Automatic speech recognition (audio to text).

**Input:** Audio waveform (numpy array or file path). No text input.

**Output:** Raw text chunks with timestamps. Whisper produces its own text --
batchalign3 does not send any text to Whisper for ASR.

**Preprocessing by Rust:** None for ASR input (audio only). The language code
is converted from ISO 639-3 to a Whisper language name via
`iso3_to_language_name()` in `asr.py` (e.g., `eng` -> `english`, `yue` ->
`Cantonese`).

**Post-processing by Rust (`asr_postprocess/`):** The raw Whisper output goes
through a multi-stage Rust pipeline before becoming CHAT:

1. Compound merging (3,584 known compound pairs)
2. Multi-word splitting (space-separated tokens get timestamp interpolation)
3. Number expansion (digits to word form, 12 languages)
4. Cantonese normalization (simplified to traditional + domain replacements, `lang=yue` only)
5. Long turn splitting (>300 words)
6. Retokenization (split into utterances by punctuation)
7. Disfluency replacement (`um` -> `&-um`, `'cause` -> `(be)cause`)
8. N-gram retrace detection (repeated n-grams wrapped in `<...> [/]`)

**CHAT markers in output:** Not applicable -- Whisper produces raw natural
language text. CHAT structural notation is synthesized by Rust during CHAT
assembly.

**Unicode:** Whisper handles Unicode natively (multilingual model). Output
contains Unicode text for all supported languages.

**Source files:**
- `python/batchalign/inference/asr.py` (Python: `infer_whisper_prepared_audio()`, `_infer_whisper()`)
- `crates/batchalign/src/runner/dispatch/transcribe_pipeline.rs` (Rust: orchestration)
- `crates/talkbank-transform/src/asr_postprocess/` (Rust: post-processing pipeline)

---

### 2. Rev.AI -- ASR

**Task:** Cloud ASR (audio to text with speaker diarization).

**Input:** Audio file uploaded via HTTP multipart. No text input.

**Output:** JSON transcript with speaker-attributed monologues containing
timed word elements.

**Preprocessing by Rust:** The Rust `batchalign::revai` module
(`crates/batchalign/src/revai/`) handles:
- Audio file upload via `submit_local_file()` with retry logic
- Language specification (ISO 639-1 code or `"auto"` for auto-detection)
- Speaker count hint for diarization
- `skip_postprocessing` option

**Post-processing by Rust:** Rev.AI transcript words go through
`extract_timed_words()` in `client.rs` which:
- Sorts elements by start timestamp
- Trims whitespace from word values
- Filters empty values
- Converts seconds to milliseconds

The resulting timed words then enter the same Rust ASR post-processing
pipeline as Whisper output (compound merging, number expansion, etc.).

**CHAT markers in output:** Not applicable -- Rev.AI produces raw natural
language text.

**Unicode:** Rev.AI handles Unicode natively.

**Source files:**
- `crates/batchalign/src/revai/client.rs` (Rust: HTTP client, transcript extraction)
- `crates/batchalign/src/revai/types.rs` (Rust: API types)
- `crates/batchalign/src/transcribe/infer.rs` (Rust: dispatch)

---

### 3. Stanza (Stanford NLP) -- Morphosyntax

**Task:** POS tagging, lemmatization, dependency parsing.

**Input:** Space-joined cleaned words, one utterance per "sentence", multiple
sentences separated by `\n\n`.

**What Stanza receives:**

```text
cleaned_word1 cleaned_word2 cleaned_word3\n\ncleaned_word4 cleaned_word5
```

This is constructed in `morphosyntax.py` line 272:

```python
text = " ".join(words).strip()
```

Then multiple utterances are joined:

```python
combined = "\n\n".join(texts)
```

**Preprocessing pipeline (Rust -> Python):**

1. **Rust parses CHAT** into AST via `parse_lenient()`
2. **Rust extracts words** via `collect_utterance_content()` using `TierDomain::Mor`
3. Each word's `cleaned_text()` is collected -- this strips all CHAT markers
4. Word texts are sent as `Vec<String>` in the `MorphosyntaxBatchItem`
5. **Python joins** words with spaces: `" ".join(words).strip()`

Note: The `morphosyntax/preprocess.rs` file defines `prepare_text()` which
also strips parentheses, but this is used only in certain Rust-side paths.
The Python `morphosyntax.py` explicitly avoids stripping parentheses (line
268-271 comment: "Do NOT strip parentheses here -- Rust cleaned_text() already
handles CHAT notation").

**Stanza pipeline configuration (`_stanza_loading.py`):**

| Language | `tokenize_pretokenized` | `tokenize_no_ssplit` | `tokenize_postprocessor` | MWT |
|---|---|---|---|---|
| Japanese (`ja`) | `True` | `True` | None | No |
| Non-MWT languages | `True` | `True` | None | No |
| English (`en`) | (model decides) | `True` | Custom realigner | Yes (gum) |
| MWT languages (fr, de, it, ...) | (model decides) | `True` | Custom realigner | Yes |

**Key Stanza behaviors:**

- **`tokenize_pretokenized=True`**: Stanza preserves the input word boundaries
  exactly. "ice-cream" stays as one token. "don't" stays as one token.
  Stanza does not re-tokenize.

- **`tokenize_no_ssplit=True`**: Stanza treats `\n\n` as sentence boundaries
  (matching batchalign's one-utterance-per-sentence convention) but does not
  attempt automatic sentence splitting within a sentence.

- **MWT languages with `tokenize_postprocessor`**: Stanza's neural tokenizer
  runs but a custom realignment callback (`_tokenizer_realign.py`) merges
  any spurious splits back to match the original CHAT words. English
  contractions (e.g., "don't") are flagged as MWT candidates so Stanza's
  MWT processor can expand them (do + n't). Words like "o'clock" are
  suppressed from MWT expansion.

- **Mandarin retokenize mode** (`--retokenize` with `cmn`/`zho`): A separate
  Stanza pipeline with `tokenize_pretokenized=False` is loaded. Words are
  joined without spaces (`"".join(words)`) and Stanza's neural tokenizer
  segments them.

- **Cantonese retokenize mode** (`--retokenize` with `yue`): PyCantonese's
  `segment()` function re-segments per-character tokens before Stanza
  processes them. Only runs when all CJK tokens are single characters
  (per-character ASR output).

**What would break with CHAT markers:**

- Stanza's tokenizer would split on structural markers (e.g., `word` containing
  embedded overlap points, or form-type suffixes like `@c`), producing
  incorrect token counts that would cause word-count mismatches in the
  injection layer.
- POS tagging would be severely degraded -- unknown tokens containing `@`, `+`,
  Unicode prosodic markers would all get `X` (unknown) POS tags.
- Lemmatization would fail to find dictionary entries.
- Dependency parsing would produce garbage structures.

**Unicode:** Stanza handles Unicode correctly. All text is already Unicode
(CHAT is UTF-8).

**Source files:**
- `python/batchalign/inference/morphosyntax.py` (Python: `batch_infer_morphosyntax()`)
- `python/batchalign/inference/_tokenizer_realign.py` (Python: MWT realignment)
- `python/batchalign/worker/_stanza_loading.py` (Python: pipeline configuration)
- `crates/talkbank-transform/src/morphosyntax/payload.rs` (Rust: payload collection)
- `crates/talkbank-transform/src/extract.rs` (Rust: word extraction)

---

### 4. Stanza -- Utterance Segmentation (Constituency Parsing)

**Task:** Constituency parsing to determine utterance boundaries.

**Input:** Space-joined cleaned words, one utterance per call.

**What Stanza receives:**

```python
doc = nlp(" ".join(item.words))
```

(from `utseg.py` line 120)

**Preprocessing:** Same as morphosyntax -- words arrive as `Vec<String>` of
cleaned text from Rust extraction, joined with spaces in Python.

**Stanza pipeline:** Configured with `tokenize_pretokenized=True` (preserves
word boundaries) and constituency parsing enabled. The pipeline is constructed
per-request via `build_stanza_config_from_langs()` in `_stanza_loading.py`.

**Output:** Constituency tree bracket notation strings. Rust parses the tree
strings and computes word-to-utterance assignments.

**Source files:**
- `python/batchalign/inference/utseg.py` (Python: `batch_infer_utseg()`)
- `python/batchalign/worker/_stanza_loading.py` (Python: `load_utseg_builder()`)
- `crates/batchalign/src/utseg.rs` (Rust: assignment computation)

---

### 5. Stanza -- Coreference Resolution

**Task:** Coreference chain detection (English only).

**Input:** Pre-tokenized sentences (list of word lists) joined as:

```python
text = "\n\n".join(" ".join(s) for s in item.sentences)
```

(from `coref.py` line 97)

**Stanza pipeline:** Configured with `tokenize_pretokenized=True` and
processors `"tokenize, coref"` with the
`ontonotes-singletons_roberta-large-lora` package.

**Preprocessing:** Words arrive from Rust as `list[list[str]]` (sentences of
cleaned words). Python joins them and Stanza preserves the tokenization.

**Output:** Per-word coreference chain annotations with chain IDs and
start/end flags. Rust builds CHAT `%xcoref` bracket notation from these.

**Source files:**
- `python/batchalign/inference/coref.py` (Python: `batch_infer_coref()`)
- `crates/batchalign/src/coref.rs` (Rust: injection)

---

### 6. Whisper / Wave2Vec -- Forced Alignment

**Task:** Align known words to audio to produce word-level timestamps.

**Input (Whisper FA):** Audio chunk (tensor) + space-joined words as a single
string:

```python
detokenized = " ".join(item.words)
detokenized = detokenized.replace("_", " ").strip()
```

(from `fa.py` lines 344-345)

Whisper FA uses `handle.processor()` which tokenizes the text internally. The
text is fed through as a "forced" transcription target.

**Input (Wave2Vec FA):** Audio chunk (tensor) + word list. Wave2Vec
operates at the character level:

```python
transcript = torch.tensor([
    dictionary.get(c, dictionary["*"])
    for word in words
    for c in word.lower()
])
```

(from `fa.py` lines 255-258)

Each word is lowercased and decomposed into individual characters for CTC
forced alignment. Unknown characters map to the `*` wildcard token.

**Preprocessing by Rust:** Words are extracted from the CHAT AST using
`TierDomain::Wor` (the %wor alignment domain). `cleaned_text()` is used.
Rust groups words by audio time windows before sending to Python.

**What would break with CHAT markers:**
- Whisper FA: The text would not match the audio content, producing garbage
  alignments or alignment failures.
- Wave2Vec FA: CHAT markers like `@`, `+`, Unicode prosodic markers would
  map to `*` (wildcard), corrupting the character-level CTC alignment.

**Source files:**
- `python/batchalign/inference/fa.py` (Python: `infer_whisper_fa()`, `infer_wave2vec_fa()`)
- `crates/batchalign/src/chat_ops/fa/extraction.rs` (Rust: word extraction for FA)
- `crates/batchalign/src/chat_ops/fa/` (Rust: grouping, injection, postprocess)

---

### 7. Cantonese FA (Wave2Vec + PyCantonese Jyutping)

**Task:** Forced alignment for Cantonese, with hanzi-to-jyutping preprocessing.

**Input:** Audio chunk + word list. Before alignment, hanzi characters are
converted to Jyutping romanization (tone-stripped, syllables joined with
apostrophes) via PyCantonese's `characters_to_jyutping()`. Non-Cantonese
words pass through unchanged.

**Preprocessing:** Same Rust extraction as standard FA (`TierDomain::Wor`,
`cleaned_text()`), then Python-side jyutping conversion.

**Source files:**
- `python/batchalign/inference/languages/cantonese/_cantonese_fa.py` (Python)

---

### 8. Tencent Cloud ASR -- Cantonese/Mandarin ASR

**Task:** Cloud ASR for Cantonese and Mandarin.

**Input:** Audio file path. No text input -- this is pure ASR.

**Output:** Speaker-attributed monologues with timed word elements. The
`TencentRecognizer` uploads audio to Tencent COS, submits an ASR task,
polls for completion, and returns raw results.

**Post-processing:** Raw Tencent output is returned as `MonologueAsrResponse`
to Rust. Rust then applies the standard ASR post-processing pipeline
(compound merging, number expansion, Cantonese normalization via `ferrous-opencc`, etc.).

**Cantonese normalization:** Tencent returns simplified Chinese characters.
Rust's `cantonese.rs` module applies:
1. `ferrous-opencc` crate: simplified to traditional Chinese conversion
2. 31-entry Aho-Corasick domain replacement table for Cantonese-specific
   character corrections

**Source files:**
- `python/batchalign/inference/languages/cantonese/_tencent_asr.py` (Python: transport)
- `python/batchalign/inference/languages/cantonese/_tencent_api.py` (Python: Tencent SDK wrapper)
- `crates/talkbank-transform/src/asr_postprocess/cantonese.rs` (Rust: normalization)

---

### 9. Aliyun NLS -- Cantonese ASR

**Task:** Cloud ASR for Cantonese via Aliyun's streaming websocket API.

**Input:** WAV audio streamed over websocket. No text input.

**Output:** Sentence-level results with per-word timestamps. The websocket
`SentenceEnd` callbacks return `AliyunSentenceResult` objects containing
words and sentence text.

**Post-processing:** Raw Aliyun sentences are projected to the standard
monologue format via `batchalign_core.aliyun_sentences_to_asr()` (a Rust
function exposed to Python). Rust owns the sentence-only fallback
tokenization plus the monologue/timed-word projection. Standard ASR
post-processing (including Cantonese normalization) follows.

**Source files:**
- `python/batchalign/inference/languages/cantonese/_aliyun_asr.py` (Python: websocket transport)

---

### 10. FunASR (SenseVoice) -- Cantonese/Multilingual ASR

**Task:** Local ASR using the FunASR/SenseVoice model.

**Input:** Audio file path. No text input.

**Output:** Speaker-attributed monologues with timed elements, returned as
`MonologueAsrResponse`. The `FunAudioRecognizer` wraps the FunASR `AutoModel`
with VAD (voice activity detection) and timestamp output.

**Post-processing:** Same as other ASR engines -- Rust applies compound
merging, number expansion, Cantonese normalization, etc.

**Source files:**
- `python/batchalign/inference/languages/cantonese/_funaudio_asr.py` (Python: provider)
- `python/batchalign/inference/languages/cantonese/_funaudio_common.py` (Python: FunASR model wrapper)

---

### 11. PyCantonese -- Cantonese Word Segmentation and POS

**Task:** (a) CJK word segmentation for Cantonese; (b) POS tag override for
Cantonese.

**Input (segmentation):** Joined string of per-character tokens:

```python
text = "".join(words)
return pycantonese.segment(text)
```

(from `morphosyntax.py` line 119)

Only runs when `--retokenize` is requested for `lang=yue` AND all CJK tokens
are single characters (indicating per-character ASR output). Multi-character
CJK tokens are preserved as-is.

**Input (POS override):** List of word texts extracted from Stanza's UD output:

```python
texts = [w.get("text", "") for w in ud_words]
tagged = pycantonese.pos_tag(texts)
```

(from `morphosyntax.py` lines 136-140)

This runs for ALL Cantonese morphotag operations (not just retokenize),
replacing Stanza's `upos` tags with PyCantonese's POS tags. Stanza's
Mandarin-trained model scores ~50% on Cantonese vocabulary; PyCantonese
scores ~94%.

**What would break with CHAT markers:** PyCantonese's segmenter would produce
incorrect word boundaries if CHAT markers were embedded in the input string.
The POS tagger would assign incorrect tags to tokens containing `@`, `+`, etc.

**Source files:**
- `python/batchalign/inference/morphosyntax.py` (Python: `_segment_cantonese()`, `_override_pos_with_pycantonese()`)

---

### 12. Google Translate / Seamless M4T -- Translation

**Task:** Translate utterance text to English.

**Input:** Space-joined cleaned words as a single string:

```python
translated = _translate(item.text, src_lang)
```

(from `translate.py` line 72)

The `text` field arrives pre-processed from Rust. Rust collects translation
payloads via `collect_translate_payloads()` in `translate.rs`, which joins
extracted `ChatCleanedText` words with spaces.

**What would break with CHAT markers:** Translation quality would degrade --
the model would attempt to translate CHAT notation as natural language. CHAT
markers like `&-um`, `(be)cause`, `@s:eng` would produce nonsense translations.

**Source files:**
- `python/batchalign/inference/translate.py` (Python: `batch_infer_translate()`)
- `crates/batchalign/src/translate.rs` (Rust: payload collection and injection)

---

### 13. Pyannote / NeMo -- Speaker Diarization

**Task:** Identify speaker turns from audio.

**Input:** Audio waveform (numpy array) + sample rate. No text input.

**Output:** Timestamped speaker segments with speaker IDs.

**Source files:**
- `python/batchalign/inference/speaker.py` (Python: `infer_speaker_prepared_audio()`)

---

## Summary Table

| Engine | Task | Input Type | Text Format | Pre-tokenized? | CHAT Markers Safe? |
|---|---|---|---|---|---|
| Whisper ASR | ASR | Audio only | N/A | N/A | N/A |
| Rev.AI | ASR | Audio only | N/A | N/A | N/A |
| Stanza (morphosyntax) | POS/lemma/dep | Space-joined words | `cleaned_text()` | Yes (most langs) | No |
| Stanza (utseg) | Constituency | Space-joined words | `cleaned_text()` | Yes | No |
| Stanza (coref) | Coreference | Space-joined sentences | `cleaned_text()` | Yes | No |
| Whisper FA | Alignment | Space-joined words + audio | `cleaned_text()` | N/A (decoder input) | No |
| Wave2Vec FA | Alignment | Word list + audio | `cleaned_text()`, lowercased | N/A (char-level CTC) | No |
| Cantonese FA | Alignment | Word list + audio | `cleaned_text()`, jyutping | N/A | No |
| Tencent ASR | ASR | Audio only | N/A | N/A | N/A |
| Aliyun NLS | ASR | Audio only | N/A | N/A | N/A |
| FunASR | ASR | Audio only | N/A | N/A | N/A |
| PyCantonese (seg) | Segmentation | Joined chars | `cleaned_text()`, joined | Own tokenizer | No |
| PyCantonese (POS) | POS tagging | Word list | Stanza output text | N/A | No |
| Google Translate | Translation | Joined text | `cleaned_text()` | N/A | No |
| Pyannote/NeMo | Diarization | Audio only | N/A | N/A | N/A |

## CHAT Structural Markers That Are Stripped

For reference, these are the CHAT markers that `cleaned_text()` and the
extraction pipeline remove before any text reaches an NLP engine:

| Marker | Example | Meaning |
|---|---|---|
| Category prefixes | `&-um`, `&~well`, `&+oh`, `0word` | Filled pause, filler, phonological fragment, omitted word |
| Form-type suffixes | `word@c`, `word@b`, `word@s:eng` | Child form, babbling, second language |
| Overlap points | (Unicode markers within words) | Conversational analysis overlap |
| CA prosodic markers | (Unicode markers within words) | Pitch, stress, prosody |
| Lengthening | `wo:rd` | Lengthened syllable |
| Syllable pause | `wo^rd` | Pause within word |
| Compound marker | `ice+cream` | Compound word boundary |
| Stress markers | (Unicode markers) | Primary/secondary stress |
| Shortening parens | `(be)cause` | Omitted sound (restored to `because`) |
| Timing bullets | `[bullet ranges]` | Word-level timestamps |
| Scoped annotations | `[= text]`, `[: text]`, `[!]`, etc. | Error coding, replacement, emphasis |

All of these are parsed into structured AST nodes by the CHAT parser and
are excluded from `cleaned_text()`, which concatenates only `WordContent::Text`
and `WordContent::Shortening` elements.
