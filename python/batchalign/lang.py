"""ISO-639-3-first language resolution. Single source of truth.

Every ASR backend accepts a `LanguageCode`. The CLI validates user
input once at parse time and passes the resolved record downstream;
backends read the field they need (`alpha_3` / `alpha_2` / `name`)
instead of running their own pycountry pretzels.

Ground truth for what each engine wants is documented per backend.
Examples:
  - Rev.ai / WhisperX / vLLM / Tencent → alpha_2 (ISO-639-1)
  - HF Whisper / openai-whisper PyPI / Qwen3-ASR → English name
  - ChatWhisper / FunAudio → alpha_3 (ISO-639-3)
"""

from __future__ import annotations

from dataclasses import dataclass

import pycountry


@dataclass(frozen=True)
class LanguageCode:
    """Validated language carrying every form a backend might need.

    `alpha_3` is canonical (always present, ISO-639-3). `alpha_2` is
    present iff pycountry knows one — some regional languages
    (`yue` = Cantonese, `cmn` is Mandarin which does have `zh`) have
    no ISO-639-1 code. `name` is the English language name as
    pycountry reports it.
    """

    alpha_3: str
    alpha_2: str | None
    name: str

    @classmethod
    def from_str(cls, raw: str) -> "LanguageCode":
        """Validate raw user input and return the resolved record.

        Accepts only valid ISO-639-3 alpha_3 codes (3 lowercase
        letters; case + whitespace are normalized). Anything else
        raises `ValueError` — callers at the CLI boundary wrap this
        into `typer.BadParameter`.
        """
        s = (raw or "").strip().lower()
        if len(s) != 3:
            raise ValueError(
                f"language must be a 3-letter ISO-639-3 code "
                f"(e.g. 'eng', 'cmn', 'yue', 'spa'); got {raw!r}."
            )
        rec = pycountry.languages.get(alpha_3=s)
        if rec is None:
            raise ValueError(
                f"language {raw!r} is not a known ISO-639-3 code. "
                f"Examples: eng (English), cmn (Mandarin), spa (Spanish), "
                f"yue (Cantonese)."
            )
        return cls(
            alpha_3=s,
            alpha_2=getattr(rec, "alpha_2", None),
            name=rec.name,
        )

    @property
    def alpha_2_or_3(self) -> str:
        """`alpha_2` if present, else `alpha_3`.

        Backends that prefer 2-letter (rev, whisperx, tencent) but
        must still handle Cantonese / minority languages use this —
        Cantonese has alpha_3 `yue` but no alpha_2.
        """
        return self.alpha_2 or self.alpha_3


__all__ = ["LanguageCode"]
