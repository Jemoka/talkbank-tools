"""Qwen-family language-name mapping (ISO-639-3 → English name Qwen accepts).

Qwen3-ASR and Qwen3-ForcedAligner both want an English language *name*
("English", "Cantonese", "Spanish", …) on their `language=` kwarg.
`pycountry`'s `.name` covers most codes; two need overrides:

* `yue` → "Cantonese" — pycountry calls it "Yue Chinese"; Qwen silently
  drops the academic form to auto-detect.
* `ell` / `gre` → "Greek" — pycountry's verbose form is "Modern Greek
  (1453-)", which Qwen doesn't recognize.

Single source of truth so the ASR + FA backends don't drift.
"""

from __future__ import annotations

from batchalign.lang import LanguageCode

_QWEN_NAME_OVERRIDE: dict[str, str] = {
    "yue": "Cantonese",
    "ell": "Greek",
    "gre": "Greek",
}


def qwen_language_name(lang: LanguageCode) -> str:
    """Map a validated `LanguageCode` to the English name Qwen expects."""
    return _QWEN_NAME_OVERRIDE.get(lang.alpha_3, lang.name)


__all__ = ["qwen_language_name"]
