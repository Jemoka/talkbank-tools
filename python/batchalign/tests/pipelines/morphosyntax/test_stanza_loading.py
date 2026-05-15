"""Focused tests for worker-side Stanza language mapping."""

from __future__ import annotations

import logging
import sys
import types

import pytest
from batchalign.worker._stanza_capabilities import (
    StanzaCapabilityTable,
    StanzaLanguageCapability,
)
from batchalign.worker import _stanza_loading
from batchalign.worker._stanza_loading import (
    UnsupportedLanguageError,
    iso3_to_alpha2,
    load_stanza_models,
    should_request_mwt,
)


def test_iso3_to_alpha2_maps_known_languages() -> None:
    assert iso3_to_alpha2("eng") == "en"
    # ``yue`` (Cantonese) routes through the Chinese model. Both ``zh`` and
    # ``zh-hans`` are accepted by ``stanza.Pipeline`` (``zh`` is an alias
    # to ``zh-hans`` in resources.json), but we standardize on the
    # alias-resolved key so this matches the capability-table override
    # (``_ISO3_OVERRIDES`` in ``_stanza_capabilities.py``). Drift between
    # the two would re-create the 2026-05-06 ``mar`` failure shape.
    assert iso3_to_alpha2("yue") == "zh-hans"


def test_iso3_to_alpha2_preserves_existing_alpha2_codes() -> None:
    assert iso3_to_alpha2("en") == "en"
    assert iso3_to_alpha2("ja") == "ja"


def test_iso3_to_alpha2_leaves_unknown_iso3_unchanged(caplog) -> None:
    with caplog.at_level(logging.WARNING, logger="batchalign.worker"):
        assert iso3_to_alpha2("zzz") == "zzz"

    assert "Unknown ISO-639-3 code 'zzz' - passing through unchanged for Stanza" in caplog.text


# Regression test for the 2026-05-06 morphotag failure on
# ``childes-other-data/Biling/Gelman/Bystander/25.cha``.
#
# 25.cha has ``@Languages: eng, mar`` and one utterance with the
# ``[- mar]`` whole-utterance precode. The morphotag dispatcher correctly
# grouped that utterance under ``mar`` and the worker bootstrap then
# called ``load_stanza_models("mar")``. The bootstrap's preflight gate
# (``capability_table.supports_morphosyntax("mar")``) passes — the table
# is keyed by ISO-639-3 via pycountry, so it resolves ``mar`` to
# ``mr`` and reports "yes, Stanza ships morphosyntax for Marathi."
#
# But the *next* line is ``alpha2 = iso3_to_alpha2(lang)``, and
# ``iso3_to_alpha2`` only understands the hardcoded mapping. ``mar`` is
# not in the dict, so the function logs a warning and returns ``"mar"``
# unchanged. ``stanza.Pipeline(lang="mar", ...)`` then crashes with
# ``ValueError: Language mar is currently unsupported`` because Stanza's
# ``resources.json`` is keyed by alpha-2 and has no ``"mar"`` entry. The
# worker dies before emitting its ready signal; the file is recorded as
# a worker-bootstrap failure even though Stanza fully supports Marathi.
#
# The fix is to align ``iso3_to_alpha2`` with the same pycountry-backed
# resolution the capability table already uses. Hardcoded language
# tables in two places that need to agree but don't is the exact shape
# of the original bug.
def test_iso3_to_alpha2_resolves_marathi_via_pycountry() -> None:
    """``mar`` (Marathi) must convert to ``mr`` so Stanza's catalog finds it."""
    assert iso3_to_alpha2("mar") == "mr"


def test_iso3_to_alpha2_resolves_pycountry_languages_outside_hardcoded_dict() -> None:
    """Any ISO-639-3 code with a pycountry alpha-2 must resolve, not pass through.

    ``mar`` is the regression-driving case. Two more (``ben`` Bengali and
    ``hin`` Hindi already in the hardcoded dict, ``swa`` Swahili NOT in
    the dict) pin the broader principle so a future hardcoded-dict
    regression cannot pass without also breaking pycountry resolution.
    """
    # Bengali — already in the hardcoded dict; serves as control.
    assert iso3_to_alpha2("ben") == "bn"
    # Swahili — not in the hardcoded dict, must resolve via pycountry.
    assert iso3_to_alpha2("swa") == "sw"


