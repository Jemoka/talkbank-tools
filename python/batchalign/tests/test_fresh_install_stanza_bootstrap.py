"""End-to-end fresh-install regression test for Stanza catalog bootstrap.

This test exercises the on-demand download path against the real upstream
Stanza catalog. It is the canonical regression gate for the contract:
*"a fresh install with no Stanza cache must produce a usable capability
table without any user action."*

Marked ``@pytest.mark.golden`` because it requires network access on first
run (subsequent runs hit the upstream cache, but we always download into a
fresh temp directory to actually exercise the bootstrap path). Excluded
from the default ``pytest`` profile.

If this test fails, the on-demand contract is broken: a fresh external
user cannot run ``batchalign3 morphotag`` without manual setup. That is
the bug the entire ``_stanza_capabilities.py`` rewrite of 2026-05-06 was
written to prevent.
"""

from __future__ import annotations

from collections.abc import Iterator
from pathlib import Path

import pytest

# Skip the test cleanly if we can't reach the upstream catalog. The
# fixture below decides whether the network is available before importing
# anything that might trigger a real network call.


@pytest.fixture
def isolated_stanza_resources_dir(
    tmp_path: Path, monkeypatch: pytest.MonkeyPatch
) -> Iterator[Path]:
    """Point Stanza at a fresh, empty ``STANZA_RESOURCES_DIR`` for this test.

    The directory is created empty: no ``resources.json``, no language packs.
    Stanza's own ``DEFAULT_MODEL_DIR`` is computed at import time, so we
    must reload the relevant modules (or at least clear our own caches)
    after setting the env var so it's read with the new value.
    """
    target = tmp_path / "stanza_resources"
    target.mkdir(parents=True)
    monkeypatch.setenv("STANZA_RESOURCES_DIR", str(target))

    # Clear our lru_cache from any prior test in the same process.
    from batchalign.worker._stanza_capabilities import (
        get_cached_capability_table,
    )

    get_cached_capability_table.cache_clear()
    yield target
    get_cached_capability_table.cache_clear()


@pytest.mark.golden
def test_fresh_install_bootstraps_catalog_on_first_call(
    isolated_stanza_resources_dir: Path,
) -> None:
    """A fresh empty ``STANZA_RESOURCES_DIR`` triggers catalog auto-download.

    Steps the test exercises end-to-end:

    1. Set ``STANZA_RESOURCES_DIR`` to a fresh, empty temp directory
       (already done by the fixture).
    2. Call ``get_cached_capability_table()`` — the same path the worker
       takes when it first needs Stanza.
    3. Assert: the call returns a populated ``StanzaCapabilityTable``
       (not ``None``, not raising).
    4. Assert: ``resources.json`` now exists on disk at the expected
       sub-path under ``DEFAULT_MODEL_DIR``.

    A failure here means the on-demand contract is broken — a user with
    no Stanza cache cannot run ``batchalign3 morphotag`` without manual
    seeding. That is precisely the bug the bootstrap rewrite of
    2026-05-06 was meant to prevent.
    """
    # Reload Stanza's resource module so it picks up our env var. Stanza
    # computes ``DEFAULT_MODEL_DIR`` at import time, and the venv may have
    # already imported it from a previous test.
    import importlib

    import stanza.resources.common as src

    src = importlib.reload(src)
    assert src.DEFAULT_MODEL_DIR.startswith(str(isolated_stanza_resources_dir)), (
        f"Stanza DEFAULT_MODEL_DIR {src.DEFAULT_MODEL_DIR!r} did not pick up "
        f"our STANZA_RESOURCES_DIR={isolated_stanza_resources_dir!r}; the "
        "test cannot exercise the fresh-install path. This usually means "
        "Stanza was already imported with a different env value earlier "
        "in the test session — investigate test isolation."
    )

    from batchalign.worker._stanza_capabilities import (
        get_cached_capability_table,
    )

    table = get_cached_capability_table()

    # The bootstrap must produce a usable table without manual seeding.
    assert table is not None, (
        "On-demand contract violated: get_cached_capability_table() "
        "returned None on a fresh install. The bootstrap path in "
        "_stanza_capabilities.py should have downloaded resources.json "
        "and rebuilt the table."
    )
    assert "eng" in table.languages, (
        "Bootstrap completed but the resulting table is missing English; "
        "the upstream catalog is unexpectedly malformed or the iso3 "
        "mapping is broken."
    )

    # The catalog file should now exist on disk in the temp directory we
    # set up. Stanza's exact filename varies with the resources version
    # (e.g., ``resources.json`` or ``resources_v1.11.0.json``); accept any
    # ``resources*.json`` under the temp dir.
    found = list(Path(isolated_stanza_resources_dir).rglob("resources*.json"))
    assert found, (
        f"Bootstrap reported success but no resources*.json was written "
        f"under {isolated_stanza_resources_dir}. Either the download "
        "silently failed (a serious bug) or it wrote to a path outside "
        "STANZA_RESOURCES_DIR (env-var override broken)."
    )

    # Sanity: every PATH-shaped value in the env-var case must be inside our
    # temp dir, otherwise the test isn't actually exercising the env override.
    for path in found:
        assert str(path).startswith(str(isolated_stanza_resources_dir)), (
            f"Stanza wrote {path} OUTSIDE the test's "
            f"STANZA_RESOURCES_DIR={isolated_stanza_resources_dir}. The "
            "env-var override is not being honored — this is a deployment "
            "regression in Stanza or in our use of it."
        )


@pytest.mark.golden
def test_fresh_install_emits_user_visible_download_event(
    isolated_stanza_resources_dir: Path,
    capsys: pytest.CaptureFixture[str],
) -> None:
    """Bootstrap path emits a ``progress_v2`` event the user/UI can see.

    Time-transparency principle: the user must always know what BA3 is
    doing during a long wait. The catalog download is short (~1 MB) but
    must still be surfaced — silent waits are UX bugs.
    """
    import importlib
    import json

    import stanza.resources.common as src

    src = importlib.reload(src)

    from batchalign.worker._stanza_capabilities import (
        get_cached_capability_table,
    )

    get_cached_capability_table()

    captured = capsys.readouterr().out
    # The protocol channel writes one JSON line per event. We accept either
    # the start or completion event (both are emitted).
    progress_lines = [
        line
        for line in captured.splitlines()
        if line.startswith("{") and "progress_v2" in line
    ]
    assert progress_lines, (
        "No progress_v2 events emitted during catalog bootstrap. The "
        "time-transparency contract requires every long operation to "
        "surface to the UI. Captured stdout was:\n" + captured
    )
    catalog_events = [
        json.loads(line)
        for line in progress_lines
        if "downloading_stanza_catalog" in line
    ]
    assert catalog_events, (
        "progress_v2 events were emitted but none mention "
        "downloading_stanza_catalog. The bootstrap-specific stage "
        "identifier is missing — the UI will fall back to a generic "
        "label and the user won't know what's downloading."
    )
