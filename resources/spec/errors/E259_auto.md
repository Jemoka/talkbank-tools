# E259: Comma after non-spoken content

## Description

Comma without any preceding spoken word in the utterance

## Metadata

- **Error Code**: E259
- **Category**: validation
- **Level**: word
- **Layer**: validation
- **Status**: implemented

## Example 1

**Source**: `spec`
**Trigger**: Paralinguistic event before comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&=coughs , sure .
@End
```

## Example 2

**Source**: `spec`
**Trigger**: Paralinguistic event before attached comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&=coughs, sure .
@End
```

## Example 3

**Source**: `spec`
**Trigger**: Filler before comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-ah , more .
@End
```

## Example 4

**Source**: `spec`
**Trigger**: Filler before attached comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&-ah, more .
@End
```

## Example 5

**Source**: `spec`
**Trigger**: Nonword before comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&~ah , more .
@End
```

## Example 6

**Source**: `spec`
**Trigger**: Nonword before attached comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	&~ah, more .
@End
```

## Example 7

**Source**: `spec`
**Trigger**: Untranscribed speech before comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	www , the rest .
@End
```

## Example 8

**Source**: `spec`
**Trigger**: Untranscribed speech before attached comma
**Expected Error Codes**: E259

```chat
@UTF8
@Begin
@Languages:	eng
@Participants:	CHI Target_Child
@ID:	eng|corpus|CHI|||||Target_Child|||
*CHI:	www, the rest .
@End
```

## Expected Behavior

The parser should successfully parse this CHAT file, but validation should report the error.

**Trigger**: See examples above

## CHAT Rule

See CHAT manual sections on word-level syntax and special markers. The CHAT manual is available at: https://talkbank.org/0info/manuals/CHAT.pdf

## Notes

- The rule name "comma after non-spoken content" is slightly misleading: the check is not about the immediate predecessor but about whether *any* spoken word has appeared earlier in the utterance. A more accurate description is "comma without prior spoken content in utterance."
- The rule uses a latching boolean: once any comma-licensing content is seen, all subsequent commas are valid regardless of what immediately precedes them. This means `hello &=coughs , more .` is valid (because `hello` was seen), even though the comma immediately follows a non-spoken item.
- Non-licensing items: paralinguistic events (`&=coughs`), fillers (`&-uh`), nonwords (`&~gaga`), phonological fragments (`&+w`), untranscribed markers (`xxx`, `yyy`, `www`)
- Examples like `and yyy , the rest .` are valid because `and` is a real word seen before the comma
- CLAN CHECK enforces this as error 48 ("Illegal character(s) ',' found"), including both attached (`&=coughs,`) and space-separated (`&=coughs ,`) forms

### Pauses license commas

Pauses `(.)`, `(..)`, `(...)` count as comma-licensing content, matching CLAN CHECK behavior. In CLAN's `check.cpp`, this is an artifact of `(` not being in `PUNCTUATION_SET` (`",[]<>;.?!"`), so pauses are processed as "words" that set `CommaWordFound`. We follow this behavior by design decision: `(...) , hello .` is valid, while `&-uh , hello .` is not.

### Divergence: omission words (`0word`)

CLAN CHECK treats omission words (`0is`, `0det`) as comma-licensing because `0` is not in its word-prefix exclusion list (`+`, `&`, `[`). We intentionally diverge: omissions do not license commas because the word was not actually spoken. This affects ~20 cases across the full TalkBank corpus.

### Divergence: language codes (`[- lang]`)

CLAN CHECK exempts commas after `[- lang]` codes via a dedicated `isPreLanguageCodeFound` flag. We do not replicate this exemption — language codes are directives, not content, and do not appear in the content item stream.
