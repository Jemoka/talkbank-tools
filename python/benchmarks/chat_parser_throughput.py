#!/usr/bin/env python3
"""Benchmark CHAT parser throughput using batchalign_core."""

from __future__ import annotations

import argparse
import json
import sys
import time
from pathlib import Path
from typing import Any

_DEFAULT_CHAT = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\teng\n"
    "@Participants:\tPAR0 Participant\n"
    "@ID:\teng|bench|PAR0|||||Participant|||\n"
    "*PAR0:\thello there .\n"
    "*PAR0:\thow are you ?\n"
    "*PAR0:\ti am fine .\n"
    "@End\n"
)


def _load_chat(path: str | None) -> str:
    if path is None:
        return _DEFAULT_CHAT
    return Path(path).read_text(encoding="utf-8")


def _run(chat_text: str, iterations: int, warmup: int) -> dict[str, Any]:
    try:
        import batchalign_core
    except ImportError as exc:
        raise SystemExit(
            "batchalign_core is required for this benchmark. "
            "Install/editable-build batchalign3 first."
        ) from exc

    for _ in range(max(0, warmup)):
        batchalign_core.parse_and_serialize(chat_text)

    start = time.perf_counter()
    for _ in range(iterations):
        batchalign_core.parse_and_serialize(chat_text)
    elapsed_s = time.perf_counter() - start

    bytes_per_doc = len(chat_text.encode("utf-8"))
    total_bytes = bytes_per_doc * iterations
    docs_per_s = (iterations / elapsed_s) if elapsed_s > 0 else 0.0
    mib_per_s = ((total_bytes / (1024 * 1024)) / elapsed_s) if elapsed_s > 0 else 0.0

    return {
        "benchmark": "chat_parser_throughput",
        "iterations": iterations,
        "bytes_per_doc": bytes_per_doc,
        "elapsed_s": round(elapsed_s, 6),
        "docs_per_s": round(docs_per_s, 3),
        "mib_per_s": round(mib_per_s, 3),
    }


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--chat-file", type=str, default=None,
                        help="Optional CHAT file to benchmark")
    parser.add_argument("--iterations", type=int, default=500,
                        help="Number of parse+serialize iterations")
    parser.add_argument("--warmup", type=int, default=25,
                        help="Warmup iterations (excluded from timing)")
    parser.add_argument("--json", action="store_true",
                        help="Emit JSON only")
    args = parser.parse_args()

    if args.iterations <= 0:
        raise SystemExit("--iterations must be > 0")

    chat_text = _load_chat(args.chat_file)
    metrics = _run(chat_text, args.iterations, args.warmup)

    if args.json:
        print(json.dumps(metrics, sort_keys=True))
        return 0

    print(
        "CHAT parser throughput: "
        f"{metrics['docs_per_s']} docs/s, {metrics['mib_per_s']} MiB/s "
        f"({metrics['iterations']} iterations in {metrics['elapsed_s']}s)"
    )
    print(json.dumps(metrics, indent=2, sort_keys=True))
    return 0


if __name__ == "__main__":
    sys.exit(main())
