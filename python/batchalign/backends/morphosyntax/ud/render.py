"""UD → CHAT `%mor` / `%gra` rendering — a faithful port of BA2.

This module is a deliberate line-by-line port of Batchalign2's
`batchalign/pipelines/morphosyntax/ud.py` handler layer (the `handler*`
functions, `HANDLERS`, `handle`, and `parse_sentence`). It takes a Stanza
``sentence`` object (with ``.words`` / ``.tokens`` carrying ``.text``,
``.lemma``, ``.upos``, ``.feats``, ``.head``, ``.deprel``, ``.id``) and emits
the exact ``%mor`` / ``%gra`` tier strings BA2 produced — lowercase CHAT POS
(``pron``/``verb``/``det`` …), per-POS ordered + combined features
(``S1``/``S3`` person+number), cleaned lemmas, and dependency triples with
clitic (``$``) / auxiliary+MWT (``~``) joining.

Parity over elegance: the code intentionally mirrors BA2's quirks (the
``door zogen`` fix, the ``~part|s verb`` post-substitution upstream, the
janky ``$ZERO$`` skip marker) so the rendered tiers are byte-identical.
Do not "clean this up" without a parity test proving the output is unchanged.

Source of truth: ``batchalign2/batchalign/pipelines/morphosyntax/ud.py``
lines 44–596. Language-specific helpers (``en/irr.py``, ``fr/case.py``,
``fr/apm.py``, ``ja/verbforms.py``) are copied verbatim into sibling
subpackages.
"""

from __future__ import annotations

import re
from typing import Any


# --- feature helpers (BA2 ud.py:44-54) ------------------------------------


def parse_feats(word: Any) -> dict[str, str]:
    """Parse a Stanza ``feats`` string into a ``{Key: Value}`` dict."""
    try:
        return {i.split("=")[0]: i.split("=")[1] for i in word.feats.split("|")}
    except AttributeError:
        return {}


def stringify_feats(*feats: str) -> str:
    """Join non-empty feature values with leading/inner dashes (BA2 form)."""
    template = ("-" + "-".join(filter(lambda x: x != "", feats))).strip()

    if template == "-":
        return ""
    return template.replace(",", "")


# --- POS handlers (BA2 ud.py:60-340) --------------------------------------


def handler(word: Any, lang: str | None = None) -> str:
    """The generic handler: clean the lemma and emit ``pos|lemma``."""
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
    return f"{'' if not unknown else '0'}{pos}|{target}"


def handler__PRON(word: Any, lang: str | None = None) -> str:
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

    return handler(word, lang) + stringify_feats(
        feats.get("PronType", "Int"),
        case.replace(",", ""),
        reflex,
        number_string,
    )


def handler__DET(word: Any, lang: str | None = None) -> str:
    try:
        feats = parse_feats(word)
    except AttributeError:
        return handler(word)

    number = feats.get("Number", "")
    gender_str = "-" + feats.get(
        "Gender", "" if lang != "fr" else ("" if number == "Plur" else "Masc")
    ).replace(",", "")

    number_psor = feats.get("Number[psor]", "")[:1]
    person_psor = feats.get("Person[psor]", "")
    psor = number_psor + person_psor

    if gender_str == "-Com,Neut" or gender_str == "-Com" or gender_str == "-":
        gender_str = ""

    return (
        handler(word, lang)
        + gender_str
        + "-"
        + feats.get("Definite", "Def")
        + stringify_feats(feats.get("PronType", ""), number, psor)
    )


def handler__ADJ(word: Any, lang: str | None = None) -> str:
    feats = parse_feats(word)
    deg = feats.get("Degree", "Pos")
    case = feats.get("Case", "").replace(",", "")
    number = feats.get("Number", "S")[0]
    person = str(feats.get("Person", 1))
    if person == "0":
        person = "4"

    if deg == "Pos":
        deg = ""

    return handler(word, lang) + stringify_feats(deg, case, number[:1] + person)


