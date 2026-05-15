"""End-to-end CHAT pipeline test: Hebrew and Greek MWT splits land
correctly in ``%mor`` after the 2026-04-15 capability-driven loader
fix. Companion to ``test_stanza_he_el_et_mwt_splits.py`` (pure
Stanza observation tests) — this file verifies that what Stanza
splits, BA3 actually writes as tilde-joined ``%mor``.

This is the regression gate. If a future BA3 change suppresses MWT
for he/el — by reintroducing a hardcoded include/exclude set, by
breaking the capability table, or by mis-handling MWT in
``morphosyntax/inject.rs`` — these tests fail loudly.

Discovery date: 2026-04-15. Defect 5 in
``book/src/reference/stanza-limitations.md`` carries the full
write-up (Ignas's BA2 ``fec893e1`` history, the BA3 regression,
the capability-driven fix, and per-language behavior).

Pattern source: ``test_preserve_mwt_end_to_end.py`` (one CLI
invocation per fixture, module-scoped to amortize cargo cost).
"""

from __future__ import annotations

import subprocess
import tempfile
from pathlib import Path

import pytest


# Minimal valid CHAT requires @UTF8, @Begin, @Languages, @Participants,
# @ID, utterances, @End. The constructions are the same ones whose
# Stanza splits are pinned in test_stanza_he_el_et_mwt_splits.py.

_HEBREW_FIXTURE = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\theb\n"
    "@Participants:\tPAR Adult\n"
    "@ID:\theb|test|PAR|||||Adult|||\n"
    "*PAR:\tבבית גדול .\n"
    "*PAR:\tמהילד הזה .\n"
    "*PAR:\tלאישה היפה .\n"
    "@End\n"
)

# Surface tokens that MUST appear as tilde-joined splits in their
# corresponding %mor lines (one entry per *PAR utterance, in order).
_HEBREW_MWT_SURFACES_PER_LINE: list[list[str]] = [
    ["בבית"],            # line 1: בבית splits as ב+בית
    ["מהילד", "הזה"],    # line 2: מהילד and הזה both split
    ["לאישה"],           # line 3: לאישה splits as ל+אישה
]


_GREEK_FIXTURE = (
    "@UTF8\n"
    "@Begin\n"
    "@Languages:\tell\n"
    "@Participants:\tPAR Adult\n"
    "@ID:\tell|test|PAR|||||Adult|||\n"
    "*PAR:\tστο σπίτι μου .\n"
    "*PAR:\tστον δρόμο .\n"
    "*PAR:\tστις πέντε .\n"
    "@End\n"
)

_GREEK_MWT_SURFACES_PER_LINE: list[list[str]] = [
    ["στο"],
    ["στον"],
    ["στις"],
]


def _run_morphotag(chat_text: str, lang: str) -> str:
    """Run ``batchalign3 morphotag --sequential`` on ``chat_text``.

    Returns the output CHAT text. Raises via ``assert`` on non-zero
    exit (the CLI runs validation internally, so a non-zero exit
    means either pipeline failure or invalid output CHAT).
    """
    with tempfile.TemporaryDirectory() as tmpdir:
        input_path = Path(tmpdir) / f"mwt_e2e_{lang}.cha"
        output_dir = Path(tmpdir) / "output"
        input_path.write_text(chat_text)

        result = subprocess.run(
            [
                "cargo", "run", "-p", "batchalign", "--",
                "--no-open-dashboard", "--override-media-cache",
                "morphotag",
                str(input_path),
                "-o", str(output_dir),
                "--lang", lang,
                "--sequential",
            ],
            capture_output=True,
            text=True,
            timeout=240,
            cwd=str(Path(__file__).resolve().parents[4]),  # batchalign3 root
        )
        assert result.returncode == 0, (
            f"morphotag CLI failed for lang={lang} (exit "
            f"{result.returncode}):\nSTDOUT: {result.stdout[-500:]}\n"
            f"STDERR: {result.stderr[-500:]}"
        )
        output_file = output_dir / input_path.name
        assert output_file.exists(), (
            f"Output file not created for lang={lang}: {output_dir}"
        )
        return output_file.read_text()


def _mor_lines(chat_text: str) -> list[str]:
    """Return the ``%mor:`` tier lines from a CHAT document, in order."""
    return [line for line in chat_text.splitlines() if line.startswith("%mor:")]


# ---------------------------------------------------------------------------
# Hebrew
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def hebrew_morphotag_output() -> str:
    return _run_morphotag(_HEBREW_FIXTURE, "heb")


