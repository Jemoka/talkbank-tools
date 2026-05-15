"""Per-language decision-probe case registry.

Sibling to :mod:`.._cases` (MWT cases). The Phase-2 landing point
is intentionally empty: Phase 3 will seed English cases, and other
languages will join later as their normalization programs come
online.

The registry is keyed by :class:`..._cases.LanguageKey` so decision
probes can reuse the existing Stanza pipeline fixtures keyed by the
same type — no parallel language-key system.
"""

from __future__ import annotations

from .._cases import ENG, LanguageKey
from .._decision_probe_types import DecisionProbeCase
from . import english


DECISION_LANGUAGE_MATRIX: dict[LanguageKey, tuple[DecisionProbeCase, ...]] = {
    ENG: english.CASES,
}


def all_decision_cases() -> list[tuple[LanguageKey, DecisionProbeCase]]:
    """Flatten the registry into a parametrize-friendly list.

    Returns an empty list until Phase 3 seeds the first case.
    """
    return [
        (lang, case)
        for lang, cases in DECISION_LANGUAGE_MATRIX.items()
        for case in cases
    ]
