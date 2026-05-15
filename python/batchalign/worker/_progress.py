"""User-visible progress events for time-transparency UX.

Time transparency is a project-wide UX principle: any operation that takes
more than ~1 second must surface to every UI surface batchalign3 exposes
(CLI, TUI, desktop app, web dashboard). Silent waits are UX bugs — the user
must always know what BA3 is doing and roughly how long it'll take.

This module provides the small, consistent helper every long-running site
uses to emit such events. It builds on ``write_progress_event`` from
``_protocol`` and standardizes the user-facing wording for the most common
class of long wait: model downloads.

Why a separate module?
    Worker module imports of ``_protocol`` would create a circular
    dependency for any site that already imports ``_progress`` for emit
    helpers. Keeping a thin facade here lets every emit site call one
    function with the same shape.
"""

from __future__ import annotations

import logging
from collections.abc import Sequence
from typing import Optional

from batchalign.worker._protocol import write_progress_event

L = logging.getLogger(__name__)


def emit_download_event(
    stage: str,
    user_message: str,
    request_id: Optional[str] = None,
    size_bytes_estimate: Optional[int] = None,
) -> None:
    """Surface a download event to the user via the progress_v2 channel.

    Downloads are one-time costs paid on first use of a model. Users must
    see them so they don't mistake download wait time for batchalign3
    being slow. Pair every ``emit_download_event(<stage>_start)`` with a
    follow-up ``emit_download_event(<stage>_complete)`` so the UI clears
    the label after the work is done.

    Args:
        stage: Short, machine-readable identifier for the event stage,
            e.g. ``"downloading_stanza_catalog"`` or
            ``"downloading_hf_whisper_large"``. The Rust runner uses this
            to derive a default label when ``user_message`` isn't
            forwarded; tests may also assert on its value.
        user_message: Human-readable wording shown directly to the end
            user. Should convey: (1) what's downloading, (2) approximate
            size, (3) "one-time cost — future runs will be instant", (4)
            BA3 is not stuck. Example: ``"Downloading Whisper-large for
            ASR (one-time, ~3 GB; future runs will be instant)…"``.
        request_id: Optional V2 request id when the event happens during a
            request. ``None`` is acceptable for bootstrap-time downloads
            that occur before any request is being served.
        size_bytes_estimate: Optional estimated download size in bytes,
            for UI surfaces that want to render a progress bar. Best-
            effort; absent for catalogs and small files.

    Side effect:
        Writes a JSON line of the form
        ``{"op": "progress_v2", "event": {...}}`` to stdout, where it's
        consumed by the Rust runner's progress forwarder and then
        propagated to every UI channel.
    """
    # Use empty-string request_id when the caller doesn't have one. The Rust
    # side accepts request_id="" for daemon-bootstrap progress events that
    # are not bound to a particular V2 request.
    rid = request_id if request_id is not None else ""

    L.info("download event: %s — %s", stage, user_message)

    write_progress_event(
        request_id=rid,
        completed=0,
        total=size_bytes_estimate if size_bytes_estimate is not None else 0,
        stage=stage,
    )

    # Also log the user-facing message at INFO level so it appears in
    # daemon logs even when the Rust forwarder is not consuming events
    # (e.g., during worker startup before the runner is fully wired).
    # The structured log line lets log scrapers correlate events to wait
    # times.
    L.info("user-facing: %s", user_message)


# Approximate sizes for the largest HuggingFace models BA3 ships with — used
# only to enrich user messages ("~3 GB; future runs will be instant"). Real
# sizes vary; these are deliberately ballpark, not authoritative. If they
# drift far enough from reality to mislead users, update them — they are
# UX hints, not correctness-critical.
_HF_SIZE_HINTS_GB: dict[str, float] = {
    "openai/whisper-large-v3": 3.0,
    "openai/whisper-large-v2": 3.0,
    "openai/whisper-large": 3.0,
    "openai/whisper-medium": 1.5,
    "openai/whisper-small": 0.5,
    "openai/whisper-base": 0.15,
    "openai/whisper-tiny": 0.07,
    "talkbank/dia-fork": 0.5,
    "facebook/hf-seamless-m4t-medium": 2.4,
    "jonatasgrosman/wav2vec2-large-xlsr-53-english": 1.2,
}


def _hf_size_hint(model_id: str) -> str:
    """Return a human-friendly size hint for ``model_id``, or empty string."""
    gb = _HF_SIZE_HINTS_GB.get(model_id)
    if gb is None:
        return ""
    if gb < 1:
        return f", ~{int(gb * 1000)} MB"
    return f", ~{gb:g} GB"