@pytest.mark.golden
@pytest.mark.integration
def test_hebrew_mwt_emits_tilde_joined_mor(
    hebrew_morphotag_output: str,
) -> None:
    """Each Hebrew utterance with a contraction must have a tilde
    ``~`` in its ``%mor`` line — that is the CHAT marker for an
    MWT-joined morph sequence. If a future regression suppresses
    MWT for Hebrew (e.g., a hardcoded list reappears, or the
    capability table breaks), the contracted forms would surface
    as single morph items with no ``~`` and this test fails.
    """
    mor_lines = _mor_lines(hebrew_morphotag_output)
    assert len(mor_lines) == len(_HEBREW_MWT_SURFACES_PER_LINE), (
        f"Expected {len(_HEBREW_MWT_SURFACES_PER_LINE)} %mor lines, "
        f"got {len(mor_lines)}.\nFull output:\n{hebrew_morphotag_output}"
    )
    for utterance_idx, (mor_line, surfaces) in enumerate(
        zip(mor_lines, _HEBREW_MWT_SURFACES_PER_LINE)
    ):
        assert "~" in mor_line, (
            f"Hebrew %mor line {utterance_idx + 1} is missing the "
            f"tilde-joined MWT marker ~. The surface "
            f"contraction(s) {surfaces} should have produced "
            f"tilde-joined splits.\nLine: {mor_line}\n"
            f"Full output:\n{hebrew_morphotag_output}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_hebrew_mwt_does_not_emit_unsplit_surface_as_lemma(
    hebrew_morphotag_output: str,
) -> None:
    """The contracted surface form must not appear as a lemma of a
    single morph item — that would mean MWT was silently suppressed
    and the surface was tagged whole. Stanza's split components
    (e.g., ב, בית) are the correct lemmas after MWT fires.
    """
    mor_lines = _mor_lines(hebrew_morphotag_output)
    for utterance_idx, (mor_line, surfaces) in enumerate(
        zip(mor_lines, _HEBREW_MWT_SURFACES_PER_LINE)
    ):
        for surface in surfaces:
            # A single un-split morph item with this surface as lemma
            # would look like ``<pos>|<surface>`` somewhere in the
            # tier without being tilde-adjacent. Easiest check: the
            # surface must not appear as a bare lemma followed by a
            # space or end-of-line (i.e., not part of a tilde join).
            bad_substring = f"|{surface} "
            assert bad_substring not in mor_line + " ", (
                f"Hebrew %mor line {utterance_idx + 1} contains "
                f"the contracted surface {surface!r} as an unsplit "
                f"lemma (substring {bad_substring!r}). MWT was "
                f"suppressed.\nLine: {mor_line}\n"
                f"Full output:\n{hebrew_morphotag_output}"
            )


# ---------------------------------------------------------------------------
# Greek
# ---------------------------------------------------------------------------


@pytest.fixture(scope="module")
def greek_morphotag_output() -> str:
    return _run_morphotag(_GREEK_FIXTURE, "ell")


@pytest.mark.golden
@pytest.mark.integration
def test_greek_mwt_emits_tilde_joined_mor(
    greek_morphotag_output: str,
) -> None:
    """Same shape as the Hebrew test: every Greek utterance with a
    σε+article contraction must have a ``~`` in its ``%mor`` line.
    """
    mor_lines = _mor_lines(greek_morphotag_output)
    assert len(mor_lines) == len(_GREEK_MWT_SURFACES_PER_LINE), (
        f"Expected {len(_GREEK_MWT_SURFACES_PER_LINE)} %mor lines, "
        f"got {len(mor_lines)}.\nFull output:\n{greek_morphotag_output}"
    )
    for utterance_idx, (mor_line, surfaces) in enumerate(
        zip(mor_lines, _GREEK_MWT_SURFACES_PER_LINE)
    ):
        assert "~" in mor_line, (
            f"Greek %mor line {utterance_idx + 1} is missing the "
            f"tilde-joined MWT marker ~. The surface "
            f"contraction(s) {surfaces} should have produced "
            f"tilde-joined splits.\nLine: {mor_line}\n"
            f"Full output:\n{greek_morphotag_output}"
        )


@pytest.mark.golden
@pytest.mark.integration
def test_greek_mwt_does_not_emit_unsplit_surface_as_lemma(
    greek_morphotag_output: str,
) -> None:
    """Negative assertion mirroring the Hebrew counterpart."""
    mor_lines = _mor_lines(greek_morphotag_output)
    for utterance_idx, (mor_line, surfaces) in enumerate(
        zip(mor_lines, _GREEK_MWT_SURFACES_PER_LINE)
    ):
        for surface in surfaces:
            bad_substring = f"|{surface} "
            assert bad_substring not in mor_line + " ", (
                f"Greek %mor line {utterance_idx + 1} contains "
                f"the contracted surface {surface!r} as an unsplit "
                f"lemma (substring {bad_substring!r}). MWT was "
                f"suppressed.\nLine: {mor_line}\n"
                f"Full output:\n{greek_morphotag_output}"
            )
