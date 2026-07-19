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


@dataclass(frozen=True)
class AnalysisAnomaly:
    """One repaired Stanza field, retained for operator-visible logging."""

    word_index: int
    text: str
    field: str
    original: Any
    replacement: Any
    reason: str


@dataclass
class SentenceAnalysis:
    """Structured analysis of one utterance: the word groups plus the trailing
    terminator `%gra` relation. ``words`` empty means "no tiers" (BA2 emits
    nothing for degenerate analyses)."""

    words: list[MorWordGroup]
    terminator: tuple[int, int, str] | None  # (index, head, deprel)
    anomalies: list[AnalysisAnomaly] = field(default_factory=list)


@dataclass
class _NormalizedWord:
    """Renderer-owned copy of the Stanza fields consumed below."""

    text: str
    lemma: str
    upos: str
    feats: str | None
    head: int
    deprel: str
    id: int


@dataclass
class _NormalizedToken:
    text: str
    id: list[int]


_UD_POS = {
    "ADJ",
    "ADP",
    "ADV",
    "AUX",
    "CCONJ",
    "DET",
    "INTJ",
    "NOUN",
    "NUM",
    "PART",
    "PRON",
    "PROPN",
    "PUNCT",
    "SCONJ",
    "SYM",
    "VERB",
    "X",
}


def _anomaly(
    sink: list[AnalysisAnomaly],
    index: int,
    text: str,
    field_name: str,
    original: Any,
    replacement: Any,
    reason: str,
) -> None:
    sink.append(AnalysisAnomaly(index, text, field_name, original, replacement, reason))


def _normalize_words(
    sentence: Any,
) -> tuple[list[_NormalizedWord], list[AnalysisAnomaly]]:
    """Repair missing/invalid Stanza fields without losing lexical surfaces."""
    raw_words = list(getattr(sentence, "words", []) or [])
    anomalies: list[AnalysisAnomaly] = []
    root_id = next(
        (
            index
            for index, word in enumerate(raw_words, start=1)
            if getattr(word, "head", None) == 0
            and str(getattr(word, "deprel", "")).lower() == "root"
        ),
        1 if raw_words else 0,
    )
    normalized: list[_NormalizedWord] = []

    for index, word in enumerate(raw_words, start=1):
        raw_text = getattr(word, "text", None)
        text = raw_text if isinstance(raw_text, str) else ""
        if not text:
            _anomaly(
                anomalies, index, text, "text", raw_text, "", "missing lexical surface"
            )

        raw_lemma = getattr(word, "lemma", None)
        lemma = raw_lemma if isinstance(raw_lemma, str) else ""
        missing_lemma = not lemma
        bogus_lemma = (
            bool(text)
            and any(char.isalnum() for char in text)
            and not any(char.isalnum() for char in lemma)
        )
        if missing_lemma or bogus_lemma:
            _anomaly(
                anomalies,
                index,
                text,
                "lemma",
                raw_lemma,
                text,
                "missing lemma or punctuation-only lemma for a lexical surface",
            )
            lemma = text

        raw_upos = getattr(word, "upos", None)
        upos = raw_upos.upper() if isinstance(raw_upos, str) else ""
        if upos not in _UD_POS:
            _anomaly(
                anomalies,
                index,
                text,
                "upos",
                raw_upos,
                "X",
                "missing or unknown universal POS",
            )
            upos = "X"

        raw_head = getattr(word, "head", None)
        head_valid = (
            isinstance(raw_head, int)
            and not isinstance(raw_head, bool)
            and 0 <= raw_head <= len(raw_words)
        )
        if head_valid:
            head = raw_head
        else:
            head = 0 if index == root_id else root_id
            _anomaly(
                anomalies,
                index,
                text,
                "head",
                raw_head,
                head,
                "missing or out-of-range dependency head",
            )

        raw_deprel = getattr(word, "deprel", None)
        deprel = raw_deprel if isinstance(raw_deprel, str) else ""
        deprel_invalid = not deprel or (deprel.startswith("<") and deprel.endswith(">"))
        expected_root = head == 0
        joint_invariant_invalid = (deprel.lower() == "root") != expected_root
        if deprel_invalid or joint_invariant_invalid:
            replacement = "root" if expected_root else "dep"
            _anomaly(
                anomalies,
                index,
                text,
                "deprel",
                raw_deprel,
                replacement,
                "missing/sentinel relation or head/root invariant violation",
            )
            deprel = replacement

        raw_id = getattr(word, "id", None)
        word_id = (
            raw_id
            if isinstance(raw_id, int) and not isinstance(raw_id, bool)
            else index
        )
        if word_id != raw_id:
            _anomaly(
                anomalies,
                index,
                text,
                "id",
                raw_id,
                word_id,
                "missing or non-scalar word id",
            )

        raw_feats = getattr(word, "feats", None)
        feats = raw_feats if isinstance(raw_feats, str) else None
        normalized.append(
            _NormalizedWord(text, lemma, upos, feats, head, deprel, word_id)
        )

    return normalized, anomalies


