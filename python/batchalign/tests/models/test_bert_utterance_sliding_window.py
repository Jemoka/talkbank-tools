"""Tests for sliding-window inference in BertUtteranceModel.

Background: BertUtteranceModel.predict_actions originally tokenized
the full input in one shot and passed it to a model with
max_position_embeddings=512. For inputs that tokenize to more than 512
tokens (e.g., long Cantonese passages), this triggered a hard
tensor-shape RuntimeError that propagated through the worker protocol
and failed the entire job.

The fix: split the WordPiece-token sequence into overlapping windows
that each fit under max_position_embeddings, classify each window, and
average logits at overlap positions before argmax.

These tests use a FakePretrainedModel + FakeTokenizer pair that mimics
just enough of the HuggingFace AutoTokenizer + BertForTokenClassification
contract to exercise the sliding-window code path without downloading
hundreds of MB of model weights. The fakes deliberately enforce the
position-embedding constraint so a single-shot call on a long input
raises the same RuntimeError that the real model raises.
"""

from __future__ import annotations

from types import SimpleNamespace

import torch

from batchalign.models.utterance.infer import BertUtteranceModel


# ---------------------------------------------------------------------------
# Fake tokenizer + model that mimic the HF contract
# ---------------------------------------------------------------------------


class _FakeEncoding:
    """Mimics the HF tokenizer return value."""

    def __init__(
        self,
        input_ids: torch.Tensor,
        attention_mask: torch.Tensor,
        word_ids_list: list[int | None],
    ) -> None:
        self.input_ids = input_ids
        self.attention_mask = attention_mask
        self._word_ids = word_ids_list

    def word_ids(self, batch_idx: int = 0) -> list[int | None]:
        assert batch_idx == 0
        return self._word_ids

    def to(self, device: torch.device) -> "_FakeEncoding":
        return _FakeEncoding(
            input_ids=self.input_ids.to(device),
            attention_mask=self.attention_mask.to(device),
            word_ids_list=self._word_ids,
        )

    # Allow `**tokenized` to expand into model() call (input_ids, attention_mask).
    def keys(self):
        return ["input_ids", "attention_mask"]

    def __getitem__(self, key: str) -> torch.Tensor:
        if key == "input_ids":
            return self.input_ids
        if key == "attention_mask":
            return self.attention_mask
        raise KeyError(key)


class _FakeTokenizer:
    """Tokenizer that maps each input word to N WordPiece tokens
    deterministically (default: 1 token per word).
    """

    cls_token_id = 101
    sep_token_id = 102

    def __init__(self, tokens_per_word: int = 1) -> None:
        self.tokens_per_word = tokens_per_word

    def __call__(
        self,
        texts: list[list[str]],
        is_split_into_words: bool = False,
        return_tensors: str | None = None,
        add_special_tokens: bool = True,
    ) -> _FakeEncoding:
        assert is_split_into_words, "test fake supports is_split_into_words=True only"
        assert return_tensors == "pt", "test fake supports return_tensors='pt' only"
        assert len(texts) == 1, "test fake supports single-batch input only"

        words = texts[0]
        token_ids: list[int] = []
        word_ids: list[int | None] = []

        if add_special_tokens:
            token_ids.append(self.cls_token_id)
            word_ids.append(None)

        for word_idx, _word in enumerate(words):
            for _ in range(self.tokens_per_word):
                token_ids.append(1000 + word_idx)
                word_ids.append(word_idx)

        if add_special_tokens:
            token_ids.append(self.sep_token_id)
            word_ids.append(None)

        input_ids = torch.tensor([token_ids], dtype=torch.long)
        attention_mask = torch.ones_like(input_ids)
        return _FakeEncoding(input_ids, attention_mask, word_ids)