# ---------------------------------------------------------------------------
# should_request_mwt — capability-driven processor selection
#
# Why this matters: ``load_stanza_models`` previously consulted a hardcoded
# ``MWT_LANGS`` set. That list said Swedish (``sv``) had MWT, but the actual
# Stanza catalog does not ship a Swedish MWT model — every Swedish worker
# spawn raised ``UnsupportedProcessorError`` and the language group failed.
# The 2026-04-15 overnight morphotag run lost an entire 500-file chunk to
# this. The principled fix is to query the capability table at runtime
# rather than maintaining a hand-edited mirror of Stanza's catalog.
# CLAUDE.md: "Per-language processor availability is determined by reading
# Stanza's resources.json at worker startup, NOT by hardcoded tables."
# ---------------------------------------------------------------------------


def _table_with(
    alpha2: str,
    *,
    has_tokenize: bool = True,
    has_pos: bool = True,
    has_lemma: bool = True,
    has_depparse: bool = True,
    has_mwt: bool,
) -> StanzaCapabilityTable:
    """Build a single-entry capability table for one alpha-2 language.

    Pure construction — no Stanza, no resources.json read. Lets us pin the
    helper's behavior without coupling tests to whatever upstream catalog
    happens to ship today.
    """
    cap = StanzaLanguageCapability(
        alpha2=alpha2,
        has_tokenize=has_tokenize,
        has_pos=has_pos,
        has_lemma=has_lemma,
        has_depparse=has_depparse,
        has_mwt=has_mwt,
    )
    return StanzaCapabilityTable(languages={"xxx": cap}, iso3_to_alpha2={"xxx": alpha2})


class TestSupportsMorphosyntax:
    def test_returns_true_for_full_processor_stack(self) -> None:
        table = _table_with("en", has_mwt=True)
        assert table.supports_morphosyntax("xxx") is True

    def test_returns_false_for_partial_processor_entry(self) -> None:
        table = _table_with(
            "pa",
            has_pos=False,
            has_lemma=False,
            has_depparse=False,
            has_mwt=False,
        )
        assert table.supports_morphosyntax("xxx") is False


class TestShouldRequestMwt:
    def test_returns_false_when_capability_table_lacks_mwt(self) -> None:
        # Swedish: Stanza ships tokenize/pos/lemma/depparse but NOT mwt.
        # Asking Stanza for mwt would raise UnsupportedProcessorError.
        table = _table_with("sv", has_mwt=False)
        assert should_request_mwt("sv", table) is False

    def test_returns_true_when_capability_table_has_mwt(self) -> None:
        # English: Stanza ships mwt; we should request it.
        table = _table_with("en", has_mwt=True)
        assert should_request_mwt("en", table) is True

    def test_returns_false_when_alpha2_not_in_table(self) -> None:
        # Conservative fallback: an unknown language is safer without mwt.
        # Stanza will at minimum tokenize/pos/lemma/depparse if those exist;
        # a missing mwt processor is the failure mode we're guarding against.
        table = _table_with("en", has_mwt=True)
        assert should_request_mwt("zz", table) is False

    def test_returns_false_when_table_is_none(self) -> None:
        # ``get_cached_capability_table()`` returns None when Stanza is not
        # importable. We must not request MWT in that case — there is no
        # way to confirm support, and a wrong guess crashes the worker.
        assert should_request_mwt("en", None) is False


def test_load_stanza_models_rejects_partial_processor_language_before_pipeline(
    monkeypatch: pytest.MonkeyPatch,
) -> None:
    table = StanzaCapabilityTable(
        languages={
            "pan": StanzaLanguageCapability(
                alpha2="pa",
                has_tokenize=True,
                has_pos=False,
                has_lemma=False,
                has_depparse=False,
                has_mwt=False,
            )
        },
        iso3_to_alpha2={"pan": "pa"},
    )
    monkeypatch.setattr(_stanza_loading, "get_cached_capability_table", lambda: table)

    fake_stanza = types.ModuleType("stanza")
    fake_stanza.DownloadMethod = types.SimpleNamespace(REUSE_RESOURCES=object())

    def _pipeline_should_not_run(*args, **kwargs):
        raise AssertionError("stanza.Pipeline must not be called for partial processor entries")

    fake_stanza.Pipeline = _pipeline_should_not_run
    monkeypatch.setitem(sys.modules, "stanza", fake_stanza)

    with pytest.raises(UnsupportedLanguageError) as exc_info:
        load_stanza_models("pan")

    assert "pan" in str(exc_info.value)
    assert "morphosyntax" in str(exc_info.value).lower()
