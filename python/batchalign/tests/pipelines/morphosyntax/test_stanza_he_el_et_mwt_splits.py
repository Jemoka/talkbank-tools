"""Pure Stanza observation tests for Hebrew, Greek, and Estonian MWT.

These tests pin Stanza's actual MWT behavior on canonical
contraction/fusion constructions for each of the three languages
that flipped state in the 2026-04-15 capability-driven loader fix.
They have **no batchalign imports** — they are safe to copy verbatim
into a Stanza upstream issue if the observed behavior ever drifts.

Companion to ``test_stanza_fi_mwt_sos_leak.py`` (Defect 4 pattern).
The end-to-end CHAT-pipeline counterpart lives in
``test_he_el_mwt_end_to_end.py``.

Discovery date: 2026-04-15. The 2026-04-14 chunk_106 morphotag
outage exposed BA3's hardcoded ``MWT_LANGS`` set as drifted from
Stanza's catalog. Fixing the loader to consult the live capability
table flipped MWT-requesting behavior for several languages
(``swe`` False→False, ``heb`` False→True, ``ell`` False→True,
``est`` False→True). These tests document what each of the now-
enabled languages actually produces.

See ``book/src/reference/stanza-limitations.md`` Defect 5 for the
full write-up and the BA2-jan9 history that informed the choice.
"""

from __future__ import annotations

from typing import TYPE_CHECKING

import pytest

# Stanza is imported lazily inside each fixture so pytest collection
# doesn't pay the ~1-2s stanza+torch+numpy+transformers cascade when
# this file isn't selected.
if TYPE_CHECKING:
    import stanza


# Canonical Hebrew constructions that exercise prepositional-clitic
# attachment and definite-article fusion. Each entry is
# ``(input_sentence, surface_token, expected_expansion_words)``.
# These are linguistically standard examples drawn from elementary
# Hebrew morphology references; the splits Stanza produces match
# the underlying lexical decomposition (prep + def + noun, etc.).
_HEBREW_CASES: list[tuple[str, str, list[str]]] = [
    # "in the (big) house": בְּ "in" + הַ "the" (absorbed) + בית "house"
    ("בבית גדול", "בבית", ["ב", "בית"]),
    # "from the boy": מִ "from" + הַ "the" + יֶלֶד "boy"
    ("מהילד הזה", "מהילד", ["מ", "ה", "ילד"]),
    # "to the (beautiful) woman": לְ "to" + אישה "woman"
    ("לאישה היפה", "לאישה", ["ל", "אישה"]),
    # "this": הַ "the" + זֶה "this"
    ("מהילד הזה", "הזה", ["ה", "זה"]),
]


# Canonical Greek constructions: σε "in/at" + definite article
# contractions across gender/case/number. Each split is
# σ + το/τον/τις/etc. — the article retains its case marking on
# the second component.
_GREEK_CASES: list[tuple[str, str, list[str]]] = [
    # "in my house": σε "in" + το "the (n.acc)"
    ("στο σπίτι μου", "στο", ["σ", "το"]),
    # "on the street (m.acc)": σε + τον "the (m.acc)"
    ("στον δρόμο", "στον", ["σ", "τον"]),
    # "at five o'clock": σε + τις "the (f.pl.acc)"
    ("στις πέντε", "στις", ["σ", "τις"]),
]


# Estonian no-op cases: contracted-negation forms that UD
# Estonian-EDT does mark as MWT in the treebank, but Stanza-1.11.1's
# Estonian MWT model does not split. We pin this empirical reality
# so that a future Stanza upgrade which *starts* splitting these
# forms surfaces immediately as a test failure, prompting an
# update to the Defect 5 entry rather than silent ``%mor`` drift.
_ESTONIAN_NO_SPLIT_INPUTS: list[str] = [
    "pole tähtis",       # ei + ole "is not"
    "polnud aega",       # ei + olnud "had no"
    "ma pole näinud",    # "I have not seen"
    "ta polegi tulnud",  # "(s)he has not even come" — pole + gi clitic
]


def _split_for_token(doc: stanza.Document, surface: str) -> list[str] | None:
    """Return the words a Stanza token expanded to, or None if not split.

    Walks the document's tokens looking for one whose surface text
    equals ``surface``. Returns the list of constituent ``word.text``
    strings if the token expanded to more than one word; returns
    ``None`` if the token is present but not split, or if the surface
    form is not found at all (caller distinguishes via assertion).
    """
    for sent in doc.sentences:
        for tok in sent.tokens:
            if tok.text == surface:
                if hasattr(tok, "words") and len(tok.words) > 1:
                    return [w.text for w in tok.words]
                return None
    return None


@pytest.fixture(scope="module")
def hebrew_pipeline() -> stanza.Pipeline:
    """Hebrew Stanza pipeline with MWT — module-scoped to amortize
    the model-load cost across all Hebrew assertions in this file."""
    import stanza
    from stanza import DownloadMethod

    return stanza.Pipeline(
        lang="he",
        processors="tokenize,pos,lemma,depparse,mwt",
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        verbose=False,
    )


