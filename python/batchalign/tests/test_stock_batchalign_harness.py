"""Regression tests for the stock-batchalign comparison harness.

These tests keep the harness honest without depending on real ML models:
small fake benchmark runners emit deterministic `.compare.csv` outputs so the
script can be exercised end-to-end with curated manifests and allowlists.
"""

from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SCRIPT = ROOT / "scripts" / "stock_batchalign_harness.py"
DEFAULT_MANIFEST = ROOT / "test-fixtures" / "stock_batchalign" / "manifest.json"
DEFAULT_ALLOWLIST = ROOT / "test-fixtures" / "stock_batchalign" / "allowlist.json"



def _write_fake_runner(
    path: Path,
    *,
    wer: float,
    accuracy: float,
    matches: int,
    insertions: int,
    deletions: int,
    total_gold_words: int,
    total_main_words: int,
    compare_wer: float | None = None,
    compare_accuracy: float | None = None,
    compare_matches: int | None = None,
    compare_insertions: int | None = None,
    compare_deletions: int | None = None,
    compare_total_gold_words: int | None = None,
    compare_total_main_words: int | None = None,
    legacy_benchmark_only: bool = False,
    legacy_plain_chat_only: bool = False,
    copy_gold_chat: bool = False,
    emit_error_text: str | None = None,
) -> None:
    """Create a tiny benchmark-compatible executable for harness tests."""
    compare_wer = wer if compare_wer is None else compare_wer
    compare_accuracy = accuracy if compare_accuracy is None else compare_accuracy
    compare_matches = matches if compare_matches is None else compare_matches
    compare_insertions = insertions if compare_insertions is None else compare_insertions
    compare_deletions = deletions if compare_deletions is None else compare_deletions
    compare_total_gold_words = total_gold_words if compare_total_gold_words is None else compare_total_gold_words
    compare_total_main_words = total_main_words if compare_total_main_words is None else compare_total_main_words
    script = f'''#!/usr/bin/env python3
from __future__ import annotations

import sys
from pathlib import Path

BENCHMARK_CSV = """metric,value
wer,{wer:.4f}
accuracy,{accuracy:.4f}
matches,{matches}
insertions,{insertions}
deletions,{deletions}
total_gold_words,{total_gold_words}
total_main_words,{total_main_words}
"""
COMPARE_CSV = """metric,value
wer,{compare_wer:.4f}
accuracy,{compare_accuracy:.4f}
matches,{compare_matches}
insertions,{compare_insertions}
deletions,{compare_deletions}
total_gold_words,{compare_total_gold_words}
total_main_words,{compare_total_main_words}
"""
CHAT = "@UTF8\\n@Begin\\n@Languages:\teng\\n@Participants:\tPAR Participant\\n@ID:\teng|test|PAR|||||Participant|||\\n*PAR:\thello .\\n@End\\n"
LEGACY_BENCHMARK_ONLY = {legacy_benchmark_only!r}
LEGACY_PLAIN_CHAT_ONLY = {legacy_plain_chat_only!r}
COPY_GOLD_CHAT = {copy_gold_chat!r}
EMIT_ERROR_TEXT = {emit_error_text!r}


def _emit_benchmark(output_dir: Path, stem: str, input_dir: Path) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    if LEGACY_BENCHMARK_ONLY:
        (output_dir / f"{{stem}}.asr.cha").write_text(CHAT, encoding="utf-8")
        (output_dir / f"{{stem}}.wer.txt").write_text("{wer:.4f}", encoding="utf-8")
        (output_dir / f"{{stem}}.diff").write_text("+ stub\\n", encoding="utf-8")
        return 0
    if LEGACY_PLAIN_CHAT_ONLY:
        output_path = output_dir / f"{{stem}}.cha"
        if COPY_GOLD_CHAT:
            output_path.write_text(
                (input_dir / f"{{stem}}.cha").read_text(encoding="utf-8"),
                encoding="utf-8",
            )
        else:
            output_path.write_text(CHAT, encoding="utf-8")
        if EMIT_ERROR_TEXT is not None:
            print(EMIT_ERROR_TEXT)
        return 0
    (output_dir / f"{{stem}}.cha").write_text(CHAT, encoding="utf-8")
    (output_dir / f"{{stem}}.compare.csv").write_text(BENCHMARK_CSV, encoding="utf-8")
    return 0


def _emit_compare(output_dir: Path, stem: str) -> int:
    output_dir.mkdir(parents=True, exist_ok=True)
    (output_dir / f"{{stem}}.cha").write_text(CHAT, encoding="utf-8")
    (output_dir / f"{{stem}}.compare.csv").write_text(COMPARE_CSV, encoding="utf-8")
    return 0


def main() -> int:
    args = sys.argv[1:]
    if not args:
        return 2
    command = args[0]
    cli_args = args[1:]
    if command == "benchmark":
        if "--output" in cli_args:
            try:
                output_dir = Path(cli_args[cli_args.index("--output") + 1])
            except IndexError:
                return 3
            audio_path = Path(cli_args[-1])
            return _emit_benchmark(output_dir, audio_path.stem, audio_path.parent)
        positional = [Path(value) for value in cli_args if not value.startswith("--")]
        if len(positional) < 2:
            return 4
        input_dir, output_dir = positional[-2], positional[-1]
        audio_candidates = sorted(path for path in input_dir.iterdir() if path.suffix != ".cha")
        if not audio_candidates:
            return 5
        return _emit_benchmark(output_dir, audio_candidates[0].stem, input_dir)
    if command == "compare":
        if "--output" not in cli_args:
            return 6
        try:
            output_dir = Path(cli_args[cli_args.index("--output") + 1])
        except IndexError:
            return 7
        positional = [Path(value) for value in cli_args if not value.startswith("--")]
        if not positional:
            return 8
        input_dir = positional[-1]
        main_candidates = sorted(
            path for path in input_dir.iterdir()
            if path.suffix == ".cha" and not path.name.endswith(".gold.cha")
        )
        if not main_candidates:
            return 9
        return _emit_compare(output_dir, main_candidates[0].stem)
    return 2


if __name__ == "__main__":
    raise SystemExit(main())
'''
    path.write_text(script, encoding="utf-8")
    path.chmod(path.stat().st_mode | stat.S_IXUSR)



