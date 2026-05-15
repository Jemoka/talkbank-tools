"""Regression tests for the worker capability probe's engine_version slot.

Background: between 2026-05-07 and 2026-05-08 the capability probe in
``python/batchalign/worker/_handlers.py`` was observed emitting
``engine_versions[MORPHOSYNTAX] == "stanza"`` (the literal engine *name*)
when the worker had advertised capabilities before any Stanza model had
been loaded — because ``_state.stanza_version`` was empty and the probe
table's ``default_version`` for stanza-backed tasks was the bare string
``"stanza"``. The Rust provenance writer at
``crates/batchalign/src/provenance.rs`` then formatted that as
``engine=stanza-{engine_version}`` → ``engine=stanza-stanza`` in the
``[ba3 morphotag | ...]`` provenance comment of every output file.

The fix is to ALWAYS resolve the stanza version from the actual
importable package (``stanza.__version__``) when ``_state`` does not
already carry a model-loaded version. These tests pin the contract that
the resolver never returns the literal name as a version.
"""

from __future__ import annotations

from batchalign.worker._stanza_capabilities import resolve_stanza_version


def test_resolve_stanza_version_reads_package_when_loaded_empty():
    """When no model is loaded (empty `loaded_version`), the resolver
    must read ``stanza.__version__`` directly from the importable
    package — never return the literal engine name ``"stanza"``."""
    import stanza

    resolved = resolve_stanza_version("")

    # The actual installed Stanza version (e.g. "1.11.1") — not the bug.
    assert resolved == stanza.__version__
    # Belt-and-suspenders: explicitly assert we did not regress to the
    # literal engine-name fallback that produced "stanza-stanza".
    assert resolved != "stanza"


def test_resolve_stanza_version_prefers_loaded_when_present():
    """When the caller passes a non-empty `loaded_version` (which the
    capability probe does after a successful model load), the resolver
    must return that value — the loaded version is the authoritative
    record of what produced the current %mor."""
    assert resolve_stanza_version("1.99.99-fake-loaded") == "1.99.99-fake-loaded"
