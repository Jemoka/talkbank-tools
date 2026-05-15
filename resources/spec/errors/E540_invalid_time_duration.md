# E540: `@Time Duration` does not match a legal CLAN time pattern

## Description

An `@Time Duration` header must match one of the three time patterns
that CLAN's authoritative `depfile.cut` declares legal:

```
@Time Duration:	@t<hh:mm-hh:mm>  @t<hh:mm:ss-hh:mm:ss>  @t<hh:mm:ss>
```

That is, the value is exactly one of:

- `HH:MM-HH:MM` — two times separated by a hyphen, hours+minutes only
- `HH:MM:SS-HH:MM:SS` — two times separated by a hyphen, hours+minutes+seconds
- `HH:MM:SS` — single time, hours+minutes+seconds

Nothing else: no semicolon separator, no comma-joined multi-segment
value, no millisecond suffix, no `MM:SS` two-component form. CLAN
CHECK rejects anything else; Rust chatter must match.

## Metadata

- **Status**: implemented
- **Status note**: Extended 2026-04-21 from "free-text is flagged"
  to "must match one of three depfile.cut patterns exactly".
  Historical Rust validator also accepted semicolon separators and
  comma-joined multi-segment values; both are non-depfile. Severity
  raised from `Warning` to `Error` to match CLAN's rejection
  semantics.
- **Last updated**: 2026-04-21 23:55 EDT
- **Error Code**: E540
- **Category**: header_validation
- **Level**: header
- **Layer**: validation

## Example 1 (semicolon separator — not in depfile)

**Trigger**: `@Time Duration: 00:00:00;01:30:00` — semicolon is not
one of the depfile separators.

**Expected Error Codes**: E540

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Duration:	00:00:00;01:30:00
*CHI:	hello .
@End
```

## Example 2 (comma-joined multiple segments — not in depfile)

**Trigger**: `@Time Duration: 00:01:00, 00:02:00` — comma-joined
segments are not in the depfile template.

**Expected Error Codes**: E540

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Duration:	00:01:00, 00:02:00
*CHI:	hello .
@End
```

## Example 3 (free text)

**Trigger**: `@Time Duration: not-a-time` — not remotely a time.

**Expected Error Codes**: E540

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Duration:	not-a-time
*CHI:	hello .
@End
```

## Example 4 (MM:SS two-component — not in depfile)

**Trigger**: `@Time Duration: 30:45` — two components only, which
matches `@Time Start` patterns but not `@Time Duration` templates.

**Expected Error Codes**: E540

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;06.||||Target_Child|||
@Time Duration:	30:45
*CHI:	hello .
@End
```

## Expected Behavior

- **Parser**: accepts any non-empty string as the duration value.
- **Validator**: reports E540 at the header span when the value
  does not match exactly one of:
  - `^\d+:\d+-\d+:\d+$` — `HH:MM-HH:MM`
  - `^\d+:\d+:\d+-\d+:\d+:\d+$` — `HH:MM:SS-HH:MM:SS`
  - `^\d+:\d+:\d+$` — `HH:MM:SS`

Component widths are not fixed (`\d+` rather than `\d{2}`) because
real corpora use `1:30` as well as `01:30`; depfile.cut's `hh` /
`mm` / `ss` slots admit any digit count. The rejection is about
**shape**, not component padding.

## CHAT Rule

CLAN `depfile.cut`:

```
@Time Duration:	@t<hh:mm-hh:mm> @t<hh:mm:ss-hh:mm:ss> @t<hh:mm:ss>
```

CHAT manual: <https://talkbank.org/0info/manuals/CHAT.html#Time_Duration_Header>

## Notes

- Authoritative source: `clan-info/lib/depfile.cut:33`.
- Severity upgraded from `Warning` to `Error` to mirror CLAN's
  rejection.
- Roundtrip fidelity is preserved: invalid values are still stored
  in `TimeDuration::Unsupported` for lossless serialization.
