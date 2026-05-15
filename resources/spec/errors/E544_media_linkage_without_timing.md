# E544: `@Media` claims linkage but transcript has no timing evidence

## Description

An `@Media` header declares a linked media file (no `unlinked` /
`missing` / `notrans` status), but the transcript body contains no
evidence that any utterance is actually linked to that media. By
the CHAT manual's `@Media` semantics, an unqualified declaration is
a promise that the transcript is time-linked to the named file;
this check catches transcripts that make that promise without
keeping it.

Rust chatter does not currently enforce this rule. Legacy Java
Chatter did (as `semantic failure: not 'unlinked' or 'missing' or
'notrans', but there are not bullets in transcript`). This spec
proposes reinstating the check in Rust with a broader definition of
"timing evidence" that accounts for CHAT format evolution since
Java development ceased.

## Metadata

- **Status**: implemented
- **Status note**: Validator lives inline in
  `crates/talkbank-model/src/model/file/chat_file/validate.rs`
  (`check_media_linkage_has_timing`), called from `ChatFile::validate`
  after the E362 bullet-monotonicity check so it can reuse the
  already-collected main-tier bullets. Timing evidence: main-tier
  bullets OR positional `%wor` sidecar. Affected reference-corpus
  fixtures carry `, unlinked` status.
- **Last updated**: 2026-05-01 09:47 EDT

- **Error Code**: E544
- **Category**: header_validation
- **Level**: file
- **Layer**: validation

## Resolved design decisions

- **Rule scope**: fires iff `@Media` header is present AND its
  `status` field is `None`. Absence of `@Media` → no check.
- **Timing evidence**: any bullet anywhere in the file (main-tier
  trailing bullets, `%wor` inline bullets, `@Bg`/`@Eg` time ranges).
  Broader than Java's original "main-tier bullets only" to honor
  modern CHAT timing surfaces; the project lead did not specifically narrow
  this, so we adopt the inclusive reading.
- **Severity**: error (blocking). Matches Java Chatter's severity
  and the project lead's phrasing ("put that back in").
- **Option gating**: none. The check runs unconditionally. If
  sign-language or other convention-specific fixtures need an
  exception in practice, revisit with a narrower spec.

## CHAT background

The `@Media` header has three comma-separated fields (the third
optional):

```
@Media:	<filename>, <media-type>[, <status>]
```

- `<filename>` — media basename (no extension)
- `<media-type>` — `audio` or `video`
- `<status>` — optional; one of `unlinked`, `missing`, `notrans`

Status semantics:

| Status | Meaning | Timing required? |
|---|---|---|
| *(absent)* | Transcript is linked to the media file | **Yes** |
| `unlinked` | Media file could be linked but currently isn't | No |
| `missing` | Media file has been lost / is unavailable | No |
| `notrans` | Media exists but this file has no transcription | No |

**Timing evidence** — at minimum, a bullet on some utterance
(`\x15<start>_<end>\x15` following the main-tier text). Modern CHAT
additionally carries timing via:

- `%wor` tier with per-word bullets (`WorTimingSidecar`)
- Utterance-level start/end timestamps (some batchalign3 outputs)
- `@Bg` / `@Eg` gem boundaries with time ranges

**Open question:** which of these count as satisfying E544? See
"Open questions" at the end of this spec. A minimally-principled
implementation treats *any* bullet anywhere — main-tier utterance
bullet, `%wor` bullet, `@Bg`/`@Eg` time range — as evidence.

## Example 1

**Trigger**: `@Media` declares linked audio but no utterance carries
a timing bullet.

**Expected Error Codes**: E544

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|3;00.||||Target_Child|||
@Media:	session-01, audio
*CHI:	hello world .
@End
```

## Example 2

**Trigger**: Same pattern with `video`. The rule does not distinguish
audio from video; any unqualified `@Media` header requires timing.

**Expected Error Codes**: E544

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|2;00.||||Target_Child|||
@Media:	session-01, video
*CHI:	hello .
@End
```

## Counter-examples (documentation only — do not fire E544)

