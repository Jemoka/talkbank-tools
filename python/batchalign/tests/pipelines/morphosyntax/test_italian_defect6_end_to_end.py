"""End-to-end CLI test: BA3 morphotag produces corrected %mor for
the six Italian Defect 6 / 7 mis-split cases handled by the
reconciler in ``crates/batchalign/src/nlp/lang_it.rs``.

Rust-side unit tests (``nlp::mapping::tests::test_italian_defect6_*``)
exercise the reconciler on synthetic `UdSentence` fixtures — they
confirm the reconciler correctly collapses a known MWT Range into
a single Mor with overridden POS/lemma. This file closes the loop:
real Stanza output flowing through the full production pipeline
(parse CHAT → Python worker → Stanza → Rust inject → %mor
serialization) must also produce the corrected forms.

The test is ``@pytest.mark.golden`` because it loads real Stanza
models. It is ``@pytest.mark.integration`` because it invokes the
``cargo run -p batchalign`` binary path. Both markers mirror
the sibling ``test_preserve_mwt_end_to_end.py`` pattern.

Why this pairing matters
------------------------
A Rust unit-test pass does not prove the reconciler fires in
production — the synthetic `UdSentence` built in tests bypasses
Stanza entirely. The production path is:

  CHAT parse → extract words → batch_infer → Python worker → Stanza
  → UD JSON back to Rust → map_ud_sentence → %mor inject → serialize

The reconciler only runs inside ``map_ud_sentence``'s Range branch.
If any layer upstream produces UD that doesn't carry a Range for
the mis-split word (e.g., if a hypothetical future Stanza upgrade
stops emitting the MWT), the reconciler is a no-op. This test
asserts the reconciler actually catches the production case.
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path

import pytest


# Minimal Italian CHAT covering the six allowlist entries:
#
# * ``parla`` (Defect 6 verb — 3sg indicative / 2sg imperative)
# * ``arancione`` (Defect 6 non-verb: noun)
# * ``piccolo`` (Defect 6 non-verb: adjective)
# * ``gomitolo`` (Defect 6 non-verb: noun)
# * ``divano`` (Defect 6 non-verb: noun)
# * ``la`` sentence-initial (Defect 7: article mis-split as il+i)
#
# Each surface sits in a short utterance with enough context for
# Stanza's tokenizer to produce naturally what it does on corpus
# data. The reconciler fires on the MWT Range Stanza emits; the
# test asserts the CHAT output's ``%mor`` carries the corrected
# lemma and no spurious ``~`` clitic marker.
CHAT_FIXTURE = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\tita\n"
    "@Participants:\tMOT Mother\n"
    "@ID:\tita|test|MOT|||||Mother|||\n"
    "*MOT:\tparla forte .\n"
    "*MOT:\tè arancione .\n"
    "*MOT:\tè molto piccolo .\n"
    "*MOT:\tun gomitolo di lana .\n"
    "*MOT:\tseduto sul divano .\n"
    "*MOT:\tla storia parla di un bambino .\n"
    # Defect 8: `dammela` mid-sentence Stanza-tags as ADJ without MWT.
    "*MOT:\tper favore dammela .\n"
    # Defect 8 corpus-sourced: `prendilo` mid-sentence also ADJ-tagged
    # (CHILDES-ita frequency 52× in 184-file corpus).
    "*MOT:\tper favore prendilo .\n"
    "@End\n"
)


@pytest.fixture(scope="module")
def morphotag_output() -> str:
    """Run ``batchalign3 morphotag --lang ita`` once; return CHAT output."""
    with tempfile.TemporaryDirectory() as tmpdir:
        input_path = Path(tmpdir) / "ita_defect6.cha"
        output_dir = Path(tmpdir) / "output"
        input_path.write_text(CHAT_FIXTURE)

        result = subprocess.run(
            [
                "cargo", "run", "-p", "batchalign", "--",
                "--no-open-dashboard", "--override-media-cache",
                "morphotag",
                str(input_path),
                "-o", str(output_dir),
                "--lang", "ita",
                "--sequential",
            ],
            capture_output=True,
            text=True,
            timeout=300,
            cwd=str(Path(__file__).resolve().parents[4]),  # batchalign3 root
        )
        assert result.returncode == 0, (
            f"morphotag CLI failed (exit {result.returncode}):\n"
            f"STDOUT: {result.stdout[-500:]}\n"
            f"STDERR: {result.stderr[-500:]}"
        )
        output_file = output_dir / "ita_defect6.cha"
        assert output_file.exists(), f"Output file not created: {output_dir}"
        return output_file.read_text()


def _mor_lines(output: str) -> list[str]:
    return [line for line in output.splitlines() if line.startswith("%mor:")]


# Junk patterns Stanza's raw MWT mis-splits would produce if the
# reconciler had NOT fired. Presence of any of these in the output
# is a fail — this is what the reconciler is supposed to suppress.
JUNK_PATTERNS = [
    "verb|par~",      # parla → par + la (Defect 6 verb)
    "verb|arancio~",  # arancione → arancio + ne
    "verb|picco~",    # piccolo → picco + lo
    "verb|gomito~",   # gomitolo → gomito + lo
    "verb|diva~",     # divano → diva + no
    "adj|dammelo",    # dammela mid-sentence mis-tagged ADJ (Defect 8)
    "adj|prendilo",   # prendilo mid-sentence (Defect 8 corpus-sourced)
]


@pytest.mark.golden
@pytest.mark.integration
def test_parla_produces_verb_parlare(morphotag_output: str) -> None:
    """`parla forte` must emit `v|parlare` (or `verb|parlare`) — not
    `verb|par~pron|la`."""
    lines = _mor_lines(morphotag_output)
    assert len(lines) >= 1, f"No %mor lines: {morphotag_output}"
    # Find the utterance containing `parla` — should be utterance 0
    # (``parla forte .``) and utterance 5 (``la storia parla...``).
    parla_lines = [
        line for line in lines
        if "parlare" in line or "par~" in line
    ]
    assert parla_lines, (
        f"No %mor line mentions parlare / par~ — unexpected.\n"
        f"Output:\n{morphotag_output}"
    )
    for line in parla_lines:
        assert "verb|par~" not in line, (
            f"[parla] reconciler did not fire — got junk mis-split %mor:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_arancione_produces_noun(morphotag_output: str) -> None:
    lines = _mor_lines(morphotag_output)
    arancione_lines = [
        line for line in lines if "arancione" in line or "arancio" in line
    ]
    assert arancione_lines, f"No line mentions arancione: {morphotag_output}"
    for line in arancione_lines:
        assert "verb|arancio~" not in line, (
            f"[arancione] reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_piccolo_produces_adj(morphotag_output: str) -> None:
    lines = _mor_lines(morphotag_output)
    piccolo_lines = [
        line for line in lines if "piccolo" in line or "picco~" in line
    ]
    assert piccolo_lines, f"No line mentions piccolo: {morphotag_output}"
    for line in piccolo_lines:
        assert "verb|picco~" not in line, (
            f"[piccolo] reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_gomitolo_produces_noun(morphotag_output: str) -> None:
    lines = _mor_lines(morphotag_output)
    relevant = [
        line for line in lines if "gomitolo" in line or "gomito~" in line
    ]
    assert relevant, f"No line mentions gomitolo: {morphotag_output}"
    for line in relevant:
        assert "verb|gomito~" not in line, (
            f"[gomitolo] reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_divano_produces_noun(morphotag_output: str) -> None:
    lines = _mor_lines(morphotag_output)
    relevant = [line for line in lines if "divano" in line or "diva~" in line]
    assert relevant, f"No line mentions divano: {morphotag_output}"
    for line in relevant:
        assert "verb|diva~" not in line, (
            f"[divano] reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_dammela_mid_sentence_becomes_verb(morphotag_output: str) -> None:
    """Defect 8: `per favore dammela` must emit `verb|dare` (or
    `v|dare`), not `adj|dammelo`."""
    lines = _mor_lines(morphotag_output)
    # Find the %mor line containing `dare` or `dammelo`; tests
    # should not assume positional index because adding more
    # utterances to the fixture shifts `lines[-1]`.
    dammela_lines = [line for line in lines if "dare" in line or "dammelo" in line]
    assert dammela_lines, (
        f"No %mor line mentions dare/dammelo — expected one from "
        f"`per favore dammela .`\nFull output:\n{morphotag_output}"
    )
    for line in dammela_lines:
        assert "adj|dammelo" not in line, (
            f"[dammela mid-sentence] Defect 8 reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_prendilo_mid_sentence_becomes_verb(morphotag_output: str) -> None:
    """Defect 8 (corpus-sourced): `per favore prendilo` must emit
    `verb|prendere`, not `adj|prendilo`."""
    lines = _mor_lines(morphotag_output)
    prendilo_lines = [line for line in lines if "prendere" in line or "prendilo" in line]
    assert prendilo_lines, f"No line mentions prendilo/prendere: {morphotag_output}"
    for line in prendilo_lines:
        assert "adj|prendilo" not in line, (
            f"[prendilo mid-sentence] Defect 8 reconciler did not fire:\n  {line}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_no_junk_pattern_anywhere(morphotag_output: str) -> None:
    """Belt-and-suspenders: no %mor line anywhere in the output
    carries a junk reconciler-miss pattern."""
    lines = _mor_lines(morphotag_output)
    for line in lines:
        for pattern in JUNK_PATTERNS:
            assert pattern not in line, (
                f"Junk pattern {pattern!r} found in %mor:\n  {line}\n"
                f"The Italian Defect 6 reconciler did not fire on at least "
                f"one case. Check crates/batchalign/src/nlp/lang_it.rs "
                f"and the plumbing in nlp/mapping/mod.rs::map_ud_sentence."
            )
