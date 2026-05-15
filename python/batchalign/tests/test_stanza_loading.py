"""Worker-side bootstrap policy for Stanza model loading.

Pins the contract that ``load_stanza_models`` performs a typed
preflight check against the capability table BEFORE calling
``stanza.Pipeline``: languages that Stanza's installed catalog
lists but ships no processor packages for must surface as a
domain ``UnsupportedLanguageError`` rather than as a deep
``KeyError`` from inside Stanza's resource-list loader.
"""

# affects: python/batchalign/worker/_stanza_loading.py
# affects: python/batchalign/worker/_stanza_capabilities.py

from __future__ import annotations

import pytest

from batchalign.inference._domain_types import LanguageCode
from batchalign.worker._stanza_loading import (
    UnsupportedLanguageError,
    load_stanza_models,
)


def test_load_stanza_models_rejects_language_with_no_packages() -> None:
    """Loading a language Stanza has no packages for must raise a
    typed UnsupportedLanguageError before reaching stanza.Pipeline.
    """
    with pytest.raises(UnsupportedLanguageError) as exc_info:
        load_stanza_models(LanguageCode("mal"))
    msg = str(exc_info.value).lower()
    assert "mal" in msg
    # The error should be actionable — name the language, not just
    # bubble up a deep library-internal KeyError.
    assert "stanza" in msg or "packages" in msg or "support" in msg


def test_load_stanza_models_rejects_completely_unknown_code() -> None:
    """Likewise for codes that have no Stanza entry at all (not even
    a stub). Must surface as the typed error.
    """
    with pytest.raises(UnsupportedLanguageError):
        load_stanza_models(LanguageCode("xyz"))
