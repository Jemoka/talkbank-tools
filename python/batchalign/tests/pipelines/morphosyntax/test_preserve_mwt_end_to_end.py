"""End-to-end CLI test: BA3 morphotag produces CHAT-correct %mor for
the four copula-contraction constructions that originally prompted the
Rust-side ``nlp::invariants::finite_verb_main_clause`` rewrite.

This is the single place where we verify the rewrite lands correctly
through the full pipeline (parse CHAT → extract → Stanza → Rust rewrite
→ inject %mor → serialize). The Python-layer observation tests in
``test_stanza_mwt_copula_observations.py`` document Stanza's
intermediate output (which is wrong for sink/lady); this file asserts
BA3's final output regardless of what Stanza did along the way.

Both assertions share a single CLI invocation via the
``morphotag_output`` fixture to keep test runtime bounded — one
``cargo run`` dominates the cost here.
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path

import pytest


CHAT_FIXTURE = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\teng\n"
    "@Participants:\tPAR Adult\n"
    "@ID:\teng|test|PAR|||||Adult|||\n"
    "*PAR:\tthe stool's going over .\n"
    "*PAR:\tand he's falling over .\n"
    "*PAR:\tand the sink's overflowing .\n"
    "*PAR:\tthe lady's washing dishes .\n"
    "@End\n"
)


# CHAT-correct %mor substring that every contracted 's case must produce.
# "aux|be-Fin-Ind-Pres-S3" is the contracted copula "is" in the CHAT
# morphological conventions BA3 emits.
REQUIRED_CLITIC_MOR = "~aux|be-Fin-Ind-Pres-S3"


@pytest.fixture(scope="module")
def morphotag_output() -> str:
    """Run ``batchalign3 morphotag`` once per module; return the output CHAT text.

    The CLI build + run dominates the cost; running it twice (once per
    test function) would double end-to-end test time for no coverage gain.
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        input_path = Path(tmpdir) / "copula_contractions.cha"
        output_dir = Path(tmpdir) / "output"
        input_path.write_text(CHAT_FIXTURE)

        result = subprocess.run(
            [
                "cargo", "run", "-p", "batchalign", "--",
                "--no-open-dashboard", "--override-media-cache",
                "morphotag",
                str(input_path),
                "-o", str(output_dir),
                "--lang", "eng",
                "--sequential",
            ],
            capture_output=True,
            text=True,
            timeout=180,
            cwd=str(Path(__file__).resolve().parents[4]),  # batchalign3 root
        )
        assert result.returncode == 0, (
            f"morphotag CLI failed (exit {result.returncode}):\n"
            f"STDOUT: {result.stdout[-500:]}\n"
            f"STDERR: {result.stderr[-500:]}"
        )
        output_file = output_dir / "copula_contractions.cha"
        assert output_file.exists(), f"Output file not created: {output_dir}"
        return output_file.read_text()


@pytest.mark.golden
@pytest.mark.integration
def test_all_four_copula_contractions_produce_correct_mor(
    morphotag_output: str,
) -> None:
    """Every ``<subject>'s <present-participle>`` utterance must emit
    tilde-joined ``~aux|be-Fin-Ind-Pres-S3`` — no exceptions, no
    possessive readings allowed through."""
    mor_lines = [line for line in morphotag_output.splitlines() if line.startswith("%mor:")]
    assert len(mor_lines) == 4, (
        f"Expected 4 %mor lines, got {len(mor_lines)}. Output:\n{morphotag_output}"
    )

    surface_labels = ["stool's", "he's", "sink's", "lady's"]
    for surface, mor_line in zip(surface_labels, mor_lines):
        assert REQUIRED_CLITIC_MOR in mor_line, (
            f"[{surface}] BA3 did not produce {REQUIRED_CLITIC_MOR!r} for "
            f"the contracted 's. Full %mor line:\n  {mor_line}\n"
            f"Full output:\n{morphotag_output}"
        )
        for bad in (
            "noun|stool-Plur",
            "noun|he-Plur",
            "noun|lady-Plur",
            "~part|s",  # possessive reading leaking through
        ):
            assert bad not in mor_line, (
                f"[{surface}] Degraded analysis {bad!r} present in "
                f"%mor. BA3 emitted:\n  {mor_line}\n"
            )


@pytest.mark.golden
@pytest.mark.integration
def test_gra_tier_consistent_with_rewritten_mor(
    morphotag_output: str,
) -> None:
    """Every utterance's %gra must declare AUX, NSUBJ, and ROOT relations
    matching the rewritten copula-progressive structure. Catches the
    class of bug where %mor and %gra diverge after a rewrite."""
    gra_lines = [line for line in morphotag_output.splitlines() if line.startswith("%gra:")]
    assert len(gra_lines) == 4, f"Expected 4 %gra lines, got {len(gra_lines)}"

    for gra_line in gra_lines:
        assert "AUX" in gra_line, (
            f"%gra line missing AUX relation for copula 's:\n  {gra_line}"
        )
        assert "NSUBJ" in gra_line, (
            f"%gra line missing NSUBJ (subject) relation:\n  {gra_line}"
        )
        assert "ROOT" in gra_line, (
            f"%gra line missing ROOT relation:\n  {gra_line}"
        )
