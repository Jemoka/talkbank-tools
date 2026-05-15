"""Change-aware test selection via colocated ``affects:`` metadata.

Phase C1 of the test-cost revamp. Instead of a central
``test-selection.toml`` that rots silently, each test file declares
the code paths it covers in its own header:

Python::

    # affects: python/batchalign/inference/morphosyntax.py
    # affects: crates/batchalign/src/nlp/**

Rust::

    // affects: crates/batchalign/src/retokenize/**

Patterns are gitignore-style (``**`` supported) and matched via
``pathspec``. A test file with no ``affects:`` lines is treated as
"runs always" — backward compatible, gradual adoption.

Design note — the opposite convention (a central file mapping dir →
test subset) was considered in the research plan. It was rejected
in the Plan-agent critique (2026-04-23): a central TOML rots
silently when new test files or code paths land, and the drift
is invisible to review. Colocated metadata lives or dies with the
test it annotates, so stale declarations surface when the test
itself is touched.
"""

from __future__ import annotations

import re
from dataclasses import dataclass
from pathlib import Path
from typing import Iterable

import pathspec

# Matches both Python (``#``) and Rust (``//``) header-style comments.
# Anchored to start-of-line with optional leading whitespace so that
# declarations inside prose (e.g., rendered in a backtick code span in
# a docstring) don't count — comments start at column 0 or after
# whitespace only.
_AFFECTS_RE = re.compile(r"(?m)^\s*(?:#|//)\s*affects:\s*(.+?)\s*$")


@dataclass(frozen=True)
class AffectsDeclaration:
    """The declared code-path coverage of a single test file.

    ``patterns`` is the raw list; ``_matcher`` caches the compiled
    ``pathspec.PathSpec`` so repeated matches against many changed
    paths don't re-parse the globs on every call.
    """

    test_file: Path
    patterns: tuple[str, ...]

    @property
    def _matcher(self) -> pathspec.PathSpec | None:
        cached = getattr(self, "_cached_matcher", None)
        if cached is not None:
            return cached[0]  # unwrap from one-tuple sentinel
        matcher = (
            pathspec.PathSpec.from_lines("gitwildmatch", self.patterns)
            if self.patterns
            else None
        )
        # frozen=True blocks __setattr__; use object.__setattr__ to
        # stash a lazy cache. Wrapping in a tuple distinguishes "not
        # yet computed" (None) from "computed, result is None".
        object.__setattr__(self, "_cached_matcher", (matcher,))
        return matcher


def parse_affects(test_file: Path, content: str | None = None) -> AffectsDeclaration:
    """Extract every ``affects:`` declaration from ``test_file``.

    Pass ``content`` to avoid re-reading a file the caller already has.
    Returns an empty patterns tuple when no declarations are found —
    the test file will be treated as "runs always" by
    :func:`select_tests`.
    """
    if content is None:
        content = test_file.read_text()
    patterns = tuple(match.strip() for match in _AFFECTS_RE.findall(content))
    return AffectsDeclaration(test_file=test_file, patterns=patterns)


def diff_matches_declaration(
    changed_paths: Iterable[str], decl: AffectsDeclaration
) -> bool:
    """True iff any ``changed_paths`` entry matches any ``decl.patterns``.

    Used to decide whether a test file is relevant to a given diff.
    Returns False when either side is empty — an undeclared test file
    (empty patterns) should be handled as "runs always" by the caller,
    not as "does not match anything".
    """
    spec = decl._matcher
    if spec is None:
        return False
    return any(spec.match_file(p) for p in changed_paths)


def select_tests(
    test_files: Iterable[Path], changed_paths: Iterable[str]
) -> tuple[list[Path], list[Path]]:
    """Partition ``test_files`` into ``(selected, run_always)``.

    * **selected** — file has ``affects:`` declarations AND at least
      one pattern matches ``changed_paths``.
    * **run_always** — file has NO ``affects:`` declarations; it opts
      out of change-aware selection and always runs.

    Files with declarations but no matches are implicitly "skip for
    this diff" and do not appear in either bucket. Order of
    ``test_files`` is preserved within each bucket to keep downstream
    command construction deterministic.

    Materialize ``changed_paths`` exactly once — we consume it
    multiple times below (once per test file), so an iterator would
    silently drop matches after the first test.
    """
    changed_list = list(changed_paths)
    selected: list[Path] = []
    run_always: list[Path] = []
    for tf in test_files:
        decl = parse_affects(tf)
        if not decl.patterns:
            run_always.append(tf)
        elif diff_matches_declaration(changed_list, decl):
            selected.append(tf)
    return selected, run_always
