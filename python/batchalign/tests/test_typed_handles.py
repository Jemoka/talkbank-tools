"""Tests for typed model handles — behavioral contracts only.

Verifies non-trivial logic in WhisperASRHandle (gen_kwargs branching,
callable forwarding) and WhisperFAHandle (instance-local monkey-patching).
Field-storage tests removed: dataclass fields storing what was passed in
is tested by Python itself, not us.
"""

from __future__ import annotations

from batchalign.inference.audio import bind_whisper_token_timestamp_extractor
from batchalign.inference.types import (
    WhisperASRHandle,
)


class TestWhisperASRHandle:
    """WhisperASRHandle replaces monkey-patched _ba_* attributes."""

    def test_callable_forwards_to_pipe(self) -> None:
        calls: list[tuple[str, dict[str, int | dict[str, str]]]] = []

        def fake_pipe(audio: str, **kwargs: int | dict[str, str]) -> dict[str, list[dict[str, str | tuple[float, float]]]]:
            calls.append((audio, kwargs))
            return {"chunks": [{"text": "hello", "timestamp": (0.0, 1.0)}]}

        handle = WhisperASRHandle(
            pipe=fake_pipe,
            config="cfg",
            lang="english",
            sample_rate=16000,
        )
        result = handle("audio_data", batch_size=1, generate_kwargs={"task": "transcribe"})
        assert len(calls) == 1
        assert result["chunks"][0]["text"] == "hello"  # type: ignore[index]

    def test_gen_kwargs_normal_language(self) -> None:
        handle = WhisperASRHandle(
            pipe=None,
            config="my_config",
            lang="english",
            sample_rate=16000,
        )
        kw = handle.gen_kwargs("english")
        assert kw["task"] == "transcribe"
        assert kw["language"] == "english"
        assert kw["generation_config"] == "my_config"

    def test_gen_kwargs_auto_omits_language(self) -> None:
        """When lang is ``"auto"``, Whisper should auto-detect — no ``language`` key."""
        handle = WhisperASRHandle(
            pipe=None,
            config="my_config",
            lang="auto",
            sample_rate=16000,
        )
        kw = handle.gen_kwargs("auto")
        assert "language" not in kw, "auto-detect must omit 'language' so Whisper detects it"
        assert kw["generation_config"] == "my_config"
        assert kw["repetition_penalty"] == 1.001

    def test_gen_kwargs_cantonese(self) -> None:
        handle = WhisperASRHandle(
            pipe=None,
            config="my_config",
            lang="Cantonese",
            sample_rate=16000,
        )
        kw = handle.gen_kwargs("Cantonese")
        assert "task" not in kw
        assert "language" not in kw
        assert kw["generation_config"] == "my_config"

    def test_gen_kwargs_skip_language_force_omits_task_and_language(self) -> None:
        """HuggingFace fine-tunes bake language/task into their own
        ``generation_config``; re-forcing them via ``generate_kwargs``
        produces gibberish. When ``skip_language_force=True``,
        ``gen_kwargs`` must omit ``task`` and ``language`` regardless of
        the requested language — the request language is otherwise
        authoritative in the V2 inference path
        (``infer_whisper_prepared_audio`` passes ``gen_kwargs(request_lang)``),
        so a handle-level flag is the only correct boundary for
        fine-tune behavior.
        """
        handle = WhisperASRHandle(
            pipe=None,
            config="ft_config",
            lang="auto",
            sample_rate=16000,
            skip_language_force=True,
        )
        # Request lang is concrete (malayalam), but the fine-tune flag
        # must suppress the task/language force on generate().
        kw = handle.gen_kwargs("malayalam")
        assert "task" not in kw, (
            "fine-tune handle must not pass task='transcribe' — the model's "
            "generation_config already pins task and re-forcing produces gibberish"
        )
        assert "language" not in kw, (
            "fine-tune handle must not pass a language hint — the model's "
            "generation_config already pins language"
        )

    def test_gen_kwargs_skip_language_force_also_omits_generation_config(
        self,
    ) -> None:
        """Fine-tunes come with their own ``generation_config`` baked into
        the checkpoint — things like language hints, suppress-token sets,
        beam / temperature settings tuned during training. The pipeline
        uses that by default.

        ``load_whisper_asr`` currently starts from
        ``GenerationConfig.from_pretrained(base)`` and applies
        ``no_repeat_ngram_size=4`` + ``use_cache=True`` on top. When
        ``generate_kwargs={"generation_config": <that>}`` is passed at
        ``generate()`` time, it **overrides** the checkpoint's own
        ``generation_config`` — including any settings the fine-tune
        author relied on. Empirically (2026-04-22, thennal/whisper-medium-ml
        on a Malayalam sample) this overrides produces cross-script
        gibberish with ``"letter"`` substrings and stray digits, while
        empty ``generate_kwargs`` produces clean 62-word folk-tale
        output.

        The invariant: when ``skip_language_force=True``, ``gen_kwargs``
        must return an empty dict. Any non-empty dict risks overriding
        part of the fine-tune's config and breaking the model.

        If we later need to add knobs for fine-tunes (e.g., a
        whisper_hub-specific ``repetition_penalty``), we should build
        them into a dedicated handle or a new mode — NOT by
        reintroducing overrides here.
        """
        handle = WhisperASRHandle(
            pipe=None,
            config="ft_config",
            lang="auto",
            sample_rate=16000,
            skip_language_force=True,
        )
        kw = handle.gen_kwargs("malayalam")
        assert kw == {"max_new_tokens": 444}, (
            "fine-tune handle must pass exactly {'max_new_tokens': 444}: "
            "no language/task/generation_config/repetition_penalty "
            "(those override the checkpoint's baked config and degrade "
            "output), and the runaway-generation cap (Whisper's "
            "max_target_positions=448 minus 3 special start tokens "
            "minus 1 margin)"
        )


class TestWhisperFAHandle:
    """WhisperFAHandle replaces (model, processor, sample_rate) tuple."""

    def test_timestamp_extractor_binding_stays_instance_local(self) -> None:
        """The Whisper workaround should patch one model instance, not a class."""

        class _FakeModel:
            """Small test double for an instance-bound Whisper override."""

            marker = "fake-model"

        model = _FakeModel()
        bind_whisper_token_timestamp_extractor(model)  # type: ignore[arg-type]

        bound = model._extract_token_timestamps  # type: ignore[attr-defined]
        assert bound.__self__ is model
