# stacked\_ca\_markers

Word with multiple stacked CA markers before the text body. In
Conversation Analysis transcription, markers commonly stack: `°↑` is
piano voice + pitch up, `°°` is pianissimo (double piano), `⌈°` is
overlap begin + piano voice.

The grammar must accept any number of leading structural markers before
the first text segment. A single marker before text was always supported;
this spec covers two or more stacked markers — a regression gate for the
`repeat1` fix to `word_body`'s marker-initial path.

## Input

```standalone_word
°↑hello°
```

## Expected CST

```cst
(standalone_word
  (word_body
    (ca_delimiter)
    (ca_element)
    (word_segment)
    (ca_delimiter)))
```

## Metadata

- **Level**: word
- **Category**: word
