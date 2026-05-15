"""Stanza and language-code loading helpers for the worker process.

This module exists to keep Stanza-specific bootstrap policy out of the generic
worker entrypoint and the request-time inference routers. It owns:

- ISO language-code normalization for Stanza
- the MWT/non-MWT processor policy (capability-driven; see ``should_request_mwt``)
- installation of preloaded Stanza pipelines into worker runtime state
- the utseg-specific stanza-config builder used by inference dispatch
"""

from __future__ import annotations

import logging
import threading

from batchalign.inference._domain_types import LanguageCode, LanguageCode2
from batchalign.worker._stanza_capabilities import (
    _ISO3_OVERRIDES,
    StanzaCapabilityTable,
    get_cached_capability_table,
)
from batchalign.worker._types import _state

L = logging.getLogger("batchalign.worker")


class UnsupportedLanguageError(ValueError):
    """Stanza has no usable pipeline for the requested language.

    Distinct from a configuration error: the request itself cannot be
    served by this worker, so callers should reject the job upstream
    rather than retry. Surfaced as a typed error so downstream code
    can branch on it cleanly instead of pattern-matching the deep
    ``KeyError`` Stanza would otherwise raise from
    ``maintain_processor_list``.
    """


def should_request_mwt(
    alpha2: LanguageCode2, table: StanzaCapabilityTable | None
) -> bool:
    """Decide whether to request the ``mwt`` processor for ``alpha2``.

    Single source of truth: the Stanza capability table built from the
    installed catalog's ``resources.json`` (see ``_stanza_capabilities``).
    A previous hardcoded ``MWT_LANGS`` set drifted from the catalog and
    requested ``mwt`` for languages Stanza no longer ships it for (e.g.
    Swedish on Stanza 1.11), crashing the worker at bootstrap.

    Returns False when the table is unavailable: the conservative choice
    is to omit ``mwt`` and let Stanza tokenize/POS/lemma/depparse only,
    rather than guess and risk an ``UnsupportedProcessorError``.
    """
    if table is None:
        return False
    for cap in table.languages.values():
        if cap.alpha2 == alpha2:
            return cap.has_mwt
    return False


def iso3_to_alpha2(iso3: LanguageCode) -> LanguageCode2:
    """Convert ISO-639-3 language code to ISO-639-1 for Stanza.

    Batchalign uses ISO-639-3 broadly, but Stanza is configured with mostly
    ISO-639-1-style identifiers plus a few special cases. This function is
    the canonical bridge so the rest of the worker code does not embed ad
    hoc language-code fallbacks or guess at unsupported codes.

    Resolution order:

    1. **Special-case overrides** — for codes where ISO 639-3 disagrees with
       Stanza's own catalog labelling (e.g. ``yue``/``cmn`` both routing to
       Stanza ``zh``, ``nor``→``nb`` for Norwegian Bokmål as the default).
       These cases are *not* recoverable via pycountry because pycountry
       only encodes ISO-639's standard 1-to-1 mapping, which would point
       at codes Stanza does not ship.
    2. **pycountry** — for every other code with a standard ISO 639-1
       counterpart (``mar``→``mr``, ``swa``→``sw``, ...). This must be the
       fallback rather than a duplicate hardcoded dict, otherwise the
       hardcoded list inevitably drifts out of sync — Stanza adds a
       language, the capability table picks it up via pycountry, but
       ``iso3_to_alpha2`` returns the iso3 verbatim and ``stanza.Pipeline``
       crashes with "Language X is currently unsupported".
       (2026-05-06: Marathi ``mar`` failed exactly this way on
       ``childes-other-data/Biling/Gelman/Bystander/25.cha``.)
    3. **Pass-through with warning** — for genuinely unmapped codes
       (length-2 codes assumed to already be alpha-2; everything else
       hits the warning path).
    """
    # 1. Stanza-specific overrides — single source of truth shared with
    #    the capability-table builder. A second independent override dict
    #    here is the drift hazard that caused the 2026-05-06 ``mar``
    #    failure; do not reintroduce one.
    if iso3 in _ISO3_OVERRIDES:
        return _ISO3_OVERRIDES[iso3]

    # 2. pycountry for the standard ISO 639-3 ↔ ISO 639-1 cases.
    #    The capability table already uses pycountry; this keeps the two
    #    code paths honest about which languages they each understand.
    try:
        import pycountry

        lang = pycountry.languages.get(alpha_3=iso3)
        if lang is not None:
            alpha2 = getattr(lang, "alpha_2", None)
            if isinstance(alpha2, str) and alpha2:
                return alpha2
    except ImportError:
        # Fall through to the warning path.
        pass

    # 3. Already alpha-2? Pass through silently — common when callers feed a
    #    Stanza-style code straight in.
    if len(iso3) == 2:
        return iso3

    L.warning(
        "Unknown ISO-639-3 code %r - passing through unchanged for Stanza",
        iso3,
    )
    return iso3


