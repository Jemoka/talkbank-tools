"""Closed, retireable workarounds for confirmed Italian Stanza defects.

Each rule names the upstream defect family and the observation that should be
re-probed when Stanza is upgraded. Unknown surfaces always pass through.
"""

from __future__ import annotations

from dataclasses import dataclass


@dataclass(frozen=True)
class CliticRule:
    surface: str
    lemma: str
    feats: str
    deprel: str


@dataclass(frozen=True)
class CompoundImperativeRule:
    defect: int
    surface: str
    stem_surface: str
    verb_lemma: str
    clitics: tuple[CliticRule, ...]
    retire_when: str
    observed_upos: frozenset[str] = frozenset({"ADJ", "NOUN", "VERB"})


@dataclass(frozen=True)
class FalseMwtRule:
    """One ordinary word Stanza incorrectly expands as an Italian MWT."""

    defect: int
    surface: str
    upos: str
    lemma: str
    feats: str
    retire_when: str


_ME = CliticRule("me", "me", "Number=Sing|Person=1|PronType=Prs", "iobj")
_LA = CliticRule("la", "la", "Gender=Fem|Number=Sing|Person=3|PronType=Prs", "obj")
_LO = CliticRule("lo", "lo", "Gender=Masc|Number=Sing|Person=3|PronType=Prs", "obj")
_LI = CliticRule("li", "li", "Gender=Masc|Number=Plur|Person=3|PronType=Prs", "obj")
_LE = CliticRule("le", "le", "Gender=Fem|Number=Plur|Person=3|PronType=Prs", "obj")

_REPROBE = (
    "retire only after a pinned Italian Stanza probe returns a correct MWT expansion"
)

COMPOUND_IMPERATIVE_RULES: tuple[CompoundImperativeRule, ...] = (
    CompoundImperativeRule(8, "dammela", "da", "dare", (_ME, _LA), _REPROBE),
    CompoundImperativeRule(8, "dammelo", "da", "dare", (_ME, _LO), _REPROBE),
    CompoundImperativeRule(8, "prendilo", "prendi", "prendere", (_LO,), _REPROBE),
    CompoundImperativeRule(8, "prendila", "prendi", "prendere", (_LA,), _REPROBE),
    CompoundImperativeRule(8, "prendili", "prendi", "prendere", (_LI,), _REPROBE),
    CompoundImperativeRule(8, "prendile", "prendi", "prendere", (_LE,), _REPROBE),
    CompoundImperativeRule(8, "aprila", "apri", "aprire", (_LA,), _REPROBE),
    CompoundImperativeRule(8, "aprili", "apri", "aprire", (_LI,), _REPROBE),
    CompoundImperativeRule(8, "finila", "fini", "finire", (_LA,), _REPROBE),
    CompoundImperativeRule(12, "aprilo", "apri", "aprire", (_LO,), _REPROBE),
    CompoundImperativeRule(13, "leggila", "leggi", "leggere", (_LA,), _REPROBE),
)

_BY_SURFACE = {rule.surface: rule for rule in COMPOUND_IMPERATIVE_RULES}

FALSE_MWT_RULES: tuple[FalseMwtRule, ...] = (
    FalseMwtRule(
        6,
        "parla",
        "VERB",
        "parlare",
        "Mood=Ind|Number=Sing|Person=3|Tense=Pres|VerbForm=Fin",
        _REPROBE,
    ),
    FalseMwtRule(
        6, "arancione", "NOUN", "arancione", "Gender=Masc|Number=Sing", _REPROBE
    ),
    FalseMwtRule(6, "piccolo", "ADJ", "piccolo", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(
        6, "gomitolo", "NOUN", "gomitolo", "Gender=Masc|Number=Sing", _REPROBE
    ),
    FalseMwtRule(6, "divano", "NOUN", "divano", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(6, "pallone", "NOUN", "pallone", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(6, "bastone", "NOUN", "bastone", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(
        6, "cappello", "NOUN", "cappello", "Gender=Masc|Number=Sing", _REPROBE
    ),
    FalseMwtRule(6, "difficile", "ADJ", "difficile", "Number=Sing", _REPROBE),
    FalseMwtRule(6, "seggiola", "NOUN", "seggiola", "Gender=Fem|Number=Sing", _REPROBE),
    FalseMwtRule(6, "piccola", "ADJ", "piccolo", "Gender=Fem|Number=Sing", _REPROBE),
    FalseMwtRule(6, "trottola", "NOUN", "trottola", "Gender=Fem|Number=Sing", _REPROBE),
    FalseMwtRule(6, "bottone", "NOUN", "bottone", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(6, "cielo", "NOUN", "cielo", "Gender=Masc|Number=Sing", _REPROBE),
    FalseMwtRule(6, "normale", "ADJ", "normale", "Number=Sing", _REPROBE),
    FalseMwtRule(
        6, "cavallone", "NOUN", "cavallone", "Gender=Masc|Number=Sing", _REPROBE
    ),
    FalseMwtRule(6, "coccole", "NOUN", "coccole", "Gender=Fem|Number=Plur", _REPROBE),
)

_FALSE_MWT_BY_SURFACE = {rule.surface: rule for rule in FALSE_MWT_RULES}


def rule_for(surface: str, upos: str) -> CompoundImperativeRule | None:
    """Return a confirmed rule only for an observed bad POS/surface pair."""
    rule = _BY_SURFACE.get(surface.casefold())
    if rule is None or upos.upper() not in rule.observed_upos:
        return None
    return rule


def false_mwt_rule_for(surface: str) -> FalseMwtRule | None:
    """Return a closed Defect-6 collapse rule for an original token surface."""
    return _FALSE_MWT_BY_SURFACE.get(surface.casefold())
