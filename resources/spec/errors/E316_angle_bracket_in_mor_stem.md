# E316: Angle-bracketed annotation inside %mor stem is invalid

## Description

A `%mor` tier entry contains an angle-bracketed prefix inside the stem
position (e.g., `noun|<sos>tos`, `sconj|<sos>tos~aux|...`). The CHAT
manual's %mor grammar uses these separators inside the stem:
`-` (feature), `&` (fusion), `#` (prefix), `:` (category),
`~` (clitic), `+` (compound). **Angle brackets are not valid stem
content.** The parser produces an ERROR node at the `<` and the
validator reports E316 on the surrounding `|<stem>~...` region.

This pattern is a data error. It is observed across the CHILDES Finnish
Kirjavainen-MPI corpus (18 files as of 2026-04-14) where an annotator
introduced `<sos>` as a non-standard annotation prefix on a specific
Finnish stem. No other bank in the public corpus uses it. CLAN's
`check` behavior on this pattern should be consulted before any
grammar change; this spec locks in the current-correct rejection so
the parser cannot silently start accepting invalid CHAT later.

## Metadata
- **Status**: implemented
- **Last updated**: 2026-04-14 22:00 EDT

- **Error Code**: E316
- **Category**: Dependent tier validation
- **Level**: tier
- **Layer**: parser

## Example 1

**Trigger**: `<sos>` annotation appears as a prefix on the stem of a
`%mor` tier entry. The parser cannot tokenize the boundary between the
POS pipe and the stem text.

**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	fin
@Participants:	CHI Target_Child
@ID:	fin|test|CHI|||||Target_Child|||
*CHI:	kato tos .
%mor:	intj|kato noun|<sos>tos .
@End
```

## Example 2

**Trigger**: Same angle-bracket stem prefix inside a clitic chain, as
seen in the real Kirjavainen-MPI data: `sconj|<sos>tos~aux|...`. The
error still fires at the `|<` boundary; the clitic `~` is never
reached.

**Expected Error Codes**: E316

```chat
@UTF8
@Begin
@Languages:	fin
@Participants:	CHI Target_Child
@ID:	fin|test|CHI|||||Target_Child|||
*CHI:	tos ei .
%mor:	sconj|<sos>tos~aux|ei-Fin-Neg-S3 .
@End
```

## Expected Behavior

The parser must emit E316 on the unparsable `|<stem>` region. Downstream
tooling should surface the error with the file path, line, and column
so the corpus maintainer can locate and correct the data.

## Remediation guidance (for data maintainers)

When E316 fires on this pattern, the fix is in the corpus, not the
parser. The `<sos>` annotation should be removed from the stem or
replaced with a documented CHAT convention (the CHAT manual's `#`
prefix separator or a comment tier may be appropriate depending on
intent). See the Kirjavainen-MPI README for any documented annotator
conventions before deciding on the exact replacement.