Two cases where the check must not fire. These are recorded inline for
clarity but are not separate `Example` sections because the spec
generator would treat them as positive test cases.

1. **`@Media` with `unlinked` / `missing` / `notrans` status.** Author
   is honest about no linkage; check does not apply.
   ```
   @Media:	session-01, audio, unlinked
   ```

2. **`@Media` without status, but at least one utterance carries a
   timing bullet.** The linkage promise is kept.
   ```
   @Media:	session-01, audio
   *CHI:	hello world . ·0_1200·
   ```

## Expected Behavior

- **Parser**: must succeed — syntax is valid; this is a consistency
  check between header state and body state.
- **Validator**: must report E544 at the `@Media` header's span when
  both conditions hold:
  1. The `@Media` header's `status` field is `None`.
  2. The `ChatFile`'s utterance bodies contain no timing evidence
     (see "CHAT background" above for the precise scope of "timing
     evidence" — final scope pending open-question resolution).

The check is file-level: it requires a single pass over `file.lines`
inspecting the media header plus every utterance's main/dependent
tiers. It is cheap and should run unconditionally during validation.

## Remediation guidance (for data maintainers)

When E544 fires, the `@Media` declaration is inconsistent with
the transcript content. Three principled fixes:

1. **Add the correct status** — if media isn't actually linked,
   change `@Media: foo, audio` to `@Media: foo, audio, unlinked`.
   This is almost always the right fix for test fixtures.
2. **Add timing evidence** — if the transcript *should* be linked,
   add bullets (or a `%wor` tier with bullets, etc.) to at least
   one utterance.
3. **Remove the `@Media` header** — if the file does not relate to
   any media at all, drop the declaration entirely.

## Affected reference-corpus files

The following three files in `resources/corpus/reference/` currently pass
Rust validation but would be re-classified as invalid once E544
lands:

- `resources/corpus/reference/annotation/long-features.cha`
  — `@Media: long-features, audio`, no bullets
- `resources/corpus/reference/annotation/groups-sign.cha`
  — `@Media: groups-sign, video`, no bullets
- `resources/corpus/reference/tiers/sin.cha`
  — `@Media: sin, video`, no bullets

These are test fixtures exercising tier / annotation grammar; they
do not model realistic linked-media scenarios. The recommended fix
is option 1 above (add `, unlinked`). No corpus data outside
`reference/` is expected to be affected, but a full-corpus scan
should run before landing the validator to quantify breadth.

## Notes

- Related: E535 (unsupported `@Media` type), E536 (unsupported
  `@Media` status). Those fire on malformed field values; E544 is
  the semantic-integrity counterpart.
- Related: E401 (duplicate dependent tiers), E404 (orphaned
  dependent tier) — similar header/body cross-consistency checks
  already exist at the file level.
- Rust types touched: `MediaHeader` at
  `crates/talkbank-model/src/model/header/media.rs` and
  `MediaStatus` at `crates/talkbank-model/src/model/header/enums.rs`.
- Implementation home: a new validator module at
  `crates/talkbank-model/src/validation/header/media_linkage.rs`
  (suggested — reuses the per-file validation pattern of E401 /
  E404).

## Review history

- **The project lead approved the rule.** Email exchange:
  "I noticed that I didn't carry over a Java chatter
  validation to the new chatter. The Java chatter had a requirement
  that every transcript must have bullets unless the Media header
  said 'unlinked' or 'missing' or 'notrans'. Should I put this
  validation back?" → Reply: "Yes, please put that back in.
  However, if there is no `@Media` header then it is assumed that
  there are no bullets." The clarification scopes the check to
  files that *have* an `@Media` header; see "Resolved design
  decisions" above. The three remaining design questions (timing-
  evidence scope, severity, option gating) were not specifically
  raised with the project lead and are resolved with the defaults listed in
  that section.

## CHAT Rule

See the CHAT manual, `@Media` header section:
https://talkbank.org/0info/manuals/CHAT.html

The manual specifies the status-field semantics that this check
enforces. The cross-consistency rule itself is implicit in those
semantics — a `<status>`-less declaration asserts linkage, and
asserting without evidence is ill-formed data.
