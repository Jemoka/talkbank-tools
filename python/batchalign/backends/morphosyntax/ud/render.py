"""UD → structured `%mor` / `%gra` analysis — a faithful port of BA2.

This module ports Batchalign2's `batchalign/pipelines/morphosyntax/ud.py`
handler layer (the `handler*` functions, `HANDLERS`, `handle`, and
`parse_sentence`). It takes a Stanza ``sentence`` object (with ``.words`` /
``.tokens`` carrying ``.text``, ``.lemma``, ``.upos``, ``.feats``, ``.head``,
``.deprel``, ``.id``) and produces the **structured** morphological analysis
BA2 produced — lowercase CHAT POS (``pron``/``verb``/``det`` …), per-POS ordered
+ combined features (``S1``/``S3`` person+number), cleaned lemmas, and
dependency triples — with clitic / auxiliary / MWT joining expressed as *word
grouping* rather than string concatenation.

Unlike BA2, **this module never builds `%mor` / `%gra` tier text.** It emits
structured tuples — `(pos, lemma, features, index, head, deprel)` per morpho-unit
— and the Rust morphosyntax taskrunner turns those into typed `talkbank_model`
`MorTier` / `GraTier` values via the official CHAT writer. Building CHAT by
string concatenation is forbidden (see ``CLAUDE.md``); the analysis here stays
structured end to end.

Parity over elegance: the per-POS feature logic and BA2's quirks (the
``door zogen`` fix, the ROOT-head ``actual_indicies[head-1]`` wrap, the FLAT
override for special forms) are mirrored so the resulting tiers are
byte-identical. Do not "clean this up" without a parity test proving the output
is unchanged.

Source of truth: ``batchalign2/batchalign/pipelines/morphosyntax/ud.py``.
Language-specific helpers (``en/irr.py``, ``fr/case.py``, ``fr/apm.py``,
``ja/verbforms.py``) are copied verbatim into sibling subpackages.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from typing import Any


# --- structured result types ----------------------------------------------


@dataclass
class MorUnit:
    """One morpho-unit: a single ``pos|lemma-feat...`` analysis plus its `%gra`
    relation. Maps 1:1 to the proto ``MorphosyntaxUnit`` and to one typed
    ``MorWord`` / one `%gra` chunk."""

    pos: str
    lemma: str
    features: list[str] = field(default_factory=list)
    index: int = 0
    head: int = 0
    deprel: str = ""


@dataclass
class MorWordGroup:
    """One main-tier word: a head unit followed by ``~``-joined post-clitics.
    Maps to the proto ``MorphosyntaxToken`` (and a typed ``Mor``)."""

    text: str
    units: list[MorUnit]


@dataclass
class SentenceAnalysis:
    """Structured analysis of one utterance: the word groups plus the trailing
    terminator `%gra` relation. ``words`` empty means "no tiers" (BA2 emits
    nothing for degenerate analyses)."""

    words: list[MorWordGroup]
    terminator: tuple[int, int, str] | None  # (index, head, deprel)


# --- feature helpers (BA2 ud.py:44-54) ------------------------------------


def parse_feats(word: Any) -> dict[str, str]:
    """Parse a Stanza ``feats`` string into a ``{Key: Value}`` dict."""
    try:
        return {i.split("=")[0]: i.split("=")[1] for i in word.feats.split("|")}
    except AttributeError:
        return {}


def feat_list(*feats: str) -> list[str]:
    """Structured analogue of BA2's ``stringify_feats``: keep the non-empty
    feature values (in order), stripping commas. BA2 joined these with dashes
    into the tier string; here they stay a list and the typed writer hyphen-
    joins them."""
    return [f.replace(",", "") for f in feats if f != ""]


# --- POS handlers (BA2 ud.py:60-340) --------------------------------------
#
# Each handler returns a structured `(pos, lemma, features)` triple instead of
# the BA2 `pos|lemma-feat` string. `handler()` (the generic base) returns just
# `(pos, lemma)`; the per-POS handlers add their feature list.


def handler(word: Any, lang: str | None = None) -> tuple[str, str]:
    """The generic handler: clean the lemma and emit ``(pos, lemma)``."""
    target = word.lemma
    if target.strip() == "」" or target.strip() == "「":
        target = word.text

    if target == '"':
        target = word.text
    if not target:
        target = word.text
    target = target.replace("」", "")
    target = target.replace("「", "")

    unknown = False

    # leading 0 → unknown word; strip and flag
    if target[0] == "0":
        target = word.text[1:]
        unknown = True

    # a stray <SOS> sequence-start token means the model went sideways
    if "<SOS>" in target:
        target = word.text

    target = target.replace("$", "")
    target = target.replace(".", "")

    if target != "" and target[0] == "-":
        target = target[1:]
    if target != "" and target[-1] == "-":
        target = target[:-1]

    target = target.replace("--", "-")
    target = target.replace("--", "-")
    target = target.replace("<unk>", "")
    target = target.replace("<SOS>", "")

    target = target.replace(",", "")
    target = target.replace("'", "")
    target = target.replace("~", "")
    target = target.replace("/100", "")
    target = target.replace("/r", "")
    target = target.replace("(", "")
    target = target.replace("(", "").replace(")", "")

    if "|" in target:
        target = target.split("|")[0].strip()

    target = target.replace("_", "")
    target = target.replace("+", "")

    if target == "door zogen":
        target = word.text

    target = target.replace("-", "–")

    if "“" in target:
        target = word.text

    pos = word.upos.lower()

    if lang == "ja":
        from .ja.verbforms import verbform

        pos, target = verbform(pos, target, word.text)
        target = target.replace(",", "cm")

    target = re.sub(r"@\w$", "", target).strip()
    return (f"{'0' if unknown else ''}{pos}", target)


def handler__PRON(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    feats = parse_feats(word)
    person = str(feats.get("Person", 1))

    if person == "0":
        person = "4"

    case = feats.get("Case", "")
    reflex = str(feats.get("Reflex", "")).strip()
    if reflex == "Yes":
        reflex = "reflx"
    if lang == "fr":
        from .fr.case import case as caser

        case = caser(word.text)

    number_string = feats.get("Number", "S")[:1] + person
    if word.text in ["that", "who"]:
        number_string = ""

    pos, lemma = handler(word, lang)
    return pos, lemma, feat_list(
        feats.get("PronType", "Int"),
        case.replace(",", ""),
        reflex,
        number_string,
    )


def handler__DET(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    try:
        feats = parse_feats(word)
    except AttributeError:
        pos, lemma = handler(word)
        return pos, lemma, []

    number = feats.get("Number", "")
    gender = feats.get(
        "Gender", "" if lang != "fr" else ("" if number == "Plur" else "Masc")
    ).replace(",", "")

    number_psor = feats.get("Number[psor]", "")[:1]
    person_psor = feats.get("Person[psor]", "")
    psor = number_psor + person_psor

    if gender in ("Com,Neut", "Com", ""):
        gender = ""

    pos, lemma = handler(word, lang)
    # BA2: handler + gender_str + "-" + Definite + stringify_feats(PronType, number, psor)
    # The Definite feature is always present (defaults to "Def").
    features: list[str] = []
    if gender:
        features.append(gender)
    features.append(feats.get("Definite", "Def"))
    features += feat_list(feats.get("PronType", ""), number, psor)
    return pos, lemma, features


def handler__ADJ(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    feats = parse_feats(word)
    deg = feats.get("Degree", "Pos")
    case = feats.get("Case", "").replace(",", "")
    number = feats.get("Number", "S")[0]
    person = str(feats.get("Person", 1))
    if person == "0":
        person = "4"

    if deg == "Pos":
        deg = ""

    pos, lemma = handler(word, lang)
    return pos, lemma, feat_list(deg, case, number[:1] + person)


def handler__NOUN(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    feats = parse_feats(word)

    gender = feats.get("Gender", "ComNeut").replace(",", "")
    number = feats.get("Number", "Sing")
    case = feats.get("Case", "").replace(",", "")
    type_ = feats.get("PronType", "")

    apm = ""
    if lang == "fr" and number == "Plur":
        from .fr.apm import is_apm_noun

        apm = "Apm" if is_apm_noun(word.text) else ""

    if word.deprel == "obj" and case.strip() == "":
        case = "Acc"

    ger = ""
    if word.text.endswith("ing") and lang == "en":
        ger = "Ger"

    if gender in ("Com,Neut", "Com", "ComNeut"):
        gender = ""
    if number == "Sing":
        number = ""

    pos, lemma = handler(word, lang)
    # BA2: handler + gender_str + number_str + feats(case,type) + ger + feats(apm)
    features: list[str] = []
    if gender:
        features.append(gender)
    if number:
        features.append(number)
    features += feat_list(case, type_)
    if ger:
        features.append(ger)
    features += feat_list(apm)
    return pos, lemma, features


def handler__PROPN(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    pos, lemma, features = handler__NOUN(word)
    return pos.replace("noun", "propn"), lemma, features


def handler__VERB(word: Any, lang: str | None = None) -> tuple[str, str, list[str]]:
    feats = parse_feats(word)
    verbform = feats.get("VerbForm", "Inf").replace(",", "")
    aspect = feats.get("Aspect", "")
    mood = feats.get("Mood", "")
    person = str(feats.get("Person", ""))

    biyan = str(feats.get("HebBinyan", "")).lower()
    existential = str(feats.get("HebExistential", "")).lower()

    if person == "0":
        person = "4"
    number = feats.get("Number", "Sing")

    tense = feats.get("Tense", "")
    polarity = feats.get("Polarity", "")
    polite = feats.get("Polite", "")

    is_irr = False
    if lang == "en" and tense == "Past":
        from .en.irr import is_irregular

        is_irr = is_irregular(word.lemma, word.text)
    irr = "irr" if is_irr else ""

    pos, lemma = handler(word, lang)
    if "sconj" in pos:
        return pos, lemma, []
    elif word.text == "ろ":
        return pos, lemma, []
    elif "verb" not in pos and "aux" not in pos:
        if word.text == "たり":
            return pos, lemma, feat_list("Inf", "S")
        return pos, lemma, []
    # BA2: res + flag + stringify_feats(...) where flag = "-" + VerbForm
    features = [verbform] + feat_list(
        aspect,
        mood,
        tense,
        polarity,
        polite,
        biyan,
        existential,
        number[:1] + person,
        irr,
    )
    return pos, lemma, features


def handler__actual_PUNCT(
    word: Any, lang: str | None = None
) -> tuple[str, str, list[str]] | None:
    """Mid-utterance punctuation. Bare terminators (`.`/`!`/`?`) return None —
    the utterance terminator is carried separately and never a `%mor` word."""
    if word.lemma == "," or word.lemma == "$,":
        return ("cm", "cm", [])
    elif word.lemma in [".", "!", "?"]:
        return None
    elif word.text in "‡":
        return ("end", "end", [])
    elif word.text in "„":
        return ("end", "end", [])
    return None


def handler__PUNCT(
    word: Any, lang: str | None = None
) -> tuple[str, str, list[str]] | None:
    if word.lemma in [".", "!", "?", ",", "$,"]:
        return handler__actual_PUNCT(word, lang)
    elif word.text in ["„", "‡"]:
        return handler__actual_PUNCT(word, lang)
    elif word.text == "da":
        return ("noun", "da", [])
    elif word.text == "哎呀":
        return ("punct", "哎呀", [])
    elif re.match(r"^['\w-]+$", word.text):
        if word.text == "もん":
            return ("part", word.text, [])
        if word.text == ",":
            return ("cm", "cm", [])
        return ("x", word.text, [])
    return None


HANDLERS = {
    "PRON": handler__PRON,
    "DET": handler__DET,
    "ADJ": handler__ADJ,
    "NOUN": handler__NOUN,
    "PROPN": handler__PROPN,
    "AUX": handler__VERB,  # reuse verb handler for aux
    "VERB": handler__VERB,
    "PUNCT": handler__PUNCT,
    "SYM": handler__PUNCT,  # symbols are handled like punctuation
}


def handle(word: Any, lang: str | None) -> tuple[str, str, list[str]] | None:
    """Dispatch one Stanza word to its handler. Returns a structured
    `(pos, lemma, features)` triple, or None if the word produces no `%mor`
    unit (skipped / bare terminator)."""
    if word.lemma in [".", "!", "?", ",", "$,"]:
        return handler__actual_PUNCT(word, lang)

    h = HANDLERS.get(word.upos, handler)
    result = h(word, lang)
    if h is handler:
        # the generic base returns (pos, lemma); promote to a feature-less unit
        pos, lemma = result  # type: ignore[misc]
        return (pos, lemma, [])
    return result  # type: ignore[return-value]


# --- sentence assembler (BA2 ud.py:343-579) -------------------------------


def parse_sentence(
    sentence: Any,
    delimiter: str = ".",
    special_forms: list | None = None,
    lang: str = "$nospecial$",
) -> SentenceAnalysis:
    """Render a Stanza sentence into a structured [`SentenceAnalysis`].

    Faithful port of BA2 ``parse_sentence``, but structured: clitic/auxiliary/
    MWT joining is expressed as word grouping (which units merge into one
    main-tier word) rather than ``~``/``$`` string concatenation, and the
    ``%gra`` dependency triples are carried as integers/labels. ``delimiter`` is
    accepted for signature parity but the terminator *kind* is applied
    downstream from the typed AST; only the terminator's `%gra` triple is
    returned here.
    """
    if special_forms is None:
        special_forms = []

    # Per Stanza-word (chunk) parallel arrays, mirroring BA2's `mor`.
    analyses: list[tuple[str, str, list[str]] | None] = []
    surfaces: list[str] = []
    gra_tmp: list[tuple[int, int, str]] = []   # (index, raw_head, deprel)
    actual_indicies: list[int] = []
    num_skipped = 0
    root = 0

    mwts: list[list[int]] = []
    clitics: list[int] = []
    auxiliaries: list[int] = []

    # get mwts / clitics / auxiliaries (BA2 ud.py:369-411)
    for indx, token in enumerate(sentence.tokens):
        if token.text[0] == "-":
            auxiliaries.append(token.id[0] - 1)

        if len(token.id) > 1:
            mwts.append(list(token.id))

        if token.text.strip()[0] == "_":
            auxiliaries.append(token.id[0] - 1)
            if lang == "fr" and token.text.strip() == "_l'":
                auxiliaries.append(token.id[0])
        elif token.text.strip()[0] == "~":
            auxiliaries.append(token.id[0] - 1)
        elif lang == "it" and token.text.strip()[-3:] == "ll'":
            auxiliaries.append(token.id[-1])
        elif lang == "it" and token.text.strip() == "gliel'":
            auxiliaries.append(token.id[-1])
        elif lang == "it" and token.text.strip() == "d'":
            auxiliaries.append(token.id[-1])
        elif lang == "it" and (token.text.strip() == "c’" or token.text.strip() == "c'"):
            auxiliaries.append(token.id[-1])
        elif lang == "it" and token.text.strip() == "qual'":
            auxiliaries.append(token.id[-1])
        elif lang == "fr" and token.text.strip() == "jusqu'":
            auxiliaries.append(token.id[-1])
        elif lang == "fr" and token.text.strip() == "puisqu'":
            auxiliaries.append(token.id[-1])
        elif lang == "fr" and token.text.strip() == "quelqu'":
            auxiliaries.append(token.id[-1])
        elif lang == "fr" and token.text.strip() == "aujourd":
            auxiliaries.append(token.id[0] + 1)
        elif lang == "fr" and token.text.strip() == "aujourd'":
            auxiliaries.append(token.id[-1])
        elif (
            lang == "fr"
            and token.text.strip() == "au"
            and type(token.id) == tuple
            and indx != 0
            and sentence.tokens[indx - 1].text != "jusqu'"
        ):
            auxiliaries.append(token.id[0])
        elif lang == "fr" and len(token.text.strip()) == 2 and token.text.strip()[-1] == "'":
            auxiliaries.append(token.id[-1])

    special_forms = special_forms.copy()
    special_form_ids: list[Any] = []

    for indx, word in enumerate(sentence.words):
        unit = handle(word, lang)
        text = getattr(word, "text", "")
        if text.strip() == "0":
            # Zero/omitted word — BA2 prefixes "0" onto the following word.
            # Rare; not yet modelled in the typed path. Skip as a non-chunk so
            # indices stay aligned (TODO: represent zero-words typed).
            analyses.append(None)
            surfaces.append(text)
            num_skipped += 1
            actual_indicies.append(root)
        elif unit is not None or text.strip() in ["xbxxx", "‡", "„"]:
            if text.strip() == "‡":
                unit = ("cm", "begin", [])
            elif text.strip() == "„":
                unit = ("cm", "end", [])

            if "xbxxx" in text.strip():
                form = special_forms.pop(0)
                if form[1][0] == "s":
                    unit = ("L2", "xxx", [])
                else:
                    unit = (form[1].strip(), form[0].strip().replace(",", "cm"), [])
                special_form_ids.append(word.id)

            analyses.append(unit)
            surfaces.append(text)

            deprel = word.deprel.upper().replace(":", "-").replace("<", "").replace(">", "")
            gra_tmp.append(((indx + 1) - num_skipped, word.head, deprel))
            actual_indicies.append((indx + 1) - num_skipped)
            if word.deprel.upper() == "ROOT":
                root = (indx + 1) - num_skipped
        else:
            analyses.append(None)
            surfaces.append(text)
            num_skipped += 1
            actual_indicies.append(root)

    # Resolve each chunk's %gra triple (BA2 ud.py:450-455). The ROOT-head wrap
    # (`actual_indicies[head-1]` with head==0 → actual_indicies[-1]) is
    # preserved verbatim.
    chunk_gra: dict[int, tuple[int, int, str]] = {}
    for elem in gra_tmp:
        index, raw_head, deprel = elem
        if index in special_form_ids:
            deprel = "FLAT"
        head = actual_indicies[raw_head - 1]
        chunk_gra[index] = (index, head, deprel)

    terminator = (len(sentence.words) + 1 - num_skipped, root, "PUNCT")

    # Word grouping: each surviving chunk starts as its own word; clitic ($),
    # auxiliary (~), and MWT (~) joins merge groups. Mirrors BA2's mor_clone
    # rewrites (ud.py:457-491) on index lists instead of strings.
    groups: list[list[int] | None] = [
        [i] if analyses[i] is not None else None for i in range(len(analyses))
    ]

    while clitics:
        clitic = clitics.pop()
        try:
            if groups[clitic - 1] is not None and groups[clitic] is not None:
                groups[clitic - 1] = groups[clitic - 1] + groups[clitic]  # type: ignore[operator]
        except IndexError:
            pass
        groups[clitic] = None

    for aux in auxiliaries:
        orig_aux = aux
        while groups[aux - 1] is None:
            aux -= 1
        if groups[orig_aux] is not None and groups[aux - 1] is not None:
            groups[aux - 1] = groups[aux - 1] + groups[orig_aux]  # type: ignore[operator]
            groups[orig_aux] = None

    while mwts:
        mwt = mwts.pop(0)
        start, end = mwt[0], mwt[-1]
        merged: list[int] = []
        for j in range(start - 1, end):
            if groups[j] is not None:
                merged += groups[j]  # type: ignore[operator]
        for j in range(start, end + 1):
            groups[j - 1] = None
        groups[start - 1] = merged

    # Materialize the structured words, attaching each unit's %gra triple.
    words: list[MorWordGroup] = []
    for group in groups:
        if not group:
            continue
        units: list[MorUnit] = []
        texts: list[str] = []
        for chunk_pos in group:
            unit = analyses[chunk_pos]
            if unit is None:
                continue
            pos, lemma, features = unit
            index = actual_indicies[chunk_pos]
            _, head, deprel = chunk_gra.get(index, (index, root, "DEP"))
            units.append(MorUnit(pos, lemma, list(features), index, head, deprel))
            texts.append(surfaces[chunk_pos])
        if units:
            words.append(MorWordGroup("".join(texts), units))

    if not words:
        return SentenceAnalysis([], None)

    return SentenceAnalysis(words, terminator)


def clean_sentence(sent: str) -> str:
    """Strip ``+,``/``++``/``+"`` markers before tokenization (BA2 ud.py:581)."""
    remove = ["+,", "++", '+"']
    for i in remove:
        sent = sent.replace(i, "")
    return sent


__all__ = [
    "MorUnit",
    "MorWordGroup",
    "SentenceAnalysis",
    "parse_feats",
    "feat_list",
    "handle",
    "parse_sentence",
    "clean_sentence",
]
