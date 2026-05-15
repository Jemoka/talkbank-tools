"""Phase 2 tests for the Stanza decision-probe matrix runner.

Mirrors the Phase 1 split from the MWT harness: the runner module
exists and is importable at non-golden test time, but the actual
parametrized Stanza-loading test function is marked
``@pytest.mark.golden`` and runs only on machines with real models.

Phase 2's validation is minimal per the spec: the runner imports
cleanly, the registry starts empty, and parametrize over an empty
matrix produces no collection errors. Phase 3 will seed English
cases into the registry.
"""

from __future__ import annotations

import importlib

from batchalign.tests.investigations._decision_cases import (
    DECISION_LANGUAGE_MATRIX,
    all_decision_cases,
)


def test_decision_matrix_registry_is_populated() -> None:
    """Phase 3 seeds English into the matrix. Additional languages
    join as their normalization programs come online. The key
    invariant here is that the registry is non-empty and that
    ``all_decision_cases()`` flattens it consistently."""
    assert DECISION_LANGUAGE_MATRIX, "Phase 3 must seed at least English"
    flattened = all_decision_cases()
    assert flattened, "Non-empty registry must produce non-empty flatten"
    # Every flattened entry is a (LanguageKey, DecisionProbeCase) pair
    # whose case appears in the per-language tuple.
    for lang, case in flattened:
        assert case in DECISION_LANGUAGE_MATRIX[lang]


def test_decision_runner_module_is_importable_without_stanza() -> None:
    """The runner module must not trigger Stanza import at collection
    time — that would force every non-golden test run to pay the
    ~5s pipeline load. Lazy imports inside the golden-marked
    test function are the pattern (same as the MWT runner)."""
    mod = importlib.import_module(
        "batchalign.tests.investigations.test_stanza_decision_probe_matrix"
    )
    # The module exposes the golden-marked runner by name.
    assert hasattr(mod, "test_stanza_decision_probe")
