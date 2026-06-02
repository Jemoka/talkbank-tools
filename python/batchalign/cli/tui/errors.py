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
# Match `error[E###]: <headline> (bytes A..B)` up to the *byte span* paren,
# not the first `(`. The headline frequently contains a quoted `(` itself
# (e.g. `Unparsable content on main tier: '('`), and the older `.*?\s*\(`
# pattern stopped on that, producing truncated headlines like
# `Unparsable content on main tier: '`. Anchoring on `\(bytes` is the only
# unambiguous delimiter between headline and span.
_ERROR_CODE = re.compile(
    r"error\[(E\d+)\]:\s*(.*?)\s*\(bytes\s+\d+",
    re.IGNORECASE | re.DOTALL,
)
# Iterate every error entry in a multi-error parse message. Captures
# (code, headline, start, end) for each — the renderer stacks one caret
# block per match.
_ERROR_ENTRY = re.compile(
    r"error\[(E\d+)\]:\s*(.*?)\s*\(bytes\s+(\d+)\s*\.\.\s*(\d+)\)",
    re.IGNORECASE | re.DOTALL,
)
_PY_PREFIX  = re.compile(r"^[A-Za-z_]\w*(?:Error|Exception)\s*:\s*", re.IGNORECASE)

# Matches one bullet line from a multi-error summary (see
# `crates/batchalign/batchalign-core/src/taskrunners/morphosyntax.rs::
# format_alignment_errors`). The Rust runner emits multi-line messages
# of the form:
#
#   morphosyntax produced N misaligned tier(s):
#     error[E705]: <headline> (bytes A..B)
#     error[E706]: <headline> (bytes C..D)
#
# We detect this shape so the renderer can print one line per error
# (instead of letting Rich soft-wrap the whole blob into a paragraph).
_MULTI_ERROR_BULLET = re.compile(
    r"^\s*error\[(E\d+)\]:\s*(.*?)\s*\(bytes\s+(\d+)\s*\.\.\s*(\d+)\)\s*$"
)


def render_error(error: str | None) -> RenderableType:
    """Best-effort renderable for `error`.

    Tries in order:
      1. **CHAT parse error with byte span** → caret block (see
         `_try_parse_error`).
      2. **Multi-error summary** (e.g. a batch of `%mor` alignment
         failures from the morphosyntax runner) → one bullet per error,
         no soft-wrap (see `_try_multi_error_block`).
      3. **Anything else** → collapsed one-liner.
    """
    msg = error or "<no message>"
    rendered = _try_parse_error(msg)
    if rendered is not None:
        return rendered
    rendered = _try_multi_error_block(msg)
    if rendered is not None:
        return rendered
    return _normalise_one_line(msg)


def is_rich(rendered: Any) -> bool:
    """`render_error` returns either a Rich renderable or a plain str."""
    return not isinstance(rendered, str)


# ---------------------------------------------------------------------------

def _try_multi_error_block(error: str) -> RenderableType | None:
    """Detect and render a multi-error summary as a clean bullet list.

    Triggers when the message has at least two lines that match the
    `error[E###]: ... (bytes A..B)` bullet shape. Returns a Rich
    `Group` of one styled line per bullet — short codes get a hint of
    colour, byte spans stay dim, the headline is plain. Crucially,
    each line is rendered with `no_wrap=True` so a long headline gets
    elided with an ellipsis instead of word-wrapping into a paragraph
    and destroying the column alignment.
    """
    raw_lines = error.splitlines()
    if len(raw_lines) < 2:
        return None

    # First line is the header ("morphosyntax produced N misaligned
    # tiers:") and the remainder are bullets. We don't require the
    # exact header text — any preamble line followed by ≥2 bullets is
    # enough to engage the multi-error renderer.
    bullets: list[tuple[str, str, str, str]] = []  # (code, headline, start, end)
    preamble = ""
    for i, line in enumerate(raw_lines):
        m = _MULTI_ERROR_BULLET.match(line)
        if m is not None:
            bullets.append((m.group(1), m.group(2), m.group(3), m.group(4)))
        elif not bullets and not preamble:
            preamble = line.strip()

    if len(bullets) < 2:
        return None

    out: list[RenderableType] = []
    if preamble:
        # Strip a Python-class envelope ("ValueError: morphosyntax produced …")
        # so the preamble reads like a sentence.
        cleaned = _PY_PREFIX.sub("", preamble).strip()
        # Trim the trailing colon — the bullets ARE the list, so a
        # bare "morphosyntax produced 3 misaligned tiers" reads better
        # than "...tiers:" followed by indented items.
        cleaned = cleaned.rstrip(":").strip()
        if cleaned:
            out.append(Text(cleaned, style="bold red"))

    for code, headline, start, end in bullets:
        line = Text(no_wrap=True, overflow="ellipsis")
        line.append("    ")
        line.append(code, style="bold yellow")
        line.append("  ")
        line.append(headline)
        line.append(f"  bytes {start}..{end}", style="dim")
        out.append(line)

    return Group(*out)


