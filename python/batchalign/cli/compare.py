"""`compare` command — compare a main transcript against a gold reference.

Always tries to run morphosyntax (Stanza) on both transcripts before
the alignment so the `%xsmor:` tier carries real POS tags rather than
`?` placeholders (matches BA2's `compare` behaviour). The morphosyntax
runner short-circuits per-utterance if a `%mor:` tier is already
present, so pre-tagged input costs no inference.

If Stanza isn't installed we fall back to compare-only and emit one
warning line via the Interface log; the rest of the pipeline still
runs and `%xsrep:` is unaffected.
"""

from __future__ import annotations

import logging
from pathlib import Path

import typer

from ._common import CHAT_EXTENSIONS, _root_for, _walk, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


_log = logging.getLogger("batchalign.cli.compare")


def _pair_chat_folders(main: Path, gold: Path) -> tuple[list, Path]:
    """Walk `main` recursively and pair each `.cha` file with the same
    relative path under `gold`. Returns (inputs, main_root).
    """
    from batchalign.inputs import paired_from_paths

    main_root = _root_for(main)
    gold_root = _root_for(gold)
    inputs = []
    for src in _walk(main, CHAT_EXTENSIONS):
        rel = src.relative_to(main_root)
        gold_path = gold_root / rel
        if not gold_path.is_file():
            raise typer.BadParameter(
                f"no gold counterpart for {src}: expected {gold_path}",
            )
        inputs.append(paired_from_paths(str(src), str(gold_path), source_id=str(src)))
    return inputs, main_root


def register(app: typer.Typer) -> None:
    @app.command()
    def compare(
        ctx: typer.Context,
        main: Path = typer.Argument(
            ..., exists=True, help="Main (candidate) folder or `.cha` file."
        ),
        gold: Path = typer.Argument(
            ..., exists=True, help="Gold reference folder or `.cha` file (mirrors `main`'s structure)."
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each main file is overwritten in place.",
        ),
        language: str = typer.Option(
            "en",
            "--language",
            "-l",
            help="Stanza language code for morphosyntax (e.g. 'en', 'es', 'zh').",
        ),
    ) -> None:
        """Compare a main transcript against a gold reference.

        Pipeline: `[Morphosyntax (Stanza), Compare]`. Output is a CHAT file
        with `%xsrep:` / `%xsmor:` per utterance (BA2 format) plus a per-utt
        `%xcmp:` accuracy line and a `@Comment: ba.compare.summary:` header.
        """
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="compare",
            params={"lang": language},
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            # Try to construct a Stanza backend. If the user hasn't
            # installed `stanza` + its model deps, fall back to
            # compare-only — `%xsmor:` will be all `?`, but `%xsrep:`
            # is unaffected.
            try:
                stanza = ba.StanzaBackend(lang=language)
                pipeline = ba.recipes.compare(stanza_backend=stanza)
            except ImportError as exc:
                _log.warning(
                    "morphosyntax (Stanza) unavailable: %s — %%xsmor will be "
                    "all '?'. install with: pip install 'batchalign[stanza]'",
                    exc,
                )
                from batchalign._core import CompareBackend  # type: ignore[attr-defined]
                Task_ = ba.Task
                pipeline = ba.Pipeline(
                    tasks=[(Task_.Compare, {})],
                    backends=[CompareBackend()],
                )

            inputs, root = _pair_chat_folders(main, gold)
            for inp in inputs:
                ui.push(Task.from_input(inp))
            outcomes = list(ui.run_pipeline(pipeline, inputs))
            write_outcomes(outcomes, root, out)

        raise typer.Exit(code=ui.exit_code)
