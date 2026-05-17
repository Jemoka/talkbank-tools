"""`compare` command — compare a main transcript against a gold reference.

Always runs morphosyntax (Stanza) on both transcripts before the alignment
so the `%xsmor:` tier carries real POS tags rather than `?` placeholders
(matches BA2's `compare` behaviour). The morphosyntax runner short-circuits
per-utterance if a `%mor:` tier is already present, so pre-tagged input
costs no inference.
"""

from __future__ import annotations

from pathlib import Path

import typer

from ._common import import_ba, write_outcomes


def register(app: typer.Typer) -> None:
    @app.command()
    def compare(
        main: Path = typer.Argument(
            ..., exists=True, help="Main (candidate) transcript (.cha file)."
        ),
        gold: Path = typer.Argument(
            ..., exists=True, help="Gold reference transcript (.cha file)."
        ),
        out: Path = typer.Option(..., "--out", "-o", help="Output directory."),
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
        ba = import_ba()
        from batchalign.inputs import paired_from_paths

        # Try to construct a Stanza backend. If the user hasn't installed
        # `stanza` + its model deps, fall back to compare-only — `%xsmor:`
        # will be all `?`, but `%xsrep:` is unaffected. Tell the user the
        # exact command to flip on full BA2-style POS output.
        try:
            stanza = ba.StanzaBackend(lang=language)
            pipeline = ba.recipes.compare(stanza_backend=stanza)
        except ImportError as exc:
            typer.echo(
                "warning: morphosyntax (Stanza) unavailable — %xsmor will be "
                "all '?'. Install with `pip install 'batchalign[stanza]'` "
                f"or `uv pip install stanza transformers`. ({exc})",
                err=True,
            )
            # Compare-only path: skip Morphosyntax in the task chain. The
            # Compare backend still emits `%xsrep:`, just with `?` POS.
            from batchalign._core import CompareBackend  # type: ignore[attr-defined]
            Task = ba.Task
            pipeline = ba.Pipeline(
                tasks=[(Task.Compare, {})],
                backends=[CompareBackend()],
            )

        outcomes = pipeline.run([paired_from_paths(str(main), str(gold))])
        write_outcomes(outcomes, out)
