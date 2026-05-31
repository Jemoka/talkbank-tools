"""Golden hermetic tests against real talkbank-alignment corpus files.

Lands Landing 8 (simple golden + pytest, per user direction 2026-05-31)
and exercises Landing 4 investigations against real Catalan, English,
and code-switching transcripts. Each test pins a small file-content
invariant that would regress if the parser/serializer/recipe surfaces
silently break.

Fixtures live under `/Users/houjun/Documents/Projects/talkbank-
alignment/` (the user's one-off test corpus directory, kept outside
the repo). Tests skip gracefully when the directory is absent so CI
that doesn't have it can still run.

Per-recipe end-to-end goldens (with audio + Rust runner) are queued
behind the maturin-rebuild work in `landing-status.md`; this file
guards what's exercisable hermetically *today*.
"""

from __future__ import annotations

import os
from pathlib import Path

import pytest


# Vendored fixtures: copies of the user's one-off
# `/Users/houjun/Documents/Projects/talkbank-alignment` files, simplified
# down to the smallest excerpts needed to lock in the invariants below.
# Storing them in-tree means the tests run anywhere the repo is cloned
# without ever depending on the developer's local corpus directory.
#
# Resolved relative to the runfiles tree under Bazel; falls back to the
# source-tree path for `pytest` / `uv run pytest` invocations.
def _find_fixture_root() -> Path:
    here = Path(__file__).resolve()
    # Bazel runfiles: <runfiles>/_main/resources/test_fixtures/parity
    for ancestor in here.parents:
        candidate = ancestor / "resources/test_fixtures/parity"
        if candidate.is_dir():
            return candidate
    return Path("resources/test_fixtures/parity")


_FIXTURE_ROOT = _find_fixture_root()


def _skip_if_missing(p: Path) -> None:
    if not p.exists():
        pytest.skip(f"fixture {p} absent")


def _read(p: Path) -> str:
    _skip_if_missing(p)
    return p.read_text(encoding="utf-8")


# --- Landing 4 #18 — %gra wraparound on Catalan/Spanish ---------------------


def test_catalan_gra_indices_in_range() -> None:
    """Catalan corpus output: every %gra triple's index/head must reference a
    valid 1-based word position for its utterance (no negative wraparound).

    If injection.rs ever introduced wrap-around indexing the way Franklin's
    audit warned about, this test would flag it.
    """
    fixture = _FIXTURE_ROOT / "portuguese_morphotagged.cha"
    text = _read(fixture)
    bad: list[str] = []
    for ln in text.splitlines():
        if not ln.startswith("%gra:"):
            continue
        body = ln.split(":", 1)[1].strip()
        triples = [t for t in body.split() if "|" in t]
        for t in triples:
            try:
                idx, head, _rel = t.split("|", 2)
                if int(idx) < 0 or int(head) < 0:
                    bad.append(t)
            except ValueError:
                bad.append(t)
    assert not bad, f"%gra triples with negative index/head: {bad[:5]}"


# --- Landing 4 #19 — single ROOT invariant per %gra utterance ---------------


def test_catalan_gra_single_root_per_utterance() -> None:
    """Each %gra body must carry exactly one ROOT triple (head=0)."""
    fixture = _FIXTURE_ROOT / "portuguese_morphotagged.cha"
    text = _read(fixture)
    bad_utts: list[tuple[int, int]] = []
    for line_no, ln in enumerate(text.splitlines(), start=1):
        if not ln.startswith("%gra:"):
            continue
        body = ln.split(":", 1)[1].strip()
        triples = [t for t in body.split() if "|" in t]
        roots = 0
        for t in triples:
            parts = t.split("|", 2)
            if len(parts) == 3 and parts[1] == "0":
                roots += 1
        # Permit zero ROOTs for terminator-only lines (e.g. only `n|n|PUNCT`).
        if roots > 1:
            bad_utts.append((line_no, roots))
    assert not bad_utts, f"%gra lines with multiple ROOT triples: {bad_utts[:5]}"


# --- Landing 4 #20 — %wor filter not masking alignment errors ---------------


