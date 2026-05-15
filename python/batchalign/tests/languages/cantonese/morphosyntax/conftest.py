"""Shared fixtures for Cantonese morphosyntax integration tests.

Tests in this directory are Cantonese-specific integration tests
that validate the pipeline end-to-end with Cantonese-specific data and corpus requirements.

These are marked with @pytest.mark.cantonese_integration to allow skipping in fast CI.
The Stanza pipeline fixtures for English (and other languages) are reused from
the parent conftest in morphosyntax/.
"""

from __future__ import annotations

from pathlib import Path

import pytest

_HERE = Path(__file__).parent.resolve()


def pytest_collection_modifyitems(items):
    """Mark tests in this directory as Cantonese integration tests.

    Pytest invokes every conftest's ``pytest_collection_modifyitems`` with
    the same global items list, so this hook must filter to its own
    directory itself — otherwise every collected test in the session
    would be marked ``cantonese_integration`` and the addopts filter
    ``-m "not cantonese_integration"`` would deselect the entire suite.
    """
    for item in items:
        try:
            item_path = Path(item.path).resolve()
        except (OSError, AttributeError):
            continue
        if _HERE == item_path or _HERE in item_path.parents:
            item.add_marker(pytest.mark.cantonese_integration)
