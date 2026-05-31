"""Tests for the Hirschberg DP Python shim + empty-sequence edge cases.

Exercises the high-level `align()` entry point. When the Rust extension
is built, the shim delegates to it; when not, the in-process fallback
runs. Both paths return the same shape (`Match` / `Extra`).
"""

from __future__ import annotations

from batchalign.backends.morphosyntax.ud.dp import (
    Extra,
    ExtraType,
    Match,
    PayloadTarget,
    ReferenceTarget,
    align,
)


def test_empty_payload_returns_all_reference_extras() -> None:
    payload: list[PayloadTarget] = []
    reference = [ReferenceTarget("a"), ReferenceTarget("b")]
    out = align(payload, reference, tqdm=False)
    assert len(out) == 2
    assert all(isinstance(x, Extra) for x in out)
    assert all(x.extra_type == ExtraType.REFERENCE for x in out)


def test_empty_reference_returns_all_payload_extras() -> None:
    payload = [PayloadTarget("a", None), PayloadTarget("b", None)]
    reference: list[ReferenceTarget] = []
    out = align(payload, reference, tqdm=False)
    assert len(out) == 2
    assert all(isinstance(x, Extra) for x in out)
    assert all(x.extra_type == ExtraType.PAYLOAD for x in out)


def test_both_empty_returns_empty() -> None:
    out = align([], [], tqdm=False)
    assert out == []


def test_exact_match_pairs() -> None:
    payload = [PayloadTarget("a", 1), PayloadTarget("b", 2)]
    reference = [ReferenceTarget("a", 10), ReferenceTarget("b", 20)]
    out = align(payload, reference, tqdm=False)
    assert all(isinstance(x, Match) for x in out)
    keys = [m.key for m in out]
    assert keys == ["a", "b"]


def test_one_extra_payload_word() -> None:
    payload = [PayloadTarget("a", None), PayloadTarget("b", None), PayloadTarget("c", None)]
    reference = [ReferenceTarget("a"), ReferenceTarget("c")]
    out = align(payload, reference, tqdm=False)
    matches = [x for x in out if isinstance(x, Match)]
    extras = [x for x in out if isinstance(x, Extra)]
    assert len(matches) == 2
    assert len(extras) == 1
    assert extras[0].extra_type == ExtraType.PAYLOAD
    assert extras[0].key == "b"
