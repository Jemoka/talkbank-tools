"""Stanza configuration parity tests.

These tests pin behavior of the worker-side Stanza bootstrap. Historically
they enforced batchalign2 parity; that anchor was retired on 2026-04-15
because BA2's MWT exclusion list was a hand-curated mirror of an older
Stanza catalog and is now known-stale (Stanza ships MWT for Hebrew today,
for example, that BA2 excluded; BA2 also kept Swedish on its MWT list
even though Stanza never shipped a Swedish MWT model in the 1.x line).

The single source of truth is now Stanza's installed ``resources.json``,
read by ``batchalign.worker._stanza_capabilities``. These tests check
that the worker honors that capability table and never asks Stanza for
a processor it does not ship.

NO MOCKS. Pipeline configuration is verified by reading the loader source
and by exercising the pure ``should_request_mwt`` helper against the live
catalog.
"""

from __future__ import annotations


class TestMwtCapabilityDriven:
    """Verify ``should_request_mwt`` matches the real Stanza catalog.

    The 2026-04-15 Swedish bootstrap failure was caused by a hardcoded
    ``MWT_LANGS`` set that listed ``"sv"`` even though Stanza does not
    ship a Swedish MWT model. The worker requested ``mwt`` from Stanza,
    Stanza raised ``UnsupportedProcessorError``, and the entire Swedish
    language group failed mid-job (chunk_106, 500 files).

    These tests assert that the loader now consults the runtime catalog
    instead, so a future Stanza catalog change cannot silently break the
    worker the same way.
    """

    def test_swedish_is_not_requested_for_mwt(self) -> None:
        """Swedish must not be sent to Stanza with the ``mwt`` processor.

        Reproduces the chunk_106 outage condition: every Swedish worker
        spawn raised ``UnsupportedProcessorError`` because Stanza's
        ``resources.json`` does not list ``mwt`` for ``sv``. With the
        capability-table-driven loader, this returns False without
        any catalog edit.
        """
        import pytest

        from batchalign.worker._stanza_capabilities import (
            get_cached_capability_table,
        )
        from batchalign.worker._stanza_loading import should_request_mwt

        table = get_cached_capability_table()
        if table is None:
            pytest.skip("Stanza resources.json not available in this environment")
        assert should_request_mwt("sv", table) is False, (
            "Swedish must not be requested with the mwt processor — "
            "Stanza does not ship a Swedish MWT model. See the "
            "2026-04-15 chunk_106 outage."
        )

    def test_english_is_requested_for_mwt(self) -> None:
        """English MWT is supported by Stanza and required for retokenize.

        Without this, ``--retokenize`` cannot expand contractions like
        ``don't`` → ``do n't`` on English files. Pinning the positive
        case keeps the loader honest.
        """
        import pytest

        from batchalign.worker._stanza_capabilities import (
            get_cached_capability_table,
        )
        from batchalign.worker._stanza_loading import should_request_mwt

        table = get_cached_capability_table()
        if table is None:
            pytest.skip("Stanza resources.json not available in this environment")
        assert should_request_mwt("en", table) is True

    def test_loader_no_longer_imports_a_hardcoded_mwt_set(self) -> None:
        """The hardcoded ``MWT_LANGS`` set must stay deleted.

        It is the exact pattern that produced the 2026-04-15 outage:
        a hand-edited mirror of the upstream catalog that drifts as
        Stanza versions change. Re-introducing it would silently re-
        introduce the same class of bug.
        """
        from pathlib import Path

        loader = (
            Path(__file__).resolve().parents[3] / "worker" / "_stanza_loading.py"
        )
        source = loader.read_text()

        # Tolerate doc references in module docstrings or comments — we
        # only forbid an actual top-level definition. A naive substring
        # check on "MWT_LANGS" would flag the comment that warns future
        # contributors about the pattern.
        import ast

        tree = ast.parse(source)
        for node in ast.walk(tree):
            if isinstance(node, ast.Assign):
                for target in node.targets:
                    if (
                        isinstance(target, ast.Name)
                        and target.id == "MWT_LANGS"
                    ):
                        raise AssertionError(
                            "MWT_LANGS was reintroduced in _stanza_loading.py. "
                            "Use the capability table (should_request_mwt) "
                            "instead — see 2026-04-15 outage notes."
                        )


