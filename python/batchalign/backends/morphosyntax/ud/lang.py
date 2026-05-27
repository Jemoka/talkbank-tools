"""Language-code mapping for Stanza, mirroring BA2.

BA2 (`ud.py:719-727`) maps the document's `@Languages` codes to Stanza's
alpha-2 codes: `yue` → `zh-hant`, everything else via pycountry
alpha-3 → alpha-2. We accept codes that are *already* Stanza-shaped
(`en`, `fr`, `zh-hant`) and pass them through, so the CLI's `--language en`
and a header's `eng` both resolve correctly.
"""

from __future__ import annotations


def to_stanza(code: str) -> str:
    """Return the Stanza pipeline code for a CHAT/ISO language code.

    `yue` → `zh-hant` (BA2 special case). Three-letter ISO-639-3 codes are
    converted to alpha-2 via pycountry. Codes already in Stanza form
    (`en`, `zh-hant`, …) pass through unchanged.
    """
    c = code.strip()
    if c in ("yue", "zh-hant", "zh-hans"):
        return "zh-hant" if c == "yue" else c
    # Already alpha-2 (or a Stanza compound like `zh-hant`): pass through.
    if len(c) <= 2 or "-" in c:
        return c
    try:
        import pycountry  # type: ignore[import-not-found]

        lang = pycountry.languages.get(alpha_3=c)
        if lang is not None and getattr(lang, "alpha_2", None):
            return lang.alpha_2
    except Exception:
        pass
    return c


__all__ = ["to_stanza"]
