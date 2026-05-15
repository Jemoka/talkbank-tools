"""Tests for Cantonese particle pre-chunking in BertUtteranceModel.

Long Cantonese passages are split at sentence-final particles
(呀, 啦, 喎, 嘞, 㗎喇, 囉, 㗎, 啊, 嗯) before BERT classification.
Each chunk runs through the classifier independently; particles
are reliable boundary anchors in spoken Cantonese, so chunking
gives the model linguistically-grounded inputs and avoids the
BERT context window straddling natural sentence breaks.

The yue branch operates on a list[str] of words (not a single
passage string), matching the typed-input contract used elsewhere
in the model.
"""

from __future__ import annotations

from types import SimpleNamespace

import torch

from batchalign.models.utterance.infer import (
    BertUtteranceModel,
    _split_yue_at_particles,
)


# ---------------------------------------------------------------------------
# Fakes shared with the sliding-window tests, replicated locally so each
# test module is self-contained.
# ---------------------------------------------------------------------------


class _FakeEncoding:
    def __init__(self, input_ids, attention_mask, word_ids_list):
        self.input_ids = input_ids
        self.attention_mask = attention_mask
        self._word_ids = word_ids_list

    def word_ids(self, batch_idx=0):
        assert batch_idx == 0
        return self._word_ids

    def to(self, device):
        return _FakeEncoding(
            input_ids=self.input_ids.to(device),
            attention_mask=self.attention_mask.to(device),
            word_ids_list=self._word_ids,
        )

    def keys(self):
        return ["input_ids", "attention_mask"]

    def __getitem__(self, key):
        if key == "input_ids":
            return self.input_ids
        if key == "attention_mask":
            return self.attention_mask
        raise KeyError(key)


class _FakeTokenizer:
    cls_token_id = 101
    sep_token_id = 102

    def __init__(self):
        # Records the inputs the model saw, in order, so tests can
        # assert chunk-shape behavior.
        self.calls: list[list[str]] = []

    def __call__(
        self,
        texts,
        is_split_into_words=False,
        return_tensors=None,
        add_special_tokens=True,
    ):
        words = texts[0]
        self.calls.append(list(words))
        token_ids = []
        word_ids = []
        if add_special_tokens:
            token_ids.append(self.cls_token_id)
            word_ids.append(None)
        for word_idx in range(len(words)):
            token_ids.append(1000 + word_idx)
            word_ids.append(word_idx)
        if add_special_tokens:
            token_ids.append(self.sep_token_id)
            word_ids.append(None)
        input_ids = torch.tensor([token_ids], dtype=torch.long)
        attention_mask = torch.ones_like(input_ids)
        return _FakeEncoding(input_ids, attention_mask, word_ids)


class _FakeModel:
    def __init__(self, max_position_embeddings=512, num_labels=6):
        self.config = SimpleNamespace(
            max_position_embeddings=max_position_embeddings,
            num_labels=num_labels,
        )

    def __call__(self, input_ids, attention_mask=None):
        del attention_mask
        seq_len = input_ids.shape[1]
        if seq_len > self.config.max_position_embeddings:
            raise RuntimeError(
                f"The size of tensor a ({seq_len}) must match the size of "
                f"tensor b ({self.config.max_position_embeddings})"
            )
        batch_size, length = input_ids.shape
        logits = torch.zeros((batch_size, length, self.config.num_labels))
        logits[:, :, 0] = 1.0
        return SimpleNamespace(logits=logits)


def _make_model(*, lang: str) -> BertUtteranceModel:
    instance = BertUtteranceModel.__new__(BertUtteranceModel)
    instance.model_name = "test-fake"
    instance.lang = lang
    instance.tokenizer = _FakeTokenizer()
    instance.model = _FakeModel()
    return instance


# ---------------------------------------------------------------------------
# Pure-function tests of _split_yue_at_particles
# ---------------------------------------------------------------------------


