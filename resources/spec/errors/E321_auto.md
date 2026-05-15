# E321: UnparsableUtterance

## Description

An utterance line (starting with \*SPEAKER:) could not be parsed. The
utterance body contains syntax errors that tree-sitter cannot recover
from, and the error doesn't match any of the specifically checked
patterns (missing form type, empty replacement, unknown annotation).

## Metadata
- **Status**: not_implemented
- **Status note**: Unreachable via tree-sitter parser. The catch-all fallback at utterance error analysis is always preempted by more specific error patterns (E316, E375). The re2c parser may reach this code path.

- **Error Code**: E321
- **Category**: parser\_recovery
- **Level**: utterance
- **Layer**: parser

## Example 1

**Trigger**: Utterance body with malformed content that tree-sitter wraps in ERROR

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|test|CHI||female|||Target_Child|||
*CHI:	hello [% broken [nested unclosed .
@End
```

## Expected Behavior

The parser should report E321. The utterance body contains malformed
bracket content that produces a tree-sitter ERROR node. Since the error
doesn't match the specific patterns checked first (missing `@` form type,
empty `[:]`, or `[@` unknown annotation), the fallback E321 fires.

## Notes

- More specific utterance errors are checked first: missing form type,
  empty replacement `[:]`, unknown annotation `[@`.
- E321 is the catch-all for other utterance parse failures.
- **Status note**: The example above triggers E316 (generic parse error)
  rather than E321. Tree-sitter's error recovery routes the malformed bracket
  content through other error paths. Triggering E321 requires tree-sitter to
  produce an ERROR node in utterance context that doesn't match any of the
  specific patterns checked first.
