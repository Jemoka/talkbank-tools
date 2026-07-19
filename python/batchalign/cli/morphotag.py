"""`morphotag` command — add `%mor` / `%gra` tiers via Stanza."""

from __future__ import annotations

import shutil
import tempfile
from pathlib import Path

import typer

from ._common import collect_chat_inputs, write_outcome
from ._options import cli_options
from .tui import Interface, Task


def _strip_existing_mor_gra(src: Path, dst: Path) -> None:
    """Copy `src` to `dst`, dropping any `%mor:` / `%gra:` tier lines.

    Used when --clear-existing is set (default) so a re-run of morphotag
    on a file that already has those tiers regenerates them from scratch
    instead of being skipped by the engine's "already tagged" guard.
    """
    dropping_continuations = False
    with src.open("r", encoding="utf-8") as fin, dst.open("w", encoding="utf-8") as fout:
        for line in fin:
            if line.startswith("\t") and dropping_continuations:
                continue
            dropping_continuations = False
            stripped = line.lstrip()
            if stripped.startswith("%mor:") or stripped.startswith("%gra:"):
                dropping_continuations = True
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

        # Tempdir is created INSIDE the `with Interface.open(...)` block but
        # cleaned up AFTER it exits. The TUI summary (which may try to read
        # offending lines out of these staged files for caret-block parse-
        # error rendering — see `cli/tui/errors.py::_try_parse_error`) runs
        # in `Interface.__exit__`. Cleaning up in an inner `finally` would
        # nuke the files before the summary could read them, so any
        # `error[E###]` line gets degraded to a plain string.
        tmpdir: Path | None = None
        try:
            with Interface.open(
                command="morphotag",
                params={
                    "retokenize": retokenize,
                    "clear_existing": clear_existing,
                },
                output=out,
                verbosity=opts.verbosity,
                plain=opts.plain,
                quiet=opts.quiet,
            ) as ui:
                # Language is resolved per-file from each CHAT's `@Languages:`
                # header by the Rust runner; the backend reads it off each
                # `MorphosyntaxInput` and loads the matching Stanza pipeline
                # lazily on first use.
                pipeline = ba.recipes.morphotag(
                    stanza_backend=ba.StanzaBackend(retokenize=retokenize),
                    workers=opts.workers,
                )
                inputs, root = collect_chat_inputs(folder)

                if clear_existing and inputs:
                    # Stage stripped copies in a temp dir; rewrite each input's
                    # path to point at the staged file. The engine writes back
                    # to the original source via source_id, so this is purely
                    # a pre-processing detour for the parser.
                    tmpdir = Path(tempfile.mkdtemp(prefix="batchalign-morphotag-"))
                    staged: list = []
                    for inp in inputs:
                        src = Path(inp.path)
                        staged_path = tmpdir / src.relative_to(root)
                        staged_path.parent.mkdir(parents=True, exist_ok=True)
                        _strip_existing_mor_gra(src, staged_path)
                        inp.path = str(staged_path)
                        staged.append(inp)
                    inputs = staged

                for inp in inputs:
                    ui.push(Task.from_input(inp))
                list(
                    ui.run_pipeline(
                        pipeline,
                        inputs,
                        on_outcome=lambda outcome: write_outcome(outcome, root, out),
                    )
                )
                exit_code = ui.exit_code
        finally:
            if tmpdir is not None:
                shutil.rmtree(tmpdir, ignore_errors=True)

        raise typer.Exit(code=exit_code)
