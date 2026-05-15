"""End-to-end regression test: morphotag --retokenize on CHAT with retraces.

This test runs the actual batchalign3 CLI on a CHAT file containing
a retrace marker and verifies the pipeline completes without error.

Bug: MOST corpus 40415b.cha utterance 36 failed with "MOR item count (5)
does not match alignable word count (6)" when --retokenize was used.

Source: data/childes-other-data/Chinese/Cantonese/MOST/10002/40415b.cha
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path

import pytest

CHAT_WITH_RETRACE = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\tyue\n"
    "@Participants:\tPAR0 Participant\n"
    "@ID:\tyue|MOST|PAR0|||||Participant|||\n"
    "@Media:\t40415b, audio\n"
    # Exact utterance from MOST 40415b.cha line 46 (with NAK bullet delimiters)
    "*PAR0:\t呢 度 <下 次> [/] 下 次 食 飯 啦 . \x1510245"
    "0_112560\x15\n"
    "@End\n"
)


@pytest.mark.golden
@pytest.mark.integration
def test_morphotag_retokenize_with_retrace_succeeds() -> None:
    """morphotag --retokenize must not crash on utterances with retraces."""
    with tempfile.TemporaryDirectory() as tmpdir:
        input_path = Path(tmpdir) / "retrace_test.cha"
        output_dir = Path(tmpdir) / "output"
        input_path.write_text(CHAT_WITH_RETRACE)

        result = subprocess.run(
            [
                "cargo", "run", "-p", "batchalign", "--",
                "--no-open-dashboard", "--override-media-cache",
                "morphotag", "--retokenize",
                str(input_path),
                "-o", str(output_dir),
                "--lang", "yue",
            ],
            capture_output=True,
            text=True,
            timeout=120,
            cwd=str(Path(__file__).resolve().parents[4]),  # batchalign3 root
        )

        # Check the output
        output_file = output_dir / "retrace_test.cha"

        if result.returncode != 0:
            # THIS IS THE BUG — if it fails, capture the error
            assert False, (
                f"morphotag --retokenize failed (exit {result.returncode}):\n"
                f"STDOUT: {result.stdout[-500:]}\n"
                f"STDERR: {result.stderr[-500:]}"
            )

        assert output_file.exists(), f"Output file not created: {output_dir}"

        output_text = output_file.read_text()
        assert "%mor:" in output_text, f"Output should have %mor tiers:\n{output_text}"
        assert "[/]" in output_text, f"Retrace should be preserved:\n{output_text}"