def _write_case_manifest(tmp_path: Path) -> Path:
    """Create a one-case manifest rooted in temporary fixture files."""
    audio_path = tmp_path / "clip.mp3"
    gold_path = tmp_path / "clip.cha"
    audio_path.write_bytes(b"stub-audio")
    gold_path.write_text(
        "@UTF8\n@Begin\n@Languages:\tyue\n@Participants:\tPAR Participant\n@ID:\tyue|test|PAR|||||Participant|||\n*PAR:\t你好 。\n@End\n",
        encoding="utf-8",
    )
    manifest_path = tmp_path / "manifest.json"
    manifest_path.write_text(
        json.dumps(
            {
                "schema_version": 1,
                "cases": [
                    {
                        "id": "synthetic-case",
                        "description": "Synthetic stock comparison fixture.",
                        "command": "benchmark",
                        "audio_path": str(audio_path),
                        "gold_path": str(gold_path),
                        "argv": ["--lang", "yue", "-n", "1"],
                    }
                ],
            }
        ),
        encoding="utf-8",
    )
    return manifest_path



def _write_allowlist(tmp_path: Path, entries: list[dict[str, str | float]]) -> Path:
    """Write one harness allowlist file for the synthetic test case."""
    path = tmp_path / "allowlist.json"
    path.write_text(
        json.dumps({"schema_version": 1, "entries": entries}),
        encoding="utf-8",
    )
    return path



def _run_harness(
    tmp_path: Path,
    *,
    current_runner: Path,
    baseline_runner: Path | None,
    allowlist_entries: list[dict[str, str | float]],
    skip_baseline: bool = False,
) -> subprocess.CompletedProcess[str]:
    """Execute the harness script against fake runners and a synthetic manifest."""
    manifest_path = _write_case_manifest(tmp_path)
    allowlist_path = _write_allowlist(tmp_path, allowlist_entries)
    run_root = tmp_path / "runs"
    argv = [
        sys.executable,
        str(SCRIPT),
        "--manifest",
        str(manifest_path),
        "--allowlist",
        str(allowlist_path),
        "--current-executable",
        str(current_runner),
        "--run-root",
        str(run_root),
    ]
    if baseline_runner is not None:
        argv.extend(["--baseline-executable", str(baseline_runner)])
    if skip_baseline:
        argv.append("--skip-baseline")
    env = dict(os.environ)
    env.pop("BATCHALIGN_BASELINE_EXECUTABLE", None)
    return subprocess.run(
        argv,
        cwd=ROOT,
        env=env,
        capture_output=True,
        text=True,
    )



def test_default_manifest_points_at_in_repo_cantonese_fixture() -> None:
    """The checked-in manifest should stay wired to the audited in-repo fixture pair."""
    manifest = json.loads(DEFAULT_MANIFEST.read_text(encoding="utf-8"))
    allowlist = json.loads(DEFAULT_ALLOWLIST.read_text(encoding="utf-8"))

    case = manifest["cases"][0]
    audio_path = (DEFAULT_MANIFEST.parent / case["audio_path"]).resolve()
    gold_path = (DEFAULT_MANIFEST.parent / case["gold_path"]).resolve()

    cantonese_fixtures = ROOT / "batchalign" / "tests" / "languages" / "cantonese" / "fixtures"
    assert case["id"] == "cantonese-05b-clip-whisper"
    assert audio_path == cantonese_fixtures / "05b_clip.mp3"
    assert gold_path == cantonese_fixtures / "benchmark" / "05b_clip.cha"
    assert case["argv"] == ["--lang", "yue", "--whisper", "-n", "1"]
    assert audio_path.is_file()
    assert gold_path.is_file()
    assert allowlist == {"schema_version": 1, "entries": []}



