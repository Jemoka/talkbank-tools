# E541: `@Time Start` does not match a legal CLAN time pattern

## Description

An `@Time Start` header must match one of the two time patterns that
CLAN's authoritative `depfile.cut` declares legal:

```
@Time Start:	@t<hh:mm:ss>  @t<mm:ss>
```

That is, the value is exactly one of:

- `HH:MM:SS` — hours, minutes, seconds
- `MM:SS` — minutes, seconds

Nothing else: no millisecond suffix (`.mmm`), no range form with a
hyphen, no comma-joined segments. CLAN CHECK rejects anything else;
Rust chatter must match.

## Metadata

- **Status**: implemented
- **Status note**: Extended 2026-04-21 from "basic format check" to
  "must match one of two depfile.cut patterns exactly". Historical
  Rust validator also accepted a `.mmm` millisecond suffix; that
  form is not in depfile.cut. Severity raised from `Warning` to
  `Error` to match CLAN's rejection semantics.
- **Last updated**: 2026-04-21 23:55 EDT
- **Error Code**: E541
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1 (milliseconds — not in depfile)

**Trigger**: `@Time Start: 01:23:45.678` — the `.mmm` suffix is not
in the depfile template.

**Expected Error Codes**: E541

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Start:	01:23:45.678
*CHI:	hello .
@End
```

## Example 2 (MM:SS.mmm — not in depfile)

**Trigger**: `@Time Start: 23:45.678` — two-component with millis,
not in the depfile template.

**Expected Error Codes**: E541

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Start:	23:45.678
*CHI:	hello .
@End
```

## Example 3 (free text)

**Trigger**: `@Time Start: not-a-time`.

**Expected Error Codes**: E541

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Start:	not-a-time
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: accepts any non-empty string as the start value.
- **Validator**: reports E541 at the header span when the value
  does not match exactly one of:
  - `^\d+:\d+:\d+$` — `HH:MM:SS`
  - `^\d+:\d+$` — `MM:SS`

Component widths are not fixed (`\d+` rather than `\d{2}`) because
depfile.cut's `hh`/`mm`/`ss` slots admit any digit count.

## CHAT Rule

CLAN `depfile.cut`:

```
@Time Start:	@t<hh:mm:ss> @t<mm:ss>
```

CHAT manual: <https://talkbank.org/0info/manuals/CHAT.html#Time_Start_Header>

## Notes

- Authoritative source: `clan-info/lib/depfile.cut:34`.
- Severity upgraded from `Warning` to `Error`.
- Roundtrip fidelity is preserved: invalid values are stored in
  `TimeStart::Unsupported`.
