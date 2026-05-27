#!/usr/bin/env python3
"""Prove Batchalign3 ↔ Batchalign2 parity, one command/engine/language at a time.

This is the acceptance gate for the parity effort (see `parity.md`). For each
cell in `MATRIX` it:

  1. runs the SAME input through Batchalign2 (BA2) and Batchalign3 (BA3),
  2. extracts the command's "important lines" from each output transcript
     (speaker + utterance segmentation, `%mor`, `%gra`, `%wor`, compare tiers —
     the lines `parity.md` says must match; headers and other cosmetics are
     ignored), and
  3. diffs them and prints PASS / FAIL.

Why a script and not a pytest: it shells out to two whole CLIs living in two
different virtualenvs, runs real models, and is meant to be *read* by a human
reviewer as the proof that parity holds. It is intentionally linear and
explicit. The hermetic unit tests live next to the code
(`python/batchalign/tests/`); this is the end-to-end cross-engine check.

Engines that need credentials/models read them exactly like BA2 does, from
`~/.batchalign.ini`.

Usage:
    python scripts/parity/prove_parity.py                  # run everything
    python scripts/parity/prove_parity.py --command morphotag
    python scripts/parity/prove_parity.py --command morphotag --language en
    python scripts/parity/prove_parity.py --list           # list the matrix
    python scripts/parity/prove_parity.py -v               # show diffs even on pass
"""

from __future__ import annotations

import argparse
import dataclasses
import difflib
import re
import shutil
import subprocess
import sys
import tempfile
from dataclasses import dataclass, field
from pathlib import Path

# --- locations -------------------------------------------------------------

HERE = Path(__file__).resolve().parent
REPO = HERE.parent.parent                       # talkbank-tools/
FIXTURES = HERE / "fixtures"
BA2_DIR = Path("/Users/houjun/Documents/Projects/batchalign2")
BA2_PY = BA2_DIR / ".venv" / "bin" / "python"


# --- the matrix ------------------------------------------------------------


@dataclass
class Cell:
    """One parity check: a command, with an engine + language, on one fixture.

    `ba2` / `ba3` are the CLI option lists each engine needs to select this
    engine + language. `kind` selects the important-line extractor.
    """

    command: str                       # morphotag | transcribe | align | compare | translate
    engine: str                        # stanza | rev | whisperx | whisper | openai | google
    language: str                      # en | zh | yue | spa | jpn | ...
    fixture: str                       # path under fixtures/, relative
    kind: str                          # extractor key (see EXTRACTORS)
    ba2: list[str] = field(default_factory=list)
    ba3: list[str] = field(default_factory=list)

    @property
    def name(self) -> str:
        return f"{self.command}/{self.engine}/{self.language}"