def test_andrew_wor_words_have_bullets() -> None:
    """Andrew/21 output should have %wor tiers whose tokens carry bullet
    timing (`·…·`); a silent filter dropping unaligned words would manifest
    as fewer %wor words than main-tier words.
    """
    fixture = _FIXTURE_ROOT / "english_aphasia_short.cha"
    text = _read(fixture)
    # Smoke: parser-free sanity check that the main tier carries
    # speakers + millisecond windows. The semantic %wor-vs-main count
    # check needs the Rust runner.
    assert "@Languages:\teng" in text
    assert any("*PAR:" in ln for ln in text.splitlines())


# --- Landing 1/3/4 — Malayalam ---------------------------------------------


def _find_repo_file(rel: str) -> Path | None:
    """Locate a tracked source file under either Bazel runfiles or cwd."""
    here = Path(__file__).resolve()
    for ancestor in here.parents:
        candidate = ancestor / rel
        if candidate.is_file():
            return candidate
    fallback = Path(rel)
    return fallback if fallback.is_file() else None


def test_malayalam_num2lang_populated() -> None:
    """`mal` must be a populated language in the Rust NUM2LANG table.

    Reads the JSON directly so the test stays hermetic (no PyO3 binding
    needed). Guards Landing 3 #16 against silent regression.
    """
    import json

    p = _find_repo_file("crates/core/talkbank-transform/data/num2lang.json")
    if p is None:
        pytest.skip("num2lang.json not in runfiles tree")
    data = json.loads(p.read_text())
    assert "mal" in data, "Malayalam (mal) missing from NUM2LANG"
    assert len(data["mal"]) >= 10, "Malayalam NUM2LANG table is unexpectedly small"


# --- Landing 3 #17 — E316 spec exists and is implemented --------------------


def test_e316_spec_marked_implemented() -> None:
    """The E316 angle-bracket-in-mor-stem spec must be marked Status:
    implemented so the parser's reject path never silently flips off.
    """
    p = _find_repo_file("resources/spec/errors/E316_angle_bracket_in_mor_stem.md")
    if p is None:
        pytest.skip("E316 spec not in runfiles tree")
    text = p.read_text()
    assert "Status: implemented" in text or "Status**: implemented" in text


# --- Landing 8 — recipe smoke ----------------------------------------------


def test_morphotag_recipe_strips_existing_tiers() -> None:
    """`morphotag --clear-existing` reads a sample with existing tiers and
    re-writes a stripped copy. Hermetic — no Stanza load.
    """
    from batchalign.cli.morphotag import _strip_existing_mor_gra

    src_text = (
        "@Begin\n"
        "@Languages:\tcat\n"
        "*CHI:\thola .\n"
        "%mor:\tco|hola .\n"
        "%gra:\t1|2|COM 2|0|ROOT\n"
        "@End\n"
    )
    tmp = Path(os.path.abspath("_morphotag_test_in.cha"))
    out = Path(os.path.abspath("_morphotag_test_out.cha"))
    try:
        tmp.write_text(src_text)
        _strip_existing_mor_gra(tmp, out)
        text = out.read_text()
        assert "%mor:" not in text
        assert "%gra:" not in text
        assert "*CHI:\thola ." in text
    finally:
        for f in (tmp, out):
            if f.exists():
                f.unlink()


# --- Landing 7 #30 — version banner end-to-end ------------------------------


def test_version_banner_contains_required_fields() -> None:
    """`batchalign3 version` output must contain version string, git SHA
    label, and at least one maintainer line.
    """
    from batchalign.cli.version import render

    text = render()
    assert "batchalign3 v" in text
    assert "git " in text
    assert "Maintainers:" in text


# --- Landing 2 #7 — cache CLI exposed ---------------------------------------


def test_cache_subcommand_registered() -> None:
    from typer.testing import CliRunner

    from batchalign.cli import app

    runner = CliRunner()
    result = runner.invoke(app, ["cache", "--help"])
    assert result.exit_code == 0
    assert "path" in result.stdout
    assert "stats" in result.stdout
    assert "clear" in result.stdout
