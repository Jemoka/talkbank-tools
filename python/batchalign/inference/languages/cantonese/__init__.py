"""HK/Cantonese ASR and forced alignment engines.

Engines:
- tencent: Tencent Cloud ASR (cloud, requires credentials)
- aliyun: Aliyun NLS ASR (cloud, Cantonese-only, requires credentials)
- funaudio: FunASR/SenseVoice (local, no credentials)
- wav2vec_canto: Cantonese FA with jyutping preprocessing (local, no credentials)

These are built-in engines selected via --engine-overrides, e.g.:
    batchalign3 transcribe --engine-overrides '{"asr": "tencent"}'
"""