def _try_parse_error(error: str) -> RenderableType | None:
    """Render a CHAT parse failure as a stacked set of caret blocks.

    Handles the two shapes the engine emits today:

    - Single error: `parse <path>: CHAT parse error: error[E###]: ... (bytes A..B)`
    - Multiple errors in one message (one of `parse_and_validate`'s
      common failure modes): the same prefix, then a flat run of
      `error[E###]: ... (bytes A..B)` entries.

    Each entry gets its own caret block; the file path is shown once
    as a dim header (basename + `(line:col, …)`) rather than once per
    entry, to keep the summary tight when 5+ errors come back from one
    file.
    """
    path_m = _PARSE_PATH.search(error)
    if path_m is None:
        return None
    path = Path(path_m.group(1).strip())
    if not path.is_file():
        return None
    try:
        text = path.read_text(encoding="utf-8", errors="replace")
    except OSError:
        return None

    entries = list(_ERROR_ENTRY.finditer(error))
    if not entries:
        return None

    lines = text.splitlines()

    # Prefer parser-supplied (line, column) when present — the first error
    # in a parse-phase failure usually carries one. Computed from the byte
    # offset otherwise; same fallback the old single-error path used.
    lc_m = _LINE_COL.search(error)

    blocks: list[RenderableType] = []
    for i, m in enumerate(entries):
        code = m.group(1)
        msg = m.group(2).strip()
        start_byte = int(m.group(3))
        end_byte = int(m.group(4))
        last_byte = max(start_byte, end_byte - 1)

        if i == 0 and lc_m is not None:
            start_line = int(lc_m.group(1))
            start_col = int(lc_m.group(2))
        else:
            start_line, start_col = _byte_to_line_col(text, start_byte)
        end_line, end_col_inclusive = _byte_to_line_col(text, last_byte)
        end_col = end_col_inclusive + 1

        if not (1 <= start_line <= len(lines)):
            continue

        if i > 0:
            blocks.append(Text(""))  # spacer between stacked blocks
        blocks.append(
            _render_caret_block(
                start_line=start_line, start_col=start_col,
                end_line=end_line, end_col=end_col,
                lines=lines, code=code, msg=msg,
            )
        )

    if not blocks:
        return None

    # The failure-row label ("fail  eifersucht.cha") already names the
    # file, so we deliberately do NOT add a basename header here — that
    # would repeat the same name one line below itself.
    return Group(*blocks)


def _render_caret_block(
    *, start_line: int, start_col: int,
    end_line: int, end_col: int, lines: list[str],
    code: str, msg: str,
) -> RenderableType:
    """Build a `Group` with: header, one line of prior context, the
    offending line, and a caret marker below it.

    The file path is rendered ONCE by the caller (above the stack of
    blocks) — repeating it per block would make a 5-error summary
    unreadable.
    """
    out: list[RenderableType] = []
    out.append(Text(f"error[{code}]: {msg}", style="bold red"))
    out.append(Text(f"  at line {start_line}, column {start_col}", style="dim"))

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