def test_harness_separates_allowlisted_baseline_differences(tmp_path: Path) -> None:
    """Known stock-baseline bugs should report separately from regressions."""
    current_runner = tmp_path / "current-runner.py"
    baseline_runner = tmp_path / "baseline-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.1000,
        accuracy=0.9000,
        matches=9,
        insertions=1,
        deletions=0,
        total_gold_words=10,
        total_main_words=10,
    )
    _write_fake_runner(
        baseline_runner,
        wer=0.3000,
        accuracy=0.7000,
        matches=7,
        insertions=2,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=baseline_runner,
        allowlist_entries=[
            {
                "case_id": "synthetic-case",
                "metric": "wer",
                "reason": "stock bug drops one known token on this fixture",
                "max_delta": 0.2500,
            }
        ],
    )

    assert result.returncode == 0, result.stderr
    assert "allowlisted baseline differences:" in result.stdout
    assert "wer improved by 0.2000" in result.stdout
    assert "unexpected regressions:" not in result.stdout
    baseline_cmd_line = next(
        line for line in result.stdout.splitlines() if line.startswith("  baseline cmd:")
    )
    assert "--output" not in baseline_cmd_line
    assert "/synthetic-case/baseline/input " in baseline_cmd_line
    assert "/synthetic-case/baseline/output" in baseline_cmd_line



def test_harness_flags_unexpected_regressions(tmp_path: Path) -> None:
    """Current-worse metrics should fail the harness when they beat no allowlist."""
    current_runner = tmp_path / "current-runner.py"
    baseline_runner = tmp_path / "baseline-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.4000,
        accuracy=0.6000,
        matches=6,
        insertions=3,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
    )
    _write_fake_runner(
        baseline_runner,
        wer=0.1000,
        accuracy=0.9000,
        matches=9,
        insertions=1,
        deletions=0,
        total_gold_words=10,
        total_main_words=10,
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=baseline_runner,
        allowlist_entries=[],
    )

    assert result.returncode == 1, result.stdout
    assert "unexpected regressions:" in result.stdout
    assert "wer regressed by 0.3000" in result.stdout



def test_harness_skips_missing_baseline_but_runs_current_case(tmp_path: Path) -> None:
    """Baseline absence should stay graceful while the current benchmark still runs."""
    current_runner = tmp_path / "current-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.2000,
        accuracy=0.8000,
        matches=8,
        insertions=1,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=None,
        allowlist_entries=[],
        skip_baseline=True,
    )

    assert result.returncode == 0, result.stderr
    assert "Baseline runner: skipped" in result.stdout
    assert "status      : baseline skipped" in result.stdout
    assert "current     : wer=0.2000" in result.stdout


def test_harness_rescores_legacy_asr_output_with_current_compare(tmp_path: Path) -> None:
    """Legacy runners without `.compare.csv` should be rescored via current compare."""
    current_runner = tmp_path / "current-runner.py"
    baseline_runner = tmp_path / "baseline-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.1000,
        accuracy=0.9000,
        matches=9,
        insertions=1,
        deletions=0,
        total_gold_words=10,
        total_main_words=10,
        compare_wer=0.3000,
        compare_accuracy=0.7000,
        compare_matches=7,
        compare_insertions=2,
        compare_deletions=1,
        compare_total_gold_words=10,
        compare_total_main_words=9,
    )
    _write_fake_runner(
        baseline_runner,
        wer=0.3000,
        accuracy=0.7000,
        matches=7,
        insertions=2,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
        legacy_benchmark_only=True,
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=baseline_runner,
        allowlist_entries=[],
    )

    assert result.returncode == 0, result.stderr
    assert "baseline    : wer=0.3000 accuracy=0.7000 matches=7 insertions=2 deletions=1" in result.stdout
    recovered_metrics = tmp_path / "runs" / "synthetic-case" / "baseline" / "output" / "clip.compare.csv"
    assert recovered_metrics.is_file()
    assert "wer,0.3000" in recovered_metrics.read_text(encoding="utf-8")


