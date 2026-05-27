"""Render error messages from the Rust pipeline for the CLI summary.

Two code paths:

1. **CHAT parse / validation errors** — the message reaches us in the
   shape `parse <path>: CHAT validation failed: error[E###]: <msg> (bytes A..B)`
   (or `... (line L, column C, bytes A..B)`). We extract the file
   path + byte span, read the file, compute line/column, and render
   the offending line with a caret marker pointing at the exact
   bytes. This is the case where we have enough information to be
   useful.

2. **Everything else** — collapse internal whitespace, strip a
   leading `XxxError:` Python class prefix, and return as a single
   tight line. No multi-line wrapping, no panel chrome.

Discriminator: presence of `bytes A..B` in the message AND a
readable file path. Anything missing either piece falls through to
the one-line normaliser.
"""

from __future__ import annotations

import re
from pathlib import Path
from typing import Any

from rich.console import Group, RenderableType
from rich.text import Text


_PARSE_PATH = re.compile(r"parse\s+(/[^:\n]+):", re.IGNORECASE)
_BYTE_SPAN  = re.compile(r"bytes\s+(\d+)\s*\.\.\s*(\d+)")
_LINE_COL   = re.compile(r"line\s+(\d+),\s*column\s+(\d+)", re.IGNORECASE)
_ERROR_CODE = re.compile(r"error\[(E\d+)\]:\s*(.*?)\s*\(", re.IGNORECASE | re.DOTALL)
_PY_PREFIX  = re.compile(r"^[A-Za-z_]\w*(?:Error|Exception)\s*:\s*", re.IGNORECASE)


def render_error(error: str | None) -> RenderableType:
    """Best-effort renderable for `error`.

    Returns a Rich `Group` for parse errors that we can locate in the
    source; otherwise returns a normalised one-line string suitable
    for `console.print`.
    """
    msg = error or "<no message>"
    rendered = _try_parse_error(msg)
    if rendered is not None:
        return rendered
    return _normalise_one_line(msg)


def is_rich(rendered: Any) -> bool:
    """`render_error` returns either a Rich renderable or a plain str."""
    return not isinstance(rendered, str)


# ---------------------------------------------------------------------------

def _try_parse_error(error: str) -> RenderableType | None:
    path_m = _PARSE_PATH.search(error)
    span_m = _BYTE_SPAN.search(error)
    if path_m is None or span_m is None:
        return None
    path = Path(path_m.group(1).strip())
    if not path.is_file():
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None

    start_byte = int(span_m.group(1))
    end_byte = int(span_m.group(2))

    # Spans from Rust are half-open `[start, end)`. The last byte IN
    # the span is `end - 1`; using `end` directly puts us on the line
    # after the span whenever the trailing byte is a newline, which
    # makes the renderer incorrectly tag a single-line span as
    # multi-line.
    last_byte = max(start_byte, end_byte - 1)

    # Prefer line/column from the error if present (parse-phase errors
    # have it); else compute from byte offsets (validation-phase errors).
    lc_m = _LINE_COL.search(error)
    if lc_m is not None:
        start_line = int(lc_m.group(1))
        start_col  = int(lc_m.group(2))
        end_line, end_col_inclusive = _byte_to_line_col(text, last_byte)
        end_col = end_col_inclusive + 1
    else:
        start_line, start_col = _byte_to_line_col(text, start_byte)
        end_line, end_col_inclusive = _byte_to_line_col(text, last_byte)
        end_col = end_col_inclusive + 1

    lines = text.splitlines()
    if not (1 <= start_line <= len(lines)):
        return None

    code_m = _ERROR_CODE.search(error)
    code = code_m.group(1) if code_m else "?"
    msg  = (code_m.group(2).strip() if code_m else _normalise_one_line(error))

    return _render_caret_block(
        path=path,
        start_line=start_line, start_col=start_col,
        end_line=end_line,     end_col=end_col,
        lines=lines, code=code, msg=msg,
    )


def _render_caret_block(
    *, path: Path, start_line: int, start_col: int,
    end_line: int, end_col: int, lines: list[str],
    code: str, msg: str,
) -> RenderableType:
    """Build a `Group` with: header, file:line:col, one line of prior
    context, the offending line, and a caret marker below it.
    """
    out: list[RenderableType] = []
    out.append(Text(f"error[{code}]: {msg}", style="bold red"))
    out.append(Text(f"{path}:{start_line}:{start_col}", style="dim"))
    out.append(Text(""))  # spacer

    # One line of prior context (when available) for orientation.
    if start_line >= 2:
        prev_text = lines[start_line - 2]
        out.append(Text(f"  {start_line - 1:>4} │ {prev_text}", style="dim"))

    # The offending line (single-line span) or first line of a
    # multi-line span — we only caret the first line for clarity.
    offending = lines[start_line - 1]
    out.append(Text(f"  {start_line:>4} │ {offending}"))

    # Caret marker. start_col / end_col are character positions on
    # this line (1-based). For multi-line spans we underline to EOL
    # so the user sees that the issue continues.
    line_len = len(offending)
    if end_line > start_line:
        caret_len = max(1, line_len - (start_col - 1))
        suffix = " …continues below"
    else:
        caret_len = max(1, end_col - start_col)
        suffix = ""
    marker = " " * (start_col - 1) + "^" * caret_len
    out.append(Text(f"       │ {marker}{suffix}", style="red"))

    return Group(*out)


def _byte_to_line_col(text: str, byte_offset: int) -> tuple[int, int]:
    """Convert a UTF-8 byte offset to (1-based line, 1-based column).

    Computed by re-encoding the text and locating the offset within
    the byte stream. Column is character-based on the affected line
    (which matches how a user reads the file).
    """
    data = text.encode("utf-8")
    if byte_offset > len(data):
        byte_offset = len(data)
    if byte_offset < 0:
        byte_offset = 0
    prefix_bytes = data[:byte_offset]
    line = prefix_bytes.count(b"\n") + 1
    last_nl = prefix_bytes.rfind(b"\n")
    line_start = 0 if last_nl == -1 else last_nl + 1
    # Decode the portion of the line up to the offset to count chars.
    line_prefix = data[line_start:byte_offset].decode("utf-8", errors="replace")
    col = len(line_prefix) + 1
    return line, col


def normalise_one_line(error: str | None) -> str:
    """Strip a Python class prefix, unwrap Rust → Python error envelopes,
    and collapse internal whitespace into a single grep-friendly line.

    The Rust engine layers wrappers on a backend exception:

        worker dispatch failed: backend error: worker dispatch failed:
        Backend.call raised: <Inner Python Exception>

    Everything outside `Backend.call raised:` is dispatch boilerplate
    the user can't act on. We keep only the innermost exception.
    """
    if not error:
        return ""
    out = re.sub(r"\s+", " ", error).strip()
    # Unwrap the Rust dispatch envelope, keeping just what Python raised.
    m = re.search(r"Backend\.call raised:\s*(.+)$", out)
    if m:
        out = m.group(1).strip()
    # Strip a redundant `XxxError:` Python prefix only when no message
    # follows (i.e. it's pure noise). When the prefix carries the
    # type name with a real message after it (`Exception: msg`), keep
    # the shape so the user sees `Exception: msg`.
    if not re.search(r":\s*\S", out):
        out = _PY_PREFIX.sub("", out).strip()
    return out


# Internal alias kept for the parse-error renderer.
_normalise_one_line = normalise_one_line


__all__ = ["render_error", "is_rich", "normalise_one_line"]
