"""Error-string → hint lookup for `fail` rows.

A separate module so the copy lives in one place. `Interface` calls
`hint_for(error)` when rendering a fail row and prints the returned
string as an indented `hint:` line below the failure.

Entries are matched by case-insensitive substring on the full error
text. First match wins; order entries from specific to general.
"""

from __future__ import annotations


# Ordered list of (substring, hint). Specific → general.
_HINTS: list[tuple[str, str]] = [
    ("cuda out of memory", "try --device cpu, or a smaller --model"),
    ("cuda oom",            "try --device cpu, or a smaller --model"),
    ("out of memory",       "try --device cpu, or a smaller --model"),
    ("no module named 'stanza'",
        "install with: pip install 'batchalign[stanza]'"),
    ("no module named 'transformers'",
        "install with: pip install 'batchalign[whisper]'"),
    ("no module named 'pyannote'",
        "install with: pip install 'batchalign[pyannote]'"),
    ("no module named 'googletrans'",
        "install with: pip install 'batchalign[translate]'"),
    ("no model for lang",
        "Stanza model missing; download with `stanza.download(lang=...)`"),
    ("api key", "set the API key via env var or ~/.batchalign.ini"),
    ("no such file or directory", "verify the path and re-run"),
    # CHAT validation diagnostics — every Exxx code from the model gets a
    # documentation URL automatically; surface it inline so the user can
    # jump straight to the explanation.
    ("unsupported @options",
        "valid @Options values are 'CA' and 'NoAlign' (CHAT manual §Options)"),
]


def hint_for(error: str | None) -> str | None:
    """Return the first matching hint for `error`, or None."""
    if not error:
        return None
    needle = error.lower()
    for sub, hint in _HINTS:
        if sub in needle:
            return hint
    return None


__all__ = ["hint_for"]
