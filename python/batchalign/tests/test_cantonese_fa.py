"""Cantonese Jyutping preprocessing for the shared wav2vec2 FA backend."""

from __future__ import annotations

from types import SimpleNamespace

from batchalign.backends.fa.wav2vec2 import _alignment_words, _hanzi_to_jyutping


class _Romanizer:
    _pronunciations = {
        "你": "nei5",
        "好": "hou2",
        "我": "ngo5",
        "係": "hai6",
    }

    def characters_to_jyutping(self, text: str):
        return [(character, self._pronunciations.get(character)) for character in text]


def _language(code: str):
    return SimpleNamespace(kind="code", value=code)


def test_hanzi_to_jyutping_strips_tones_and_joins_syllables():
    romanizer = _Romanizer()

    assert _hanzi_to_jyutping("你好", romanizer) == "nei'hou"
    assert _hanzi_to_jyutping("我係", romanizer) == "ngo'hai"
    assert _hanzi_to_jyutping("unknown", romanizer) == "unknown"


def test_alignment_words_romanizes_only_resolved_cantonese():
    romanizer = _Romanizer()
    words = ["你好", "我係", "hello!"]

    assert _alignment_words(words, _language("yue"), romanizer=romanizer) == [
        "nei'hou",
        "ngo'hai",
        "hello",
    ]
    assert _alignment_words(words, _language("eng"), romanizer=romanizer) == [
        "你好",
        "我係",
        "hello",
    ]


def test_unresolved_language_does_not_guess_cantonese():
    assert _alignment_words(
        ["你好"], SimpleNamespace(kind="per_file"), romanizer=_Romanizer()
    ) == ["你好"]
