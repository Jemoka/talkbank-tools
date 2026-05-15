# affects: crates/batchalign/src/**
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]


def _find_pattern(path: Path, pattern: str) -> list[tuple[int, str]]:
    regex = re.compile(pattern)
    matches: list[tuple[int, str]] = []
    for lineno, line in enumerate(path.read_text().splitlines(), start=1):
        if regex.search(line):
            matches.append((lineno, line.strip()))
    return matches


def _scan_paths(paths: list[Path], pattern: str) -> list[tuple[str, int, str]]:
    found: list[tuple[str, int, str]] = []
    for path in paths:
        rel = path.relative_to(ROOT).as_posix()
        for lineno, line in _find_pattern(path, pattern):
            found.append((rel, lineno, line))
    return found


def test_chat_ops_dp_calls_are_allowlisted() -> None:
    dp_call_roots = [
        ROOT / "crates" / "batchalign" / "src",
        ROOT / "crates" / "talkbank-transform" / "src",
    ]
    dp_call_src = sorted(path for root in dp_call_roots for path in root.rglob("*.rs"))
    align_hits = _scan_paths(dp_call_src, r"\bdp_align::align\s*\(")
    align_chars_hits = _scan_paths(dp_call_src, r"\bdp_align::align_chars\s*\(")
    # Allowlisted dp_align::align call sites:
    # - talkbank-transform/benchmark.rs: WER evaluation
    # - talkbank-transform/compare.rs: transcript comparison
    #   (2 calls: window alignment + rotation)
    # - batchalign/chat_ops/fa/utr.rs: UTR global alignment
    #   (correctness-critical, not avoidable)
    # - batchalign/chat_ops/fa/utr/two_pass.rs: overlap-aware UTR
    #   timing recovery
    assert len(align_hits) == 5
    assert {rel for rel, _, _ in align_hits} == {
        "crates/batchalign/src/chat_ops/fa/utr.rs",
        "crates/batchalign/src/chat_ops/fa/utr/two_pass.rs",
        "crates/talkbank-transform/src/benchmark.rs",
        "crates/talkbank-transform/src/compare/engine.rs",
    }
    assert not align_chars_hits
