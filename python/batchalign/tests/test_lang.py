"""Tests for `batchalign.lang.LanguageCode` — the ISO-639-3 resolver.

The CLI calls this once at parse time and hands the resolved record to
every ASR backend. These tests pin the contract: valid alpha_3 codes
resolve to the right (alpha_3, alpha_2, name) triple, and anything
else (2-letter, English name, BCP-47 tag, "auto", garbage) raises
`ValueError` with a message naming ISO-639-3.
"""

from __future__ import annotations

import pytest

from batchalign.lang import LanguageCode


def test_eng_resolves_to_english():
    lang = LanguageCode.from_str("eng")
    assert lang.alpha_3 == "eng"
    assert lang.alpha_2 == "en"
    assert lang.name == "English"
    assert lang.alpha_2_or_3 == "en"


def test_yue_has_no_alpha_2_and_falls_through_to_alpha_3():
    lang = LanguageCode.from_str("yue")
    assert lang.alpha_3 == "yue"
    assert lang.alpha_2 is None
    assert lang.alpha_2_or_3 == "yue"
    # pycountry calls Cantonese "Yue Chinese"; we preserve that.
    assert "yue" in lang.name.lower() or "cantonese" in lang.name.lower()


def test_cmn_is_mandarin_with_no_alpha_2():
    # Mandarin specifically (not the Chinese macrolanguage `zho`).
    lang = LanguageCode.from_str("cmn")
    assert lang.alpha_3 == "cmn"
    # `cmn` is the individual Mandarin language and pycountry exposes
    # no alpha_2 for it — only `zho` (macrolanguage) has `zh`.
    assert lang.alpha_2 is None
    assert "mandarin" in lang.name.lower()


def test_zho_macrolanguage_has_alpha_2_zh():
    lang = LanguageCode.from_str("zho")
    assert lang.alpha_3 == "zho"
    assert lang.alpha_2 == "zh"


def test_spa_resolves_to_spanish():
    lang = LanguageCode.from_str("spa")
    assert lang.alpha_2 == "es"
    assert lang.name == "Spanish"


def test_case_and_whitespace_are_normalized():
    assert LanguageCode.from_str(" ENG ") == LanguageCode.from_str("eng")
    assert LanguageCode.from_str("Yue") == LanguageCode.from_str("yue")


@pytest.mark.parametrize(
    "bad",
    [
        "en",          # alpha_2
        "english",     # name
        "zh-hant",     # BCP-47
        "en-US",       # locale
        "auto",        # the sentinel we explicitly removed
        "",            # empty
        "xxx",         # 3 letters, not in registry
        "  ",          # whitespace
    ],
)
def test_rejects_non_alpha_3(bad):
    with pytest.raises(ValueError) as exc:
        LanguageCode.from_str(bad)
    # Error message must steer the user toward ISO-639-3.
    assert "ISO-639-3" in str(exc.value) or "iso-639-3" in str(exc.value).lower()


def test_none_input_raises():
    with pytest.raises(ValueError):
        LanguageCode.from_str(None)  # type: ignore[arg-type]
