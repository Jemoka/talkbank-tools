#!/usr/bin/env python3
"""Benchmark typed in-process Rust/Python callback overhead."""

from __future__ import annotations

import argparse
import sys
import time
from typing import Any

_CHAT = """\
@UTF8
@Begin
@Languages:\teng
@Participants:\tCHI Target_Child, MOT Mother
@ID:\teng|bench|CHI||female|||Target_Child|||
@ID:\teng|bench|MOT|||||Mother|||
@Media:\tbench, audio
*CHI:\tI eat cookies . \x150_4000\x15
*MOT:\tgood job . \x154000_8000\x15
@End
"""


def _load_core() -> Any:
    try:
        import batchalign_core
    except ImportError as exc:
        raise SystemExit(
            "batchalign_core is required for this benchmark. "
            "Install/editable-build batchalign3 first."
        ) from exc
    return batchalign_core


def _run_translation(core: Any, iterations: int) -> float:
    def callback(payload: Any) -> Any:
        return {"translation": payload["text"]}

    start = time.perf_counter()
    for _ in range(iterations):
        handle = core.ParsedChat.parse(_CHAT)
        handle.add_translation(callback)
    return time.perf_counter() - start


def _run_utseg(core: Any, iterations: int) -> float:
    def callback(payload: Any) -> Any:
        return {"assignments": [0] * len(payload["words"])}

    start = time.perf_counter()
    for _ in range(iterations):
        handle = core.ParsedChat.parse(_CHAT)
        handle.add_utterance_segmentation(callback)
    return time.perf_counter() - start


def _run_fa(core: Any, iterations: int) -> float:
    def callback(payload: Any) -> Any:
        words = payload["words"]
        return {
            "indexed_timings": [
                {"start_ms": i * 1000, "end_ms": (i + 1) * 1000}
                for i, _ in enumerate(words)
            ]
        }

    start = time.perf_counter()
    for _ in range(iterations):
        handle = core.ParsedChat.parse(_CHAT)
        handle.add_forced_alignment(callback)
    return time.perf_counter() - start


def _run_morphosyntax(core: Any, iterations: int) -> float:
    def callback(payload: Any, lang: str) -> Any:
        return {
            "raw_sentences": [[
                {
                    "id": i + 1,
                    "text": word,
                    "lemma": word,
                    "upos": "INTJ" if i == 0 else "NOUN",
                    "head": 0 if i == 0 else 1,
                    "deprel": "root" if i == 0 else "obj",
                }
                for i, word in enumerate(payload["words"])
            ]]
        }

    start = time.perf_counter()
    for _ in range(iterations):
        handle = core.ParsedChat.parse(_CHAT)
        handle.add_morphosyntax("eng", callback)
    return time.perf_counter() - start


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--iterations", type=int, default=200, help="Iterations per benchmark mode"
    )
    parser.add_argument(
        "--json", action="store_true", help="Emit JSON only"
    )
    args = parser.parse_args()

    if args.iterations <= 0:
        raise SystemExit("--iterations must be > 0")

    core = _load_core()
    rows = []
    for name, fn in (
        ("morphosyntax", _run_morphosyntax),
        ("translation", _run_translation),
        ("utseg", _run_utseg),
        ("fa", _run_fa),
    ):
        rows.append(
            {
                "benchmark": name,
                "iterations": args.iterations,
                "elapsed_s": round(fn(core, args.iterations), 6),
            }
        )

    if args.json:
        print(json.dumps(rows, sort_keys=True))
        return 0

    for row in rows:
        print(f"{row['benchmark']}: elapsed={row['elapsed_s']}s")
    print(json.dumps(rows, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
