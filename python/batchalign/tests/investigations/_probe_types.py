"""Typed data model for the Stanza MWT probe matrix.

Every investigation probe case is a :class:`ProbeCase`. The 6-language
registry (``_cases/__init__.py``) aggregates per-language tuples into a
single :data:`LANGUAGE_MATRIX`. The matrix-driven test module
(``test_stanza_mwt_probe_matrix.py``) reads the registry, runs each case
through the paired (free-tokenize vs postprocessor) pipelines, and
enforces the per-case invariant.

Why typed data
--------------
Before the 2026-04-22 consolidation, probe cases were hand-built
``list[tuple[str, list[str]]]`` tables scattered across 5 test modules
(~1080 lines total). Each module reinvented xfail handling, invariant
selection, and pipeline-fixture plumbing. Types here capture the
intent so every new case declares what it means to pass, fail, or
xfail — rather than encoding that in function names and module-level
comments.
"""

from __future__ import annotations

from dataclasses import dataclass
from enum import Enum


class Phenomenon(Enum):
    """The linguistic construction a probe case exercises.

    Used to group the case in per-language book tables and to help a
    contributor classify a new probe before writing it. Not referenced
    by test assertions — purely descriptive metadata.
    """

    NATIVE_MWT = "native_mwt"
    """Preposition+article or article+preposition contractions that
    Stanza's MWT processor expands natively (``au``, ``del``, ``am``)."""

    CLITIC_ELISION = "clitic_elision"
    """Apostrophe-separated clitic+host forms (``l'ami``, ``c'est``,
    ``qu'il``) where Stanza emits an MWT Range over the apostrophe."""

    CONTRACTION = "contraction"
    """English-style contractions (``don't``, ``isn't``) where Stanza
    emits an MWT hint tuple for its MWT processor to expand."""

    POSSESSIVE = "possessive"
    """Proper-name possessives or Dutch possessive idioms (``Claus's``,
    ``Maria's``, ``'s-avonds``)."""

    IDIOMATIC = "idiomatic"
    """Fixed lexical items with internal apostrophes that function as
    one word (``aujourd'hui``, ``d'água``)."""

    ELISION_PREFIX = "elision_prefix"
    """French elision-prefix tokens (``jusqu'à``, ``puisqu'il``,
    ``quelqu'un``) — the Wave 4 invariant-break family."""

    MULTI_CLITIC = "multi_clitic"
    """Stacked apostrophe-internal clitics (``d'l'attraper``)."""

    CONTROL = "control"
    """Plain words used as a baseline to confirm the pipeline is
    otherwise healthy (``maison``, ``huis``)."""


@dataclass(frozen=True)
class XfailMark:
    """Explicit xfail annotation.

    ``defect_slug`` must match an anchor in
    ``book/src/reference/stanza-limitations.md`` so a successor
    reading a surprise xfail can trace it to the registered Stanza
    defect in one step.
    """

    defect_slug: str
    reason: str


@dataclass(frozen=True)
class ProbeCase:
    """A single investigation probe case.

    Attributes
    ----------
    label
        Short identifier — used in pytest test IDs and in the rendered
        per-language book tables.
    words
        CHAT-derived word sequence fed to the Stanza pipeline. A tuple
        (not a list) so the dataclass stays hashable/frozen.
    phenomenon
        The linguistic construction class; see :class:`Phenomenon`.
    expected_post_mwt_count
        If set, the test asserts that the post-MWT-expansion Stanza
        word count equals this integer. Use ``len(words)`` for strict
        1-to-1 cases. If ``None``, the case is observe-only — the
        pipeline output is pinned via print but no assertion fires.
    xfail
        If set, the test marks itself xfail with the defect slug and
        reason. Applies only to the with-postprocessor path;
        free-tokenize always runs observe-only.
    """

    label: str
    words: tuple[str, ...]
    phenomenon: Phenomenon
    expected_post_mwt_count: int | None = None
    xfail: XfailMark | None = None

    def is_observe_only(self) -> bool:
        """True if this case does not assert post-MWT word count."""
        return self.expected_post_mwt_count is None

    @classmethod
    def observation_alone(
        cls,
        word: str,
        phenomenon: Phenomenon = Phenomenon.CLITIC_ELISION,
    ) -> "ProbeCase":
        """Build a single-word observation-only probe with label
        ``f"{word}_alone"`` and words ``(word,)``. Standard shape
        for surface pins where the probe's only job is to record
        what Stanza emits for a single token in isolation.

        Use for allowlist-candidate surfaces where you want to
        confirm Stanza's raw output before committing a reconciler
        entry.
        """
        return cls(
            label=f"{word}_alone",
            words=(word,),
            phenomenon=phenomenon,
        )

    @classmethod
    def strict_alone(
        cls,
        word: str,
        phenomenon: Phenomenon = Phenomenon.CONTROL,
    ) -> "ProbeCase":
        """Build a single-word probe with ``expected_post_mwt_count=1``
        — the standard shape for baseline/control tokens that must
        stay 1-to-1 under the postprocessor pipeline. Asserts that
        Stanza doesn't spuriously expand the token into an MWT.

        Use for CONTROL tokens (`hus`, `kot`, `dom`, `vin` etc.)
        and any other surface that should strictly preserve its
        single-token identity.
        """
        return cls(
            label=f"{word}_alone",
            words=(word,),
            phenomenon=phenomenon,
            expected_post_mwt_count=1,
        )
