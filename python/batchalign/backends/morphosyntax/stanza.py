"""StanzaBackend: Universal-Dependencies morphosyntax tagging.

Faithful port of BA2's morphosyntax handler
(`batchalign2/batchalign/pipelines/morphosyntax/ud.py`). The per-POS UD→CHAT
handlers and the `%mor`/`%gra` assembler live in `ud/render.py` (copied
line-for-line); this backend owns the Stanza pipeline and the per-utterance
preprocessing that BA2's `morphoanalyze` did (deriving the terminator,
cleaning the line, running Stanza with `tokenize_no_ssplit`, taking the first
sentence, and the `~part|s verb` post-substitution).

This backend emits a fully *structured* analysis — per main-tier word, a head
morpho-unit plus `~`-post-clitics, and per chunk a `%gra` triple (see
`render.SentenceAnalysis`). It never builds `%mor`/`%gra` tier text. The Rust
morphosyntax taskrunner turns that structure into typed `MorTier`/`GraTier`
values and serializes them with the official CHAT writer; there is no
pre-rendered-string escape hatch.

Pipeline config mirrors BA2 (`ud.py:_build_nlp`):
  - `tokenize_no_ssplit=True` — the whole utterance is one sentence.
  - English MWT uses the `gum` model; a fixed exclusion list disables MWT for
    languages where Stanza's MWT is unwanted (zh*, ja, ko, …).
  - Japanese uses the `combined` tokenize/pos/lemma/depparse models.

This project supports UD `%mor` syntax only (see CLAUDE.md). Legacy CLAN-mor
`&PRES` markers are never emitted.
"""

from __future__ import annotations

import gc
import importlib
import json
import logging
import os
import re
import threading
import urllib.request
from collections import OrderedDict
from pathlib import Path
from typing import Any

from batchalign.backends.base import Morphosyntax, BatchPolicy
from batchalign.backends.morphosyntax.ud import render
from batchalign.backends.morphosyntax.ud.lang import to_stanza
from batchalign.backends.morphosyntax.ud.tokenize import tokenizer_processor

_log = logging.getLogger("batchalign.stanza")


def _log_analysis_anomalies(item: Any, analysis: "render.SentenceAnalysis") -> None:
    for anomaly in analysis.anomalies:
        _log.warning(
            "morphotag: repaired Stanza field source=%s utterance=%d "
            "word=%d text=%r field=%s original=%r replacement=%r reason=%s",
            item.source_id,
            item.utterance_id,
            anomaly.word_index,
            anomaly.text,
            anomaly.field,
            anomaly.original,
            anomaly.replacement,
            anomaly.reason,
        )


# Module-level thread-local for the "current sentences" the tokenizer
# postprocessor should see. The closure registered with a Stanza pipeline
# reads from this rather than from a captured `self`, so a pipeline can
# be safely shared across StanzaBackend instances (per-(langset, mode)
# caching, Landing 3 #10).
#
# Stored as a list[str] — one source sentence per Stanza sentence in the
# joined input — so a single `nlp("s1\n\ns2\n\n…")` batched call can have
# its postprocessor align Stanza's tokens to the upstream word split for
# the right source per index. Single-sentence callers wrap their string
# in a one-element list.
_postproc_state = threading.local()


def _current_sentences_for(pipeline_key: tuple) -> list[str]:
    return getattr(_postproc_state, "sentences", {}).get(pipeline_key, [])  # type: ignore[no-any-return]


def _set_current_sentences(pipeline_key: tuple, value: list[str]) -> None:
    bucket = getattr(_postproc_state, "sentences", None)
    if bucket is None:
        bucket = {}
        _postproc_state.sentences = bucket
    bucket[pipeline_key] = value


def _clear_current_sentences(pipeline_key: tuple) -> None:
    bucket = getattr(_postproc_state, "sentences", None)
    if bucket is None:
        return
    bucket.pop(pipeline_key, None)
    if not bucket:
        del _postproc_state.sentences


# Process-wide LRU: (frozenset(langs), retokenize) -> Stanza Pipeline.
# Two resident language sets preserve locality when files alternate languages
# without retaining every full model encountered by a multilingual corpus.
_PIPELINE_CACHE_CAPACITY = 2
_pipeline_cache: OrderedDict[tuple, Any] = OrderedDict()
_pipeline_cache_lock = threading.Lock()

