# E210: Deprecated — replaced by E387

**Status:** Current
**Last updated:** 2026-04-04 08:15 EDT

## Description

**Deprecated.** This error code was replaced by E387 (`ReplacementOnFragment`).
The validation logic now emits E387 instead of E210 for the same condition.

## Metadata

- **Error Code**: E210
- **Category**: Word validation
- **Level**: word
- **Layer**: validation
- **Status**: deprecated

## Example 1

**Trigger**: Replacement on word with `&+` prefix (phonological fragment)
**Expected Error Codes**: E387

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	&+fri [: friend] world .
@End
```

## Notes

- E210 is deprecated. See E387 for the active error code.
- The example above triggers E387, not E210.
