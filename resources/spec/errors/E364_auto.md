# E364: Malformed word content

## Description

Word content is structurally malformed — the parser recognized a word node but its internal structure is invalid (e.g., `@s:+` with `+` instead of a language code).

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-13 14:42 EDT
- **Status note**: Difficult to trigger via tree-sitter parser. E364 requires tree-sitter to insert a MISSING node where a word is expected, creating a structurally valid word node with malformed internal content. Tree-sitter's error recovery typically produces E316 (unparsable content), E305 (empty utterance), or more specific error codes instead. The `@s:+` example triggers E246+E249, not E364. An utterance with only a terminator (` .`) triggers E305. The re2c parser may reach this code path.

- **Error Code**: E364
- **Category**: validation
- **Level**: utterance
- **Layer**: parser

## Example 1

**Source**: `E2xx_word_errors/E364_malformed_word_content.cha`
**Trigger**: `@s:+` — language marker with `+` instead of language code
**Expected Error Codes**: E246, E249

Note: The parser successfully parses `hello@s:+` but the `+` after `@s:` is
not a valid language code. This triggers E246 (LengtheningMarkerPosition, since
`+` is misinterpreted) and E249 (MissingLanguageContext), rather than E316
(UnparsableContent) or E364 (MalformedWordContent).

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Child
@ID:	eng|corpus|CHI|||||Child|||
*CHI:	hello@s:+ .
@End
```

## Expected Behavior

The parser should report E364 when a word node is structurally recognized
by tree-sitter but has malformed internal content. However, tree-sitter's
error recovery typically produces other, more specific error codes.

## CHAT Rule

See CHAT manual on word structure. Words must have valid content including
any special markers (`@s:`, `@l`, etc.).

## Notes

- E364 check exists in the parser but requires a specific tree-sitter error
  recovery pattern (MISSING node in word position) that is hard to trigger
- The `@s:+` example produces E246 (LengtheningMarkerPosition) and E249
  (MissingLanguageContext), not E364
- A space-only utterance (`*CHI:  .`) produces E305 (EmptyUtterance)
- The re2c parser may have different error recovery producing E364