def test_harness_rescores_legacy_plain_chat_output_with_current_compare(tmp_path: Path) -> None:
    """Legacy runners that only emit one transcript `.cha` should still be rescored."""
    current_runner = tmp_path / "current-runner.py"
    baseline_runner = tmp_path / "baseline-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.1000,
        accuracy=0.9000,
        matches=9,
        insertions=1,
        deletions=0,
        total_gold_words=10,
        total_main_words=10,
        compare_wer=0.3000,
        compare_accuracy=0.7000,
        compare_matches=7,
        compare_insertions=2,
        compare_deletions=1,
        compare_total_gold_words=10,
        compare_total_main_words=9,
    )
    _write_fake_runner(
        baseline_runner,
        wer=0.3000,
        accuracy=0.7000,
        matches=7,
        insertions=2,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
        legacy_plain_chat_only=True,
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=baseline_runner,
        allowlist_entries=[],
    )

    assert result.returncode == 0, result.stderr
    assert "baseline    : wer=0.3000 accuracy=0.7000 matches=7 insertions=2 deletions=1" in result.stdout
    recovered_metrics = tmp_path / "runs" / "synthetic-case" / "baseline" / "output" / "clip.compare.csv"
    assert recovered_metrics.is_file()
    assert "wer,0.3000" in recovered_metrics.read_text(encoding="utf-8")


def test_harness_flags_copied_gold_plain_chat_as_baseline_failure(tmp_path: Path) -> None:
    """A copied gold `.cha` with no metrics should surface as a silent baseline failure."""
    current_runner = tmp_path / "current-runner.py"
    baseline_runner = tmp_path / "baseline-runner.py"
    _write_fake_runner(
        current_runner,
        wer=0.1000,
        accuracy=0.9000,
        matches=9,
        insertions=1,
        deletions=0,
        total_gold_words=10,
        total_main_words=10,
    )
    _write_fake_runner(
        baseline_runner,
        wer=0.3000,
        accuracy=0.7000,
        matches=7,
        insertions=2,
        deletions=1,
        total_gold_words=10,
        total_main_words=9,
        legacy_plain_chat_only=True,
        copy_gold_chat=True,
        emit_error_text="ERROR on file clip.mp3: torchcodec missing.",
    )

    result = _run_harness(
        tmp_path,
        current_runner=current_runner,
        baseline_runner=baseline_runner,
        allowlist_entries=[],
    )

    assert result.returncode == 1, result.stderr
    assert "status      : baseline runner failed" in result.stdout
    assert "identical to the gold input" in result.stdout
    assert "torchcodec missing" in result.stdout


def test_harness_cleans_up_isolated_daemon_pid(tmp_path: Path) -> None:
    """The harness should kill exact isolated daemon PIDs after one case run."""
    from scripts.stock_batchalign_harness import _cleanup_runner_daemons

    case_root = tmp_path / "case-root"
    state_dir = case_root / "current" / "state"
    state_dir.mkdir(parents=True)
    proc = subprocess.Popen(
        [sys.executable, "-c", "import time; time.sleep(60)"],
        start_new_session=True,
    )
    try:
        (state_dir / "daemon.json").write_text(
            json.dumps({"pid": proc.pid, "port": 8000, "version": "test"}),
            encoding="utf-8",
        )
        _cleanup_runner_daemons(case_root, "current")
        proc.wait(timeout=5)
        time.sleep(0.2)
        assert proc.poll() is not None
    finally:
        if proc.poll() is None:
            proc.terminate()
            proc.wait(timeout=5)


def test_prepare_runner_env_seeds_minimal_whisper_config(tmp_path: Path) -> None:
    """Isolated runner homes should get a noninteractive whisper config by default."""
    from scripts.stock_batchalign_harness import _prepare_runner_env

    env = _prepare_runner_env(tmp_path, "baseline")
    config_path = Path(env["HOME"]) / ".batchalign.ini"
    assert config_path.read_text(encoding="utf-8") == "[asr]\nengine = whisper\n"


def test_prepare_runner_env_copies_explicit_config_source(tmp_path: Path) -> None:
    """Credentialed runs can inject an explicit config file into the isolated home."""
    from scripts.stock_batchalign_harness import _prepare_runner_env

    source = tmp_path / "source.ini"
    source.write_text("[asr]\nengine = rev\nengine.rev.key = test-key\n", encoding="utf-8")
    previous = os.environ.get("BATCHALIGN_STOCK_CONFIG_SOURCE")
    os.environ["BATCHALIGN_STOCK_CONFIG_SOURCE"] = str(source)
    try:
        env = _prepare_runner_env(tmp_path, "baseline")
    finally:
        if previous is None:
            os.environ.pop("BATCHALIGN_STOCK_CONFIG_SOURCE", None)
        else:
            os.environ["BATCHALIGN_STOCK_CONFIG_SOURCE"] = previous
    config_path = Path(env["HOME"]) / ".batchalign.ini"
    assert config_path.read_text(encoding="utf-8") == source.read_text(encoding="utf-8")
