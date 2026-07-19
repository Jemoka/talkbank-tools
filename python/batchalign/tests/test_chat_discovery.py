from pathlib import Path

import pytest
import typer

from batchalign.cli._common import CHAT_EXTENSIONS, _walk
from batchalign.cli.align import _infer_lang


def test_align_language_inference_skips_appledouble_sidecar(tmp_path: Path) -> None:
    (tmp_path / "._session.cha").write_bytes(b"\x00\x05AppleDouble metadata")
    (tmp_path / "session.cha").write_text(
        "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello .\n@End\n",
        encoding="utf-8",
    )

    assert [path.name for path in _walk(tmp_path, CHAT_EXTENSIONS)] == ["session.cha"]
    assert _infer_lang(tmp_path).alpha_3 == "eng"


def test_explicit_appledouble_sidecar_is_not_a_chat_input(tmp_path: Path) -> None:
    sidecar = tmp_path / "._session.cha"
    sidecar.write_bytes(b"\x00\x05AppleDouble metadata")

    assert _walk(sidecar, CHAT_EXTENSIONS) == []
    with pytest.raises(typer.BadParameter, match="no @Languages"):
        _infer_lang(sidecar)
