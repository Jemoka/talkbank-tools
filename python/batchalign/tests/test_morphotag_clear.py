"""Test the --clear-existing pre-processing helper for morphotag.

Verifies that `_strip_existing_mor_gra` drops `%mor:` / `%gra:` lines
and continuation lines (tab-indented) while preserving every other
tier and the file header.
"""

from __future__ import annotations

from pathlib import Path

from batchalign.cli.morphotag import _strip_existing_mor_gra


SAMPLE = """\
@Begin
@Languages:\teng
@Participants:\tCHI Child, INV Investigator
@ID:\teng|sample|CHI|3;0|||Target_Child|||
*INV:\thello there .
%mor:\tco|hello adv|there .
%gra:\t1|2|COM 2|0|ROOT
*CHI:\thi .
%mor:\tco|hi .
*CHI:\twhat is that ?
@End
"""


def test_strip_drops_mor_gra(tmp_path: Path) -> None:
    src = tmp_path / "in.cha"
    dst = tmp_path / "out.cha"
    src.write_text(SAMPLE)
    _strip_existing_mor_gra(src, dst)
    out = dst.read_text()
    assert "%mor:" not in out
    assert "%gra:" not in out
    # Preserved main tier + headers.
    assert "*INV:\thello there ." in out
    assert "*CHI:\twhat is that ?" in out
    assert "@Begin" in out
    assert "@End" in out


def test_strip_preserves_unrelated_tiers(tmp_path: Path) -> None:
    src = tmp_path / "in.cha"
    dst = tmp_path / "out.cha"
    src.write_text(
        "@Begin\n"
        "*CHI:\tword .\n"
        "%mor:\tn|word .\n"
        "%com:\tnote on this utterance\n"
        "%pho:\tw3rd .\n"
        "@End\n"
    )
    _strip_existing_mor_gra(src, dst)
    out = dst.read_text()
    assert "%mor:" not in out
    assert "%com:\tnote on this utterance" in out
    assert "%pho:\tw3rd ." in out