class TestSplitYueAtParticles:
    """Unit tests for the chunking helper."""

    def test_empty_input_returns_no_chunks(self):
        assert _split_yue_at_particles([]) == []

    def test_no_particles_yields_single_chunk(self):
        words = ["我", "係", "好"]
        assert _split_yue_at_particles(words) == [(0, 3)]

    def test_single_particle_at_end_yields_one_chunk(self):
        words = ["我", "係", "好", "啊"]
        # Particle is included in the chunk; one chunk total.
        assert _split_yue_at_particles(words) == [(0, 4)]

    def test_single_particle_in_middle_yields_two_chunks(self):
        words = ["我", "係", "好", "啊", "今日", "好", "天"]
        # First chunk includes the particle (boundary marker).
        assert _split_yue_at_particles(words) == [(0, 4), (4, 7)]

    def test_multi_char_particle_recognized(self):
        # 㗎喇 is two characters → two word entries → one particle.
        words = ["佢", "走", "咗", "㗎", "喇", "我", "知"]
        assert _split_yue_at_particles(words) == [(0, 5), (5, 7)]

    def test_multi_char_particle_does_not_match_partial(self):
        # 㗎 alone (without 喇) is also a particle. Two-char must
        # take precedence over the standalone 㗎.
        words = ["佢", "走", "咗", "㗎", "喇"]
        # Should match 㗎喇 as a single particle, not 㗎 alone.
        assert _split_yue_at_particles(words) == [(0, 5)]

    def test_consecutive_particles_split_correctly(self):
        words = ["啊", "啦", "好"]
        # Both 啊 and 啦 are particles; each ends its own chunk.
        assert _split_yue_at_particles(words) == [(0, 1), (1, 2), (2, 3)]

    def test_particle_at_start_makes_first_chunk_just_the_particle(self):
        words = ["啊", "我", "係", "好"]
        assert _split_yue_at_particles(words) == [(0, 1), (1, 4)]

    def test_all_particles(self):
        words = ["呀", "啦", "喎", "嘞", "囉", "啊", "嗯"]
        # Each particle is its own chunk.
        assert _split_yue_at_particles(words) == [
            (0, 1), (1, 2), (2, 3), (3, 4), (4, 5), (5, 6), (6, 7),
        ]


# ---------------------------------------------------------------------------
# Integration tests: the yue branch in BertUtteranceModel actually
# uses the chunking helper; the eng/zho branches do not.
# ---------------------------------------------------------------------------


class TestYueBranchChunksAtParticles:
    """The yue model dispatches each chunk independently."""

    def test_yue_model_chunks_at_particle(self):
        model = _make_model(lang="yue")
        words = ["我", "係", "好", "啊", "今日", "好", "天"]
        actions = model.predict_actions(words)
        assert len(actions) == 7
        # The fake tokenizer records each model call's word list.
        # Two chunks expected: ['我','係','好','啊'] and ['今日','好','天'].
        assert model.tokenizer.calls == [
            ["我", "係", "好", "啊"],
            ["今日", "好", "天"],
        ]

    def test_yue_model_with_no_particles_runs_one_chunk(self):
        model = _make_model(lang="yue")
        words = ["我", "係", "好", "今日"]
        actions = model.predict_actions(words)
        assert len(actions) == 4
        assert model.tokenizer.calls == [["我", "係", "好", "今日"]]

    def test_yue_model_with_multi_char_particle(self):
        model = _make_model(lang="yue")
        words = ["佢", "走", "咗", "㗎", "喇", "我", "知"]
        actions = model.predict_actions(words)
        assert len(actions) == 7
        assert model.tokenizer.calls == [
            ["佢", "走", "咗", "㗎", "喇"],
            ["我", "知"],
        ]


class TestNonYueBranchDoesNotChunk:
    """The eng/zho models do NOT chunk at Cantonese particles, even
    if the same characters happen to appear."""

    def test_eng_model_does_not_chunk_at_yue_particles(self):
        model = _make_model(lang="eng")
        # Same characters that would chunk under yue — should not chunk here.
        words = ["我", "係", "好", "啊", "今日", "好", "天"]
        actions = model.predict_actions(words)
        assert len(actions) == 7
        # One model call: full input.
        assert model.tokenizer.calls == [
            ["我", "係", "好", "啊", "今日", "好", "天"],
        ]

    def test_zho_model_does_not_chunk(self):
        model = _make_model(lang="zho")
        words = ["他", "走", "了", "我", "知道"]
        actions = model.predict_actions(words)
        assert len(actions) == 5
        assert model.tokenizer.calls == [["他", "走", "了", "我", "知道"]]


