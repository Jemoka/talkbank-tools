"""Hermetic tests for the post-ASR cleanup (disfluency + retrace).

`batchalign.backends.utseg.cleanup` ports BA2's filled-pause/replacement
word-lists and the n-gram retrace marker. These run with no models.
"""

from __future__ import annotations

from batchalign.backends.utseg import cleanup
from batchalign.backends.utseg.chatutterance import CHATUtteranceBackend
from batchalign._core.proto import (
    AsrSegment,
    LanguageSpecPerFile,
    UtSegInput,
)


def test_disfluency_marks_filled_pauses():
    table = cleanup.load_cleanup("eng")
    # uh/um/ur → &-uh/&-um/&-ur (BA2 filled_pauses.eng).
    assert cleanup.apply_disfluency("i am honored uh to meet you", table) == (
        "i am honored &-uh to meet you"
    )
    assert cleanup.apply_disfluency("um well", table).startswith("&-um")


def test_disfluency_applies_replacements():
    table = cleanup.load_cleanup("eng")
    # cuz → (be)cause (BA2 replacements.eng).
    assert "(be)cause" in cleanup.apply_disfluency("i left cuz tired", table)


def test_retrace_marks_single_word_repeat():
    assert cleanup.mark_retraces("los los datos", "es") == "los [/] los datos"
    assert cleanup.mark_retraces("para para nosotros", "es") == "para [/] para nosotros"


def test_retrace_wraps_multiword_repeat():
    assert cleanup.mark_retraces("a b a b c", "en") == "<a b> [/] a b c"


def test_retrace_no_false_positive():
    assert cleanup.mark_retraces("the red ball", "en") == "the red ball"


def test_clean_utterance_disfluency_then_retrace():
    table = cleanup.load_cleanup("eng")
    # uh → &-uh, and the repeated "the the" → the [/] the.
    assert cleanup.clean_utterance("uh the the dog", table, "en") == "&-uh the [/] the dog"


def test_chatutterance_strips_internal_periods_from_segmenter_output():
    backend = CHATUtteranceBackend.__new__(CHATUtteranceBackend)
    backend._lang = "eng"
    backend._cleanup = {}
    backend._segmenter = lambda _text: [
        "d. oh two seven thirtieth of September twenty [/] twenty three yeah."
    ]

    [out] = backend.call(
        [
            UtSegInput(
                source_id="fixture.wav",
                segments=[
                    AsrSegment(
                        start_ms=0,
                        end_ms=1000,
                        text="D oh two seven thirtieth of september twenty twenty three yeah",
                        speaker=None,
                        words=[],
                    )
                ],
                language=LanguageSpecPerFile(kind="per_file"),
                stanza_fallback=False,
            )
        ]
    )

    assert out.utterances[0].text == (
        "d oh two seven thirtieth of September twenty [/] twenty three yeah."
    )
