# affects: batchalign/tests/_affects.py
"""Unit tests for the ``affects:`` metadata parser + diff matcher
(Phase C1 of the test-cost revamp).

Convention: a test file opts into change-aware selection by declaring
one or more ``# affects: <glob>`` lines (Python) or
``// affects: <glob>`` lines (Rust) in its module header. The
resulting pattern set is matched against the set of paths touched
by a git diff. A test file with no ``affects:`` declarations is
treated as "runs always" — backward compatible, gradual adoption.

Patterns are gitignore-style (``**`` supported) via ``pathspec``.
"""

from __future__ import annotations

from pathlib import Path

from batchalign.tests._affects import (
    AffectsDeclaration,
    diff_matches_declaration,
    parse_affects,
    select_tests,
)


# ---------- parse_affects ------------------------------------------------

def test_parse_python_single_pattern(tmp_path: Path) -> None:
    f = tmp_path / "test_x.py"
    f.write_text(
        '"""docstring"""\n'
        "# affects: python/batchalign/inference/morphosyntax.py\n"
        "def test_a(): pass\n"
    )
    decl = parse_affects(f)
    assert decl.patterns == ("python/batchalign/inference/morphosyntax.py",)


def test_parse_rust_single_pattern(tmp_path: Path) -> None:
    f = tmp_path / "a.rs"
    f.write_text(
        "// affects: crates/batchalign/src/retokenize/**\n"
        "fn test_x() {}\n"
    )
    decl = parse_affects(f)
    assert decl.patterns == ("crates/batchalign/src/retokenize/**",)


def test_parse_multiple_patterns(tmp_path: Path) -> None:
    f = tmp_path / "test_x.py"
    f.write_text(
        "# affects: python/batchalign/inference/morphosyntax.py\n"
        "# affects: crates/batchalign/src/nlp/**\n"
        "# affects: batchalign/tests/_test_history.py\n"
        "def test_a(): pass\n"
    )
    decl = parse_affects(f)
    assert decl.patterns == (
        "python/batchalign/inference/morphosyntax.py",
        "crates/batchalign/src/nlp/**",
        "batchalign/tests/_test_history.py",
    )


def test_parse_empty_when_no_declarations(tmp_path: Path) -> None:
    f = tmp_path / "test_x.py"
    f.write_text('"""no affects lines here"""\ndef test_a(): pass\n')
    decl = parse_affects(f)
    assert decl.patterns == ()


def test_parse_ignores_non_header_comments(tmp_path: Path) -> None:
    """affects: lines that happen inside a prose comment shouldn't be
    declarations — but the current regex treats every ``# affects:``
    line as one. Document the behavior: declarations are cheap
    enough to spot at review time, and false positives caused by
    accidental prose affect only the selection set (over-select), not
    correctness."""
    f = tmp_path / "test_x.py"
    f.write_text(
        '"""The `# affects:` prefix signals change-aware selection."""\n'
        "# affects: python/batchalign/inference/morphosyntax.py\n"
        "def test_a(): pass\n"
    )
    decl = parse_affects(f)
    # The regex matches the real declaration. The docstring mention
    # is inside a string literal, not a comment, so it's excluded.
    assert decl.patterns == ("python/batchalign/inference/morphosyntax.py",)


def test_parse_strips_trailing_whitespace(tmp_path: Path) -> None:
    f = tmp_path / "test_x.py"
    f.write_text("# affects: foo/bar   \ndef test_a(): pass\n")
    decl = parse_affects(f)
    assert decl.patterns == ("foo/bar",)


def test_parse_tolerates_content_arg(tmp_path: Path) -> None:
    """Callers can pass content directly to avoid a re-read."""
    f = tmp_path / "unused.py"
    decl = parse_affects(f, content="# affects: x/y\n")
    assert decl.patterns == ("x/y",)


# ---------- diff_matches_declaration -------------------------------------

def test_match_exact_path() -> None:
    decl = AffectsDeclaration(Path("t.py"), ("foo/bar.rs",))
    assert diff_matches_declaration(["foo/bar.rs"], decl) is True
    assert diff_matches_declaration(["foo/baz.rs"], decl) is False


def test_match_double_star_glob() -> None:
    decl = AffectsDeclaration(Path("t.py"), ("crates/batchalign/src/retokenize/**",))
    assert diff_matches_declaration(
        ["crates/batchalign/src/retokenize/mod.rs"], decl
    ) is True
    assert diff_matches_declaration(
        ["crates/batchalign/src/retokenize/deep/nested/file.rs"], decl
    ) is True
    assert diff_matches_declaration(
        ["crates/batchalign/src/nlp/lang_it.rs"], decl
    ) is False


def test_match_any_of_multiple_patterns() -> None:
    decl = AffectsDeclaration(
        Path("t.py"),
        ("a/b.py", "c/d/**"),
    )
    assert diff_matches_declaration(["a/b.py"], decl) is True
    assert diff_matches_declaration(["c/d/x.py"], decl) is True
    assert diff_matches_declaration(["e/f.py"], decl) is False


def test_match_with_empty_patterns_is_false() -> None:
    decl = AffectsDeclaration(Path("t.py"), ())
    assert diff_matches_declaration(["anything"], decl) is False


def test_match_with_empty_changed_paths_is_false() -> None:
    decl = AffectsDeclaration(Path("t.py"), ("a/b",))
    assert diff_matches_declaration([], decl) is False


# ---------- select_tests -------------------------------------------------

def test_select_tests_run_always_bucket(tmp_path: Path) -> None:
    f_with = tmp_path / "test_with.py"
    f_with.write_text("# affects: a/b.py\n")
    f_without = tmp_path / "test_without.py"
    f_without.write_text('"""no affects"""\n')

    selected, run_always = select_tests([f_with, f_without], ["a/b.py"])
    assert selected == [f_with]
    assert run_always == [f_without]


def test_select_tests_no_match(tmp_path: Path) -> None:
    f = tmp_path / "test_x.py"
    f.write_text("# affects: a/b.py\n")
    selected, run_always = select_tests([f], ["unrelated.py"])
    assert selected == []
    assert run_always == []


def test_select_tests_preserves_order(tmp_path: Path) -> None:
    files = []
    for i in range(5):
        f = tmp_path / f"test_{i}.py"
        f.write_text("" if i % 2 else "# affects: hit.py\n")
        files.append(f)
    selected, run_always = select_tests(files, ["hit.py"])
    # Files 0, 2, 4 have affects matching hit.py; files 1, 3 have no affects.
    assert [p.name for p in selected] == ["test_0.py", "test_2.py", "test_4.py"]
    assert [p.name for p in run_always] == ["test_1.py", "test_3.py"]
