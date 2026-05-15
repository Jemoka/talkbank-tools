"""Test-only harness for the staged worker-protocol V2 FA roundtrip.

Rust integration tests use this script to prove the current staged boundary is
already coherent across languages:

1. Rust writes a typed V2 execute request with prepared artifacts.
2. Python reads that request and runs the staged V2 FA executor.
3. Python writes a typed V2 execute response.
4. Rust reads the response back into the established FA alignment domain.

The harness intentionally uses a deterministic fake FA host rather than real
models. Its job is to prove the contract, not model quality.
"""

from __future__ import annotations

import json
from pathlib import Path
import sys

from batchalign.worker._fa_v2 import (
    ForcedAlignmentExecutionHostV2,
    execute_forced_alignment_request_v2,
)
from batchalign.worker._types_v2 import ExecuteRequestV2


def _fake_whisper_runner(
    _audio,
    text: str,
    _pauses: bool,
) -> list[tuple[str, float]]:
    """Return deterministic fake token timings for the roundtrip test.

    The spacing-based tokenization is intentional because the current roundtrip
    harness only exercises the staged Whisper + `space_joined` path.
    """

    words = [word for word in text.split(" ") if word]
    return [(word, 0.10 + (index * 0.15)) for index, word in enumerate(words)]


def main() -> int:
    """Read a staged V2 FA request, execute it with a fake host, and write the response."""

    if len(sys.argv) != 3:
        print(
            "usage: worker_fa_v2_roundtrip.py <request.json> <response.json>",
            file=sys.stderr,
        )
        return 2

    request_path = Path(sys.argv[1])
    response_path = Path(sys.argv[2])

    request = ExecuteRequestV2.model_validate_json(request_path.read_text(encoding="utf-8"))
    response = execute_forced_alignment_request_v2(
        request,
        ForcedAlignmentExecutionHostV2(whisper_runner=_fake_whisper_runner),
    )
    response_path.write_text(
        json.dumps(response.model_dump(mode="json"), indent=2, sort_keys=True),
        encoding="utf-8",
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
