"""CLI entry point for batchalign model training.

Invoked by Rust CLI: ``batchalign3 models train -- train ...``

Data preparation (CHAT→text) is handled by the Rust CLI:
``batchalign3 models prep`` — uses the Rust CHAT parser for correct extraction.
"""
from __future__ import annotations

import logging

import click

L = logging.getLogger("batchalign.models")


@click.group()  # type: ignore[untyped-decorator]  # click decorators use Callable[..., Any]
def cli() -> None:
    """Batchalign model training utilities."""
    logging.basicConfig(
        format="%(message)s",
        level=logging.ERROR,
        handlers=[logging.StreamHandler()],
    )
    L.setLevel(logging.DEBUG)


@cli.command()  # type: ignore[untyped-decorator]
@click.argument("run_name")  # type: ignore[untyped-decorator]
@click.argument("data_dir", type=click.Path(exists=True, file_okay=False))  # type: ignore[untyped-decorator]
@click.argument("model_dir", type=click.Path(file_okay=False))  # type: ignore[untyped-decorator]
@click.option("--lr", type=float, default=3.5e-5, show_default=True, help="Learning rate.")  # type: ignore[untyped-decorator]
@click.option("--batch-size", type=int, default=5, show_default=True, help="Batch size.")  # type: ignore[untyped-decorator]
@click.option("--epochs", type=int, default=2, show_default=True, help="Number of epochs.")  # type: ignore[untyped-decorator]
@click.option("--window", type=int, default=20, show_default=True, help="Utterance merge window.")  # type: ignore[untyped-decorator]
@click.option(  # type: ignore[untyped-decorator]
    "--min-length", type=int, default=10, show_default=True, help="Min utterance length."
)
@click.option(  # type: ignore[untyped-decorator]
    "--bert",
    type=str,
    default="bert-base-uncased",
    show_default=True,
    help="Base BERT model.",
)
@click.option("--wandb", is_flag=True, default=False, help="Enable wandb tracking.")  # type: ignore[untyped-decorator]
@click.option("--wandb-name", type=str, default=None, help="Wandb run name.")  # type: ignore[untyped-decorator]
@click.option("--wandb-user", type=str, default=None, help="Wandb entity.")  # type: ignore[untyped-decorator]
def train(
    run_name: str,
    data_dir: str,
    model_dir: str,
    lr: float,
    batch_size: int,
    epochs: int,
    window: int,
    min_length: int,
    bert: str,
    wandb: bool,
    wandb_name: str | None,
    wandb_user: str | None,
) -> None:
    """Train an utterance segmentation model.

    Expects prepared .train.txt and .val.txt files in DATA_DIR
    (produced by ``batchalign3 models prep``).
    """
    from batchalign.models.utterance.train import train_utterance_model

    train_utterance_model(
        run_name=run_name,
        data_dir=data_dir,
        model_dir=model_dir,
        lr=lr,
        batch_size=batch_size,
        epochs=epochs,
        window=window,
        min_length=min_length,
        bert_base=bert,
        use_wandb=wandb,
        wandb_name=wandb_name,
        wandb_user=wandb_user,
    )


if __name__ == "__main__":
    cli()