# Default artifacts probed for HuggingFace models. ``config.json`` alone is
# the cheapest "is the model meaningfully cached?" indicator; loaders that
# also pull tokenizer/processor/generation-config artifacts should pass a
# wider set so a partial-cache state (model weights present, tokenizer
# config evicted) doesn't bypass the probe and produce a silent download.
_HF_DEFAULT_PROBE_ARTIFACTS: tuple[str, ...] = ("config.json",)

# Conventional artifact sets per loader family — useful presets so call
# sites don't have to remember which files HuggingFace hides where.
HF_ARTIFACTS_WHISPER: tuple[str, ...] = (
    "config.json",
    "generation_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
    "preprocessor_config.json",
)
HF_ARTIFACTS_BERT_TOKEN_CLASSIFICATION: tuple[str, ...] = (
    "config.json",
    "tokenizer.json",
    "tokenizer_config.json",
)
HF_ARTIFACTS_SEAMLESS: tuple[str, ...] = (
    "config.json",
    "preprocessor_config.json",
    "tokenizer.json",
    "tokenizer_config.json",
)


def emit_hf_download_if_missing(
    model_id: str,
    kind: str,
    request_id: Optional[str] = None,
    artifacts: Optional[Sequence[str]] = None,
) -> None:
    """If ``model_id`` is not fully in the HuggingFace cache, emit an event.

    Call this *immediately before* a ``from_pretrained()`` call so the user
    sees a download-starting message rather than a silent multi-minute wait.
    HuggingFace prints its own ``tqdm`` progress to stderr, but that does not
    reach BA3's protocol channel — so we have to surface the event ourselves.

    Cache probe is best-effort and **per-artifact**: if any one of the
    configured artifacts is missing, ``from_pretrained`` will fetch it and
    we want the user to see that. The default probe checks only
    ``config.json``, which catches the common "model fully missing" case;
    pass ``artifacts=`` to probe more files when the loader pulls multiple
    config/tokenizer/processor artifacts (see ``HF_ARTIFACTS_WHISPER`` etc.
    for ready-made presets).

    Why per-artifact? The pre-2026-05-06 single-file probe could miss a
    partial-cache state where one sub-file (e.g., ``tokenizer.json``) was
    evicted while the model weights remained — the user got a silent
    download anyway. Probing each artifact the loader will actually need
    closes that gap.

    Args:
        model_id: The HuggingFace model identifier
            (e.g. ``"openai/whisper-large-v3"``).
        kind: Short human-readable role for the model in BA3, used in the
            user message. Examples: ``"ASR"``, ``"forced alignment"``,
            ``"speaker diarization"``, ``"utterance boundary detection"``,
            ``"translation"``.
        request_id: Optional V2 request id when called during a request.
        artifacts: Optional list of filenames to probe in the HF cache.
            ``None`` falls back to ``_HF_DEFAULT_PROBE_ARTIFACTS``. Pass a
            wider set when the loader pulls multiple files.
    """
    probe_artifacts = tuple(artifacts) if artifacts else _HF_DEFAULT_PROBE_ARTIFACTS

    is_fully_cached = True
    try:
        from huggingface_hub import try_to_load_from_cache
        from huggingface_hub.constants import HF_HUB_CACHE

        for artifact in probe_artifacts:
            # ``try_to_load_from_cache`` returns the cached file path on
            # hit, or None on miss, or the special sentinel
            # ``_CACHED_NO_EXIST`` if a previous probe recorded the file
            # as nonexistent. Anything other than a real path counts as
            # "miss" — we let from_pretrained handle the actual fetch
            # semantics.
            cached = try_to_load_from_cache(
                repo_id=model_id, filename=artifact, cache_dir=HF_HUB_CACHE
            )
            if not isinstance(cached, str):
                is_fully_cached = False
                break
    except Exception as probe_exc:  # noqa: BLE001 — best effort
        L.debug("HF cache probe failed for %s: %s", model_id, probe_exc)
        # If the probe itself can't run, emit anyway — false-positive
        # notifications are a much smaller UX cost than silent waits.
        is_fully_cached = False

    if is_fully_cached:
        return

    size_hint = _hf_size_hint(model_id)
    emit_download_event(
        stage=f"downloading_hf_{model_id.replace('/', '_')}",
        user_message=(
            f"Downloading {model_id} for {kind} (one-time{size_hint}; "
            "future runs will use the local cache)…"
        ),
        request_id=request_id,
    )
