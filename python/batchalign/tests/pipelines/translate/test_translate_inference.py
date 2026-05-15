"""Tests for the thin Python translation inference boundary."""

from __future__ import annotations

from collections.abc import Iterator

from batchalign.inference._domain_types import TranslationBackend
from batchalign.inference.translate import TranslateBatchItem, batch_infer_translate
from batchalign.providers import BatchInferRequest


def _monotonic_values(*values: float) -> Iterator[float]:
    """Yield deterministic monotonic values for one test."""

    yield from values


class TestTranslateModels:
    """Verify the typed translate wire models remain stable."""

    def test_translate_batch_item_roundtrip(self) -> None:
        item = TranslateBatchItem(text="hola")
        assert item.model_dump() == {"text": "hola"}
        assert TranslateBatchItem.model_validate(item.model_dump()) == item


class TestBatchInferTranslate:
    """Verify the thin Python translation adapter behavior."""

    def test_uses_request_language_and_rewrites_first_elapsed(self, monkeypatch) -> None:
        calls: list[tuple[str, str]] = []
        monotonic = _monotonic_values(100.0, 104.5)

        monkeypatch.setattr(
            "batchalign.inference.translate.time.monotonic",
            lambda: next(monotonic),
        )

        def translate_fn(text: str, src_lang: str) -> str:
            calls.append((text, src_lang))
            return text.upper()

        response = batch_infer_translate(
            BatchInferRequest(
                task="translate",
                lang="spa",
                items=[{"text": "hola"}, {"text": "adios"}],
            ),
            translate_fn,
            TranslationBackend.SEAMLESS,
        )

        assert calls == [("hola", "spa"), ("adios", "spa")]
        assert response.results[0].result == {"raw_translation": "HOLA"}
        assert response.results[0].elapsed_s == 4.5
        assert response.results[1].result == {"raw_translation": "ADIOS"}
        assert response.results[1].elapsed_s == 0.0

    def test_defaults_lang_to_eng_and_skips_blank_items(self, monkeypatch) -> None:
        calls: list[tuple[str, str]] = []
        monotonic = _monotonic_values(10.0, 12.0)

        monkeypatch.setattr(
            "batchalign.inference.translate.time.monotonic",
            lambda: next(monotonic),
        )

        def translate_fn(text: str, src_lang: str) -> str:
            calls.append((text, src_lang))
            return f"{src_lang}:{text}"

        response = batch_infer_translate(
            BatchInferRequest(
                task="translate",
                lang="",
                items=[{"text": "   "}, {"text": "hello"}],
            ),
            translate_fn,
            TranslationBackend.SEAMLESS,
        )

        assert calls == [("hello", "eng")]
        assert response.results[0].result == {"raw_translation": ""}
        assert response.results[0].elapsed_s == 2.0
        assert response.results[1].result == {"raw_translation": "eng:hello"}

    def test_reports_invalid_and_runtime_error_items_and_google_sleeps(self, monkeypatch) -> None:
        calls: list[tuple[str, str]] = []
        sleep_calls: list[float] = []
        monotonic = _monotonic_values(0.0, 3.0)

        monkeypatch.setattr(
            "batchalign.inference.translate.time.monotonic",
            lambda: next(monotonic),
        )
        monkeypatch.setattr(
            "batchalign.inference.translate.time.sleep",
            lambda seconds: sleep_calls.append(seconds),
        )

        def translate_fn(text: str, src_lang: str) -> str:
            calls.append((text, src_lang))
            if text == "boom":
                raise RuntimeError("translator exploded")
            return f"{src_lang}:{text.upper()}"

        response = batch_infer_translate(
            BatchInferRequest(
                task="translate",
                lang="yue",
                items=[
                    {"bad": "shape"},
                    {"text": "hello"},
                    {"text": "boom"},
                    {"text": "   "},
                ],
            ),
            translate_fn,
            TranslationBackend.GOOGLE,
        )

        assert calls == [("hello", "yue"), ("boom", "yue")]
        assert sleep_calls == [1.5, 1.5]
        assert response.results[0].error == "Invalid batch item"
        assert response.results[0].elapsed_s == 3.0
        assert response.results[1].result == {"raw_translation": "yue:HELLO"}
        assert response.results[2].error == "Translation failed: translator exploded"
        assert response.results[3].result == {"raw_translation": ""}

    def test_handles_empty_batches_without_touching_translation(self, monkeypatch) -> None:
        touched: list[str] = []
        monotonic = _monotonic_values(5.0, 5.0)

        monkeypatch.setattr(
            "batchalign.inference.translate.time.monotonic",
            lambda: next(monotonic),
        )

        def translate_fn(text: str, src_lang: str) -> str:
            touched.append(f"{src_lang}:{text}")
            return text

        response = batch_infer_translate(
            BatchInferRequest(task="translate", lang="eng", items=[]),
            translate_fn,
            TranslationBackend.SEAMLESS,
        )

        assert touched == []
        assert response.results == []
