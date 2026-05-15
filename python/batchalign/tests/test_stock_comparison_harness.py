from __future__ import annotations

import json
import os
import stat
import subprocess
import sys
from pathlib import Path
from typing import Any

REPO = Path(__file__).resolve().parents[2]
HARNESS = REPO / "scripts" / "compare_stock_batchalign.py"


def _write_fake_cli(path: Path, mode: str) -> None:
    script = f"""#!/usr/bin/env python3
from pathlib import Path
import sys

MODE = {mode!r}

args = sys.argv[1:]
assert args[0] == "morphotag", args
cli_args = args[1:]
if "--output" in cli_args:
    output_index = cli_args.index("--output")
    out_dir = Path(cli_args[output_index + 1])
    inputs = []
    for value in cli_args[output_index + 2:]:
        if value.startswith("--"):
            continue
        inputs.append(Path(value))
else:
    positional = [Path(value) for value in cli_args if not value.startswith("--")]
    in_dir, out_dir = positional[-2], positional[-1]
    inputs = sorted(path for path in in_dir.rglob("*") if path.is_file())

out_dir.mkdir(parents=True, exist_ok=True)

payloads = {{
    "exact": "@UTF8\\n@Begin\\n*CHI:\\thello .\\n%mor:\\tpro|hello .\\n@End\\n",
    "baseline_allowed": "@UTF8\\n@Begin\\n*CHI:\\thello .\\n%wor:\\tbiberon@s nonon@c\\n@End\\n",
    "current_allowed": "@UTF8\\n@Begin\\n*CHI:\\thello .\\n%wor:\\tbiberon nonon\\n@End\\n",
    "baseline_unexpected": "@UTF8\\n@Begin\\n*CHI:\\thello .\\n%mor:\\tL2|xxx\\n@End\\n",
    "current_unexpected": "@UTF8\\n@Begin\\n*CHI:\\thello .\\n%mor:\\tx|biberon\\n@End\\n",
}}

for src in inputs:
    (out_dir / src.name).write_text(payloads[MODE])
"""
    path.write_text(script)
    path.chmod(path.stat().st_mode | stat.S_IXUSR)


def _run_compare(
    tmp_path: Path,
    *,
    current_mode: str,
    baseline_mode: str,
) -> tuple[subprocess.CompletedProcess[str], dict[str, Any]]:
    current = tmp_path / "current-cli"
    baseline = tmp_path / "baseline-cli"
    _write_fake_cli(current, current_mode)
    _write_fake_cli(baseline, baseline_mode)

    input_file = tmp_path / "sample.cha"
    input_file.write_text("@UTF8\n@Begin\n*CHI:\thello .\n@End\n")

    report = tmp_path / "report.json"
    result = subprocess.run(
        [
            sys.executable,
            str(HARNESS),
            "--current-bin",
            str(current),
            "--baseline-bin",
            str(baseline),
            "--report",
            str(report),
            str(input_file),
        ],
        cwd=REPO,
        text=True,
        capture_output=True,
        check=False,
        env={**os.environ, "PYTHONPATH": str(REPO)},
    )
    assert report.exists(), result.stderr
    return result, json.loads(report.read_text())


def test_comparison_harness_reports_exact_matches(tmp_path: Path) -> None:
    result, report = _run_compare(tmp_path, current_mode="exact", baseline_mode="exact")
    assert result.returncode == 0, result.stderr
    assert report["summary"]["exact_matches"] == 1
    assert report["summary"]["allowed_differences"] == 0
    assert report["summary"]["unexpected_differences"] == 0
    assert report["files"][0]["status"] == "exact"


def test_comparison_harness_accepts_allowed_difference_policy(tmp_path: Path) -> None:
    result, report = _run_compare(
        tmp_path,
        current_mode="current_allowed",
        baseline_mode="baseline_allowed",
    )
    assert result.returncode == 0, result.stderr
    assert report["summary"]["exact_matches"] == 0
    assert report["summary"]["allowed_differences"] == 1
    assert report["summary"]["unexpected_differences"] == 0
    assert report["files"][0]["status"] == "allowed_difference"
    assert report["files"][0]["applied_rules"] == ["strip-wor-form-markers"]
    assert "--output" in report["runs"]["current"]["argv"]
    assert "--output" not in report["runs"]["baseline"]["argv"]


def test_comparison_harness_flags_unexpected_differences(tmp_path: Path) -> None:
    result, report = _run_compare(
        tmp_path,
        current_mode="current_unexpected",
        baseline_mode="baseline_unexpected",
    )
    assert result.returncode == 1
    assert report["summary"]["exact_matches"] == 0
    assert report["summary"]["allowed_differences"] == 0
    assert report["summary"]["unexpected_differences"] == 1
    assert report["files"][0]["status"] == "unexpected_difference"
    diff_path = Path(report["files"][0]["diff_path"])
    assert diff_path.exists()
    diff_text = diff_path.read_text()
    assert "L2|xxx" in diff_text
    assert "x|biberon" in diff_text
