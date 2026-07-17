"""Tests for `inputs.sibling_media_for_chat` (Landing 3 #15)."""

from __future__ import annotations

from pathlib import Path

from batchalign.inputs import sibling_media_for_chat


def test_finds_wav_by_stem(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text("@Begin\n*CHI:\thello .\n@End\n")
    wav = tmp_path / "session.wav"
    wav.write_bytes(b"RIFF")
    assert sibling_media_for_chat(cha) == wav


def test_prefers_media_header_over_stem(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text(
        "@Begin\n@Media:\tcustomname, audio\n*CHI:\thello .\n@End\n"
    )
    wav = tmp_path / "customname.wav"
    wav.write_bytes(b"RIFF")
    assert sibling_media_for_chat(cha) == wav


def test_returns_none_when_no_audio(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text("@Begin\n*CHI:\thello .\n@End\n")
    assert sibling_media_for_chat(cha) is None


def test_returns_none_when_chat_missing(tmp_path: Path) -> None:
    assert sibling_media_for_chat(tmp_path / "missing.cha") is None


def test_finds_mp3_when_wav_absent(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text("@Begin\n*CHI:\thello .\n@End\n")
    mp3 = tmp_path / "session.mp3"
    mp3.write_bytes(b"ID3")
    assert sibling_media_for_chat(cha) == mp3


def test_finds_flac(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text("@Begin\n*CHI:\thello .\n@End\n")
    flac = tmp_path / "session.flac"
    flac.write_bytes(b"fLaC")
    assert sibling_media_for_chat(cha) == flac


def test_finds_mov_video(tmp_path: Path) -> None:
    cha = tmp_path / "session.cha"
    cha.write_text("@Begin\n@Media:\tsession, video\n*CHI:\thello .\n@End\n")
    mov = tmp_path / "session.mov"
    mov.write_bytes(b"movie")
    assert sibling_media_for_chat(cha) == mov