# Process-wide memo of pipeline keys whose construction has already failed
# (e.g. unsupported Stanza language). Keeps `call()` from re-attempting the
# same broken `stanza.Pipeline(lang=...)` for every utterance in a file
# (and for every file with that language). Repeated calls still raise, but
# without re-entering the failed model constructor. Maps key → error text.
_pipeline_failures: dict[tuple, str] = {}

# PyTorch lazily imports a number of internal modules while deserializing
# models. Importing through two deep dependency chains on different worker
# threads can expose a partially initialized ``torch`` package (upstream
# pytorch/pytorch#182560). Batchalign's engine deliberately invokes Python
# backends from blocking worker threads, so finish the Torch/Functorch import
# synchronously, behind one process-wide lock, before constructing Stanza.
_runtime_import_lock = threading.Lock()


def _import_stanza_runtime() -> Any:
    """Prepare lazy runtime state before Stanza can reach worker threads.

    Textual temporarily replaces ``sys.stderr`` while the interactive
    dashboard owns the terminal.  Stanza's download progress bar lazily
    creates a multiprocessing lock and resource-tracker process, which reads
    that stream's file descriptor.  Prime the exact tqdm class Stanza uses
    while backend construction is still on the main thread and stderr is a
    real terminal.
    """
    with _runtime_import_lock:
        importlib.import_module("torch")
        importlib.import_module("torch._functorch.config")
        stanza = importlib.import_module("stanza")
        stanza.resources.common.tqdm.get_lock()
        return stanza

# An existing Stanza resources.json can become stale when Stanford republishes
# model artifacts under the same resources version. Refresh it once at this
# process's backend boundary before any pipeline verifies downloaded models.
_manifest_refresh_lock = threading.Lock()
_manifest_refresh_attempted = False
_MANIFEST_REFRESH_TIMEOUT_S = 10


def _refresh_stanza_resources_manifest_if_present(
    stanza: Any,
    *,
    urlopen: Any = urllib.request.urlopen,
) -> None:
    """Atomically refresh an existing Stanza catalog, failing open offline.

    A missing manifest belongs to Stanza's normal first-run bootstrap and is
    deliberately left alone. Any fetch, validation, or filesystem failure
    preserves the cached file so an offline installation keeps working.
    """
    common = stanza.resources.common
    manifest_path = Path(common.DEFAULT_MODEL_DIR) / "resources.json"
    if not manifest_path.exists():
        return

    url = (
        f"{common.DEFAULT_RESOURCES_URL.rstrip('/')}"
        f"/resources_{common.DEFAULT_RESOURCES_VERSION}.json"
    )
    temporary_path = manifest_path.with_name(
        f".{manifest_path.name}.{os.getpid()}.tmp-refresh"
    )
    try:
        with urlopen(url, timeout=_MANIFEST_REFRESH_TIMEOUT_S) as response:
            data = response.read()
        json.loads(data)
        with temporary_path.open("wb") as output:
            output.write(data)
            output.flush()
            os.fsync(output.fileno())
        os.replace(temporary_path, manifest_path)
        _log.info("refreshed Stanza resource catalog from %s", url)
    except Exception as exc:
        try:
            temporary_path.unlink(missing_ok=True)
        except OSError:
            pass
        _log.warning(
            "could not refresh Stanza resource catalog; using cached manifest: %s",
            exc,
        )


def _refresh_stanza_resources_manifest_once(stanza: Any) -> None:
    global _manifest_refresh_attempted
    with _manifest_refresh_lock:
        if _manifest_refresh_attempted:
            return
        _manifest_refresh_attempted = True
        _refresh_stanza_resources_manifest_if_present(stanza)

# Languages for which Stanza's MWT splitter is disabled (BA2 ud.py:1034-1036).
_MWT_EXCLUSION = frozenset(
    {
        "hr",
        "zh",
        "zh-hans",
        "zh-hant",
        "ja",
        "ko",
        "sl",
        "sr",
        "bg",
        "ru",
        "et",
        "hu",
        "eu",
        "el",
        "he",
        "hi",
        "af",
        "ga",
        "da",
        "ro",
    }
)

