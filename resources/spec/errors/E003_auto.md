# E003: Empty string input

**Last updated:** 2026-04-04 08:28 EDT

## Description

The input string is empty. E003 (EmptyString) is the default error code for
empty `NonEmptyString` fields during model validation, but an empty *file*
does not trigger E003 end-to-end. Instead, the parser produces header
validation errors (missing @UTF8, @End, @Participants, etc.) and E316
(unparsable content) because there are no headers to find.

## Metadata
- **Status**: not_implemented

- **Error Code**: E003
- **Category**: validation
- **Level**: file
- **Layer**: parser

## Example 1

**Source**: `error_corpus/parse_errors/E003_empty_string.cha`
**Trigger**: Empty input — no CHAT content at all
**Expected Error Codes**: E316, E502, E503, E504

Note: E003 is not reachable from an empty file. The parser emits header
validation errors instead. E003 fires internally for empty `NonEmptyString`
fields, not at the file level.

```chat

```

## Expected Behavior

The parser should reject this CHAT input and report a parse error at the location of the invalid syntax.

**Trigger**: See example above

## CHAT Rule

See the CHAT manual for format specifications: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- E003 (EmptyString) is used as the default error code in `NonEmptyString`
  validation when no field-specific code is provided, but it is not reachable
  from an empty file input through the end-to-end parse+validate pipeline.
- An empty file produces E316 (unparsable content), E502 (missing @End),
  E503 (missing @UTF8), and E504 (missing required headers) instead.