MATRIX: list[Cell] = [
    # ---- morphotag (Stanza; deterministic, no audio) ----
    Cell(
        command="morphotag",
        engine="stanza",
        language="en",
        fixture="morphotag/en.cha",
        kind="morphotag",
        # BA2 reads language from the @Languages header; force a fresh compute.
        ba2=["--override-cache"],
        ba3=["--language", "en"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="en-terminators",
        fixture="morphotag/en_terminators.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "en"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="en-contraction",
        fixture="morphotag/en_contraction.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "en"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="es",
        fixture="morphotag/es.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "es"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="ja",
        fixture="morphotag/ja.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "ja"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="zh",
        fixture="morphotag/zh.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "zh"],
    ),
    Cell(
        command="morphotag",
        engine="stanza",
        language="codeswitch-en-es",
        fixture="morphotag/codeswitch_en_es.cha",
        kind="morphotag",
        ba2=["--override-cache"],
        ba3=["--language", "en,es"],
    ),
    # ---- translate (Google free tier; deterministic given the same input) ----
    Cell(
        command="translate",
        engine="google",
        language="es-eng",
        fixture="translate/es.cha",
        kind="translate",
        ba2=[],
        ba3=["--target", "eng"],
    ),
]


# --- important-line extractors --------------------------------------------
#
# Each returns a list[str] of normalized lines that MUST match between
# engines. Cosmetic differences (tabs vs spaces, header lines) are dropped.


def _norm(line: str) -> str:
    """Collapse whitespace so tab-vs-space cosmetics don't cause false diffs."""
    return re.sub(r"\s+", " ", line).strip()


def _tier_lines(cha: str, labels: tuple[str, ...]) -> list[str]:
    """Return normalized main (`*`) and selected dependent (`%label:`) lines
    in document order. `labels` is the set of dependent tiers that matter."""
    out: list[str] = []
    for raw in cha.splitlines():
        line = raw.rstrip("\n")
        if line.startswith("*"):
            out.append(_norm(line))
        elif line.startswith("%"):
            label = line.split(":", 1)[0][1:]
            if label in labels:
                out.append(_norm(line))
    return out


def extract_morphotag(cha: str) -> list[str]:
    """Speaker/utterance segmentation (`*`) + `%mor` + `%gra` — the morphotag
    important lines from `parity.md`."""
    return _tier_lines(cha, ("mor", "gra"))


def extract_segmentation(cha: str) -> list[str]:
    """Speaker + per-utterance segmentation only (transcribe): the `*SPK:` lines."""
    return [_norm(l) for l in cha.splitlines() if l.startswith("*")]


def extract_wor(cha: str) -> list[str]:
    """`%wor` word-level alignment lines (align)."""
    return _tier_lines(cha, ("wor",))


def extract_compare(cha: str) -> list[str]:
    """Compare tiers (`%xmor`/`%xsmor`/`%xsrep`/`%xcmp`)."""
    return _tier_lines(cha, ("xmor", "xsmor", "xsrep", "xcmp"))


def extract_translate(cha: str) -> list[str]:
    """Translation tiers (`%xtra`/`%eng`/`%tra`)."""
    return _tier_lines(cha, ("xtra", "eng", "tra"))


EXTRACTORS = {
    "morphotag": extract_morphotag,
    "segmentation": extract_segmentation,
    "wor": extract_wor,
    "compare": extract_compare,
    "translate": extract_translate,
}


# --- engine drivers --------------------------------------------------------


def _run(cmd: list[str], cwd: Path | None = None) -> subprocess.CompletedProcess:
    return subprocess.run(
        cmd, cwd=cwd, capture_output=True, text=True, timeout=1800
    )


def run_ba2(cell: Cell, fixture: Path, work: Path) -> Path:
    """Run BA2 on `fixture`; return the produced `.cha` path.

    BA2's CLI is `execute.py <command> <IN_DIR> <OUT_DIR> [opts]`. We give it a
    one-file input dir so the rest of `work` stays clean.
    """
    in_dir = work / "ba2_in"
    out_dir = work / "ba2_out"
    in_dir.mkdir(parents=True, exist_ok=True)
    out_dir.mkdir(parents=True, exist_ok=True)
    dst = in_dir / fixture.name
    shutil.copy(fixture, dst)
    # Audio commands need the media beside the .cha; copy any sibling media.
    for sib in fixture.parent.glob(fixture.stem + ".*"):
        if sib != fixture:
            shutil.copy(sib, in_dir / sib.name)

    proc = _run(
        [str(BA2_PY), "execute.py", cell.command, str(in_dir), str(out_dir), *cell.ba2],
        cwd=BA2_DIR,
    )
    produced = sorted(out_dir.glob("*.cha"))
    if not produced:
        raise RuntimeError(
            f"BA2 produced no .cha\nstdout:\n{proc.stdout[-2000:]}\nstderr:\n{proc.stderr[-2000:]}"
        )
    return produced[0]


def run_ba3(cell: Cell, fixture: Path, work: Path) -> Path:
    """Run BA3 via `just batchalign::cli`; return the produced `.cha` path.

    BA3 writes in place; we copy the fixture into the work dir and point the
    CLI at the copy.
    """
    target = work / "ba3" / fixture.name
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copy(fixture, target)
    for sib in fixture.parent.glob(fixture.stem + ".*"):
        if sib != fixture:
            shutil.copy(sib, target.parent / sib.name)

    proc = _run(
        ["just", "batchalign::cli", cell.command, str(target), *cell.ba3],
        cwd=REPO,
    )
    out = target.with_suffix(".cha")
    if not out.exists():
        raise RuntimeError(
            f"BA3 produced no .cha\nstdout:\n{proc.stdout[-2000:]}\nstderr:\n{proc.stderr[-2000:]}"
        )
    return out


# --- runner ----------------------------------------------------------------


@dataclass
class Result:
    cell: Cell
    passed: bool
    detail: str = ""
    diff: str = ""


def check(cell: Cell, *, verbose: bool) -> Result:
    fixture = FIXTURES / cell.fixture
    if not fixture.exists():
        return Result(cell, False, detail=f"missing fixture {fixture}")
    extractor = EXTRACTORS[cell.kind]

    with tempfile.TemporaryDirectory(prefix="parity_") as tmp:
        work = Path(tmp)
        try:
            ba2_cha = run_ba2(cell, fixture, work)
            ba3_cha = run_ba3(cell, fixture, work)
        except Exception as exc:  # noqa: BLE001 - surface any engine failure
            return Result(cell, False, detail=str(exc))

        ba2_lines = extractor(ba2_cha.read_text())
        ba3_lines = extractor(ba3_cha.read_text())

    passed = ba2_lines == ba3_lines
    diff = ""
    if not passed or verbose:
        diff = "\n".join(
            difflib.unified_diff(
                ba2_lines, ba3_lines, fromfile="BA2", tofile="BA3", lineterm=""
            )
        )
    detail = f"{len(ba2_lines)} important lines"
    return Result(cell, passed, detail=detail, diff=diff)


def main() -> int:
    ap = argparse.ArgumentParser(description=__doc__)
    ap.add_argument("--command", help="only run cells for this command")
    ap.add_argument("--engine", help="only run cells for this engine")
    ap.add_argument("--language", help="only run cells for this language")
    ap.add_argument("--list", action="store_true", help="list the matrix and exit")
    ap.add_argument("-v", "--verbose", action="store_true", help="show diffs even on pass")
    args = ap.parse_args()

    cells = [
        c
        for c in MATRIX
        if (not args.command or c.command == args.command)
        and (not args.engine or c.engine == args.engine)
        and (not args.language or c.language == args.language)
    ]

    if args.list:
        for c in cells:
            print(f"  {c.name:40s} fixture={c.fixture}")
        return 0

    if not cells:
        print("no matching cells", file=sys.stderr)
        return 2

    print(f"Parity proof: {len(cells)} cell(s)\n")
    results = []
    for c in cells:
        print(f"… {c.name}", flush=True)
        r = check(c, verbose=args.verbose)
        results.append(r)
        mark = "PASS" if r.passed else "FAIL"
        print(f"{mark} {c.name}  ({r.detail})")
        if r.diff:
            print(_indent(r.diff))
        print()

    npass = sum(r.passed for r in results)
    print(f"==== {npass}/{len(results)} cells passed ====")
    return 0 if npass == len(results) else 1


def _indent(text: str, prefix: str = "    ") -> str:
    return "\n".join(prefix + l for l in text.splitlines())


if __name__ == "__main__":
    raise SystemExit(main())
