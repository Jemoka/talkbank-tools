"""Tests for Malayalam SaT utterance segmentation."""

from __future__ import annotations

import sys
import types


def test_sat_sentences_keep_original_word_timestamps(monkeypatch):
    class FakeSaT:
        def __init__(self, model):
            assert model == "sat-3l-sm"

        def split(self, text):
            assert text == "ഇന്ന് നല്ല മഴയാണ് ഞാൻ പോകുന്നില്ല"
            return ["ഇന്ന് നല്ല മഴയാണ്", "ഞാൻ പോകുന്നില്ല"]

    monkeypatch.setitem(sys.modules, "wtpsplit", types.SimpleNamespace(SaT=FakeSaT))

    class Record:
        def __init__(self, **kwargs):
            self.__dict__.update(kwargs)

    class UtSegInput(Record):
        pass

    proto = types.ModuleType("batchalign._core.proto")
    proto.UtSegInput = UtSegInput
    proto.UtSegOutput = type("UtSegOutput", (Record,), {})
    proto.UtteranceSpan = type("UtteranceSpan", (Record,), {})
    monkeypatch.setitem(sys.modules, "batchalign._core.proto", proto)

    from batchalign.backends.utseg.malayalam_sat import MalayalamSaTBackend

    words = [
        Record(text=text, start_ms=i * 100, end_ms=(i + 1) * 100, confidence=None)
        for i, text in enumerate(["ഇന്ന്", "നല്ല", "മഴയാണ്", "ഞാൻ", "പോകുന്നില്ല"])
    ]
    item = UtSegInput(
        source_id="sample.wav",
        segments=[
            Record(
                text="ഇന്ന് നല്ല മഴയാണ് ഞാൻ പോകുന്നില്ല",
                start_ms=0,
                end_ms=500,
                words=words,
            )
        ],
    )

    output = MalayalamSaTBackend().call([item])[0]

    assert [span.text for span in output.utterances] == [
        "ഇന്ന് നല്ല മഴയാണ്",
        "ഞാൻ പോകുന്നില്ല",
    ]
    assert [(span.start_ms, span.end_ms) for span in output.utterances] == [
        (0, 300),
        (300, 500),
    ]
    assert [[word.text for word in span.words] for span in output.utterances] == [
        ["ഇന്ന്", "നല്ല", "മഴയാണ്"],
        ["ഞാൻ", "പോകുന്നില്ല"],
    ]


def test_public_backend_export_is_utseg():
    from batchalign.backends import MalayalamSaTBackend, UtSeg

    assert issubclass(MalayalamSaTBackend, UtSeg)
