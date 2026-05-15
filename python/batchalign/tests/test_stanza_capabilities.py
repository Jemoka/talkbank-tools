"""Tests for the Stanza capability table builder.

These tests use a deterministic resources fixture instead of depending on a
user-level Stanza cache on the CI runner.
"""

from __future__ import annotations

from unittest import mock

import pytest

from batchalign.worker._stanza_capabilities import (
    StanzaCatalogDownloadError,
    build_stanza_capability_table_from_resources,
    get_cached_capability_table,
)


_FIXTURE_RESOURCES = {
    "default": {},
    "en": {
        "tokenize": {},
        "pos": {},
        "lemma": {},
        "depparse": {},
        "mwt": {},
        "constituency": {},
    },
    "es": {
        "tokenize": {},
        "pos": {},
        "lemma": {},
        "depparse": {},
    },
    "fr": {
        "tokenize": {},
        "pos": {},
        "lemma": {},
        "depparse": {},
        "mwt": {},
    },
    "ja": {
        "tokenize": {},
        "pos": {},
        "lemma": {},
        "depparse": {},
        "constituency": {},
    },
    "nl": {
        "tokenize": {},
        "pos": {},
        "lemma": {},
        "depparse": {},
        "mwt": {},
    },
    "alias-en": "en",
}


def build_fixture_table():
    return build_stanza_capability_table_from_resources(
        _FIXTURE_RESOURCES,
        stanza_version="test-stanza",
    )


def test_table_is_non_empty():
    """Fixture resources should produce a non-empty capability table."""
    table = build_fixture_table()
    assert set(table.languages) >= {"eng", "fra", "jpn", "nld", "spa"}
    assert table.stanza_version == "test-stanza"


def test_english_has_constituency():
    """English is one of the fixture languages with constituency parsing."""
    table = build_fixture_table()
    assert "eng" in table.languages
    assert table.languages["eng"].has_constituency


def test_dutch_has_no_constituency():
    """Dutch does NOT have constituency — this caused an operator's crash."""
    table = build_fixture_table()
    assert "nld" in table.languages
    assert not table.languages["nld"].has_constituency


def test_dutch_has_core_processors():
    """Dutch has tokenize, pos, lemma, depparse — morphotag should work."""
    table = build_fixture_table()
    nl = table.languages["nld"]
    assert nl.has_tokenize
    assert nl.has_pos
    assert nl.has_lemma
    assert nl.has_depparse


def test_iso3_mapping_covers_fixture_languages():
    """The derived iso3 mapping should cover every fixture language entry."""
    table = build_fixture_table()
    expected = {"eng", "fra", "jpn", "nld", "spa"}
    missing = expected - set(table.iso3_to_alpha2.keys())
    assert not missing, f"Fixture languages should map cleanly: {missing}"


def test_mwt_matches_resources():
    """MWT availability should come from resources, not a hardcoded list."""
    table = build_fixture_table()
    assert table.languages["fra"].has_mwt
    assert table.languages["eng"].has_mwt
    assert not table.languages["jpn"].has_mwt


def test_japanese_has_constituency():
    """Japanese has constituency parsing in the fixture resources."""
    table = build_fixture_table()
    ja = table.languages["jpn"]
    assert ja.has_constituency


def test_unsupported_language_not_in_table():
    """Languages absent from the fixture should not appear."""
    table = build_fixture_table()
    assert "que" not in table.languages
    assert "jam" not in table.languages


# ---------------------------------------------------------------------------
# Bootstrap-on-missing tests (the on-demand download contract).
#
# The contract: a fresh install with no Stanza cache must still produce a
# usable capability table. We test the get_cached_capability_table() wrapper
# under three states:
#
#   1. resources.json missing → download succeeds → table populated.
#   2. resources.json missing → download fails    → typed
#      StanzaCatalogDownloadError raised, NOT silent return None.
#   3. Stanza package itself not installed         → return None (unchanged).
#
# These exist because BA3 used to silently return None on any
# ResourcesFileNotFoundError, which masked the fact that no language
# pack had ever been seeded — and the worker exit-1 that resulted was
# classified as a transient crash, retried 3×, and dumped a full Python
# traceback per attempt to the daemon log.
# ---------------------------------------------------------------------------


@pytest.fixture(autouse=True)
def _clear_capability_cache():
    """Reset the lru_cache on get_cached_capability_table between tests."""
    get_cached_capability_table.cache_clear()
    yield
    get_cached_capability_table.cache_clear()


def test_bootstrap_downloads_catalog_when_missing():
    """resources.json absent on first call → download + return populated table.

    Pre-fix: returns None silently (the bug). Post-fix: triggers
    download_resources_json, retries, returns a populated table.
    """
    from stanza.resources.common import ResourcesFileNotFoundError

    # First call to load_resources_json raises (catalog missing); after the
    # fake download_resources_json runs, the second call returns our fixture.
    fake_load = mock.Mock(
        side_effect=[
            ResourcesFileNotFoundError("/fake/path/resources.json"),
            _FIXTURE_RESOURCES,
        ]
    )
    fake_download = mock.Mock(return_value=None)

    with (
        mock.patch(
            "stanza.resources.common.load_resources_json", fake_load
        ),
        mock.patch(
            "stanza.resources.common.download_resources_json", fake_download
        ),
    ):
        table = get_cached_capability_table()

    assert table is not None, (
        "Bootstrap must download the catalog and return a populated table, "
        "not silently return None"
    )
    assert "eng" in table.languages
    fake_download.assert_called_once()
    assert fake_load.call_count == 2  # one miss, one success after download


def test_bootstrap_raises_typed_error_on_download_failure():
    """resources.json missing + download fails → typed error, not silent None.

    A real network failure must be visible. The orchestrator can later
    classify this as non-retryable; the user can read it as an actionable
    error. Silent return-None is the bug.
    """
    from stanza.resources.common import ResourcesFileNotFoundError

    fake_load = mock.Mock(
        side_effect=ResourcesFileNotFoundError("/fake/path/resources.json")
    )
    fake_download = mock.Mock(
        side_effect=ConnectionError("network unreachable")
    )

    with (
        mock.patch(
            "stanza.resources.common.load_resources_json", fake_load
        ),
        mock.patch(
            "stanza.resources.common.download_resources_json", fake_download
        ),
    ):
        with pytest.raises(StanzaCatalogDownloadError) as excinfo:
            get_cached_capability_table()

    assert "network unreachable" in str(excinfo.value) or isinstance(
        excinfo.value.__cause__, ConnectionError
    )


def test_stanza_not_installed_returns_none():
    """ImportError on stanza import → return None (unchanged behavior).

    This is the one legitimate silent-None path: BA3 deployed without
    stanza in the venv. Distinct from "catalog missing".
    """
    # Force ImportError when build_stanza_capability_table imports stanza.
    fake_load = mock.Mock(side_effect=ImportError("No module named 'stanza'"))
    with mock.patch(
        "batchalign.worker._stanza_capabilities.build_stanza_capability_table",
        fake_load,
    ):
        result = get_cached_capability_table()

    assert result is None
