"""Helper for tests that need to spawn a pytest subprocess.

Integration tests for conftest hooks (e.g. history writer, priority
ordering) can't assert about hook behavior from within their own
pytest session — the hook has already fired. They need a fresh
subprocess that imports the batchalign conftest and observes its
side effects.

Constraints on the subprocess:

* The dummy test file must live inside ``batchalign/tests/`` so
  pytest's upward conftest-discovery walk picks up the root conftest
  that owns the hooks we're testing. ``tmp_path`` is outside the
  repo; using it leaves the conftest unreachable.
* ``addopts=`` disables the repo's default pytest.ini opts (xdist,
  markers, durations) so the subprocess is deterministic.
* ``no:cacheprovider`` prevents pollution of the developer's
  ``.pytest_cache``.

Callers get back a :class:`subprocess.CompletedProcess[str]` and are
responsible for assertions on stdout / returncode / DB files.
"""

from __future__ import annotations

import os
import subprocess
import sys
import textwrap
from pathlib import Path

_DEFAULT_CI_VARS = ("CI", "GITHUB_ACTIONS", "BUILDKITE", "JENKINS_URL")

_DEFAULT_DUMMY_BODY = textwrap.dedent(
    """
    def test_passes_for_subprocess():
        assert True
    """
)


def subprocess_test_filename(tmp_path: Path, slug: str) -> str:
    """Deterministic filename the helper will write under
    ``batchalign/tests/``.

    Exposed for callers that need to know the filename BEFORE the
    subprocess runs — e.g., seeding a history DB with nodeids that
    reference the generated file.
    """
    return f"_{slug}_{tmp_path.name}.py"


def run_pytest_subprocess(
    tmp_path: Path,
    env_extra: dict[str, str],
    *,
    xdist_workers: int = 0,
    dummy_body: str = _DEFAULT_DUMMY_BODY,
    slug: str = "subprocess",
    keep_ci_env: bool = False,
    extra_args: tuple[str, ...] = (),
) -> subprocess.CompletedProcess[str]:
    """Spawn a pytest subprocess whose test file is inside this repo.

    Args:
        tmp_path: pytest-provided per-test tmp dir; its basename is
            folded into the dummy test filename so parallel test
            workers don't collide on the shared ``batchalign/tests/``
            directory.
        env_extra: additional env vars merged into the child's env.
        xdist_workers: if >0, pass ``-n N`` to enable xdist; otherwise
            ``-p no:xdist`` is passed to force single-process.
        dummy_body: body of the generated test file. Default is a
            single always-passing test — override for tests that need
            multiple test functions or specific names.
        slug: prefix for the generated test filename (helps with
            debugging if a file ever leaks).
        keep_ci_env: if False (default) we scrub the common CI env
            vars so the subprocess sees interactive defaults.

    Returns the raw ``CompletedProcess`` — caller asserts.
    """
    repo_root = Path.cwd()
    host_tests_dir = repo_root / "batchalign" / "tests"
    test_file = host_tests_dir / subprocess_test_filename(tmp_path, slug)
    test_file.write_text(dummy_body)

    env = dict(os.environ)
    if not keep_ci_env:
        for k in _DEFAULT_CI_VARS:
            env.pop(k, None)
    env.update(env_extra)

    cmd = [
        sys.executable,
        "-m",
        "pytest",
        str(test_file),
        "-p",
        "no:cacheprovider",
        "-o",
        "addopts=",
    ]
    if xdist_workers > 0:
        cmd.extend(["-n", str(xdist_workers)])
    else:
        cmd.extend(["-p", "no:xdist"])
    cmd.extend(extra_args)

    try:
        return subprocess.run(
            cmd,
            env=env,
            cwd=repo_root,
            capture_output=True,
            text=True,
            timeout=60,
        )
    finally:
        test_file.unlink(missing_ok=True)
