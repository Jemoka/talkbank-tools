"""Batchalign: ASR, forced alignment, and morphosyntax pipeline for CHAT transcripts.

Batchalign3 processes conversation audio files and transcripts in CHAT format
(Codes for the Human Analysis of Transcripts), the standard format of the
TalkBank project. The package provides inference functions for automatic speech
recognition (ASR), forced alignment, morphosyntactic analysis, speaker
diarization, utterance segmentation, translation, coreference resolution,
and audio feature extraction.

The CHAT parsing and serialization layer is implemented in Rust
(``batchalign_core``) for correctness and performance. All CHAT
manipulation goes through principled AST operations -- never ad-hoc
string or regex hacking.
"""

# batchalign supports CUDA and CPU inference backends. PyTorch's MPS fallback
# path is intentionally not enabled because the runtime does not select MPS.

import logging

# Library best practice: NullHandler so library users don't get
# "No handlers could be found" warnings.  The CLI adds its own handlers.
logging.getLogger("batchalign").addHandler(logging.NullHandler())

# Suppress noisy third-party loggers without touching the root logger
for _name in ("nemo_logger", "nemo", "pytorch_lightning"):
    logging.getLogger(_name).setLevel(logging.WARNING)

from .errors import *
