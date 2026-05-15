# E722: GRA has no ROOT

**Last updated:** 2026-04-04 08:15 EDT

## Description

`%gra` tier has no ROOT relation. Every `%gra` tier must have exactly one relation with `head=0` or `head=self` (the ROOT of the dependency tree).

## Metadata

- **Error Code**: E722
- **Category**: validation
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Source**: `error_corpus/validation_errors/E722_gra_no_root.cha`
**Trigger**: No relation with head=0 or head=self (ROOT)
**Expected Error Codes**: E722

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I want .
%mor:	pro|I v|want .
%gra:	1|2|SUBJ 2|1|OBJ
@End
```

## Expected Behavior

The validator should report E722 (warning) because the `%gra` tier has no ROOT relation — neither relation has `head=0` or `head=self`.

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The previous example had a trailing ` .` on the `%gra` tier which caused tree-sitter to produce E316 (unparsable content). `%gra` tiers do not have terminators — removed the trailing period to allow the tier to parse correctly so the E722 structural validation can fire.
- E722 is emitted as a Warning (not Error) since 2026-02-14 due to non-conforming corpus data.
- Review and enhance this specification as needed
