"""Post-ASR text cleanup shared by the utterance-segmentation backends.

BA2's transcribe pairing applies, after ASR, a disfluency stage and a retrace
stage before utterance segmentation. We fold the equivalent text transforms in
here (applied per segmented utterance, since BA3 segments first):

- `apply_disfluency`  — filled-pause / replacement word-lists (`uh` → `&-uh`),
  BA2 `cleanup/disfluencies.py` + `support/{filled_pauses,replacements}.<lang>`.
- `mark_retraces`     — consecutive n-gram repeats get the earlier occurrence
  marked `[/]` (`los los` → `los [/] los`), BA2 `cleanup/retrace.py`.

Both are deterministic and unit-tested with no models.
"""

from __future__ import annotations

import functools
import pathlib

# CHATUtterance/transcribe language → BA2 support-file suffix.
SUPPORT_SUFFIX = {"eng": "eng", "en": "eng", "zho": "zho", "zh": "zho", "zh-hans": "zho"}

_CJK = {"yue", "zho", "zh", "zh-hant", "zh-hans", "cmn"}


@functools.lru_cache(maxsize=8)
def load_cleanup(suffix: str) -> dict[str, str]:
    """`{original_lower: main_line_form}` from BA2's filled-pause + replacement
    word-lists (`uh` → `&-uh`, `cuz` → `(be)cause`). Empty if none ship."""
    out: dict[str, str] = {}
    base = pathlib.Path(__file__).parent / "support"
    for name in (f"filled_pauses.{suffix}", f"replacements.{suffix}"):
        path = base / name
        if not path.is_file():
            continue
        for line in path.read_text().splitlines():
            line = line.strip()
            if not line or line.startswith("#"):
                continue
            parts = line.split(" ")
            if len(parts) >= 2:
                out[parts[0].lower()] = parts[1]
    return out


def apply_disfluency(sentence: str, table: dict[str, str]) -> str:
    """Word-level filled-pause/replacement substitution (BA2 `_mark_utterance`)."""
    if not table:
        return sentence
    return " ".join(table.get(w.lower(), w) for w in sentence.split(" ") if w != "")


def mark_retraces(sentence: str, lang: str) -> str:
    """Mark consecutive repeated n-grams with `[/]` (BA2 `NgramRetraceEngine`).

    The earlier occurrence(s) of a run that repeats immediately are the
    retrace: `los los` → `los [/] los`; a multi-word repeat is wrapped:
    `a b a b` → `<a b> [/] a b`. CJK starts at bigrams (BA2 skips unigram
    retraces for zh/yue). Faithful for single-repeat cases; rare multi-repeat
    chains may group differently from BA2's serializer.
    """
    content = [w for w in sentence.split(" ") if w != ""]
    n_tokens = len(content)
    if n_tokens < 2:
        return sentence
    is_retrace = [False] * n_tokens
    start_n = 2 if lang in _CJK else 1
    for n in range(start_n, n_tokens):
        begin = 0
        while begin < n_tokens - n:
            gram = content[begin : begin + n]
            root = begin
            while content[root + n : root + 2 * n] == gram:
                for j in range(begin, begin + n):
                    is_retrace[j] = True
                root += n
            begin += 1

    out: list[str] = []
    i = 0
    while i < n_tokens:
        if is_retrace[i]:
            j = i
            while j < n_tokens and is_retrace[j]:
                j += 1
            group = content[i:j]
            out.append("<" + " ".join(group) + ">" if len(group) > 1 else group[0])
            out.append("[/]")
            i = j
        else:
            out.append(content[i])
            i += 1
    return " ".join(out)


def clean_utterance(sentence: str, table: dict[str, str], lang: str) -> str:
    """Apply disfluency then retrace marking (BA2 order: disfluency → retrace)."""
    return mark_retraces(apply_disfluency(sentence, table), lang)


__all__ = ["SUPPORT_SUFFIX", "load_cleanup", "apply_disfluency", "mark_retraces", "clean_utterance"]
