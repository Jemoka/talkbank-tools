"""`morphotag` command — add `%mor` / `%gra` tiers via Stanza."""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import typer

from ._common import collect_chat_inputs, write_outcomes
from ._options import cli_options
from .tui import Interface, Task


def _strip_existing_mor_gra(src: Path, dst: Path) -> None:
    """Copy `src` to `dst`, dropping any `%mor:` / `%gra:` tier lines.

    Used when --clear-existing is set (default) so a re-run of morphotag
    on a file that already has those tiers regenerates them from scratch
    instead of being skipped by the engine's "already tagged" guard.
    """
    with src.open("r", encoding="utf-8") as fin, dst.open("w", encoding="utf-8") as fout:
        for line in fin:
            stripped = line.lstrip()
            if stripped.startswith("%mor:") or stripped.startswith("%gra:"):
                # Multi-line tiers (continuation lines start with tab); drop
                # only the header. The engine re-injects whole tiers.
                continue
            fout.write(line)


def register(app: typer.Typer) -> None:
    @app.command()
    def morphotag(
        ctx: typer.Context,
        folder: Path = typer.Argument(
            ...,
            exists=True,
            help="Folder to walk recursively for CHAT files (single file also accepted).",
        ),
        out: Path | None = typer.Option(
            None,
            "--out",
            "-o",
            help="Optional output folder; if omitted, each source file is overwritten in place.",
        ),
        language: str = typer.Option("en", "--language", help="Stanza language code."),
        retokenize: bool = typer.Option(False, "--retokenize/--no-retokenize"),
        clear_existing: bool = typer.Option(
            True,
            "--clear-existing/--keep-existing",
            help=(
                "If true (default), drop any pre-existing %mor:/%gra: tiers "
                "from each input before tagging so re-runs regenerate. Use "
                "--keep-existing to preserve them and let the engine skip "
                "already-tagged utterances."
            ),
        ),
    ) -> None:
        """Add `%mor` and `%gra` tiers via Stanza."""
        import batchalign as ba

        opts = cli_options(ctx)

        with Interface.open(
            command="morphotag",
            params={
                "lang": language,
                "retokenize": retokenize,
                "clear_existing": clear_existing,
            },
            output=out,
            verbosity=opts.verbosity,
            plain=opts.plain,
            quiet=opts.quiet,
        ) as ui:
            pipeline = ba.recipes.morphotag(
                stanza_backend=ba.StanzaBackend(lang=language, retokenize=retokenize),
            )
            inputs, root = collect_chat_inputs(folder)

            tmpdir: Path | None = None
            if clear_existing and inputs:
                # Stage stripped copies in a temp dir; rewrite each input's
                # path to point at the staged file. The engine writes back
                # to the original source via source_id, so this is purely
                # a pre-processing detour for the parser.
                tmpdir = Path(tempfile.mkdtemp(prefix="batchalign-morphotag-"))
                staged: list = []
                for inp in inputs:
                    src = Path(inp.path)
                    stem = src.stem
                    staged_path = tmpdir / f"{stem}.cha"
                    _strip_existing_mor_gra(src, staged_path)
                    inp.path = str(staged_path)
                    staged.append(inp)
                inputs = staged

            try:
                for inp in inputs:
                    ui.push(Task.from_input(inp))
                outcomes = list(ui.run_pipeline(pipeline, inputs))
                write_outcomes(outcomes, root, out)
            finally:
                if tmpdir is not None:
                    shutil.rmtree(tmpdir, ignore_errors=True)

        raise typer.Exit(code=ui.exit_code)
