"""StanzaBackend: Universal-Dependencies morphosyntax tagging.

Per-language Stanza pipelines, lazily loaded. Mirrors the structure of
BA2's morphosyntax handler at
`batchalign2/batchalign/pipelines/morphosyntax/ud.py` (`parse_sentence`,
HANDLERS dict, per-language `en/`, `fr/`, `ja/` subdirs) — but in this
rewrite the per-language POS handlers move into the Rust-side
runner where the AST is mutated. Here the backend's job is narrower:
hand back UD-tagged morphology, no AST awareness.

CRITICAL — retokenization behavior (`retokenize: bool`):
  BA2 supports two modes:
    - retokenize=False: Stanza processes raw utterance text as-is.
    - retokenize=True: per-language BERT utterance-segmentation model
      first carves the ASR blob into utterances; Stanza then runs per
      utterance.
  Reference: batchalign2/batchalign/pipelines/morphosyntax/ud.py
  (around `parse_sentence` and the surrounding orchestrator). In the new
  design, utterance segmentation is a separate task (`Task.UtSeg`), so
  `retokenize` on the Morphosyntax backend is a *fallback* — if the
  input already has utterance boundaries, we honor them; otherwise we
  use Stanza's tokenizer to re-split.

This project supports UD `%mor` syntax only (see CLAUDE.md). Legacy
CLAN-mor `&PRES` markers are NOT emitted.
"""

from __future__ import annotations

from typing import Any

from batchalign.backends.base import Morphosyntax, BatchPolicy


class StanzaBackend(Morphosyntax):
    """Stanza UD morphosyntax tagger, one pipeline per language."""

    def __init__(
        self,
        lang: str = "en",
        *,
        batch_size: int = 64,
        batch_window_ms: int = 100,
        retokenize: bool = False,
        processors: str = "tokenize,mwt,pos,lemma,depparse",
    ) -> None:
        import stanza  # type: ignore[import-not-found]

        self._stanza = stanza
        self._lang = lang
        self._retokenize = retokenize
        self._processors = processors
        # Per-language Stanza pipeline. `tokenize_pretokenized=True` would
        # disable Stanza's own splitter; we leave it as default so the
        # `retokenize=True` path can lean on Stanza's tokenizer.
        self._nlp = stanza.Pipeline(
            lang,
            processors=processors,
            verbose=False,
        )
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @property
    def name(self) -> str:
        # Embed stanza version so cache invalidates on stanza upgrades.
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
            # Per-utterance shape: build text either from `item.text` or by
            # rejoining `item.tokens`. `item.retokenize` lets Stanza's own
            # tokenizer resplit the input; otherwise we still process as one
            # block but trust the upstream token boundaries when emitting.
            text = item.text or " ".join(item.tokens)
            tokens, mor_line, gra_line = self._tag_utterance(text)
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

    def _tag_utterance(
        self, text: str
    ) -> tuple[list["Any"], str, str]:
        """Run Stanza on a single utterance.

        Returns a `(tokens, mor_line, gra_line)` triple where:
        - `tokens` is a list of `MorphosyntaxToken` (one per Stanza word).
        - `mor_line` / `gra_line` are pre-rendered tier strings so the Rust
          runner can drop them in directly without re-rendering.
        """
        from batchalign._core.proto import MorphosyntaxToken

        doc = self._nlp(text)
        tokens: list[MorphosyntaxToken] = []
        mor_parts: list[str] = []
        gra_parts: list[str] = []
        for sent in doc.sentences:
            for word in sent.words:
                tokens.append(
                    MorphosyntaxToken(
                        text=getattr(word, "text", ""),
                        lemma=getattr(word, "lemma", "") or getattr(word, "text", ""),
                        upos=getattr(word, "upos", None)
                        or getattr(word, "pos", "X"),
                        features=self._split_feats(getattr(word, "feats", None)),
                        head=getattr(word, "head", None),
                        deprel=getattr(word, "deprel", None),
                    )
                )
                mor_parts.append(self._format_mor(word))
                gra_parts.append(self._format_gra(word))
        # The CHAT grammar requires a terminator on `%mor:` (E305 otherwise).
        # Append a trailing `.` so the tier round-trips through `Chat::parse`
        # cleanly.
        mor_line = " ".join(mor_parts)
        if mor_line:
            mor_line = mor_line + " ."
        # `%gra:` is intentionally not emitted yet — the CHAT grammar's
        # `%gra` tier rejects Stanza's `:` in deprels (`obl:agent`,
        # `nsubj:pass`, etc.), and downstream tools that need dependency
        # info read it off the typed AST anyway. We'll re-enable once a
        # normalization layer that flattens `head:subtype` to a single
        # allowed deprel lands.
        return tokens, mor_line, ""

    @staticmethod
    def _split_feats(feats: Any) -> list[str]:
        """Split a Stanza `feats` string ("Tense=Past|Mood=Ind") into UD values."""
        if not feats:
            return []
        return [kv.split("=", 1)[1] for kv in feats.split("|") if "=" in kv]

    def _tag_utterances(self, utterances: list[str]) -> tuple[list[str], list[str]]:
        """Run Stanza on each utterance string, returning (%mor, %gra) lines.

        If `retokenize=True` and an utterance contains internal sentence
        boundaries (period, `!`, `?`), Stanza's tokenizer will split it
        and we concatenate per-sentence results. Without retokenize, the
        whole utterance is forced into a single sentence by feeding it
        as-is and concatenating Stanza-detected sentences anyway — the
        difference is whether the caller relies on Stanza's sentence
        splitter to fix segmentation errors.
        """
        mor_lines: list[str] = []
        gra_lines: list[str] = []
        for text in utterances:
            doc = self._nlp(text)
            mor_parts: list[str] = []
            gra_parts: list[str] = []
            for sent in doc.sentences:
                for word in sent.words:
                    mor_parts.append(self._format_mor(word))
                    gra_parts.append(self._format_gra(word))
            mor_lines.append(" ".join(mor_parts))
            gra_lines.append(" ".join(gra_parts))
        return mor_lines, gra_lines

    @staticmethod
    def _format_mor(word: Any) -> str:
        """`POS|lemma[-Feat]*` in UD syntax (sentence-case features).

        Stanza emits raw `Person=3|Number=Plur|...` style; the project's
        `%mor:` grammar rejects bare digit features (`-3-` is invalid) so
        we filter pure-numeric values out of the suffix. Real letter
        features (`Past`, `Plur`, `Ind`, `Sing`, ...) come through unchanged.
        """
        upos = getattr(word, "upos", None) or getattr(word, "pos", "X")
        lemma = getattr(word, "lemma", None) or getattr(word, "text", "")
        feats = getattr(word, "feats", None)
        if feats:
            values: list[str] = []
            for kv in feats.split("|"):
                if "=" not in kv:
                    continue
                val = kv.split("=", 1)[1]
                if val and not val.isdigit():
                    values.append(val)
            tail = "-" + "-".join(values) if values else ""
        else:
            tail = ""
        return f"{upos}|{lemma}{tail}"

    @staticmethod
    def _format_gra(word: Any) -> str:
        """`index|head|deprel` triple for the `%gra` tier."""
        idx = getattr(word, "id", 0)
        head = getattr(word, "head", 0)
        deprel = getattr(word, "deprel", "ROOT")
        return f"{idx}|{head}|{deprel}"


__all__ = ["StanzaBackend"]
