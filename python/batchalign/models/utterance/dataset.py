"""Utterance boundary detection dataset for BERT fine-tuning."""

from __future__ import annotations

import random
import re
from typing import TYPE_CHECKING, Any

from torch.utils.data import dataset
from transformers import AutoTokenizer

if TYPE_CHECKING:
    import torch

# Token labels for utterance boundary classification.
TOKENS: dict[str, int] = {
    "U": 0,   # normal word
    "OC": 1,  # first letter capitalized (sentence onset)
    "E.": 2,  # period (sentence boundary)
    "E?": 3,  # question mark
    "E!": 4,  # exclamation mark
    "E,": 5,  # comma
}

# Label indices that represent sentence boundaries.
BOUNDARIES: list[int] = [2, 3, 4]


class UtteranceBoundaryDataset(dataset.Dataset):  # type: ignore[misc]  # Dataset base is Any
    """Dataset that windows raw sentences and produces token-classified inputs.

    Each ``__getitem__`` call merges a random window of consecutive sentences,
    labels each word with a token from :data:`TOKENS`, and tokenises via a
    HuggingFace tokenizer.  Sub-word pieces that are not the first piece of a
    word receive the ignore label ``-100``.
    """

    raw_data: list[str]
    max_length: int
    tokenizer: AutoTokenizer
    window: int
    min_length: int

    def __init__(
        self,
        f: str,
        tokenizer: AutoTokenizer,
        window: int = 10,
        max_length: int = 1000,
        min_length: int = 10,
    ) -> None:
        with open(f) as df:
            d = df.readlines()
        self.raw_data = [i.strip() for i in d]
        self.window = window
        self.max_length = max_length
        self.min_length = min_length
        self.tokenizer = tokenizer

    def __call__(self, passage: str) -> dict[str, Any]:
        """Tokenise *passage* and generate per-token labels."""
        tokenizer = self.tokenizer

        sentence_raw = re.sub(r" ?(\W)", r"\1", passage)
        sentence_tokenized = sentence_raw.split(" ")

        labels: list[int] = []
        for word in sentence_tokenized:
            if word[0].isupper():
                labels.append(TOKENS["OC"])
            elif word[-1] in [".", "?", "!", ","]:
                labels.append(TOKENS[f"E{word[-1]}"])
            else:
                labels.append(TOKENS["U"])

        sentence_tokenized = [re.sub(r"[.?!,]", r"", i) for i in sentence_tokenized]

        tokenized = tokenizer(
            sentence_tokenized,
            truncation=True,
            is_split_into_words=True,
            max_length=self.max_length,
        )

        final_labels: list[int] = []
        prev_word_idx: int | None = None
        for elem in tokenized.word_ids(0):
            if elem is None:
                final_labels.append(-100)
            elif elem != prev_word_idx:
                final_labels.append(labels[elem])
                prev_word_idx = elem
            else:
                final_labels.append(-100)

        tokenized["labels"] = final_labels
        return tokenized  # type: ignore[no-any-return]  # tokenizer returns Any

    def __getitem__(self, index: int) -> dict[str, Any]:
        sents = self.raw_data[
            index * self.window : index * self.window + random.randint(1, self.window)
        ]
        sents = [i for i in sents if len(i) >= self.min_length]
        if len(sents) == 0:
            return self[index + 1] if index < len(self) - 1 else self[index - 1]
        return self(" ".join(sents))

    def __len__(self) -> int:
        return len(self.raw_data) // self.window


def calculate_acc_prec_rec_f1(
    preds: "torch.Tensor", labs: "torch.Tensor"
) -> tuple[float, float, float, float]:
    """Calculate accuracy, precision, recall, and F1 on sentence boundaries.

    Labels of ``-100`` are ignored (sub-word padding).
    """
    import torch  # noqa: F811 — lazy for top-level import cost

    tp = 0
    fp = 0
    fn = 0

    boundaries = labs.clone().apply_(lambda x: x in BOUNDARIES)
    boundaries = (boundaries == 1).nonzero().tolist()

    boundaries_hat = preds.clone().apply_(lambda x: x in BOUNDARIES)
    boundaries_hat = (boundaries_hat == 1).nonzero().tolist()

    boundaries_hat = list(
        filter(lambda x: labs[x[0]][x[1]] != -100, boundaries_hat)
    )

    for elem in boundaries_hat:
        if elem in boundaries:
            tp += 1
        else:
            fp += 1
    for elem in boundaries:
        if elem not in boundaries_hat:
            fn += 1

    count = len((labs != -100).nonzero())
    tn = count - (tp + fp + fn)

    acc = (tp + tn) / count
    prec = tp / (tp + fp) if (tp + fp) > 0 else 0.0
    recc = tp / (tp + fn) if (tp + fn) > 0 else 0.0
    f1 = 2 * ((prec * recc) / (prec + recc)) if (prec + recc) > 0 else 0.0

    return acc, prec, recc, f1