def load_stanza_models(lang: LanguageCode) -> None:
    """Load Stanza morphosyntax models for one language.

    The resulting pipeline, tokenizer context, and lock are installed into the
    shared worker state so request handlers can do pure inference routing
    without rebuilding Stanza pipelines on every call.
    """
    import stanza
    from stanza import DownloadMethod

    from batchalign.inference._tokenizer_realign import (
        TokenizerContext,
        make_tokenizer_postprocessor,
    )

    # Preflight gate: consult the capability table BEFORE calling
    # stanza.Pipeline. The capability table is built from the installed
    # Stanza catalog (resources.json) and is the only source of truth
    # that stays correct across Stanza upgrades. Hardcoded lists
    # (Rust SUPPORTED_STANZA_CODES, the iso3_to_alpha2 mapping below)
    # have drifted multiple times and are now treated as advisory.
    # Without this gate, an unsupported language reaches stanza.Pipeline
    # which raises KeyError('packages') deep in maintain_processor_list —
    # the worker dies before emitting its ready signal, the daemon sees a
    # generic IPC error, and the user gets "transcription failed" with
    # the linguistic root cause buried in stderr.
    table = get_cached_capability_table()
    if table is None:
        # Post-2026-05-06: ``get_cached_capability_table()`` returns ``None``
        # ONLY when the Stanza Python package is missing from the worker
        # venv. The historical "resources.json missing" path now bootstraps
        # the catalog automatically and either returns a populated table or
        # raises ``StanzaCatalogDownloadError``. So if we reach here, it's a
        # genuine deploy-config error: BA3 was installed without Stanza.
        raise UnsupportedLanguageError(
            f"Cannot load Stanza for {lang!r}: the Stanza Python package "
            "is not installed in the worker environment. Reinstall "
            "batchalign3 with the morphosyntax extras enabled, or contact "
            "an operator to fix the deploy."
        )
    if not table.supports_morphosyntax(lang):
        sample = sorted(table.languages.keys())[:8]
        raise UnsupportedLanguageError(
            f"Stanza lacks the core morphosyntax processors for language {lang!r}. "
            f"It may appear in Stanza's resources.json as a stub or partial "
            f"entry, but no usable morphotag Pipeline can be built. "
            f"Languages with full morphosyntax support include: {sample} (and "
            f"{len(table.languages) - len(sample)} more)."
        )

    alpha2 = iso3_to_alpha2(lang)

    # MWT availability comes from Stanza's installed resources.json — never
    # from a hardcoded list. A stale list silently crashes the worker when
    # upstream drops a model (see the 2026-04-15 Swedish bootstrap failure).
    has_mwt = should_request_mwt(alpha2, table)
    processors = "tokenize,pos,lemma,depparse"
    if has_mwt:
        processors += ",mwt"

    ctx = TokenizerContext()
    lock = threading.Lock()

    # If the language pack for ``alpha2`` is not yet on disk, ``stanza.Pipeline``
    # will block while downloading several hundred MB of model files. Surface
    # that wait via the progress channel so every UI shows it to the user.
    _emit_stanza_lang_download_event_if_missing(lang, alpha2)

    # The Stanza pipeline shape varies by language because tokenization and MWT
    # support are not uniform across the supported languages.
    if alpha2 == "ja":
        nlp = stanza.Pipeline(
            lang=alpha2,
            processors=processors,
            download_method=DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
            package={
                "tokenize": "combined",
                "pos": "combined",
                "lemma": "combined",
                "depparse": "combined",
            },
        )
    elif not has_mwt:
        nlp = stanza.Pipeline(
            lang=alpha2,
            processors=processors,
            download_method=DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_pretokenized=True,
        )
    elif alpha2 == "en":
        nlp = stanza.Pipeline(
            lang=alpha2,
            processors=processors,
            download_method=DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_postprocessor=make_tokenizer_postprocessor(ctx, alpha2),
            package={"mwt": "gum"},
        )
    else:
        nlp = stanza.Pipeline(
            lang=alpha2,
            processors=processors,
            download_method=DownloadMethod.REUSE_RESOURCES,
            tokenize_no_ssplit=True,
            tokenize_postprocessor=make_tokenizer_postprocessor(ctx, alpha2),
        )

    # Preserve any pipelines already loaded for other languages in this worker.
    existing_pipelines = _state.stanza_pipelines or {}
    existing_contexts = _state.stanza_contexts or {}
    existing_pipelines[lang] = nlp
    existing_contexts[lang] = ctx
    _state.stanza_pipelines = existing_pipelines
    _state.stanza_contexts = existing_contexts
    _state.stanza_nlp_lock = lock

    try:
        _state.stanza_version = stanza.__version__
    except AttributeError:
        _state.stanza_version = "unknown"