def _apply_italian_compound_imperatives(
    words: list[_NormalizedWord],
    tokens: list[Any],
    anomalies: list[AnalysisAnomaly],
) -> tuple[list[_NormalizedWord], list[Any]]:
    """Expand confirmed single-token Stanza defects into verb+clitic MWTs."""
    from .it.workarounds import rule_for

    # These rules target Stanza's missing-MWT shape. If this sentence already
    # contains an MWT or token/word cardinality differs, leave it untouched.
    if len(tokens) != len(words):
        return words, tokens
    token_ids: list[int] = []
    for token in tokens:
        ids = list(getattr(token, "id", []) or [])
        if len(ids) != 1 or not isinstance(ids[0], int):
            return words, tokens
        token_ids.append(ids[0])
    if token_ids != list(range(1, len(words) + 1)):
        return words, tokens

    rules = [rule_for(word.text, word.upos) for word in words]
    if not any(rules):
        return words, tokens

    old_to_new: dict[int, int] = {}
    next_id = 1
    for old_id, rule in enumerate(rules, start=1):
        old_to_new[old_id] = next_id
        next_id += 1 + (len(rule.clitics) if rule is not None else 0)

    expanded_words: list[_NormalizedWord] = []
    expanded_tokens: list[Any] = []
    imperative_feats = "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin"

    for old_id, (word, token, rule) in enumerate(
        zip(words, tokens, rules, strict=True), start=1
    ):
        main_id = old_to_new[old_id]
        mapped_head = old_to_new.get(word.head, word.head)
        if rule is None:
            expanded_words.append(
                _NormalizedWord(
                    word.text,
                    word.lemma,
                    word.upos,
                    word.feats,
                    mapped_head,
                    word.deprel,
                    main_id,
                )
            )
            expanded_tokens.append(_NormalizedToken(token.text, [main_id]))
            continue

        expanded_words.append(
            _NormalizedWord(
                rule.stem_surface,
                rule.verb_lemma,
                "VERB",
                imperative_feats,
                mapped_head,
                word.deprel,
                main_id,
            )
        )
        span_ids = [main_id]
        for offset, clitic in enumerate(rule.clitics, start=1):
            clitic_id = main_id + offset
            span_ids.append(clitic_id)
            expanded_words.append(
                _NormalizedWord(
                    clitic.surface,
                    clitic.lemma,
                    "PRON",
                    clitic.feats,
                    main_id,
                    clitic.deprel,
                    clitic_id,
                )
            )
        expanded_tokens.append(_NormalizedToken(rule.surface, span_ids))
        _anomaly(
            anomalies,
            old_id,
            word.text,
            f"italian_defect_{rule.defect}",
            {"upos": word.upos, "lemma": word.lemma},
            {"upos": "VERB", "lemma": rule.verb_lemma, "chunks": len(span_ids)},
            rule.retire_when,
        )

    return expanded_words, expanded_tokens


