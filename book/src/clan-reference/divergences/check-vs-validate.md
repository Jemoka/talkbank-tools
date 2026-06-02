# CHECK vs `chatter validate`

**Status:** Current
**Last updated:** 2026-05-12 21:18 EDT

The `chatter` CLI has two validation tools with different purposes:

| | `chatter clan check` | `chatter validate` |
|---|---|---|
| **Purpose** | CLAN CHECK compatibility | Modern validation workflow |
| **Audience** | Users migrating from CLAN | Day-to-day development |
| **Output format** | CLAN-style `*** File "path": line N.` | Rich diagnostics with context, spans, suggestions |
| **Error codes** | CHECK numbers (1–161) | Typed codes (`E###`, `W###`) |
| **Flags** | CLAN `+flag` syntax | Modern `--flag` syntax |
| **Caching** | No | Yes (SQLite-backed) |
| **Directory support** | Single file | Recursive with parallelism |
| **JSON output** | Via `--format json` | Via `--format json` |
| **Fix suggestions** | No | Yes (some errors) |
| **Exit code** | 0 = clean, 1 = errors | 0 = clean, 1 = errors |

## Same Validation Engine

Both tools run the same underlying `parse_and_validate_streaming` pipeline from `talkbank-transform`. They catch the same errors — the difference is how errors are *presented*.

```text
                    ┌──────────────────────────────┐
                    │  talkbank-transform           │
                    │  parse_and_validate_streaming  │
                    └──────────┬───────────────────┘
                               │
              ┌────────────────┼────────────────┐
              ▼                                 ▼
   chatter clan check                  chatter validate
   (CLAN-compatible output)            (modern diagnostics)
```

## When to Use Which

### Use `chatter clan check` when:

- **Migrating from CLAN**: You have scripts that parse CHECK output format
- **Error filtering**: You need CHECK's `+eN`/`-eN` to filter by error number
- **Specific CLAN checks**: You need `+g2` (Target_Child), `+g5` (unused speakers)
- **Comparing with colleagues** who use original CLAN CHECK

### Use `chatter validate` when:

- **Day-to-day work**: Richer error messages with context and suggestions
- **Batch validation**: Directory-wide validation with caching and parallelism (`-j` flag)
- **CI/CD pipelines**: JSON output (`--format json`) for machine parsing
- **Performance**: SQLite cache avoids re-validating unchanged files
- **Watch mode**: `chatter watch` provides continuous validation on save

## Output Comparison

The same error looks different in each tool. For a file declaring `MOT` in `@Participants:` but with no matching `@ID:` line:

**`chatter clan check sample.cha`**:
```text
*** File "sample.cha": line 4.
@Participants:	CHI Target_Child, MOT Mother
Speaker 'MOT' declared in @Participants but has no matching @ID header(60)
```

**`chatter validate sample.cha`**:
```text
× error[E522]: Speaker 'MOT' declared in @Participants but has no matching
│ @ID header (line 4, column 1, bytes 29..73)
 ╭─[sample.cha:4:1]
 4 │ @Participants:  CHI Target_Child, MOT Mother
   · ──────────────────────┬─────────────────────
   ·                       ╰── here
 ╰────
help: Add @ID header: @ID:	<lang>|<corpus>|MOT|<age>|<sex>|<group>|<ses>|
      Mother|<edu>|<custom>|
```

Same underlying check (`E522` on the typed side ↔ CHECK number `60` on the legacy side); different rendering.

## Additional Checks in CHECK

`chatter clan check` supports a few checks not available in `chatter validate`:

| Check | Flag | Description |
|-------|------|-------------|
| Target_Child | `+g2` / `--check-target` | Verifies CHI participant has Target_Child role |
| Unused speakers | `+g5` / `--check-unused` | Reports speakers in @Participants but never used |
| UD features | `+u` / `--check-ud` | Validates Universal Dependencies features on %mor |

These are CHECK-specific because they are CLAN research conventions rather than
CHAT format requirements. `chatter validate` checks format correctness; CHECK
additionally checks research conventions.

## Additional Features in `chatter validate`

| Feature | Description |
|---------|-------------|
| Caching | SQLite cache skips unchanged files |
| Parallelism | `-j N` for multi-core directory validation |
| Watch mode | `chatter watch` for continuous validation |
| Fix suggestions | Some errors include actionable `help:` suggestions |
| Roundtrip test | `--roundtrip` serializes and re-parses to verify fidelity |
| Quiet mode | `--quiet` suppresses success output (exit code only) |
| Max errors | `--max-errors N` stops after N errors |

## Error Number Mapping

The mapping table from typed `ErrorCode` to CHECK number 1–161 lives in
`crates/clan/clan-core/src/commands/check/error_map.rs` (`check_error_number`).
The match arms there are the live source of truth — count them directly
rather than caching a stale number in this doc. Variants without a CHECK
equivalent fall through to `0`; those errors render as `[<code>]` in
CHECK output instead of `(<n>)`.

Use `chatter clan check +e` to see all 161 CHECK error messages (the
canonical strings from CLAN's `check_mess()`).