def load_stanza_retokenize_model(lang: LanguageCode) -> None:
    """Lazy-load a Stanza pipeline with neural tokenization for Chinese retokenize.

    Unlike the default Chinese pipeline (which uses ``tokenize_pretokenized=True``),
    this variant lets Stanza's neural tokenizer segment the text into words.
    Used when ``--retokenize`` is requested for Mandarin (``cmn``/``zho``).

    The pipeline is stored under key ``"{lang}:retok"`` in worker state so it
    coexists with the standard pretokenized pipeline.
    """
    import stanza
    from stanza import DownloadMethod

    from batchalign.inference._tokenizer_realign import TokenizerContext

    alpha2 = iso3_to_alpha2(lang)
    if alpha2 != "zh":
        L.warning(
            "load_stanza_retokenize_model called for non-Chinese lang %s — skipping",
            lang,
        )
        return

    processors = "tokenize,pos,lemma,depparse"
    ctx = TokenizerContext()

    # The Mandarin retokenize pipeline uses the neural tokenizer (a separate
    # ~200 MB model from the ``tokenize_pretokenized`` variant). Surface the
    # wait if it's about to download.
    _emit_stanza_lang_download_event_if_missing(lang, alpha2)

    nlp = stanza.Pipeline(
        lang=alpha2,
        processors=processors,
        download_method=DownloadMethod.REUSE_RESOURCES,
        tokenize_no_ssplit=True,
        tokenize_pretokenized=False,
    )

    retok_key = f"{lang}:retok"
    existing_pipelines = _state.stanza_pipelines or {}
    existing_contexts = _state.stanza_contexts or {}
    existing_pipelines[retok_key] = nlp
    existing_contexts[retok_key] = ctx
    _state.stanza_pipelines = existing_pipelines
    _state.stanza_contexts = existing_contexts

    L.info("Loaded Stanza retokenize pipeline for %s (key=%s)", lang, retok_key)