def _collapse_italian_false_mwts(
    words: list[_NormalizedWord],
    tokens: list[Any],
    anomalies: list[AnalysisAnomaly],
) -> tuple[list[_NormalizedWord], list[Any]]:
    """Collapse closed Italian false MWT expansions into one lexical word."""
    from .it.workarounds import false_mwt_rule_for

    by_id = {word.id: word for word in words}
    collapses: dict[int, tuple[list[int], Any, Any]] = {}
    for token in tokens:
        ids = list(getattr(token, "id", []) or [])
        rule = false_mwt_rule_for(str(getattr(token, "text", "")))
        if (
            rule is not None
            and len(ids) > 1
            and all(isinstance(word_id, int) and word_id in by_id for word_id in ids)
        ):
            collapses[ids[0]] = (ids, token, rule)
    if not collapses:
        return words, tokens

    old_to_new: dict[int, int] = {}
    next_id = 1
    for word in words:
        containing = next(
            (ids for ids, _token, _rule in collapses.values() if word.id in ids),
            None,
        )
        if containing is not None:
            if containing[0] not in old_to_new:
                for old_id in containing:
                    old_to_new[old_id] = next_id
                next_id += 1
        else:
            old_to_new[word.id] = next_id
            next_id += 1

    collapsed_ids = {
        word_id for ids, _token, _rule in collapses.values() for word_id in ids
    }
    rewritten_words: list[_NormalizedWord] = []
    for word in words:
        collapse = collapses.get(word.id)
        if collapse is not None:
            ids, token, rule = collapse
            sample = by_id[ids[0]]
            rewritten_words.append(
                _NormalizedWord(
                    str(token.text),
                    rule.lemma,
                    rule.upos,
                    rule.feats,
                    old_to_new.get(sample.head, sample.head),
                    sample.deprel,
                    old_to_new[sample.id],
                )
            )
            _anomaly(
                anomalies,
                sample.id,
                str(token.text),
                f"italian_defect_{rule.defect}",
                {"chunks": len(ids)},
                {"upos": rule.upos, "lemma": rule.lemma, "chunks": 1},
                rule.retire_when,
            )
        elif word.id not in collapsed_ids:
            rewritten_words.append(
                _NormalizedWord(
                    word.text,
                    word.lemma,
                    word.upos,
                    word.feats,
                    old_to_new.get(word.head, word.head),
                    word.deprel,
                    old_to_new[word.id],
                )
            )

    rewritten_tokens: list[Any] = []
    for token in tokens:
        ids = list(getattr(token, "id", []) or [])
        if ids and ids[0] in collapses:
            rewritten_tokens.append(_NormalizedToken(token.text, [old_to_new[ids[0]]]))
        else:
            mapped = list(
                dict.fromkeys(old_to_new.get(word_id, word_id) for word_id in ids)
            )
            rewritten_tokens.append(_NormalizedToken(token.text, mapped))
    return rewritten_words, rewritten_tokens


