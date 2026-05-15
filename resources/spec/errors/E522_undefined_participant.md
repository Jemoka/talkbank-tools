# E522: Undefined Participant in Utterance

## Description

An utterance uses a speaker code that was not defined in the @Participants header. All speaker codes used in utterances must be declared in the @Participants header.

## Metadata

- **Category**: header_validation
- **Level**: utterance
- **Layer**: validation

## Example

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*MOT:	hello dear .
*CHI:	hi mommy .
@End
```

## Expected Behavior

- **Parser**: Should succeed - syntax is valid
- **Validator**: Should report E521 - speaker 'MOT' is not defined in @Participants

## CHAT Rule

Every speaker code used in utterances (e.g., *MOT:, *CHI:, *INV:) must be declared in the @Participants header. This header defines all participants in the interaction and their roles.

## Notes

This is a cross-reference validation error. The validator must collect all participant codes from the @Participants header, then verify that every utterance speaker is in that set. Common mistakes include typos in speaker codes or forgetting to add a participant to the header when they appear later in the transcript.
