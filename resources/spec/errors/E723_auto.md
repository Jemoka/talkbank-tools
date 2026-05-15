# E723: GRA has multiple ROOTs

**Last updated:** 2026-04-04 08:15 EDT

## Description

`%gra` tier has multiple ROOT relations. Every `%gra` tier should have exactly one ROOT (relation with `head=0` or `head=self`).

## Metadata

- **Error Code**: E723
- **Category**: validation
- **Level**: tier
- **Layer**: validation
- **Status**: implemented

## Example 1

**Source**: `error_corpus/validation_errors/E723_gra_multiple_roots.cha`
**Trigger**: Multiple relations with head=self (ROOT), excluding terminator PUNCT
**Expected Error Codes**: E723

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	I want cookie .
%mor:	pro|I v|want n|cookie .
%gra:	1|1|ROOT 2|2|ROOT 3|1|OBJ 4|1|PUNCT
@End
```

## Expected Behavior

The validator should report E723 (warning) because the `%gra` tier has 2 ROOT relations (indices 1 and 2 both have `head=self`).

**Trigger**: See example above

## CHAT Rule

See CHAT manual sections on dependent tier formats (%mor, %gra, %pho, etc.). Each tier type has specific syntax requirements. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The previous example had a trailing ` .` on the `%gra` tier and only 2 words. `%gra` tiers do not have terminators. Also, the validation excludes the last item (terminator PUNCT) from root counting, so a 3-word example is needed to trigger E723 with 2 non-terminator roots.
- E723 is emitted as a Warning (not Error) since 2026-02-14 due to non-conforming corpus data.
- Review and enhance this specification as needed
