"""Utterance-model bootstrap helpers for worker startup."""

from __future__ import annotations

import logging

from batchalign.inference._domain_types import LanguageCode
from batchalign.models.utterance import BertUtteranceModel, resolve_utterance_model
from batchalign.worker._types import _state

L = logging.getLogger("batchalign.worker")


def load_utterance_model(lang: LanguageCode) -> None:
    """Load the BA2 utterance model for one language when available."""
    _state.utterance_boundary_model = None
    _state.utterance_model_name = ""

    model_name = resolve_utterance_model(lang)
    if model_name is None:
        L.info("No utterance boundary model configured for %s", lang)
        return

    _state.utterance_boundary_model = BertUtteranceModel(model_name, lang=lang)
    _state.utterance_model_name = model_name
    L.info("Loaded utterance boundary model %s for %s", model_name, lang)
