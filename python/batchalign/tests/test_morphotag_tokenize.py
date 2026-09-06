"""CHAT-boundary preservation for Stanza tokenization."""

from batchalign.backends.morphosyntax.ud.tokenize import tokenizer_processor


def test_native_mwt_marker_survives_inside_one_chat_word() -> None:
    assert tokenizer_processor(
        [("nel", True), "gatto"],
        ["it"],
        "nel gatto",
    ) == [("nel", True), "gatto"]


def test_italian_native_mwt_uses_closed_validated_inventory() -> None:
    assert tokenizer_processor(
        [
            (surface, True)
            for surface in (
                "bianche quale tutti puzzle rastrello piccoli triangoli cavallo "
                "pistola dispetti bene vola cane dormi devi portala eccolo"
            ).split()
        ],
        ["it"],
        (
            "bianche quale tutti puzzle rastrello piccoli triangoli cavallo "
            "pistola dispetti bene vola cane dormi devi portala eccolo"
        ),
    ) == [
        *(
            "bianche quale tutti puzzle rastrello piccoli triangoli cavallo "
            "pistola dispetti bene vola cane dormi devi"
        ).split(),
        ("portala", True),
        ("eccolo", True),
    ]


def test_conventional_english_spellings_keep_established_boundaries() -> None:
    assert tokenizer_processor(
        [
            ("dunno", True),
            ("gonna", True),
            ("gotta", True),
            ("wanna", True),
            ("whatnot", True),
        ],
        ["en"],
        "dunno gonna gotta wanna whatnot",
    ) == ["dunno", "gonna", "gotta", "wanna", "whatnot"]


def test_english_native_mwt_rejects_names_but_keeps_contractions() -> None:
    assert tokenizer_processor(
        [
            ("connor", True),
            ("anna", True),
            ("arent", True),
            ("cannot", True),
        ],
        ["en"],
        "connor anna arent cannot",
    ) == ["connor", "anna", ("arent", True), ("cannot", True)]


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


def test_italian_elision_keeps_native_tokens_within_chat_word() -> None:
    assert tokenizer_processor(
        ["prendo", "l'", "uva", "con", "l'", "amico"],
        ["it"],
        "prendo l'uva con l'amico",
    ) == ["prendo", "l'", "uva", "con", "l'", "amico"]


def test_italian_preposition_elision_preserves_native_mwt_candidate() -> None:
    assert tokenizer_processor(
        [("sull'", True), "altalena", ("sulla", True), "sedia"],
        ["it"],
        "sull'altalena sulla sedia",
    ) == [("sull'", True), "altalena", ("sulla", True), "sedia"]


def test_mwt_candidate_crossing_chat_boundary_is_not_preserved() -> None:
    assert tokenizer_processor(
        [("sull' altalena", True)],
        ["it"],
        "sull' altalena",
    ) == ["sull'", "altalena"]


def test_ambiguous_italian_dai_is_left_for_contextual_pos() -> None:
    assert tokenizer_processor(
        ["tu", ("dai", True), "il", "libro"],
        ["it"],
        "tu dai il libro",
    ) == ["tu", "dai", "il", "libro"]
