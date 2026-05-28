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

# tbtbt is a SECOND batchalign reference (a more advanced fork with
# Qwen3-ASR, Tencent TMT, NLLB-1.3B, Aliyun MT). For engines BA2 doesn't
# carry, we diff BA3 against tbtbt instead. Set `oracle="tbtbt"` on a Cell
# to route it through `run_tbtbt`. See [[GOLD_PATH_BATCHALIGN.md]] for the
# inline-build recipe (`uv run batchalign3 …`).
TBTBT_DIR = Path("/Users/houjun/Documents/Projects/tbtbt")
TBTBT_BIN = TBTBT_DIR / "target" / "debug" / "batchalign3"


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
    # BA2 global flags that go BEFORE the subcommand (e.g. --force-cpu,
    # --workers); `execute.py <ba2_global> <command> <IN> <OUT> <ba2>`.
    ba2_global: list[str] = field(default_factory=list)
    # When BA2 is itself broken for this combination (e.g. its CHATWhisper
    # Cantonese path emits an empty transcript), there is no oracle to diff
    # against. We then only require that BA3 produces a non-empty, diarized
    # result — the parity bar the user set for such languages ("ignore that
    # language specifically as long as utterances [are] diarized").
    ba2_broken: bool = False
    # When byte-parity is blocked by an ENVIRONMENTAL difference rather than a
    # porting gap — specifically the whisper_fa attention-DTW, whose
    # millisecond bullets are float-sensitive to torch (BA2 2.6.0 vs BA3 2.10.0)
    # — we still diff BA2↔BA3, but with the `%wor`/main timing bullets
    # normalized out. This proves the WORD alignment + segmentation are
    # identical (the port is faithful) while acknowledging the ms shift.
    env_limited: bool = False
    # Which reference to diff BA3 against. Default `"ba2"` (the BA2 fork at
    # /Users/houjun/Documents/Projects/batchalign2). `"tbtbt"` uses the
    # /Users/houjun/Documents/Projects/tbtbt fork as the oracle — chosen for
    # engines BA2 doesn't carry (Qwen3-ASR, NLLB, Tencent TMT, Aliyun MT).
    oracle: str = "ba2"
    # tbtbt-specific CLI args (`tbtbt` oracle only). The driver invokes
    # `batchalign3 <command> <PATH> -o <OUT> <tbtbt>`.
    tbtbt: list[str] = field(default_factory=list)

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
    # --retokenize on Japanese. BA2's --retokenize path emits no %mor here
    # (broken), so there is no oracle — we require BA3 to support it and
    # produce typed %mor/%gra (verified: pron|私-Int-S1 adp|は noun|犬 …).
    Cell(
        command="morphotag",
        engine="stanza",
        language="ja-retokenize",
        fixture="morphotag/ja.cha",
        kind="morphotag",
        ba2_broken=True,
        ba3=["--language", "ja", "--retokenize"],
    ),
    # ---- transcribe (rev: cloud ASR, deterministic; + CHATUtterance seg) ----
    Cell(
        command="transcribe",
        engine="rev",
        language="en",
        fixture="transcribe/en.wav",
        kind="segmentation",
        # BA2's default ASR engine is rev; lang from --lang, 1 speaker.
        ba2=["--lang", "eng", "-n", "1"],
        ba3=["--engine", "rev", "--language", "en", "-n", "1"],
    ),
    Cell(
        command="transcribe",
        engine="rev",
        language="zh",
        fixture="transcribe/zh.wav",
        kind="segmentation",
        ba2=["--lang", "zho", "-n", "1"],
        ba3=["--engine", "rev", "--language", "zh", "-n", "1"],
    ),
    Cell(
        command="transcribe",
        engine="rev",
        language="es",
        fixture="transcribe/es.wav",
        kind="segmentation",
        ba2=["--lang", "spa", "-n", "1"],
        ba3=["--engine", "rev", "--language", "es", "-n", "1"],
    ),
    # ---- transcribe (chatwhisper = BA2 `--whisper`: TalkBank CHATWhisper +
    #      CHATUtterance BERT segmenter). CPU-forced: Whisper's bf16 attention
    #      kernel is unsupported on Apple MPS. ----
    Cell(
        command="transcribe",
        engine="chatwhisper",
        language="en",
        fixture="transcribe/en.wav",
        kind="segmentation",
        ba2_global=["--force-cpu"],
        ba2=["--whisper", "--lang", "eng", "-n", "1"],
        ba3=["--engine", "chatwhisper", "--language", "en", "--force-cpu"],
    ),
    # openai engine (BA2 `--whisper_oai`): the original openai-whisper package's
    # `turbo` model + the same CHATUtterance BERT segmenter.
    Cell(
        command="transcribe",
        engine="openai",
        language="en",
        fixture="transcribe/en.wav",
        kind="segmentation",
        ba2_global=["--force-cpu"],
        ba2=["--whisper_oai", "--lang", "eng", "-n", "1"],
        ba3=["--engine", "openai", "--language", "en", "--force-cpu"],
    ),
    # Cantonese: chatwhisper resolves the alvanlii Cantonese model + the
    # Cantonese-specific BERT utterance segmenter (BertCantoneseUtteranceModel).
    # BA2's CHATWhisper Cantonese path is broken here (emits an empty
    # transcript), so there is no oracle to diff — we only require that BA3
    # produces diarized Cantonese utterances (verified: 媽媽我哋留個充電器 . / …).
    Cell(
        command="transcribe",
        engine="chatwhisper",
        language="yue",
        fixture="transcribe/yue.wav",
        kind="segmentation",
        ba2_broken=True,
        ba3=["--engine", "chatwhisper", "--language", "yue", "--force-cpu"],
    ),
    # FunAudio (BA2 FunAudioEngine): FunASR SenseVoiceSmall on Cantonese +
    # OpenCC s2hk + Cantonese word fixups + the Cantonese BERT segmenter.
    Cell(
        command="transcribe",
        engine="funaudio",
        language="yue",
        fixture="transcribe/yue.wav",
        kind="segmentation",
        ba2_global=["--force-cpu"],
        ba2=["--funaudio", "--lang", "yue"],
        ba3=["--engine", "funaudio", "--language", "yue", "--force-cpu"],
    ),
    # paraformer-zh (BA2 FunAudioEngine, second model): the funasr Mandarin
    # paraformer + VAD + ct-punc-c. BA2 quirk: even on paraformer-zh it always
    # segments with `BertCantoneseUtteranceModel` (char-level / particle); BA3
    # mirrors that via `cantonese_inference=True` in the funaudio UtSeg path.
    Cell(
        command="transcribe",
        engine="paraformer",
        language="zh",
        fixture="transcribe/zh.wav",
        kind="segmentation",
        ba2_global=["--force-cpu"],
        ba2=["--paraformer", "--lang", "zho"],
        ba3=["--engine", "funaudio", "--model", "paraformer-zh", "--language", "zh", "--force-cpu"],
    ),
    # Tencent Cloud ASR: COS upload → async CreateRecTask with diarization →
    # poll → ResultDetail words. Faithful port of BA2's `TencentEngine`:
    # `16k_zh_large` for zh/yue/wuu/nan/hak, OpenCC s2hk + word_replacements
    # for yue, and `" ".join(words)` as the segmenter input.
    #
    # NO BYTE-IDENTICAL ORACLE: Tencent's async ASR is non-deterministic
    # across calls — the same audio bytes produce different `ResultDetail`
    # groupings (some calls return per-utterance RDs; some merge a speaker
    # turn into one long RD) and the SpeakerId cluster assignment varies.
    # That means BA2-vs-BA2 calls wouldn't agree either, so we cannot diff
    # BA2 against BA3. We use `ba2_broken=True` semantics (sanity-only) and
    # verify BA3 produces a non-empty, diarized Cantonese transcript.
    # `yue_long.wav` is yue.wav concatenated 2× so the COS upload exceeds the
    # qcloud_cos 1 MB single-PUT threshold (BA2 errors on smaller files).
    Cell(
        command="transcribe",
        engine="tencent",
        language="yue",
        fixture="transcribe/yue_long.wav",
        kind="segmentation",
        ba2_broken=True,
        ba3=["--engine", "tencent", "--language", "yue"],
    ),
    # ---- align (wav2vec MMS_FA; %wor word-level timings) ----
    Cell(
        command="align",
        engine="wav2vec",
        language="en",
        fixture="align/en.cha",
        kind="wor",
        # BA2 align defaults to wav2vec + --wor (and runs UTR, which re-derives
        # the same rev utterance times the fixture already carries).
        ba2=[],
        ba3=["--engine", "wav2vec"],
    ),
    # ---- align (whisper_fa: Whisper cross-attention DTW; BA2 --whisper_fa) ----
    # env_limited: the DTW attention is float-sensitive to torch (BA2 2.6.0 vs
    # BA3 2.10.0), so word-level %wor *bullets* shift by <200ms on multi-
    # utterance chunks. The faithful port is byte-identical on a single chunk
    # (utterance 1) and matches word-for-word everywhere; only the ms differ.
    Cell(
        command="align",
        engine="whisper_fa",
        language="en",
        fixture="align/en.cha",
        kind="wor",
        env_limited=True,
        ba2_global=["--force-cpu"],
        ba2=["--whisper_fa"],
        ba3=["--engine", "whisper_fa", "--force-cpu"],
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
    # ---- translate (NLLB-1.3B local; sanity-only) ----
    # tbtbt is the architectural reference; BA2 has no NLLB. Greedy decode
    # on facebook/nllb-200-distilled-1.3B is bit-deterministic only when
    # the underlying torch float ops match — and torch 2.11 (BA3 venv) vs
    # 2.12 (tbtbt venv) drives sub-token lexical drift on close
    # translations (same env-limited class as `whisper_fa`). Per the
    # parity charter ("if it's largely correct it's fine"), we don't
    # enforce byte-identical here; the cell verifies BA3 produces a
    # non-empty `%xtra:` translation tier.
    Cell(
        command="translate",
        engine="nllb",
        language="es-eng",
        fixture="translate/es.cha",
        kind="translate",
        ba2_broken=True,
        ba3=["--engine", "nllb", "--target", "eng"],
    ),
    # ---- translate (Tencent TMT cloud; sanity-only) ----
    # `[asr] engine.tencent.*` creds must be set. zh source — Tencent TMT
    # does not support yue (route Cantonese through NLLB or Aliyun). The
    # cloud TMT model is updated server-side and produces small word-level
    # drift between calls; sanity-only check verifies BA3 produces a
    # non-empty `%xtra:` translation.
    Cell(
        command="translate",
        engine="tencent",
        language="zh-eng",
        fixture="translate/zh.cha",
        kind="translate",
        ba2_broken=True,
        ba3=["--engine", "tencent", "--target", "eng"],
    ),
    # ---- translate (Aliyun MT cloud) — implementation only, no cell ----
    # The Aliyun MT service (product code `alimt`) is **not** activated on
    # the current cloud account, so the cell would hard-fail with
    # `Auth check failed! ... product:[alimt] ... is not activated`.
    # The backend (`batchalign.backends.translate.aliyun.AliyunTranslateBackend`)
    # is tbtbt-parity wired; once `alimt` is activated in the Aliyun
    # console, restore this cell (uncomment) to add yue→eng coverage:
    #
    #   Cell(
    #       command="translate", engine="aliyun", language="yue-eng",
    #       fixture="translate/yue.cha", kind="translate",
    #       ba2_broken=True,
    #       ba3=["--engine", "aliyun", "--target", "eng"],
    #   ),
    # ---- transcribe (Qwen3-ASR; sanity-only; Cantonese) ----
    # Open-weight ASR LLM from Alibaba. Local model (~3.4 GB fp16); first
    # run downloads from HuggingFace. tbtbt is the architectural
    # reference. `--force-cpu` because MPS is reportedly degraded on the
    # 1.7B model (tbtbt empirical eval 2026-05-26). Same torch-precision
    # drift class as NLLB — sanity-only.
    Cell(
        command="transcribe",
        engine="qwen3",
        language="yue",
        fixture="transcribe/yue.wav",
        kind="segmentation",
        ba2_broken=True,
        ba3=["--engine", "qwen3", "--language", "yue", "--force-cpu"],
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


def _strip_bullet(line: str) -> str:
    """Drop the alignment timing bullet from a main-tier line. BA2 emits a
    trailing ` <start>_<end>` (or a `\\x15..\\x15` bullet); for transcribe the
    important lines are speaker + segmentation + text, not the timing."""
    line = re.sub(r"\x15[^\x15]*\x15", "", line)   # \x15-delimited bullet
    line = re.sub(r"\s+\d+_\d+\s*$", "", line)     # plain " start_end"
    return line


def extract_segmentation(cha: str) -> list[str]:
    """Speaker + per-utterance segmentation + text (transcribe): the `*SPK:`
    lines, with timing bullets stripped (those are alignment/cosmetic here)."""
    return [_norm(_strip_bullet(l)) for l in cha.splitlines() if l.startswith("*")]


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
        [str(BA2_PY), "execute.py", *cell.ba2_global, cell.command, str(in_dir), str(out_dir), *cell.ba2],
        cwd=BA2_DIR,
    )
    produced = sorted(out_dir.glob("*.cha"))
    if not produced:
        raise RuntimeError(
            f"BA2 produced no .cha\nstdout:\n{proc.stdout[-2000:]}\nstderr:\n{proc.stderr[-2000:]}"
        )
    return produced[0]


def run_tbtbt(cell: Cell, fixture: Path, work: Path) -> Path:
    """Run tbtbt on `fixture`; return the produced `.cha` path.

    tbtbt's CLI is `batchalign3 <command> <PATH> -o <OUT> [opts]` (one or
    more inputs accepted; we pass a single file). Uses
    `--no-tui --no-server --sequential` for deterministic batch behavior
    and `--force-cpu` to avoid MPS-induced drift on Apple Silicon.
    """
    in_dir = work / "tbtbt_in"
    out_dir = work / "tbtbt_out"
    in_dir.mkdir(parents=True, exist_ok=True)
    out_dir.mkdir(parents=True, exist_ok=True)
    dst = in_dir / fixture.name
    shutil.copy(fixture, dst)
    for sib in fixture.parent.glob(fixture.stem + ".*"):
        if sib != fixture:
            shutil.copy(sib, in_dir / sib.name)

    cmd = [
        str(TBTBT_BIN),
        cell.command,
        "--no-tui", "--no-server", "--sequential", "--force-cpu",
        "-o", str(out_dir),
        *cell.tbtbt,
        str(dst),
    ]
    proc = _run(cmd, cwd=TBTBT_DIR)
    produced = sorted(out_dir.glob("*.cha"))
    if not produced:
        raise RuntimeError(
            f"tbtbt produced no .cha\nstdout:\n{proc.stdout[-2000:]}\n"
            f"stderr:\n{proc.stderr[-2000:]}"
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
    if proc.returncode != 0:
        # BA3 left the input .cha in place (since the input lives at
        # `target` and the output overwrites it on success). A non-zero
        # exit means the pipeline aborted mid-task — `out.exists()` alone
        # is not a sufficient liveness signal. Surface the failure with
        # the tail of stdout so the user sees the actual pipeline error.
        raise RuntimeError(
            f"BA3 exited {proc.returncode}\n"
            f"stdout:\n{proc.stdout[-2000:]}\nstderr:\n{proc.stderr[-2000:]}"
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

    # BA2-broken combination (e.g. Cantonese CHATWhisper): no oracle to diff —
    # only require that BA3 produces a non-empty, diarized result.
    if cell.ba2_broken:
        with tempfile.TemporaryDirectory(prefix="parity_") as tmp:
            try:
                ba3_cha = run_ba3(cell, fixture, Path(tmp))
            except Exception as exc:  # noqa: BLE001
                return Result(cell, False, detail=str(exc))
            ba3_lines = extractor(ba3_cha.read_text())
        passed = len(ba3_lines) > 0
        detail = (
            f"BA2 broken on this combination; BA3 produced "
            f"{len(ba3_lines)} important line(s)"
        )
        diff = "\n".join(ba3_lines) if (verbose or not passed) else ""
        return Result(cell, passed, detail=detail, diff=diff)

    with tempfile.TemporaryDirectory(prefix="parity_") as tmp:
        work = Path(tmp)
        try:
            if cell.oracle == "tbtbt":
                ref_cha = run_tbtbt(cell, fixture, work)
            else:
                ref_cha = run_ba2(cell, fixture, work)
            ba3_cha = run_ba3(cell, fixture, work)
        except Exception as exc:  # noqa: BLE001 - surface any engine failure
            return Result(cell, False, detail=str(exc))

        ba2_lines = extractor(ref_cha.read_text())
        ba3_lines = extractor(ba3_cha.read_text())

    # Environmentally-limited combinations (whisper_fa DTW): compare with the
    # timing bullets normalized out — word alignment + segmentation must match,
    # exact ms may differ by torch float precision.
    if cell.env_limited:
        ba2_lines = [_norm(_strip_bullet(l)) for l in ba2_lines]
        ba3_lines = [_norm(_strip_bullet(l)) for l in ba3_lines]

    passed = ba2_lines == ba3_lines
    ref_label = "TBTBT" if cell.oracle == "tbtbt" else "BA2"
    diff = ""
    if not passed or verbose:
        diff = "\n".join(
            difflib.unified_diff(
                ba2_lines, ba3_lines, fromfile=ref_label, tofile="BA3", lineterm=""
            )
        )
    detail = f"{len(ba2_lines)} important lines (oracle={cell.oracle})"
    if cell.env_limited:
        detail += " (env_limited)"
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
