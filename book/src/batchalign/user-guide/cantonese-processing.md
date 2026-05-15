# Cantonese Engines

**Status:** Current
**Last updated:** 2026-05-11 15:46 EDT

Batchalign includes alternative ASR and forced alignment engines for Cantonese.
These are built-in modules activated via `--engine-overrides` and shipped in
the base package.

## Available Engines

| Engine | Task | Description |
|--------|------|-------------|
| `tencent` | ASR | Tencent Cloud speech recognition with speaker diarization |
| `aliyun` | ASR | Alibaba Cloud NLS real-time speech recognition (Cantonese only) |
| `funaudio` | ASR | FunASR/SenseVoice local model (no cloud credentials needed) |
| `wav2vec_canto` | FA | Cantonese forced alignment with jyutping preprocessing |

## Installation

The standard install already includes these engines:

```bash
uv tool install batchalign3
```

For a source checkout, the standard build (`cargo build -p batchalign-cli`
plus `uv run maturin develop` for the PyO3 bridge) already includes
these engines. There are no Cantonese-specific extras to install.

## Usage

Select an alternative engine with `--engine-overrides`:

```bash
# Transcribe with Tencent Cloud ASR
batchalign3 transcribe input/ -o output/ --lang yue \
  --engine-overrides '{"asr": "tencent"}'

# Transcribe with FunASR (local, no credentials)
batchalign3 transcribe input/ -o output/ --lang yue \
  --engine-overrides '{"asr": "funaudio"}'

# Benchmark against a gold CHAT companion in the input directory
batchalign3 benchmark input/ --output output/ --lang yue -n 1 \
  --engine-overrides '{"asr": "tencent"}'

# Force align with Cantonese FA engine
batchalign3 align input/ -o output/ --lang yue \
  --engine-overrides '{"fa": "wav2vec_canto"}'
```

## Credential Configuration

Cloud engines (Tencent, Aliyun) require API credentials in
`~/.batchalign.ini`:

### Tencent Cloud

```ini
[asr]
engine.tencent.id = <secret-id>
engine.tencent.key = <secret-key>
engine.tencent.region = ap-guangzhou
engine.tencent.bucket = <cos-bucket-name>
```

### Aliyun NLS

```ini
[asr]
engine.aliyun.ak_id = <access-key-id>
engine.aliyun.ak_secret = <access-key-secret>
engine.aliyun.ak_appkey = <appkey>
```

Missing or empty credentials raise `ConfigError` with a clear message
indicating which keys are needed.

## Cantonese Text Normalization

All Cantonese ASR output is automatically normalized from simplified/mixed
Chinese to Traditional Chinese. This normalization:

1. **Simplified → Traditional** via the `ferrous-opencc` Rust engine
   (embedded OpenCC `S2hk` conversion tables)
2. **Domain-specific corrections** via a 31-entry replacement table for
   Cantonese character variants (e.g., 系→係, 呀→啊, 中意→鍾意)

Normalization is built into the Rust extension (`batchalign_core`) and runs
automatically during ASR post-processing for `lang=yue`. No additional Python
dependencies (like OpenCC) are required.

## Engine Details

### Tencent Cloud ASR

- Supports speaker diarization with configurable speaker count
- Uploads audio to COS (Tencent Cloud Object Storage), submits ASR job, polls
  for results
- 10-minute safety timeout on ASR polling
- Automatic COS cleanup after transcription
- Per-word timestamps with speaker attribution

### Aliyun NLS ASR

- Cantonese only (`lang=yue` required, other languages rejected at load time)
- WebSocket streaming with real-time sentence callbacks
- Automatic token refresh (23-hour TTL)
- WAV format input required (16 kHz mono)
- Shared result shaping and Cantonese fallback tokenization happen in Rust,
  not in the Python transport adapter

### FunASR/SenseVoice

- Local model — no cloud credentials, no network required
- Default model is `FunAudioLLM/SenseVoiceSmall`. Pass
  `--engine-overrides '{"asr": "funaudio", "funaudio_model": "<hf-id>"}'`
  to swap to a different FunASR model (e.g. a Paraformer variant); the
  loader's downstream code branches on whether the chosen model name
  contains `paraformer`.
- VAD (Voice Activity Detection) built in via `fsmn-vad`
- Per-character Cantonese tokenization for timestamp alignment

### Cantonese FA

- Converts Chinese characters to jyutping romanization (via pycantonese)
- Strips tones from jyutping (Wave2Vec MMS expects toneless input)
- Runs Wave2Vec forced alignment on the romanized text
- Maps word-level timings back to original Chinese characters

## See Also

- [Cantonese and CJK — Architecture](../../architecture/language-and-multilingual/cantonese-and-cjk.md) — engine architecture, normalization pipeline, segmenter selection
- [Adding Inference Providers](../developer/adding-engines.md) — how to add new built-in engines