@pytest.fixture(scope="module")
def greek_pipeline() -> stanza.Pipeline:
    """Greek Stanza pipeline with MWT — module-scoped."""
    import stanza
    from stanza import DownloadMethod

    return stanza.Pipeline(
        lang="el",
        processors="tokenize,pos,lemma,depparse,mwt",
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        verbose=False,
    )


@pytest.fixture(scope="module")
def estonian_pipeline() -> stanza.Pipeline:
    """Estonian Stanza pipeline with MWT — module-scoped."""
    import stanza
    from stanza import DownloadMethod

    return stanza.Pipeline(
        lang="et",
        processors="tokenize,pos,lemma,depparse,mwt",
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        verbose=False,
    )


# ---------------------------------------------------------------------------
# Hebrew
# ---------------------------------------------------------------------------


@pytest.mark.golden
@pytest.mark.parametrize("sentence,surface,expected", _HEBREW_CASES)
def test_hebrew_mwt_split_matches_linguistic_decomposition(
    hebrew_pipeline: stanza.Pipeline,
    sentence: str,
    surface: str,
    expected: list[str],
) -> None:
    """Stanza Hebrew MWT must split these contractions into their
    documented underlying morphemes.

    Why this assertion shape: Hebrew prepositional clitics
    (``ב/ל/מ``) and the definite article ``ה`` are real morphemes
    that combine orthographically with the head noun. CHAT ``%mor``
    expresses them as separate units joined with ``~``. If Stanza
    starts merging them back into a single token, BA3 ``%mor``
    output would lose information silently — so we lock the split
    here.
    """
    doc = hebrew_pipeline(sentence)
    actual = _split_for_token(doc, surface)
    assert actual is not None, (
        f"Stanza did not split {surface!r} in {sentence!r}; "
        f"either the surface form was tokenized differently or MWT "
        f"is not firing. Update Defect 5 in stanza-limitations.md "
        f"if Stanza Hebrew MWT behavior has regressed."
    )
    assert actual == expected, (
        f"Stanza Hebrew MWT split {surface!r} as {actual} but the "
        f"linguistic decomposition is {expected}. If Stanza changed "
        f"its split shape, decide whether the new shape is "
        f"defensible (update assertion) or file upstream."
    )


# ---------------------------------------------------------------------------
# Greek
# ---------------------------------------------------------------------------


@pytest.mark.golden
@pytest.mark.parametrize("sentence,surface,expected", _GREEK_CASES)
def test_greek_mwt_split_matches_linguistic_decomposition(
    greek_pipeline: stanza.Pipeline,
    sentence: str,
    surface: str,
    expected: list[str],
) -> None:
    """Stanza Greek MWT must split σε+article contractions into the
    preposition and the case-marked definite article.

    The article retains its inflection on the second component
    (``το``/``τον``/``τις``); merging back into ``στο``/``στον``/
    ``στις`` would erase the case information from BA3 ``%mor``.
    """
    doc = greek_pipeline(sentence)
    actual = _split_for_token(doc, surface)
    assert actual is not None, (
        f"Stanza did not split {surface!r} in {sentence!r}; "
        f"Greek MWT may have regressed. Update Defect 5 in "
        f"stanza-limitations.md if so."
    )
    assert actual == expected, (
        f"Stanza Greek MWT split {surface!r} as {actual}; expected "
        f"{expected}. Verify against UD Greek-GDT before changing "
        f"the assertion."
    )


# ---------------------------------------------------------------------------
# Estonian (pinning the no-op)
# ---------------------------------------------------------------------------


@pytest.mark.golden
@pytest.mark.parametrize("sentence", _ESTONIAN_NO_SPLIT_INPUTS)
def test_estonian_mwt_does_not_split_pole_class_negation(
    estonian_pipeline: stanza.Pipeline,
    sentence: str,
) -> None:
    """Stanza Estonian MWT does NOT split contracted-negation forms.

    UD Estonian-EDT marks ``pole`` (= ei+ole), ``polnud`` (= ei+olnud),
    and ``polegi`` (= ei+ole+gi) as MWT in the treebank, but Stanza-
    1.11.1's Estonian MWT model leaves them unsplit on conversational
    input. This test pins that behavior so a future upstream change
    that *does* start splitting them surfaces as a loud failure
    rather than silent ``%mor`` drift in production output.

    If this test fails (Stanza now splits), the right response is:

    1. Update Defect 5 in stanza-limitations.md with the new behavior.
    2. Add positive assertions for whatever splits Stanza now produces.
    3. Verify the splits are linguistically correct against UD.
    4. Decide whether BA3 ``%mor`` should incorporate them.
    """
    doc = estonian_pipeline(sentence)
    splits_found = []
    for sent in doc.sentences:
        for tok in sent.tokens:
            if hasattr(tok, "words") and len(tok.words) > 1:
                splits_found.append(
                    f"{tok.text}=[{'+'.join(w.text for w in tok.words)}]"
                )
    assert not splits_found, (
        f"Estonian MWT now splits tokens in {sentence!r}: "
        f"{splits_found}. This is a behavior change from the "
        f"Stanza-1.11.1 baseline — see Defect 5 in "
        f"stanza-limitations.md for the procedure."
    )
