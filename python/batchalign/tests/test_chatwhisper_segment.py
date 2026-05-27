"""Hermetic tests for the CHATWhisper utterance-segmentation glue.

`segment_words` is the deterministic part of the CHATWhisper ASR backend
(BA2's `retokenize_with_engine`): given a flat timed word stream and a
sentence segmenter, it groups words into utterances and recovers the
terminator. We drive it with a fake segmenter so it runs with no models,
no torch, no audio.
"""

from __future__ import annotations

from batchalign.backends.asr.chatwhisper import segment_words, num2words_en


def _fake_segmenter(sentences):
    """Return a segmenter callable that yields the given sentence strings,
    ignoring its input (the real BERT model decides the split)."""
    return lambda _passage: list(sentences)


def test_segment_words_splits_on_segmenter_sentences():
    words = [
        ("hello", 0, 500),
        ("world", 500, 1000),
        ("how", 1000, 1500),
        ("are", 1500, 2000),
        ("you", 2000, 2500),
    ]
    seg = _fake_segmenter(["Hello world.", "How are you?"])
    utts = segment_words(words, 0, seg)

    assert len(utts) == 2
    (_spk0, w0, d0), (_spk1, w1, d1) = utts
    assert [t for t, _, _ in w0] == ["Hello", "world"]
    assert d0 == "."
    assert [t for t, _, _ in w1] == ["How", "are", "you"]
    assert d1 == "?"
    # Original timings carry through, aligned by position.
    assert w0[0][1] == 0 and w0[1][2] == 1000
    assert w1[2][2] == 2500


def test_segment_words_defaults_terminator_to_period():
    words = [("ok", 0, 100)]
    utts = segment_words(words, 0, _fake_segmenter(["ok"]))
    assert utts[0][2] == "."


def test_segment_words_strips_preexisting_punctuation_before_segmenting():
    seen = {}

    def seg(passage):
        seen["passage"] = passage
        return ["Hi."]

    segment_words([("Hi,", 0, 100)], 0, seg)
    # The segmenter must receive lowercased, punctuation-free text.
    assert seen["passage"] == "hi"


def test_num2words_en_leaves_non_digits_alone():
    # Non-digit words pass through untouched regardless of num2words install.
    assert num2words_en("hello") == "hello"
    assert num2words_en("world") == "world"


def test_num2words_en_expands_digits_when_available():
    import importlib.util

    if importlib.util.find_spec("num2words") is None:
        # Graceful fallback: digits pass through unchanged when num2words is
        # not installed (BA2 ships it; the backend degrades cleanly without).
        assert num2words_en("3") == "3"
    else:
        assert num2words_en("3") == "three"
        assert num2words_en("twenty-3") == "twenty-three"