def load_utseg_builder(lang: LanguageCode) -> None:
    """Load the utseg config builder for one primary language.

    Utterance segmentation uses a lighter-weight configuration boundary than
    morphosyntax. Instead of preloading full pipelines here, the worker stores a
    callable that can derive the necessary Stanza config bundle from a set of
    languages at inference time.
    """
    alpha2 = iso3_to_alpha2(lang)
    mwt_exclude = {"zh", "ja", "ko", "th", "vi", "my"}
    has_mwt = alpha2 not in mwt_exclude

    def build_stanza_config_from_langs(
        langs: list[str],
    ) -> tuple[list[str], dict[str, dict[str, str | bool]]]:
        """Build the Stanza config payload expected by utseg inference.

        Processor selection is per-language: only request processors that
        Stanza actually supports for each language (from the capability
        table). Languages without constituency get sentence-boundary
        segmentation instead.
        """
        from batchalign.worker._stanza_capabilities import get_cached_capability_table

        table = get_cached_capability_table()

        lang_alpha2: list[str] = []
        configs: dict[str, dict[str, str | bool]] = {}
        for language in langs:
            alpha2_code = iso3_to_alpha2(language)
            if alpha2_code == "zh":
                alpha2_code = "zh-hans"
            lang_alpha2.append(alpha2_code)

            processors: set[str] = {"tokenize", "pos", "lemma"}

            # Only add constituency if the language explicitly supports it.
            # When capability data is unavailable, prefer the safe
            # sentence-boundary fallback over guessing and crashing.
            lang_caps = table.languages.get(language) if table else None
            if lang_caps is not None and lang_caps.has_constituency:
                processors.add("constituency")

            # Only add MWT if the language supports it.
            if lang_caps is not None and lang_caps.has_mwt:
                processors.add("mwt")
            elif table is None and has_mwt:
                processors.add("mwt")

            configs[alpha2_code] = {
                "processors": ",".join(sorted(processors)),
                "tokenize_pretokenized": True,
            }
        return lang_alpha2, configs

    _state.utseg_config_builder = build_stanza_config_from_langs

    try:
        import stanza

        _state.utseg_version = stanza.__version__
    except (ImportError, AttributeError):
        _state.utseg_version = "unknown"


# ---------------------------------------------------------------------------
# Language-pack download notification helper.
# ---------------------------------------------------------------------------


def _emit_stanza_lang_download_event_if_missing(
    lang: LanguageCode, alpha2: LanguageCode2
) -> None:
    """Emit a user-visible event if Stanza needs to download ``alpha2``.

    Probes the configured Stanza model directory for the presence of any
    files under ``<model_dir>/<alpha2>/``. Absence implies ``stanza.Pipeline``
    will block on a multi-hundred-MB download. The notification surfaces
    that wait through the same progress channel UIs already render for
    model loading.

    Best-effort: if the probe fails for any reason, we emit anyway. False-
    positive notifications are a much smaller UX cost than silent waits.
    """
    import os

    from batchalign.worker._progress import emit_download_event

    is_present = False
    try:
        import stanza.resources.common as src

        lang_dir = os.path.join(src.DEFAULT_MODEL_DIR, alpha2)
        # Stanza scatters language packs across subdirectories named after
        # processor packages (e.g. ``en/tokenize/combined.pt``); presence of
        # *any* file under the language directory means at least some pack
        # has been seeded and the download is partial-or-done. The Pipeline
        # call may still pull a few small files; that's fine — the user has
        # already been informed via past events.
        if os.path.isdir(lang_dir):
            for _root, _dirs, files in os.walk(lang_dir):
                if files:
                    is_present = True
                    break
    except Exception as probe_exc:  # noqa: BLE001 — best effort
        L.debug("Stanza lang-pack probe failed for %s: %s", alpha2, probe_exc)

    if is_present:
        return

    emit_download_event(
        stage=f"downloading_stanza_lang_{alpha2}",
        user_message=(
            f"Downloading Stanza language pack for {lang} ({alpha2}) "
            "(one-time, ~250–500 MB; future runs will use the local cache)…"
        ),
    )


