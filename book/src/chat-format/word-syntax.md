# Word Syntax

**Status:** Reference
**Last updated:** 2026-05-11 23:33 EDT

Words are the primary content unit on the main tier. CHAT defines several word types and annotation mechanisms.

## Standalone Words

Most words are simple tokens separated by whitespace:

```chat
*CHI:	I want a cookie .
```

Words can contain Unicode characters for any language:

```chat
*CHI:	ich möchte Kekse .
```

## Compounds

Compound words join multiple elements with `+`:

```chat
*CHI:	I want ice+cream .
```

## Special Word Forms

### Shortened Forms

Parentheses mark omitted portions of a word:

```chat
*CHI:	(be)cause I want it .
```

The full form is `because`; the child produced `cause`.

### Replacements

Square brackets with colon mark what the speaker actually meant:

```chat
*CHI:	I goed [: went] to the store .
```

The speaker said "goed" but the intended word was "went".

### Language Markers

The `@s:` suffix marks a word's language in multilingual transcripts:

```chat
*CHI:	I want a Keks@s:deu .
```

Other `@` markers:
- `@l` — letter
- `@c` — child-invented form
- `@f` — family-specific word
- `@n` — neologism
- `@o` — onomatopoeia
- `@b` — babbling
- `@wp` — word play
- `@si` — signed word

## Annotations

Words and groups can carry post-positioned annotations in square brackets:

### Error Marking

```chat
*CHI:	he goed [*] to school .
```

`[*]` marks an error. More specific error codes can follow: `[* m:+ed]`.

### Explanations

```chat
*CHI:	that one [= the red ball] .
```

`[=  text]` provides an explanation or gloss.

### Replacements

```chat
*CHI:	I wanna [: want to] go .
```

`[: text]` marks the target/intended form.

### Best Guess

```chat
*CHI:	I want the birfer [?] .
```

`[?]` marks uncertain transcription.

## Events and Actions

### Paralinguistic Events

Events marked with `&=` describe non-speech sounds:

```chat
*CHI:	&=laughs I want cookie .
*CHI:	&=coughs .
```

### Fillers

Fillers are marked with `&-`:

```chat
*CHI:	&-um I want &-uh cookie .
```

### Interposed Speech (Other Speaker)

Brief background speech from a different speaker is marked with the
`&*SPK:text` prefix — it captures the interjection without creating
a full turn line:

```chat
*CHI:	I want &*MOT:careful a cookie .
```

This says CHI was speaking and MOT briefly said "careful" mid-turn.
If the intervention is substantial enough to constitute its own turn,
transcribe it as a separate `*MOT:` utterance instead. Model:
`crates/core/talkbank-model/src/model/content/other_spoken.rs`.

(Note: `[^ text]` is a *freecode* — a standalone free-form
researcher annotation that sits as its own content item on the main
tier (variant of `UtteranceContent::Freecode`, sibling of `Word` and
`Group` — it is NOT attached to any word). See `grammar/grammar.js`
rule `freecode` and
`crates/core/talkbank-model/src/model/content/utterance_content/`. Used
for transcriber notes that are independent of any single word; for
notes about a single word use `[% text]` or `[= text]` instead.)

## Pauses

```chat
*CHI:	I (.) want (..) a (...) cookie .
*CHI:	I (1.5) want a cookie .
```

- `(.)` — short pause
- `(..)` — medium pause
- `(...)` — long pause
- `(N.N)` — timed pause in seconds

## Overlap

Overlapping speech between speakers uses angle brackets and overlap markers:

```chat
*MOT:	do you want <a cookie> [>] ?
*CHI:	<cookie> [<] !
```

- `[>]` — follows the overlap (this speaker started first)
- `[<]` — overlaps the previous speaker

## Retrace and Repetition

Groups followed by retrace markers indicate speech disfluencies:

```chat
*CHI:	<I want> [/] I want a cookie .
*CHI:	<I want> [//] I need a cookie .
*CHI:	<I want a> [///] give me a cookie .
```

- `[/]` — partial retrace (speaker repeats the same words)
- `[//]` — full retrace (speaker restarts with different words)
- `[///]` — multiple retracing (multiple false starts)
- `[/-]` — reformulation (speaker rephrases with different structure)