class _FakeModel:
    """Model that enforces max_position_embeddings.

    Returns logits indicating action 0 (no boundary) for ordinary tokens.
    """

    def __init__(self, max_position_embeddings: int = 512, num_labels: int = 6) -> None:
        self.config = SimpleNamespace(
            max_position_embeddings=max_position_embeddings,
            num_labels=num_labels,
        )

    def __call__(
        self,
        input_ids: torch.Tensor,
        attention_mask: torch.Tensor | None = None,  # noqa: ARG002 — required by HF signature
    ) -> SimpleNamespace:
        del attention_mask
        seq_len = input_ids.shape[1]
        if seq_len > self.config.max_position_embeddings:
            raise RuntimeError(
                f"The size of tensor a ({seq_len}) must match the size of "
                f"tensor b ({self.config.max_position_embeddings}) at "
                f"non-singleton dimension 1"
            )

        batch_size, length = input_ids.shape
        logits = torch.zeros((batch_size, length, self.config.num_labels))
        # All positions get action 0 with logit 1.0 (so argmax → 0).
        logits[:, :, 0] = 1.0
        return SimpleNamespace(logits=logits)


def _make_test_model(
    *,
    tokens_per_word: int = 1,
    max_position_embeddings: int = 512,
    num_labels: int = 6,
) -> BertUtteranceModel:
    """Construct a BertUtteranceModel with fakes substituted for tokenizer
    and model, bypassing the AutoTokenizer.from_pretrained network call.
    """
    instance = BertUtteranceModel.__new__(BertUtteranceModel)
    instance.model_name = "test-fake"
    instance.tokenizer = _FakeTokenizer(tokens_per_word=tokens_per_word)
    instance.model = _FakeModel(
        max_position_embeddings=max_position_embeddings,
        num_labels=num_labels,
    )
    instance.lang = None
    return instance


# ---------------------------------------------------------------------------
# Tests
# ---------------------------------------------------------------------------


class TestSingleShotShortInput:
    """Inputs that fit in one window must continue to work."""

    def test_short_input_returns_one_action_per_word(self) -> None:
        model = _make_test_model()
        actions = model.predict_actions(["I", "eat", "cookies", "right", "now"])
        assert actions == [0, 0, 0, 0, 0]

    def test_short_input_with_punctuation_normalized(self) -> None:
        model = _make_test_model()
        actions = model.predict_actions(["yes,", "I", "agree."])
        assert len(actions) == 3

    def test_empty_or_single_word_short_circuits(self) -> None:
        model = _make_test_model()
        assert model.predict_actions(["only"]) == [0]
        assert model.predict_actions([]) == []


class TestSlidingWindowLongInput:
    """The fix: long inputs must NOT crash and must return one action per word."""

    def test_long_input_does_not_raise(self) -> None:
        # 600 words with 1 token each = 600 inner tokens + 2 specials = 602.
        # Exceeds max_position_embeddings=512. Without the fix, single-shot
        # raises RuntimeError. With sliding-window, this must succeed.
        model = _make_test_model(tokens_per_word=1, max_position_embeddings=512)
        long_input = [f"w{i}" for i in range(600)]
        actions = model.predict_actions(long_input)
        assert len(actions) == 600

    def test_long_input_at_2x_max_returns_correct_length(self) -> None:
        # Multi-window iteration territory.
        model = _make_test_model(tokens_per_word=1, max_position_embeddings=512)
        long_input = [f"w{i}" for i in range(1000)]
        actions = model.predict_actions(long_input)
        assert len(actions) == 1000

    def test_long_cjk_style_input_does_not_raise(self) -> None:
        # CJK BERT models tokenize each character as one WordPiece token;
        # a 550-character Cantonese passage produces ~550 tokens. The MOST
        # corpus 41104c.cha 489-word *CHI: utterance is the empirical case
        # that motivated this test (real error: tensor a (551) must match
        # tensor b (512)).
        model = _make_test_model(tokens_per_word=1, max_position_embeddings=512)
        cjk_input = [f"c{i}" for i in range(550)]
        actions = model.predict_actions(cjk_input)
        assert len(actions) == 550


class TestRegressionOnExistingBehavior:
    """Sliding-window refactor must not change behavior on inputs that
    already worked."""

    def test_two_word_input_returns_two_actions(self) -> None:
        model = _make_test_model()
        actions = model.predict_actions(["hello", "world"])
        assert actions == [0, 0]

    def test_input_with_filler_words_lowercased(self) -> None:
        model = _make_test_model()
        actions = model.predict_actions(["Hello", "World", "yes"])
        assert len(actions) == 3
        assert all(a == 0 for a in actions)
