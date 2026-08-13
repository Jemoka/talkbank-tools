"""Path and overwrite-safety tests for ``batchalign convert``."""

from pathlib import Path

import pytest
import typer

from batchalign.cli.convert import _collect, _target_for


def test_target_is_sibling_without_output_directory(tmp_path: Path) -> None:
    source = tmp_path / "nested" / "clip.m4a"
    assert _target_for(source, tmp_path, None, ".mp3") == source.with_suffix(".mp3")


def test_target_mirrors_tree_with_output_directory(tmp_path: Path) -> None:
    source = tmp_path / "inputs" / "nested" / "clip.flac"
    output = tmp_path / "converted"
    assert _target_for(source, tmp_path / "inputs", output, ".wav") == (
        output / "nested" / "clip.wav"
    )


def test_same_format_uses_converted_suffix(tmp_path: Path) -> None:
    source = tmp_path / "clip.wav"
    assert _target_for(source, tmp_path, None, ".wav") == tmp_path / "clip.converted.wav"


def test_collect_refuses_existing_destination(tmp_path: Path) -> None:
    source = tmp_path / "clip.m4a"
    source.write_bytes(b"source")
    (tmp_path / "clip.mp3").write_bytes(b"existing")

    with pytest.raises(typer.BadParameter, match="target already exists"):
        _collect(source, None, ".mp3")


def test_collect_refuses_two_inputs_mapping_to_same_destination(tmp_path: Path) -> None:
    (tmp_path / "clip.wav").write_bytes(b"wav")
    (tmp_path / "clip.mp3").write_bytes(b"mp3")

    with pytest.raises(typer.BadParameter, match="multiple inputs map"):
        _collect(tmp_path, tmp_path / "out", ".wav")
