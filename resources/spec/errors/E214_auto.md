# E214: Empty scoped annotation content

## Description

A scoped annotation (e.g., error annotation `[*]`, replacement `[: ...]`) has
an empty content list. The validator reports E214 when annotated content has
zero scoped annotations attached.

**Validation not yet implemented for this spec example.** The example uses
`hello [*] .` which the parser treats as a valid (though empty) error code
annotation. The `EmptyAnnotatedContentAnnotations` check in `annotated.rs`
validates that annotated content has at least one scoped annotation, but `[*]`
is parsed as a single annotation (an error marker with no specific code), so the
list is not empty.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E214
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E214_empty_scoped_annotation.cha`
**Trigger**: Annotated content with empty annotation list
**Expected Error Codes**: E214

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Scoped annotation cannot be empty
@Comment:	Invalid: 'hello [*]' - Empty error annotation
*CHI:	hello [*] .
@End
```

## Expected Behavior

The validator should report E214 when annotated content has an empty annotations
list. The check exists in `crates/talkbank-model/src/model/annotation/annotated.rs`
but the example does not trigger it because `[*]` is parsed as one annotation.

**Trigger conditions**: An `AnnotatedContent` node whose scoped annotations
list is empty (zero annotations). This may only be constructible
programmatically, not from CHAT text, since the parser requires at least one
annotation to create annotated content.

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT
manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists in `annotated.rs` but may only be triggerable via
  programmatic construction (parser always produces at least one annotation)
- The code IS emitted in the codebase as a model-level invariant check
