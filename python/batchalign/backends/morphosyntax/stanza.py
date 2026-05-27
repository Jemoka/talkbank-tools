"""StanzaBackend: Universal-Dependencies morphosyntax tagging.

Faithful port of BA2's morphosyntax handler
(`batchalign2/batchalign/pipelines/morphosyntax/ud.py`). The per-POS UD→CHAT
handlers and the `%mor`/`%gra` assembler live in `ud/render.py` (copied
line-for-line); this backend owns the Stanza pipeline and the per-utterance
preprocessing that BA2's `morphoanalyze` did (deriving the terminator,
cleaning the line, running Stanza with `tokenize_no_ssplit`, taking the first
sentence, and the `~part|s verb` post-substitution).

The Rust morphosyntax runner passes the backend's pre-rendered `mor`/`gra`
strings straight through (`render_mor`/`render_gra` return `out.mor`/`out.gra`
verbatim when present), so producing BA2-identical tier strings here is
sufficient for `%mor`/`%gra` parity — no Rust changes needed for rendering.

Pipeline config mirrors BA2 (`ud.py:_build_nlp`):
  - `tokenize_no_ssplit=True` — the whole utterance is one sentence.
  - English MWT uses the `gum` model; a fixed exclusion list disables MWT for
    languages where Stanza's MWT is unwanted (zh*, ja, ko, …).
  - Japanese uses the `combined` tokenize/pos/lemma/depparse models.

This project supports UD `%mor` syntax only (see CLAUDE.md). Legacy CLAN-mor
`&PRES` markers are never emitted.
"""

from __future__ import annotations

import re
from typing import Any

from batchalign.backends.base import Morphosyntax, BatchPolicy
from batchalign.backends.morphosyntax.ud import render
from batchalign.backends.morphosyntax.ud.lang import to_stanza
from batchalign.backends.morphosyntax.ud.tokenize import tokenizer_processor

# Languages for which Stanza's MWT splitter is disabled (BA2 ud.py:1034-1036).
_MWT_EXCLUSION = frozenset(
    {
        "hr", "zh", "zh-hans", "zh-hant", "ja", "ko", "sl", "sr", "bg", "ru",
        "et", "hu", "eu", "el", "he", "af", "ga", "da", "ro",
    }
)

# Post-substitution BA2 applies to the rendered %mor (ud.py:826).
_PART_S_VERB_RE = re.compile(r"~part\|s verb\|(\w+)-Ger-S")

# CHAT-marker cleanup applied to the line before Stanza (BA2 ud.py:730).
_CLEANUP_RE = re.compile(r"\+<|\+/|\(|\)|\+\^|\+//|\+\.\.\.|_|[#]")


