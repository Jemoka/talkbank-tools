"""RED/GREEN test: Dutch utseg must not crash on missing constituency model.

Source: an operator's bug report, 2026-03-28.
"""

from batchalign.worker._stanza_capabilities import (
    StanzaCapabilityTable,
    StanzaLanguageCapability,
)


def test_utseg_config_builder_skips_constituency_for_dutch_when_table_missing(
    monkeypatch,
):
    """The safe fallback must not guess constituency for Dutch."""
    from batchalign.worker._stanza_loading import load_utseg_builder
    from batchalign.worker import _stanza_capabilities
    from batchalign.worker._types import _state

    monkeypatch.setattr(
        _stanza_capabilities,
        "get_cached_capability_table",
        lambda: None,
    )

    load_utseg_builder("nld")
    assert _state.utseg_config_builder is not None

    lang_alpha2, configs = _state.utseg_config_builder(["nld"])
    assert "nl" in lang_alpha2
    nl_config = configs.get("nl", {})
    processors = nl_config.get("processors", "")

    assert "constituency" not in processors, (
        f"Dutch should NOT include constituency, got: {processors}"
    )


def test_utseg_config_builder_includes_constituency_for_english(monkeypatch):
    """English SHOULD include constituency when the table reports it."""
    from batchalign.worker._stanza_loading import load_utseg_builder
    from batchalign.worker import _stanza_capabilities
    from batchalign.worker._types import _state

    monkeypatch.setattr(
        _stanza_capabilities,
        "get_cached_capability_table",
        lambda: StanzaCapabilityTable(
            languages={
                "eng": StanzaLanguageCapability(
                    alpha2="en",
                    has_tokenize=True,
                    has_pos=True,
                    has_lemma=True,
                    has_constituency=True,
                )
            },
            iso3_to_alpha2={"eng": "en"},
            stanza_version="test-stanza",
        ),
    )

    load_utseg_builder("eng")
    assert _state.utseg_config_builder is not None

    lang_alpha2, configs = _state.utseg_config_builder(["eng"])
    en_config = configs.get("en", {})
    processors = en_config.get("processors", "")

    assert "constituency" in processors, (
        f"English SHOULD include constituency, got: {processors}"
    )
