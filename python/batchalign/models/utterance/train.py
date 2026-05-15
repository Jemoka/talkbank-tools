"""Training loop for utterance boundary segmentation model.

Fine-tunes a ``BertForTokenClassification`` to detect sentence boundaries
from unlabelled text.  Data files (``{run_name}.train.txt``,
``{run_name}.val.txt``) are prepared by the Rust CLI:
``batchalign3 models prep``.
"""

from __future__ import annotations

import logging
import os
import random

import torch
from torch.optim import AdamW
from torch.utils.data.dataloader import DataLoader
from tqdm import tqdm
from transformers import (
    AutoTokenizer,
    BertForTokenClassification,
    DataCollatorForTokenClassification,
)

from batchalign.models.utterance.dataset import TOKENS, UtteranceBoundaryDataset

L = logging.getLogger("batchalign.models")

DEVICE = torch.device("cuda") if torch.cuda.is_available() else torch.device("cpu")


def _move_dict(d: dict[str, torch.Tensor], device: torch.device) -> None:
    """Move every tensor in *d* to *device* in-place."""
    for key in d:
        d[key] = d[key].to(device)


def train_utterance_model(
    *,
    run_name: str,
    data_dir: str,
    model_dir: str,
    lr: float = 3.5e-5,
    batch_size: int = 5,
    epochs: int = 2,
    window: int = 20,
    min_length: int = 10,
    bert_base: str = "bert-base-uncased",
    use_wandb: bool = False,
    wandb_name: str | None = None,
    wandb_user: str | None = None,
) -> None:
    """Train an utterance segmentation model.

    Parameters
    ----------
    run_name:
        Name used for output directory and data file prefix.
    data_dir:
        Directory containing ``{run_name}.train.txt`` / ``.val.txt``.
    model_dir:
        Parent directory for saved model checkpoints.
    lr:
        Learning rate for AdamW.
    batch_size:
        Training batch size.
    epochs:
        Number of training epochs.
    window:
        Number of sentences merged per training example.
    min_length:
        Minimum character length to keep a sentence.
    bert_base:
        HuggingFace model name for the base BERT.
    use_wandb:
        Whether to log to Weights & Biases.
    wandb_name:
        W&B run display name (defaults to *run_name*).
    wandb_user:
        W&B entity / username.
    """
    output_path = os.path.join(model_dir, run_name)
    if os.path.exists(output_path):
        L.info("Path %s exists, skipping training.", output_path)
        return

    config: dict[str, object] = {
        "lr": lr,
        "batch_size": batch_size,
        "epochs": epochs,
        "window": window,
        "min_length": min_length,
        "bert_base": bert_base,
    }

    run = None
    if use_wandb:
        import wandb

        run = wandb.init(
            project="batchalign",
            name=wandb_name or run_name,
            entity=wandb_user,
            config=config,
        )
        config = dict(run.config)

    tokenizer = AutoTokenizer.from_pretrained(config["bert_base"])

    train_data = UtteranceBoundaryDataset(
        os.path.join(data_dir, f"{run_name}.train.txt"),
        tokenizer,
        window=config["window"],  # type: ignore[arg-type]  # config vals are object
        min_length=config["min_length"],  # type: ignore[arg-type]  # config vals are object
    )
    test_data = UtteranceBoundaryDataset(
        os.path.join(data_dir, f"{run_name}.val.txt"),
        tokenizer,
        window=config["window"],  # type: ignore[arg-type]  # config vals are object
        min_length=config["min_length"],  # type: ignore[arg-type]  # config vals are object
    )

    data_collator = DataCollatorForTokenClassification(tokenizer, return_tensors="pt")

    train_dataloader = DataLoader(
        train_data,
        batch_size=config["batch_size"],
        shuffle=True,
        collate_fn=lambda x: x,
    )
    test_dataloader = DataLoader(
        test_data,
        batch_size=config["batch_size"],
        shuffle=True,
        collate_fn=lambda x: x,
    )

    model = BertForTokenClassification.from_pretrained(
        config["bert_base"],
        num_labels=len(TOKENS),
    ).to(DEVICE)
    optim = AdamW(model.parameters(), lr=config["lr"])

    val_data = list(iter(test_dataloader))

    if run is not None:
        run.watch(model)

    for epoch in range(config["epochs"]):  # type: ignore[call-overload]  # config vals are object
        L.info("Training epoch %d", epoch)

        for indx, batch in tqdm(
            enumerate(iter(train_dataloader)), total=len(train_dataloader)
        ):
            batch = data_collator(batch)
            _move_dict(batch, DEVICE)

            output = model(**batch)
            output.loss.backward()
            optim.step()
            optim.zero_grad()

            if run is not None:
                run.log({"loss": output.loss.cpu().item()})

            if indx % 10 == 0 and val_data:
                val_batch = data_collator(random.choice(val_data))
                _move_dict(val_batch, DEVICE)
                output = model(**val_batch)
                if run is not None:
                    run.log({"val_loss": output.loss.cpu().item()})

    os.makedirs(output_path, exist_ok=True)
    model.save_pretrained(output_path)
    tokenizer.save_pretrained(output_path)
    L.info("Model saved to %s", output_path)
