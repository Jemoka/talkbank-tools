"""`compare` command — compare each transcript against a gold template.

Gold lookup mirrors BA2 (`dispatch.py`), but lives in the SAME input folder
(parity.md): for each `FILE.cha` the gold is, in order of precedence,

  1. `FILE.gold.cha`     — a per-file gold beside the transcript, else
  2. `template.gold.cha` — one shared template for everything in the folder.

`*.gold.cha` files are themselves skipped as inputs. This replaces the old
parallel-gold-folder pairing (one folder of templates mirroring the inputs),
which parity.md calls out as the wrong structure.

Always runs morphosyntax (Stanza) on both transcripts before the alignment so
the `%xsmor:` tier carries real POS tags. The morphosyntax runner short-
circuits per-utterance when a `%mor:` tier is already present, so pre-tagged
input costs no inference. If Stanza isn't installed we fall back to
compare-only and emit one warning line.
"""

from __future__ import annotations

import logging
from pathlib import Path

import typer

from ._common import CHAT_EXTENSIONS, _root_for, _walk, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


_log = logging.getLogger("batchalign.cli.compare")

GOLD_SUFFIX = ".gold.cha"
TEMPLATE_GOLD = "template.gold.cha"


def _find_gold(main: Path) -> Path | None:
    """Resolve the gold transcript for `main` (BA2 precedence).

    `FILE.gold.cha` beside the file wins; otherwise the folder-wide
    `template.gold.cha`. Returns `None` if neither exists.
    """
    per_file = main.parent / (main.stem + GOLD_SUFFIX)
    if per_file.is_file():
        return per_file
    template = main.parent / TEMPLATE_GOLD
    if template.is_file():
        return template
    return None


def _pair_folder_with_gold(folder: Path) -> tuple[list, Path]:
    """Walk `folder` for `.cha` inputs and pair each with its gold template.

    Returns `(paired_inputs, root)`. Skips any `*.gold.cha` (those are golds,
    not inputs). Raises if an input has no gold.
    """
    from batchalign.inputs import paired_from_paths

    root = _root_for(folder)
    inputs = []
    for src in _walk(folder, CHAT_EXTENSIONS):
        if src.name.endswith(GOLD_SUFFIX):
            continue
        gold = _find_gold(src)
        if gold is None:
            raise typer.BadParameter(
                f"no gold for {src}: expected a sibling {src.stem}{GOLD_SUFFIX} "
                f"or a {TEMPLATE_GOLD} in {src.parent}"
            )
        inputs.append(paired_from_paths(str(src), str(gold), source_id=str(src)))
    if not inputs:
        raise typer.BadParameter(
            f"no transcripts to compare in {folder} (only gold files, or empty)"
        )
    return inputs, root


def register(app: typer.Typer) -> None:
    @app.command()
    def compare(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder of `.cha` transcripts to compare. The gold is a sibling "
            "`FILE.gold.cha` or a shared `template.gold.cha` in the same folder.",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each transcript is overwritten in place.",
        ),
    ) -> None:
        """Compare each transcript in FOLDER against its gold template.

        Pipeline: `[Morphosyntax (Stanza), Compare]`. Output is a CHAT file
        with the compare tiers per utterance plus a `@Comment` summary header.
        """
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="compare",
            params={},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            # Language is resolved per-file from each CHAT's `@Languages:`
            # header by the Rust runner (see morphotag for the same pattern);
            # the backend reads it off each `MorphosyntaxInput` and loads the
            # matching Stanza pipeline lazily on first use, so we don't pass
            # `lang=` here.
            try:
                stanza = ba.StanzaBackend()
            except ImportError as exc:
                _log.warning(
                    "morphosyntax (Stanza) unavailable: %s — compare POS tiers "
                    "will be placeholders. install with: pip install 'batchalign[stanza]'",
                    exc,
                )
                stanza = None
            pipeline = ba.recipes.compare(stanza_backend=stanza)

            inputs, root = _pair_folder_with_gold(folder)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