class StanzaBackend(Morphosyntax):
    """Stanza UD morphosyntax tagger, one pipeline per language."""

    def __init__(
        self,
        lang: str = "en",
        *,
        batch_size: int = 64,
        batch_window_ms: int = 100,
        retokenize: bool = False,
        processors: str | None = None,
    ) -> None:
        import stanza  # type: ignore[import-not-found]

        self._stanza = stanza
        # `lang` may arrive as ISO-639-3 (`eng`), already Stanza-shaped (`en`),
        # or a comma/space-separated list for code-switching (`en,es`). The
        # handler dispatch always uses the FIRST language (BA2 `parse_sentence`
        # is called with `lang[0]`); a multi-language doc gets a Stanza
        # MultilingualPipeline that auto-detects per utterance.
        parts = [p for p in lang.replace(",", " ").split() if p]
        self._langs = [self._normalize_pipeline_lang(to_stanza(p)) for p in parts] or ["en"]
        self._lang = self._langs[0]
        self._retokenize = retokenize
        # The tokenize postprocessor (retokenize=False) needs the raw sentence
        # currently being tagged so it can align Stanza's tokens to the
        # upstream word split. We stash it here before each `nlp()` call.
        self._current_sentence = ""
        self._nlp = self._build_pipeline(stanza)
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @staticmethod
    def _normalize_pipeline_lang(lang: str) -> str:
        """`zh` → `zh-hans` for the Stanza pipeline (BA2 `_build_nlp`)."""
        return "zh-hans" if lang == "zh" else lang

    def _lang_config(self, lang: str) -> dict[str, Any]:
        """Per-language Stanza config matching BA2's `_build_nlp`."""
        config: dict[str, Any] = {
            "processors": {
                "tokenize": "default",
                "pos": "default",
                "lemma": "default",
                "depparse": "default",
            },
            "tokenize_no_ssplit": True,
            "verbose": False,
        }
        if lang not in _MWT_EXCLUSION:
            config["processors"]["mwt"] = "gum" if lang == "en" else "default"
        if lang == "ja":
            for proc in ("tokenize", "pos", "lemma", "depparse"):
                config["processors"][proc] = "combined"
        # When NOT retokenizing, force Stanza's tokenizer to honor the upstream
        # word split (BA2's `tokenize_postprocessor`). The postprocessor sees
        # the full language list (BA2 passes `list(langs_alpha2)`).
        if not self._retokenize:
            langs = list(self._langs)

            def _postproc(sentences):
                return [
                    tokenizer_processor(sent, langs, self._current_sentence)
                    for sent in sentences
                ]

            config["tokenize_postprocessor"] = _postproc
        return config

    def _build_pipeline(self, stanza: Any) -> Any:
        """Construct a single- or multi-language Stanza pipeline (BA2 parity)."""
        if len(self._langs) > 1:
            # Code-switching: MultilingualPipeline auto-detects per utterance.
            configs = {lang: self._lang_config(lang) for lang in self._langs}
            return stanza.MultilingualPipeline(
                lang_configs=configs,
                lang_id_config={"langid_lang_subset": list(self._langs)},
            )
        lang = self._langs[0]
        return stanza.Pipeline(lang=lang, **self._lang_config(lang))

    @property
    def name(self) -> str:
        version = getattr(self._stanza, "__version__", "unknown")
        retok = "retok" if self._retokenize else "noretok"
        return f"stanza:{self._lang}:{version}:{retok}"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(self, batch: list[Any]) -> list[Any]:
        from batchalign._core.proto import (
            MorphosyntaxInput,
            MorphosyntaxOutput,
            MorphosyntaxToken,
        )

        outputs: list[Any] = []
        for item in batch:
            if not isinstance(item, MorphosyntaxInput):
                raise NotImplementedError(
                    f"StanzaBackend does not handle input type: {type(item).__name__}"
                )
            text = item.text or " ".join(item.tokens)
            # The Rust runner strips the utterance terminator before building
            # the token list, so it ships it separately. Pre-`terminator`
            # builds (no field) default to a period.
            ending = getattr(item, "terminator", "") or "."
            tokens, mor_line, gra_line = self._tag_utterance(text, ending)
            outputs.append(
                MorphosyntaxOutput(
                    source_id=item.source_id,
                    utterance_id=item.utterance_id,
                    tokens=tokens,
                    mor=mor_line,
                    gra=gra_line,
                )
            )
        return outputs

    # ----- internals -----------------------------------------------------

    def _tag_utterance(self, text: str, ending: str) -> tuple[list[Any], str, str]:
        """Run Stanza on one utterance and render BA2-faithful tiers.

        Returns `(tokens, mor_line, gra_line)`: `tokens` is a best-effort
        list of `MorphosyntaxToken` (one per Stanza word); `mor_line` /
        `gra_line` are the pre-rendered tier strings (no leading label) that
        the Rust runner drops straight into the AST.
        """
        from batchalign._core.proto import MorphosyntaxToken

        line_cut = render.clean_sentence(text)
        line_cut = _CLEANUP_RE.sub("", line_cut).strip()
        if not line_cut:
            return [], "", ""

        # BA2 spaces commas out before tokenizing so they tokenize as their own
        # word (`cm|cm`). The runner drops main-tier separators, so this only
        # fires for commas that survived into `text`.
        line_cut = line_cut.replace(",", " ,").replace("  ", " ")

        # The postprocessor aligns Stanza's tokens to this exact sentence.
        self._current_sentence = line_cut.replace("(", "").replace(")", "").strip()
        doc = self._nlp(self._current_sentence)
        sents = getattr(doc, "sentences", [])
        if not sents:
            return [], "", ""

        # BA2 processes only the first sentence (tokenize_no_ssplit makes the
        # whole utterance one sentence).
        sent = sents[0]
        mor_line, gra_line = render.parse_sentence(sent, ending, [], self._lang)
        mor_line = _PART_S_VERB_RE.sub(r"~aux|is verb|\1-Part-Pres-S", mor_line)

        tokens: list[MorphosyntaxToken] = []
        for word in sent.words:
            tokens.append(
                MorphosyntaxToken(
                    text=getattr(word, "text", ""),
                    lemma=getattr(word, "lemma", "") or getattr(word, "text", ""),
                    upos=getattr(word, "upos", None) or getattr(word, "pos", "X"),
                    features=self._split_feats(getattr(word, "feats", None)),
                    head=getattr(word, "head", None),
                    deprel=getattr(word, "deprel", None),
                )
            )
        return tokens, mor_line, gra_line

    @staticmethod
    def _split_feats(feats: Any) -> list[str]:
        """Split a Stanza `feats` string ("Tense=Past|Mood=Ind") into UD values."""
        if not feats:
            return []
        return [kv.split("=", 1)[1] for kv in feats.split("|") if "=" in kv]


__all__ = ["StanzaBackend"]