def handler__NOUN(word: Any, lang: str | None = None) -> str:
    feats = parse_feats(word)

    gender_str = "-" + feats.get("Gender", "ComNeut").replace(",", "")
    number_str = "-" + feats.get("Number", "Sing")
    case = feats.get("Case", "").replace(",", "")
    type_ = feats.get("PronType", "")

    apm = ""
    if lang == "fr" and number_str == "-Plur":
        from .fr.apm import is_apm_noun

        apm = "Apm" if is_apm_noun(word.text) else ""

    if word.deprel == "obj" and case.strip() == "":
        case = "Acc"

    ger = ""
    if word.text.endswith("ing") and lang == "en":
        ger += "-Ger"

    if gender_str == "-Com,Neut" or gender_str == "-Com" or gender_str == "-ComNeut":
        gender_str = ""
    if number_str == "-Sing":
        number_str = ""

    return (
        handler(word, lang)
        + gender_str
        + number_str
        + stringify_feats(case, type_)
        + ger
        + stringify_feats(apm)
    )


def handler__PROPN(word: Any, lang: str | None = None) -> str:
    parsed = handler__NOUN(word)
    return parsed.replace("noun", "propn")


def handler__VERB(word: Any, lang: str | None = None) -> str:
    feats = parse_feats(word)
    flag = ""
    flag += "-" + feats.get("VerbForm", "Inf").replace(",", "")
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

    res = handler(word, lang)
    if "sconj" in res:
        return res
    elif word.text == "ろ":
        return res
    elif "verb" not in res and "aux" not in res:
        if word.text == "たり":
            return res + stringify_feats("Inf", "S")
        else:
            return res
    else:
        return res + flag + stringify_feats(
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


def handler__actual_PUNCT(word: Any, lang: str | None = None) -> str | None:
    if word.lemma == "," or word.lemma == "$,":
        return "cm|cm"
    elif word.lemma in [".", "!", "?"]:
        return word.lemma
    elif word.text in "‡":
        return "end|end"
    elif word.text in "„":
        return "end|end"
    return None


def handler__PUNCT(word: Any, lang: str | None = None) -> str | None:
    if word.lemma in [".", "!", "?", ",", "$,"]:
        return handler__actual_PUNCT(word, lang)
    elif word.text in ["„", "‡"]:
        return handler__actual_PUNCT(word, lang)
    elif word.text == "da":
        return "noun|da"
    elif word.text == "哎呀":
        return "punct|哎呀"
    elif re.match(r"^['\w-]+$", word.text):
        if word.text == "もん":
            return f"part|{word.text}"
        if word.text == ",":
            return "cm|cm"
        else:
            return f"x|{word.text}"
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


def handle(word: Any, lang: str | None) -> str | None:
    if word.lemma in [".", "!", "?", ",", "$,"]:
        return handler__actual_PUNCT(word, lang)

    return HANDLERS.get(word.upos, handler)(word, lang)


# --- sentence assembler (BA2 ud.py:343-579) -------------------------------


def parse_sentence(
    sentence: Any,
    delimiter: str = ".",
    special_forms: list | None = None,
    lang: str = "$nospecial$",
) -> tuple[str, str]:
    """Render a Stanza sentence into ``(%mor, %gra)`` strings.

    Faithful port of BA2 ``parse_sentence``. ``delimiter`` is the utterance
    terminator (``.``/``?``/``!``/``+//.`` …) recovered upstream; it is
    appended to ``%mor`` and pointed at ROOT in ``%gra``.
    """
    if special_forms is None:
        special_forms = []

    mor: list[str | None] = []
    gra: list[str] = []

    root = 0

    actual_indicies: list[int] = []
    num_skipped = 0

    gra_tmp: list[tuple[int, int, str]] = []

    mwts: list[list[int]] = []
    clitics: list[int] = []
    auxiliaries: list[int] = []

    # get mwts / clitics / auxiliaries
    for indx, token in enumerate(sentence.tokens):
        if token.text[0] == "-":
            auxiliaries.append(token.id[0] - 1)

        if len(token.id) > 1:
            mwts.append(token.id)

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
    special_form_ids = []
    for indx, word in enumerate(sentence.words):
        mor_word = handle(word, lang)
        if word.text.strip() == "0":
            mor.append("$ZERO$")
            num_skipped += 1
            actual_indicies.append(root)
        elif mor_word or word.text.strip() in ["xbxxx", "‡", "„"]:
            if word.text.strip() == "‡":
                mor_word = "cm|begin"
            elif word.text.strip() == "„":
                mor_word = "cm|end"

            if "xbxxx" in word.text.strip():
                form = special_forms.pop(0)
                if form[1][0] == "s":
                    mor.append("L2|xxx")
                else:
                    mor.append(f"{form[1].strip()}|{form[0].strip().replace(',', 'cm')}")
                special_form_ids.append(word.id)
            else:
                mor.append(mor_word)

            deprel = word.deprel.upper()
            deprel = deprel.replace(":", "-")
            gra_tmp.append(
                ((indx + 1) - num_skipped, word.head, deprel.replace("<", "").replace(">", ""))
            )
            actual_indicies.append((indx + 1) - num_skipped)
            if word.deprel.upper() == "ROOT":
                root = (indx + 1) - num_skipped
        else:
            mor.append(None)
            num_skipped += 1
            actual_indicies.append(root)

    for i, elem in enumerate(gra_tmp):
        if elem[0] in special_form_ids:
            elem = (elem[0], elem[1], "FLAT")
        gra.append(f"{elem[0]}|{actual_indicies[elem[1] - 1]}|{elem[2]}")

    gra.append(f"{len(sentence.words) + 1 - num_skipped}|{root}|PUNCT")

    mor_clone = mor.copy()

    while len(clitics) > 0:
        clitic = clitics.pop()
        try:
            prev_item = mor_clone[clitic - 1]
            curr_item = mor_clone[clitic]
            if prev_item is not None and curr_item is not None:
                mor_clone[clitic - 1] = prev_item + "$" + curr_item
        except IndexError:
            pass
        mor_clone[clitic] = None

    for aux in auxiliaries:
        orig_aux = aux
        while not mor_clone[aux - 1]:
            aux -= 1

        orig_item = mor_clone[orig_aux]
        prev_item = mor_clone[aux - 1]
        if orig_item and prev_item:
            mor_clone[aux - 1] = prev_item + "~" + orig_item
            mor_clone[orig_aux] = None

    while len(mwts) > 0:
        mwt = mwts.pop(0)
        mwt_start = mwt[0]
        mwt_end = mwt[-1]

        mwt_str = "~".join([i for i in mor_clone[mwt_start - 1 : mwt_end] if i])

        for j in range(mwt_start, mwt_end + 1):
            mor_clone[j - 1] = None

        mor_clone[mwt_start - 1] = mwt_str

    mor_str = (" ".join(x for x in mor_clone if x is not None)).strip().replace(",", "")
    gra_str = (" ".join(gra)).strip()

    mor_str = mor_str.replace("$ZERO$ ", "0")

    if len(mor_str) != 1:
        mor_str = mor_str + " " + delimiter

    mor_str = mor_str.replace("<UNK>", "")
    gra_str = gra_str.replace("<UNK>", "")

    if mor_str.strip() in ["+//.", "+//?", "+//!"]:
        mor_str = ""

    if mor_str.strip() == "" or gra_str.strip() == "" or mor_str.strip() == ".":
        mor_str = ""
        gra_str = ""

    return (mor_str, gra_str)


def clean_sentence(sent: str) -> str:
    """Strip ``+,``/``++``/``+"`` markers before tokenization (BA2 ud.py:581)."""
    remove = ["+,", "++", '+"']
    for i in remove:
        sent = sent.replace(i, "")
    return sent


__all__ = [
    "parse_feats",
    "stringify_feats",
    "handle",
    "parse_sentence",
    "clean_sentence",
]
