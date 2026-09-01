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


@dataclass(frozen=True)
class ComponentRewriteRule:
    """One MWT whose first component needs a curated lexical analysis."""

    defect: int
    surface: str
    head_upos: str
    head_lemma: str
    head_feats: str
    retire_when: str


@dataclass(frozen=True)
class LegacyMwtComponent:
    """One exact component in a corpus-pinned legacy Italian MWT analysis."""

    surface: str
    upos: str
    lemma: str
    feats: str | None


@dataclass(frozen=True)
class LegacyMwtRule:
    """An Italian MWT whose Stanza analysis must retain the legacy contract."""

    defect: int
    surface: str
    components: tuple[LegacyMwtComponent, ...]
    retire_when: str


_ME = CliticRule("me", "me", "Number=Sing|Person=1|PronType=Prs", "iobj")
_CI = CliticRule("ci", "ci", "Number=Plur|Person=1|PronType=Prs", "obj")
_SI = CliticRule("si", "si", "Number=Sing|Person=3|PronType=Prs", "obj")
_LA = CliticRule("la", "la", "Gender=Fem|Number=Sing|Person=3|PronType=Prs", "obj")
_LO = CliticRule("lo", "lo", "Gender=Masc|Number=Sing|Person=3|PronType=Prs", "obj")
_LI = CliticRule("li", "li", "Gender=Masc|Number=Plur|Person=3|PronType=Prs", "obj")
_LE = CliticRule("le", "le", "Gender=Fem|Number=Plur|Person=3|PronType=Prs", "obj")

_REPROBE = (
    "retire only after a pinned Italian Stanza probe returns a correct MWT expansion"
)

_MASC_SING_ART = "Definite=Def|Gender=Masc|Number=Sing|PronType=Art"
_FEM_SING_ART = "Definite=Def|Gender=Fem|Number=Sing|PronType=Art"
_MASC_PLUR_ART = "Definite=Def|Gender=Masc|Number=Plur|PronType=Art"


def _adp(surface: str, lemma: str) -> LegacyMwtComponent:
    return LegacyMwtComponent(surface, "ADP", lemma, None)


def _det(surface: str, feats: str) -> LegacyMwtComponent:
    return LegacyMwtComponent(surface, "DET", "il", feats)


def _verb(surface: str, lemma: str, feats: str) -> LegacyMwtComponent:
    return LegacyMwtComponent(surface, "VERB", lemma, feats)


def _pron(surface: str, lemma: str, feats: str) -> LegacyMwtComponent:
    return LegacyMwtComponent(surface, "PRON", lemma, feats)


def _infinitive_clitic(
    surface: str,
    lemma: str,
    clitic: CliticRule,
    defect: int = 15,
) -> LegacyMwtRule:
    return LegacyMwtRule(
        defect,
        surface,
        (
            _verb(lemma, lemma, "VerbForm=Inf"),
            _pron(clitic.surface, clitic.lemma, clitic.feats),
        ),
        _LEGACY_REPROBE,
    )


def _imperative_clitic(
    surface: str,
    stem_surface: str,
    lemma: str,
    clitic: CliticRule,
    defect: int = 15,
) -> LegacyMwtRule:
    return LegacyMwtRule(
        defect,
        surface,
        (
            _verb(
                stem_surface,
                lemma,
                "Mood=Imp|Number=Sing|Person=2|Tense=Pres|VerbForm=Fin",
            ),
            _pron(clitic.surface, clitic.lemma, clitic.feats),
        ),
        _LEGACY_REPROBE,
    )


_LEGACY_REPROBE = (
    "retire only when the pinned Stanza model reproduces the established "
    "Italian TalkBank morphology on the real legacy fixture"
)