# NOTE: BA2 applies one post-render string fixup to the %mor tier
# (`~part|s verb|X-Ger-S` → `~aux|is verb|X-Part-Pres-S`, ud.py:826) for a rare
# English gerund+'s pattern. It operated on rendered tier text, which we no
# longer build; reproducing it structurally is deferred (TODO) until a parity
# fixture exercises it.

# CHAT-marker cleanup applied to the line before Stanza (BA2 ud.py:730).
_CLEANUP_RE = re.compile(r"\+<|\+/|\(|\)|\+\^|\+//|\+\.\.\.|_|[#]")

# `word@s`, `word@s:eng`, `word@n`, etc — the `@`-tagged CHAT word-class
# markers (BA2 ud.py:787-791). Without substitution, Stanza splits on `@`
# and the resulting %mor item count disagrees with the main tier's
# single alignable word. We mask each occurrence as `xbxxx` (a token
# Stanza treats as an unknown unit) and remember the original
# `(stem, tag)` pair so the renderer can re-emit the right POS/lemma —
# see `render.parse_sentence`'s `special_forms` handling.
# The word surface may contain combining marks that Python's ``\w`` does not
# include (for example the Devanagari vowel sign in ``जी@s:hin`` or Tamil
# combining signs). Match the complete non-whitespace surface up to ``@``;
# the marker itself remains deliberately narrow.
_SPECIAL_FORM_RE = re.compile(r"[^\s@]+@[\w:]+")

# CA-notation marker cleanup — Conversation-Analysis transcripts use a
# rich set of prosodic markers (°, ↑, ↓, ∆, ⌈⌉, ⌊⌋, ≈, ≋, ‡, etc.)
# that Stanza tokenizes incorrectly, producing bogus morphology. Strip
# them from the line passed to Stanza so we still get %mor on the
# words; the original text (with markers) is preserved for serialization
# by the Rust writer.
#
# Landing 3 #12 from the BA3 cutover plan (per user direction: don't
# blanket-skip CA transcripts; strip the markers so morphology survives).
_CA_NOTATION_RE = re.compile(
    # Vertical pitch-step markers + miscellaneous CA prosody glyphs.
    r"[°↑↓∆⌈⌉⌊⌋≈≋‡↻↺⁎⁋∇∅⌃⌄]|"
    # Terminal-contour arrows: falling (↘), rising (↗), and their less
    # common ↖/↙ variants. CHAT CA mode uses these as utterance-final tone
    # markers; without stripping, Stanza tokenizes each as an unmapped
    # SYM/PUNCT and the renderer returns None for it, silently shrinking
    # the %mor count by one per arrow. Estonian33 alone uses ↘ 148× in
    # the multilingual fixture — one of the dominant skip causes.
    r"[↗↘↖↙]|"
    r"\bH\*|"  # high pitch accent (word-start anchored)
    r"\bL\*|"  # low pitch accent (word-start anchored)
    r"[‐-―]+"  # Unicode dashes used as CA continuations
)


