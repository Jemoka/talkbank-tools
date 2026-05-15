"""Static regression gate for the on-demand download contract.

Asserts that no code in the ``batchalign`` package opts out of the upstream
libraries' default auto-download behavior. Every model family must download
on first use; every blocking wait must surface to the UI through the
``progress_v2`` event channel.

If a future PR needs an exception (e.g., an offline-test fixture), it must
be opt-in via a code-path-specific flag, not a default — the matchers below
will reject any unconditional opt-out.

This test exists because BA3 once silently opted out of Stanza's
catalog auto-download via a ``REUSE_RESOURCES`` download method paired
with a defensive ``return None`` swallow. A fresh install on any host
that had never seeded Stanza failed mysteriously and the orchestrator's
retry storm dumped a full Python traceback per attempt, ballooning the
daemon log into many gigabytes per day.
"""

from __future__ import annotations

import re
from pathlib import Path

import pytest

# Path resolution: this test file lives at
# ``batchalign/tests/test_progress_audit.py``; the package root is its
# grandparent.
_PACKAGE_ROOT = Path(__file__).resolve().parents[1]


def _python_sources_under(root: Path) -> list[Path]:
    """All ``.py`` files under ``root``, excluding test fixtures and venvs."""
    out: list[Path] = []
    for path in root.rglob("*.py"):
        # Exclude test files (they may legitimately reference banned patterns
        # in mocks/assertions). Exclude generated worker_v2 stubs which are
        # auto-generated from JSON schemas and never touch model loading.
        parts = path.parts
        if "tests" in parts:
            continue
        if "generated" in parts and "worker_v2" in parts:
            continue
        out.append(path)
    return out


# ---------------------------------------------------------------------------
# Banned patterns: each (pattern, reason) is a regex that, if matched in any
# non-test source file, indicates the on-demand contract has been violated.
#
# Patterns are deliberately conservative — they match a literal on-the-line
# token. False positives (e.g., a comment that happens to mention the
# pattern) are addressed by the inline-comment exemption below.
# ---------------------------------------------------------------------------

_BANNED_PATTERNS: list[tuple[str, str]] = [
    (
        r"local_files_only\s*=\s*True",
        "Disables HuggingFace auto-download. The on-demand contract requires "
        "every model to download on first use. If you need a model present, "
        "trust from_pretrained() to fetch it.",
    ),
    (
        r"DownloadMethod\.NONE",
        "Disables Stanza auto-download. Use DownloadMethod.REUSE_RESOURCES "
        "(reuses cached files; downloads what's missing) — the default "
        "auto-download behavior the on-demand contract relies on.",
    ),
    (
        r"os\.environ\[\s*['\"]HF_HUB_OFFLINE['\"]\s*\]\s*=",
        "Forces HuggingFace offline mode in BA3-controlled environment. "
        "Offline mode is a deployment choice (set externally if needed); BA3 "
        "code must not impose it as a default.",
    ),
    (
        r"os\.environ\[\s*['\"]TRANSFORMERS_OFFLINE['\"]\s*\]\s*=",
        "Forces transformers offline mode in BA3-controlled environment. "
        "See HF_HUB_OFFLINE entry above.",
    ),
]


def _line_is_exempt(line: str) -> bool:
    """Return True iff the line carries an explicit allowlist comment.

    We do not currently have any legitimate exceptions — but the mechanism
    exists so future contributors can opt in to a banned pattern with a
    documented reason rather than disabling the test wholesale. Format:
    ``# audit: allow <reason>``.
    """
    return "# audit: allow" in line


@pytest.mark.parametrize("pattern,reason", _BANNED_PATTERNS)
def test_no_download_opt_outs(pattern: str, reason: str) -> None:
    """No file in the package opts out of upstream auto-download.

    See the module docstring and individual ``_BANNED_PATTERNS`` entries
    for why each pattern is forbidden.
    """
    rx = re.compile(pattern)
    hits: list[tuple[Path, int, str]] = []
    for source in _python_sources_under(_PACKAGE_ROOT):
        try:
            text = source.read_text(encoding="utf-8")
        except UnicodeDecodeError:
            continue
        for lineno, line in enumerate(text.splitlines(), start=1):
            if _line_is_exempt(line):
                continue
            if rx.search(line):
                hits.append((source.relative_to(_PACKAGE_ROOT.parent), lineno, line.strip()))

    if hits:
        formatted = "\n".join(f"  {p}:{ln}: {snippet}" for p, ln, snippet in hits)
        pytest.fail(
            f"On-demand download contract violation. Pattern: {pattern!r}\n"
            f"Reason: {reason}\n"
            f"Offending lines:\n{formatted}\n\n"
            f"If you genuinely need an exception, add ``# audit: allow <reason>`` "
            f"to the line and document why in the PR description."
        )
