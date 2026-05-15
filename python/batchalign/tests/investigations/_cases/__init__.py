"""Per-language probe case registry.

The :data:`LANGUAGE_MATRIX` maps each supported language to its tuple
of :class:`~.._probe_types.ProbeCase` instances. Adding a new language
means creating a new ``_cases/<lang>.py`` module and appending an entry
to this dict.

Language codes are kept as ``(alpha2, alpha3)`` pairs so the matrix
harness can feed both Stanza (which wants alpha2) and the per-language
book chapters (which use alpha3 conventions from
``reference/languages/overview.md``).
"""

from __future__ import annotations

from dataclasses import dataclass

from . import (
    arabic,
    catalan,
    czech,
    danish,
    dutch,
    english,
    estonian,
    finnish,
    french,
    german,
    greek,
    hebrew,
    indonesian,
    italian,
    korean,
    norwegian,
    polish,
    portuguese,
    romanian,
    russian,
    spanish,
    swedish,
    thai,
    turkish,
    ukrainian,
    vietnamese,
)
from .._probe_types import ProbeCase


@dataclass(frozen=True)
class LanguageKey:
    """(ISO-639-1, ISO-639-3) pair identifying a probe language.

    The harness needs both: alpha2 selects the Stanza pipeline and
    alpha3 labels the per-language book chapter. Tying them together
    prevents the common bug of mixing the two codes at the seams.
    """

    alpha2: str
    alpha3: str


FRA = LanguageKey(alpha2="fr", alpha3="fra")
ITA = LanguageKey(alpha2="it", alpha3="ita")
POR = LanguageKey(alpha2="pt", alpha3="por")
NLD = LanguageKey(alpha2="nl", alpha3="nld")
SPA = LanguageKey(alpha2="es", alpha3="spa")
DEU = LanguageKey(alpha2="de", alpha3="deu")
# English joins the shared key registry for decision probes; there
# are no English MWT cases, so it does not appear in LANGUAGE_MATRIX.
ENG = LanguageKey(alpha2="en", alpha3="eng")

# Tier B pilot (2026-04-23): three typologies validated
# (Romance/Slavic/Semitic+RTL).
CAT = LanguageKey(alpha2="ca", alpha3="cat")
RUS = LanguageKey(alpha2="ru", alpha3="rus")
ARA = LanguageKey(alpha2="ar", alpha3="ara")

# Tier B second batch (2026-04-23 16:10).
POL = LanguageKey(alpha2="pl", alpha3="pol")
HEB = LanguageKey(alpha2="he", alpha3="heb")
ELL = LanguageKey(alpha2="el", alpha3="ell")
TUR = LanguageKey(alpha2="tr", alpha3="tur")
SWE = LanguageKey(alpha2="sv", alpha3="swe")
IND = LanguageKey(alpha2="id", alpha3="ind")

# Tier B third batch (2026-04-23 17:00): remaining Stanza-supported
# languages across Slavic, Romance, Uralic, Germanic, Koreanic,
# Tai-Kadai, and Austroasiatic typologies. Chinese (zho) is
# handled separately via hk/* engines and is deferred.
CES = LanguageKey(alpha2="cs", alpha3="ces")
UKR = LanguageKey(alpha2="uk", alpha3="ukr")
RON = LanguageKey(alpha2="ro", alpha3="ron")
FIN = LanguageKey(alpha2="fi", alpha3="fin")
EST = LanguageKey(alpha2="et", alpha3="est")
NOB = LanguageKey(alpha2="no", alpha3="nob")
DAN = LanguageKey(alpha2="da", alpha3="dan")
KOR = LanguageKey(alpha2="ko", alpha3="kor")
THA = LanguageKey(alpha2="th", alpha3="tha")
VIE = LanguageKey(alpha2="vi", alpha3="vie")


LANGUAGE_MATRIX: dict[LanguageKey, tuple[ProbeCase, ...]] = {
    FRA: french.CASES,
    ITA: italian.CASES,
    POR: portuguese.CASES,
    NLD: dutch.CASES,
    SPA: spanish.CASES,
    DEU: german.CASES,
    ENG: english.CASES,
    # Tier B pilot (Phase 3):
    CAT: catalan.CASES,
    RUS: russian.CASES,
    ARA: arabic.CASES,
    # Tier B second batch:
    POL: polish.CASES,
    HEB: hebrew.CASES,
    ELL: greek.CASES,
    TUR: turkish.CASES,
    SWE: swedish.CASES,
    IND: indonesian.CASES,
    # Tier B third batch:
    CES: czech.CASES,
    UKR: ukrainian.CASES,
    RON: romanian.CASES,
    FIN: finnish.CASES,
    EST: estonian.CASES,
    NOB: norwegian.CASES,
    DAN: danish.CASES,
    KOR: korean.CASES,
    THA: thai.CASES,
    VIE: vietnamese.CASES,
}


def all_cases() -> list[tuple[LanguageKey, ProbeCase]]:
    """Flatten the matrix for parametrize, preserving language order."""
    return [(lang, case) for lang, cases in LANGUAGE_MATRIX.items() for case in cases]