def _apply_english_copula_progressive(
    words: list[_NormalizedWord],
    tokens: list[Any],
    anomalies: list[AnalysisAnomaly],
) -> list[_NormalizedWord]:
    """Rescue Stanza's possessive reading of ``<subject>'s <verb-ing>``.

    Stanza can analyze a contracted copula as possessive ``PART/case`` and the
    progressive verb as a noun.  Keep this deliberately narrow: the sentence
    must have an MWT, no finite verb, one possessive ``'s``, and exactly one
    ``-ing`` noun.  This mirrors the fork's grammatical invariant and repairs
    dependencies together with morphology so ``%mor`` and ``%gra`` agree.
    """
    if any(
        word.feats is not None and "VerbForm=Fin" in word.feats
        for word in words
    ):
        return words
    if not any(len(list(getattr(token, "id", []) or [])) > 1 for token in tokens):
        return words

    parts = [
        word
        for word in words
        if word.upos == "PART"
        and word.lemma == "'s"
        and word.deprel.lower() == "case"
    ]
    ing_nouns = [
        word
        for word in words
        if word.upos == "NOUN"
        and len(word.text) >= 4
        and word.text.lower().endswith("ing")
    ]
    if len(parts) != 1 or len(ing_nouns) != 1:
        return words

    part = parts[0]
    verb = ing_nouns[0]
    by_id = {word.id: word for word in words}
    possessor = by_id.get(part.head)
    if (
        possessor is None
        or possessor.upos not in {"NOUN", "PROPN"}
        or possessor.deprel.lower() != "nmod:poss"
    ):
        return words
    roots = [
        word for word in words if word.head == 0 and word.deprel.lower() == "root"
    ]
    if len(roots) != 1:
        return words
    old_root = roots[0]

    rewritten: list[_NormalizedWord] = []
    for word in words:
        replacement = word
        if word.id == part.id:
            replacement = _NormalizedWord(
                word.text,
                "be",
                "AUX",
                "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin",
                verb.id,
                "aux",
                word.id,
            )
        elif word.id == possessor.id:
            replacement = _NormalizedWord(
                word.text,
                word.lemma,
                word.upos,
                word.feats,
                verb.id,
                "nsubj",
                word.id,
            )
        elif word.id == verb.id:
            replacement = _NormalizedWord(
                word.text,
                word.lemma,
                "VERB",
                "Tense=Pres|VerbForm=Part",
                0,
                "root",
                word.id,
            )
        elif word.id == old_root.id and old_root.id != verb.id:
            replacement = _NormalizedWord(
                word.text,
                word.lemma,
                word.upos,
                word.feats,
                verb.id,
                "obj",
                word.id,
            )
        elif (
            old_root.id != verb.id
            and word.head == old_root.id
            and word.deprel.lower() in {"cc", "punct", "discourse", "mark"}
        ):
            replacement = _NormalizedWord(
                word.text,
                word.lemma,
                word.upos,
                word.feats,
                verb.id,
                word.deprel,
                word.id,
            )
        rewritten.append(replacement)

    _anomaly(
        anomalies,
        part.id,
        part.text,
        "english_copula_progressive",
        {"upos": part.upos, "lemma": part.lemma, "deprel": part.deprel},
        {"upos": "AUX", "lemma": "be", "deprel": "aux"},
        "possessive-gerund analysis violates the finite-clause invariant",
    )
    return rewritten


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
    return (
        pos,
        lemma,
        feat_list(
            feats.get("PronType", "Int"),
            case.replace(",", ""),
            reflex,
            number_string,
        ),
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

    normalized_words, anomalies = _normalize_words(sentence)
    sentence_tokens = list(getattr(sentence, "tokens", []) or [])
    if lang == "it":
        normalized_words, sentence_tokens = _apply_italian_compound_imperatives(
            normalized_words, sentence_tokens, anomalies
        )
        normalized_words, sentence_tokens = _collapse_italian_false_mwts(
            normalized_words, sentence_tokens, anomalies
        )
    elif lang == "en":
        normalized_words = _apply_english_copula_progressive(
            normalized_words, sentence_tokens, anomalies
        )

    # Per Stanza-word (chunk) parallel arrays, mirroring BA2's `mor`.
    analyses: list[tuple[str, str, list[str]] | None] = []
    surfaces: list[str] = []
    gra_tmp: list[tuple[int, int, str]] = []  # (index, raw_head, deprel)
    actual_indicies: list[int] = []
    num_skipped = 0
    root = 0

    mwts: list[list[int]] = []
    clitics: list[int] = []
    auxiliaries: list[int] = []

    # get mwts / clitics / auxiliaries (BA2 ud.py:369-411)
    for indx, token in enumerate(sentence_tokens):
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
        elif lang == "it" and (
            token.text.strip() == "c’" or token.text.strip() == "c'"
        ):
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
            and sentence_tokens[indx - 1].text != "jusqu'"
        ):
            auxiliaries.append(token.id[0])
        elif (
            lang == "fr"
            and len(token.text.strip()) == 2
            and token.text.strip()[-1] == "'"
        ):
            auxiliaries.append(token.id[-1])

    special_forms = special_forms.copy()
    special_form_ids: list[Any] = []

    for indx, word in enumerate(normalized_words):
        if not word.text:
            analyses.append(None)
            surfaces.append("")
            num_skipped += 1
            actual_indicies.append(root)
            continue
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

            deprel = (
                word.deprel.upper().replace(":", "-").replace("<", "").replace(">", "")
            )
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

    terminator = (len(normalized_words) + 1 - num_skipped, root, "PUNCT")

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
        return SentenceAnalysis([], None, anomalies)

    return SentenceAnalysis(words, terminator, anomalies)


def clean_sentence(sent: str) -> str:
    """Strip ``+,``/``++``/``+"`` markers before tokenization (BA2 ud.py:581)."""
    remove = ["+,", "++", '+"']
    for i in remove:
        sent = sent.replace(i, "")
    return sent


__all__ = [
    "MorUnit",
    "MorWordGroup",
    "AnalysisAnomaly",
    "SentenceAnalysis",
    "parse_feats",
    "feat_list",
    "handle",
    "parse_sentence",
    "clean_sentence",
]