_ARTICULATED_PREPOSITION_RULES: tuple[LegacyMwtRule, ...] = (
    LegacyMwtRule(
        14, "nel", (_adp("in", "in"), _det("il", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "del", (_adp("di", "di"), _det("il", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "sulla", (_adp("su", "su"), _det("la", _FEM_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "dei", (_adp("di", "di"), _det("i", _MASC_PLUR_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "alla", (_adp("a", "a"), _det("la", _FEM_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14,
        "della",
        (LegacyMwtComponent("della", "DET", "il", _MASC_SING_ART),),
        _LEGACY_REPROBE,
    ),
    LegacyMwtRule(
        14, "al", (_adp("a", "a"), _det("il", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "col", (_adp("con", "con"), _det("il", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "allo", (_adp("a", "a"), _det("lo", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "sul", (_adp("su", "su"), _det("il", _MASC_SING_ART)), _LEGACY_REPROBE
    ),
    LegacyMwtRule(
        14, "ai", (_adp("a", "a"), _det("i", _MASC_PLUR_ART)), _LEGACY_REPROBE
    ),
)

_REPORTED_LEGACY_CLITIC_RULES: tuple[LegacyMwtRule, ...] = (
    _infinitive_clitic("alzarci", "alzare", _CI, defect=14),
    LegacyMwtRule(
        14,
        "eccolo",
        (
            LegacyMwtComponent("ecco", "ADV", "ecco", None),
            _pron("lo", "lo", "Gender=Masc|Number=Sing|Person=3|PronType=Prs"),
        ),
        _LEGACY_REPROBE,
    ),
    LegacyMwtRule(
        14,
        "eccola",
        (
            LegacyMwtComponent("ecco", "ADV", "ecco", None),
            _pron("la", "la", "Gender=Fem|Number=Sing|Person=3|PronType=Prs"),
        ),
        _LEGACY_REPROBE,
    ),
    _infinitive_clitic("sporcarsi", "sporcare", _SI, defect=14),
    _infinitive_clitic("vederla", "vedere", _LA, defect=14),
    _imperative_clitic("guardalo", "guarda", "guardare", _LO, defect=14),
)

# Common productive forms adjacent to the user-reported regressions. These are
# intentionally enumerated: suffix-only rewriting would mis-tag lexical
# homographs such as the noun ``fallo``.
_COMMON_INFINITIVE_CLITIC_RULES = tuple(
    _infinitive_clitic(surface, lemma, clitic)
    for surface, lemma, clitic in (
        ("alzarsi", "alzare", _SI),
        ("lavarsi", "lavare", _SI),
        ("vestirsi", "vestire", _SI),
        ("sedersi", "sedere", _SI),
        ("mettersi", "mettere", _SI),
        ("chiamarsi", "chiamare", _SI),
        ("svegliarsi", "svegliare", _SI),
        ("vederlo", "vedere", _LO),
        ("prenderlo", "prendere", _LO),
        ("prenderla", "prendere", _LA),
        ("farlo", "fare", _LO),
        ("farla", "fare", _LA),
        ("dirlo", "dire", _LO),
        ("aprirlo", "aprire", _LO),
        ("chiuderlo", "chiudere", _LO),
        ("mangiarlo", "mangiare", _LO),
        ("berlo", "bere", _LO),
        ("portarlo", "portare", _LO),
        ("portarla", "portare", _LA),
    )
)

_COMMON_IMPERATIVE_CLITIC_RULES = tuple(
    _imperative_clitic(surface, stem, lemma, clitic)
    for surface, stem, lemma, clitic in (
        ("guardala", "guarda", "guardare", _LA),
        ("guardali", "guarda", "guardare", _LI),
        ("guardale", "guarda", "guardare", _LE),
        ("mettilo", "metti", "mettere", _LO),
        ("mettila", "metti", "mettere", _LA),
        ("portalo", "porta", "portare", _LO),
        ("portala", "porta", "portare", _LA),
        ("lascialo", "lascia", "lasciare", _LO),
        ("lasciala", "lascia", "lasciare", _LA),
        ("chiudilo", "chiudi", "chiudere", _LO),
        ("chiudila", "chiudi", "chiudere", _LA),
        ("mangialo", "mangia", "mangiare", _LO),
        ("mangiala", "mangia", "mangiare", _LA),
        ("bevilo", "bevi", "bere", _LO),
        ("bevila", "bevi", "bere", _LA),
        ("leggilo", "leggi", "leggere", _LO),
    )
)

_LEGACY_MWT_RULES = (
    _ARTICULATED_PREPOSITION_RULES
    + _REPORTED_LEGACY_CLITIC_RULES
    + _COMMON_INFINITIVE_CLITIC_RULES
    + _COMMON_IMPERATIVE_CLITIC_RULES
)

_LEGACY_MWT_BY_SURFACE = {rule.surface: rule for rule in _LEGACY_MWT_RULES}

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
    FalseMwtRule(
        7,
        "la",
        "DET",
        "il",
        "Definite=Def|Gender=Fem|Number=Sing|PronType=Art",
        _REPROBE,
    ),
)

_FALSE_MWT_BY_SURFACE = {rule.surface: rule for rule in FALSE_MWT_RULES}

COMPONENT_REWRITE_RULES: tuple[ComponentRewriteRule, ...] = (
    ComponentRewriteRule(
        9,
        "dagliela",
        "VERB",
        "dare",
        "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        _REPROBE,
    ),
    ComponentRewriteRule(
        10,
        "posala",
        "VERB",
        "posare",
        "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        _REPROBE,
    ),
    ComponentRewriteRule(
        10,
        "posalo",
        "VERB",
        "posare",
        "Mood=Imp|Number=Sing|Person=2|VerbForm=Fin",
        _REPROBE,
    ),
)

_COMPONENT_REWRITE_BY_SURFACE = {rule.surface: rule for rule in COMPONENT_REWRITE_RULES}


def rule_for(surface: str, upos: str) -> CompoundImperativeRule | None:
    """Return a confirmed rule only for an observed bad POS/surface pair."""
    rule = _BY_SURFACE.get(surface.casefold())
    if rule is None or upos.upper() not in rule.observed_upos:
        return None
    return rule


def false_mwt_rule_for(surface: str) -> FalseMwtRule | None:
    """Return a closed Defect-6 collapse rule for an original token surface."""
    return _FALSE_MWT_BY_SURFACE.get(surface.casefold())


def component_rewrite_rule_for(surface: str) -> ComponentRewriteRule | None:
    """Return a closed component rewrite for an original MWT surface."""
    return _COMPONENT_REWRITE_BY_SURFACE.get(surface.casefold())


def legacy_mwt_rule_for(surface: str) -> LegacyMwtRule | None:
    """Return an exact legacy analysis only for an enumerated MWT surface."""
    return _LEGACY_MWT_BY_SURFACE.get(surface.casefold())