class TestStanzaPipelineShape:
    def test_english_uses_gum_mwt_package(self) -> None:
        """English must use the 'gum' MWT package, not 'default'.

        batchalign2 (ud.py:1044): config["processors"]["mwt"] = "gum"
        batchalign3 (_stanza_loading.py): package={"mwt": "gum"}
        """
        from pathlib import Path

        stanza_loading = Path(__file__).resolve().parents[3] / "worker" / "_stanza_loading.py"
        source = stanza_loading.read_text()

        # Verify "gum" appears in the English pipeline config
        assert '"gum"' in source or "'gum'" in source, (
            "English MWT must use the 'gum' package. "
            "Check load_stanza_models() in _stanza_loading.py."
        )

    def test_non_mwt_languages_use_pretokenized(self) -> None:
        """Languages without MWT must use tokenize_pretokenized=True.

        batchalign2: tokenize_pretokenized is implicit (no MWT processor)
        batchalign3: explicit tokenize_pretokenized=True for non-MWT langs
        """
        from pathlib import Path

        stanza_loading = Path(__file__).resolve().parents[3] / "worker" / "_stanza_loading.py"
        source = stanza_loading.read_text()

        # The non-MWT branch must have pretokenized=True
        assert "tokenize_pretokenized=True" in source, (
            "Non-MWT languages must use tokenize_pretokenized=True. "
            "Check load_stanza_models() in _stanza_loading.py."
        )


class TestJapaneseProcessorConfig:
    """Verify Japanese-specific Stanza configuration.

    batchalign2 (ud.py:1048-1052) explicitly set ALL processors to
    'combined' for Japanese:
        config["processors"]["tokenize"] = "combined"
        config["processors"]["pos"] = "combined"
        config["processors"]["lemma"] = "combined"
        config["processors"]["depparse"] = "combined"

    batchalign3 now configures this (fixed 2026-03-07).
    """

    def test_japanese_uses_combined_processors(self) -> None:
        """Japanese must use 'combined' processor packages."""
        from pathlib import Path

        stanza_loading = Path(__file__).resolve().parents[3] / "worker" / "_stanza_loading.py"
        source = stanza_loading.read_text()

        # Check if the Japanese combined processor config exists
        has_ja_combined = (
            '"combined"' in source
            and "ja" in source
        )

        assert has_ja_combined, (
            "Japanese 'combined' processor not configured in _stanza_loading.py. "
            "batchalign2 used combined processors for Japanese tokenization. "
            "See morphotag-migration-audit.md Section 4.3."
        )


class TestLanguageSpecificFeatureParity:
    """Per-word feature extraction parity tests.

    NOTE: The per-word mapping (lemma cleaning, POS-specific features, etc.)
    is tested exhaustively in the Rust test suite:

        cargo nextest run -p batchalign -E 'test(nlp::mapping)'

    46 tests cover: all POS types, all language-specific handlers, lemma
    cleaning, MWT assembly, GRA generation, and edge cases.

    See: morphotag-migration-audit.md Section 1 for the line-by-line
    correspondence between ba2 handler functions and ba3 Rust code.

    The tests below verify Python-accessible behavior only (via golden tests).
    For the Rust-level feature parity tests, see:
    - chat-ops/src/nlp/mapping.rs (46 tests)
    - chat-ops/src/nlp/lang_en.rs (irregular verbs)
    - chat-ops/src/nlp/lang_fr.rs (pronoun case, APM nouns)
    - chat-ops/src/nlp/lang_ja.rs (verb form overrides)
    """

    pass  # See Rust tests — do not duplicate here