class StanzaBackend(Morphosyntax):
    """Stanza UD morphosyntax tagger, one pipeline per language.

    Languages are resolved per-utterance from the `MorphosyntaxInput.language`
    stamped by the Rust runner (which reads each CHAT file's `@Languages:`
    header). The optional `lang` constructor arg pins the backend to a single
    language regardless of input — useful only for tests and special callers.
    """

    def __init__(
        self,
        lang: str | None = None,
        *,
        batch_size: int = 64,
        batch_window_ms: int = 100,
        retokenize: bool = False,
    ) -> None:
        stanza = _import_stanza_runtime()

        self._stanza = stanza
        _refresh_stanza_resources_manifest_once(stanza)
        # `lang` (optional override) may arrive as ISO-639-3 (`eng`), already
        # Stanza-shaped (`en`), or a comma/space-separated list for
        # code-switching (`en,es`). When None, every `call()` resolves the
        # language from each input's `language` field.
        if lang is None:
            self._pinned_langs: list[str] | None = None
        else:
            parts = [p for p in lang.replace(",", " ").split() if p]
            self._pinned_langs = [
                self._normalize_pipeline_lang(to_stanza(p)) for p in parts
            ] or ["en"]
        self._retokenize = retokenize
        # Eager-build only when a language was pinned; otherwise defer until
        # the first input arrives (header-driven dispatch).
        if self._pinned_langs is not None:
            self._nlp: Any | None = self._build_pipeline_for(stanza, self._pinned_langs)
        else:
            self._nlp = None
        self._policy = BatchPolicy(max_size=batch_size, window_ms=batch_window_ms)

    @staticmethod
    def _normalize_pipeline_lang(lang: str) -> str:
        """`zh` → `zh-hans` for the Stanza pipeline (BA2 `_build_nlp`)."""
        return "zh-hans" if lang == "zh" else lang

    @staticmethod
    def _pipeline_key_for(langs: list[str], retokenize: bool) -> tuple:
        return (frozenset(langs), retokenize)

    def _lang_config(self, lang: str, langs: list[str], key: tuple) -> dict[str, Any]:
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
            # The backend refreshes an existing resources.json once, before
            # the dashboard starts.  Avoid making every lazily constructed
            # language pipeline download that same manifest again.  Missing
            # model files are still downloaded by REUSE_RESOURCES.
            "download_method": "reuse_resources",
        }
        if lang not in _MWT_EXCLUSION:
            config["processors"]["mwt"] = "gum" if lang == "en" else "default"
        if lang == "ja":
            for proc in ("tokenize", "pos", "lemma", "depparse"):
                config["processors"][proc] = "combined"
        # When NOT retokenizing, force Stanza's tokenizer to honor the
        # upstream word split (BA2's `tokenize_postprocessor`). The closure
        # reads the "current sentence" from a module-level thread-local
        # keyed on the cache key so the pipeline can be shared across
        # StanzaBackend instances with the same (langset, retokenize).
        if not self._retokenize:
            langs_copy = list(langs)

            def _postproc(sentences):
                sources = _current_sentences_for(key)
                out = []
                for i, sent in enumerate(sentences):
                    source = sources[i] if i < len(sources) else ""
                    out.append(tokenizer_processor(sent, langs_copy, source))
                return out

            config["tokenize_postprocessor"] = _postproc
        return config

    def _build_pipeline_for(self, stanza: Any, langs: list[str]) -> Any:
        """Construct or fetch the per-(langset, mode) cached pipeline.

        First call for a given key constructs and caches; later calls
        return the cached pipeline. Code-switching files
        (`MultilingualPipeline`) and single-language files both cache
        under the same scheme.

        Raises whatever Stanza raises if model construction fails. Deferred
        header-driven construction is caught and memoized by ``call()``;
        explicitly pinned construction fails immediately in ``__init__``.
        """
        key = self._pipeline_key_for(langs, self._retokenize)
        with _pipeline_cache_lock:
            hit = _pipeline_cache.get(key)
            if hit is not None:
                _pipeline_cache.move_to_end(key)
                return hit
            if len(langs) > 1:
                configs = {lang: self._lang_config(lang, langs, key) for lang in langs}
                nlp = stanza.MultilingualPipeline(
                    lang_configs=configs,
                    lang_id_config={"langid_lang_subset": list(langs)},
                    download_method="reuse_resources",
                )
            else:
                lang = langs[0]
                nlp = stanza.Pipeline(lang=lang, **self._lang_config(lang, langs, key))
            _pipeline_cache[key] = nlp
            while len(_pipeline_cache) > _PIPELINE_CACHE_CAPACITY:
                _, evicted = _pipeline_cache.popitem(last=False)
                del evicted
                gc.collect()
            return nlp

    def _langs_for_input(self, language_spec: Any) -> list[str]:
        """Resolve the input's `LanguageSpec` to Stanza pipeline codes.

        `LanguageSpecCode("eng")` → `["en"]`; `LanguageSpecCode("eng,spa")`
        (comma-list, used for code-switching headers) → `["en", "es"]`;
        `LanguageSpecPerFile` or `LanguageSpecAuto` → `["en"]` fallback
        (the runner is expected to resolve PerFile to Code before
        dispatch — Auto is informational for ASR).
        """
        from batchalign._core.proto import LanguageSpecCode

        if isinstance(language_spec, LanguageSpecCode):
            raw = str(getattr(language_spec, "value", "") or "")
            parts = [p for p in raw.replace(",", " ").split() if p]
            langs = [self._normalize_pipeline_lang(to_stanza(p)) for p in parts]
            if langs:
                return langs
        return ["en"]

    def _resolve_langs(self, language_spec: Any) -> list[str]:
        """Pinned constructor language takes precedence; else header-driven."""
        if self._pinned_langs is not None:
            return self._pinned_langs
        return self._langs_for_input(language_spec)

    @property
    def name(self) -> str:
        version = getattr(self._stanza, "__version__", "unknown")
        retok = "retok" if self._retokenize else "noretok"
        # Cache identity covers both model/runtime behavior and Batchalign's
        # token-alignment contract. ``native-mwt1`` admits native MWT analysis
        # inside authoritative CHAT words while preserving conventional whole
        # spellings such as ``gonna`` and ambiguous Italian ``dai``.
        return f"stanza:{version}:{retok}:native-mwt1"

    @property
    def batch_policy(self) -> BatchPolicy:
        return self._policy

    def call(
        self, batch: list[Any], *, progress: Any = None, **_kwargs: Any
    ) -> list[Any]:
        """Process a batch of utterances.

        Inputs are grouped by language so utterances sharing a pipeline can
        be concatenated with `\\n\\n` and shipped to Stanza in **one**
        `nlp(joined)` call (`tokenize_no_ssplit=True` already preserves the
        sentence boundaries we feed it). That gives true per-language
        batching — Stanza's tokenizer / tagger / parser run vectorized over
        all N sentences at once.

        Failure handling distinguishes invalid individual utterances from a
        broken backend runtime:

        - **Language resolution** raises → that input gets an empty result.
        - **Pipeline init** raises (missing dependency/model, unsupported
          language, corrupt runtime) → the backend raises so the engine marks
          the command failed and the CLI does not overwrite input with empty
          tiers. The reason is memoized to avoid repeating construction.
        - **Batched `nlp(joined)` raises** → fall back to per-sentence
          processing for that group so we still rescue the inputs that
          would have parsed cleanly on their own.
        """
        from batchalign._core.proto import MorphosyntaxInput

        outputs: list[Any] = [None] * len(batch)

        # Phase 1: per-input language resolution + grouping. Outputs for
        # inputs whose language resolution itself fails are filled in here
        # with empty results; the rest get queued under their language key.
        groups: dict[tuple, list[tuple[int, Any, tuple[str, ...]]]] = {}
        for idx, item in enumerate(batch):
            if not isinstance(item, MorphosyntaxInput):
                raise NotImplementedError(
                    f"StanzaBackend does not handle input type: {type(item).__name__}"
                )
            try:
                langs = self._resolve_langs(item.language)
            except Exception as exc:  # noqa: BLE001 — never crash the batch.
                _log.warning(
                    "morphotag: skipping utterance %d in %s: language resolution failed (%s)",
                    item.utterance_id,
                    item.source_id,
                    exc,
                )
                outputs[idx] = self._empty_output(item)
                continue
            key = self._pipeline_key_for(langs, self._retokenize)
            groups.setdefault(key, []).append((idx, item, tuple(langs)))

        # Phase 2: per-language batched dispatch.
        for key, items in groups.items():
            langs = list(items[0][2])

            previous_failure = _pipeline_failures.get(key)
            if previous_failure is not None:
                raise RuntimeError(
                    "morphotag: Stanza pipeline is unavailable for "
                    f"langs={langs}: {previous_failure}"
                )

            try:
                nlp = self._build_pipeline_for(self._stanza, langs)
            except Exception as exc:  # noqa: BLE001 — surface model/runtime failures.
                if os.environ.get("BATCHALIGN_CLI_VERBOSE_TRACEBACKS"):
                    _log.exception(
                        "morphotag: Stanza pipeline initialization traceback"
                    )
                _pipeline_failures[key] = str(exc)
                raise RuntimeError(
                    "morphotag: failed to initialize Stanza pipeline for "
                    f"langs={langs}: {exc}"
                ) from exc

            self._run_language_group(key, langs, nlp, items, outputs)

        # Defensive: every slot must be filled (None would crash the
        # batcher's length-check in `crates/batchalign/batchalign-engine/
        # src/batcher.rs:130`).
        for i, item in enumerate(batch):
            if outputs[i] is None:
                outputs[i] = self._empty_output(item)

        return outputs

    def _run_language_group(
        self,
        key: tuple,
        langs: list[str],
        nlp: Any,
        items: list[tuple[int, Any, tuple[str, ...]]],
        outputs: list[Any],
    ) -> None:
        """Tag every utterance in one (langs, retokenize) group with a
        single batched Stanza call, falling back to per-utterance dispatch
        if the batched call itself raises."""
        # Preprocess every utterance's text the same way `_tag_utterance`
        # did individually. Items with empty post-clean text are recorded
        # as "no analysis" up front so they don't dilute the batched call.
        # `special_forms` rides along per utterance so the renderer can
        # re-inject the original `tag|stem` for each `xbxxx` mask.
        prepared: list[
            tuple[int, Any, str, list[list[str]]]
        ] = []  # (idx, item, line, sf)
        for idx, item, _ in items:
            text = item.text or " ".join(item.tokens)
            line_cut, special_forms = self._preprocess_text(text)
            if not line_cut:
                outputs[idx] = self._empty_output(item)
                continue
            prepared.append((idx, item, line_cut, special_forms))

        if not prepared:
            return

        sentences = [line for _, _, line, _ in prepared]

        # Publish the source-sentence list so the tokenize_postprocessor
        # closure can align each Stanza sentence to its upstream split by
        # position. Set BEFORE the `nlp()` call.
        _set_current_sentences(key, sentences)

        # Join with double newlines — Stanza's `tokenize_no_ssplit=True`
        # mode treats `\n\n` as a hard sentence boundary, so the resulting
        # `doc.sentences` will be in 1:1 correspondence with `prepared`.
        joined = "\n\n".join(sentences)

        try:
            try:
                doc = nlp(joined)
                stanza_sents = getattr(doc, "sentences", [])
            finally:
                _clear_current_sentences(key)
        except Exception as exc:  # noqa: BLE001 — recover per-input below.
            _log.warning(
                "morphotag: Stanza batched call failed for langs=%s (%d utts, %s); "
                "falling back to per-utterance",
                langs,
                len(prepared),
                exc,
            )
            self._tag_per_input_fallback(key, langs, nlp, prepared, outputs)
            return

        if len(stanza_sents) != len(prepared):
            # Stanza disagreed with our `\n\n` splitting (unlikely under
            # `tokenize_no_ssplit=True` but defensive). Fall back so we
            # don't misattribute analyses across utterances.
            _log.warning(
                "morphotag: Stanza returned %d sentences for %d batched inputs "
                "(langs=%s); falling back to per-utterance",
                len(stanza_sents),
                len(prepared),
                langs,
            )
            self._tag_per_input_fallback(key, langs, nlp, prepared, outputs)
            return

        for (idx, item, _, special_forms), sent in zip(
            prepared, stanza_sents, strict=False
        ):
            try:
                analysis = render.parse_sentence(sent, ".", special_forms, langs[0])
                outputs[idx] = self._analysis_to_output(item, analysis)
            except Exception as exc:  # noqa: BLE001 — render fault on one utt.
                _log.warning(
                    "morphotag: render failed for utterance %d in %s: %s",
                    item.utterance_id,
                    item.source_id,
                    exc,
                )
                outputs[idx] = self._empty_output(item)

    def _tag_per_input_fallback(
        self,
        key: tuple,
        langs: list[str],
        nlp: Any,
        prepared: list[tuple[int, Any, str, list[list[str]]]],
        outputs: list[Any],
    ) -> None:
        """Slow path: tag each utterance with its own `nlp()` call so a
        single explosive sentence in the batch doesn't sink the rest."""
        for idx, item, line_cut, special_forms in prepared:
            _set_current_sentences(key, [line_cut])
            try:
                try:
                    doc = nlp(line_cut)
                finally:
                    _clear_current_sentences(key)
                sents = getattr(doc, "sentences", [])
                if not sents:
                    outputs[idx] = self._empty_output(item)
                    continue
                analysis = render.parse_sentence(sents[0], ".", special_forms, langs[0])
                outputs[idx] = self._analysis_to_output(item, analysis)
            except Exception as exc:  # noqa: BLE001 — give up on just this one.
                _log.warning(
                    "morphotag: skipping utterance %d in %s (langs=%s) in fallback: %s",
                    item.utterance_id,
                    item.source_id,
                    langs,
                    exc,
                )
                outputs[idx] = self._empty_output(item)

    @staticmethod
    def _empty_output(item: Any) -> Any:
        from batchalign._core.proto import MorphosyntaxOutput

        return MorphosyntaxOutput(
            source_id=item.source_id,
            utterance_id=item.utterance_id,
            tokens=[],
            terminator=None,
        )

    @staticmethod
    def _analysis_to_output(item: Any, analysis: "render.SentenceAnalysis") -> Any:
        from batchalign._core.proto import (
            GraTerminator,
            MorphosyntaxOutput,
            MorphosyntaxToken,
            MorphosyntaxUnit,
        )

        _log_analysis_anomalies(item, analysis)

        tokens: list[Any] = []
        for word in analysis.words:
            units = [
                MorphosyntaxUnit(
                    pos=u.pos,
                    lemma=u.lemma,
                    features=list(u.features),
                    index=u.index,
                    head=0 if u.deprel.lower() == "root" else u.head,
                    deprel=u.deprel,
                )
                for u in word.units
            ]
            tokens.append(MorphosyntaxToken(text=word.text, units=units))

        terminator = None
        if tokens and analysis.terminator is not None:
            t_index, t_head, t_deprel = analysis.terminator
            terminator = GraTerminator(
                index=t_index,
                head=0 if t_deprel.lower() == "root" else t_head,
                deprel=t_deprel,
            )

        return MorphosyntaxOutput(
            source_id=item.source_id,
            utterance_id=item.utterance_id,
            tokens=tokens,
            terminator=terminator,
        )

    # ----- internals -----------------------------------------------------

    @staticmethod
    def _preprocess_text(text: str) -> tuple[str, list[list[str]]]:
        """Apply the per-utterance CHAT cleanup that BA2's `morphoanalyze`
        ran before handing each line to Stanza.

        Returns `(line_cut, special_forms)` where `special_forms` is a list
        of `[stem, tag]` pairs (one per `word@tag` found, in document
        order). The line has those occurrences replaced with the `xbxxx`
        sentinel; the renderer re-injects the original POS/lemma when it
        sees `xbxxx` (matches BA2 `ud.py:782-791` + `parse_sentence`).
        Empty line means there's nothing analyzable left.
        """
        line_cut = render.clean_sentence(text)
        line_cut = _CLEANUP_RE.sub("", line_cut)
        # CA-notation markers don't carry word content; strip them so
        # Stanza tokenizes only the underlying lexical material. The
        # original (un-stripped) text is preserved by the Rust writer
        # via the typed utterance content.
        line_cut = _CA_NOTATION_RE.sub("", line_cut)
        # Collapse double-spaces introduced by the strip passes.
        line_cut = re.sub(r"\s+", " ", line_cut).strip()
        if not line_cut:
            return "", []

        # Special-form substitution (BA2 ud.py:787-791). Find each
        # `word@tag[:lang]` BEFORE we space-out commas, then mask them all
        # as `xbxxx` so Stanza tokenizes them as a single opaque unit. The
        # split on `@` recovers `[stem, tag]` — the renderer's
        # `xbxxx`-handler will emit `tag|stem` (or `L2|xxx` when the
        # tag begins with `s`, matching BA2's secondary-language convention).
        raw_forms = _SPECIAL_FORM_RE.findall(line_cut)
        special_forms: list[list[str]] = []
        for form in raw_forms:
            line_cut = line_cut.replace(form, "xbxxx", 1)
            special_forms.append(form.split("@", 1))

        # BA2 spaces commas out before tokenizing so they tokenize as their own
        # word (`cm|cm`). The runner drops main-tier separators, so this only
        # fires for commas that survived into `text`.
        line_cut = line_cut.replace(",", " ,").replace("  ", " ")
        line_cut = line_cut.replace("(", "").replace(")", "").strip()
        return line_cut, special_forms


__all__ = ["StanzaBackend"]
