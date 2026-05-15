# E246: Lengthening marker not after spoken material

## Description

A lengthening marker (`:`) appears before any spoken material in a word rather
than after it. In CHAT, the colon `:` indicates phonological lengthening and
must follow the spoken text it modifies (e.g., `hel:o` is valid, `:hello` is
not).

**Validation not yet implemented for this spec example.** The check in
`crates/talkbank-model/src/validation/word/structure.rs` iterates over word
content and checks whether a `Lengthening` element has preceding spoken
material. However, the example `:hello` is likely parsed by the grammar as
a single token (colon followed by text) rather than as a `Lengthening` content
element followed by text, so the validator never sees the expected structure.

## Metadata
- **Status**: not_implemented
- **Last updated**: 2026-04-04 08:15 EDT

- **Error Code**: E246
- **Category**: validation
- **Level**: word
- **Layer**: validation

## Example 1

**Source**: `error_corpus/validation_errors/E246_lengthening_before_material.cha`
**Trigger**: Lengthening marker before spoken text
**Expected Error Codes**: E246

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
@Comment:	ERROR: Lengthening (:) must follow spoken material
@Comment:	Invalid: ':hello' - Lengthening before text
*CHI:	:hello .
@End
```

## Expected Behavior

The validator should report E246 when a lengthening marker appears without
preceding spoken material in the word content sequence. The check exists in
`crates/talkbank-model/src/validation/word/structure.rs`.

**Trigger conditions**: A `WordContent::Lengthening` element at a position
where no preceding element in the word content array is spoken material.

## CHAT Rule

See CHAT manual on phonological lengthening. The colon `:` lengthening marker
must follow the vowel or consonant it modifies. The CHAT manual is available at:
https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- Validation logic exists in `structure.rs` but the example does not trigger it
- The parser may not produce a `Lengthening` content element for word-initial
  colons; the grammar may treat `:hello` as a different construct
- The code IS emitted elsewhere in the codebase for words where the parser does
  produce the expected content structure
