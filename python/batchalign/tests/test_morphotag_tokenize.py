"""CHAT-boundary preservation for Stanza tokenization."""

from batchalign.backends.morphosyntax.ud.tokenize import tokenizer_processor


def test_native_mwt_marker_survives_inside_one_chat_word() -> None:
    assert tokenizer_processor(
        [("nel", True), "gatto"],
        ["it"],
        "nel gatto",
    ) == [("nel", True), "gatto"]


def test_conventional_english_spellings_keep_established_boundaries() -> None:
    assert tokenizer_processor(
        [
            ("dunno", True),
            ("gonna", True),
            ("wanna", True),
            ("whatnot", True),
        ],
        ["en"],
        "dunno gonna wanna whatnot",
    ) == ["dunno", "gonna", "wanna", "whatnot"]


def test_english_apostrophe_marks_contraction_but_not_possessive_its() -> None:
    assert tokenizer_processor(
        ["it's", "bright", "outside", "at", "its", "house"],
        ["en"],
        "it's bright outside at its house",
    ) == [("it's", True), "bright", "outside", "at", "its", "house"]


def test_native_split_is_rejoined_at_one_chat_word() -> None:
    assert tokenizer_processor(
        ["porta", "la"],
        ["it"],
        "portala",
    ) == [("portala", False)]


def test_ambiguous_italian_dai_is_left_for_contextual_pos() -> None:
    assert tokenizer_processor(
        ["tu", ("dai", True), "il", "libro"],
        ["it"],
        "tu dai il libro",
    ) == ["tu", "dai", "il", "libro"]
