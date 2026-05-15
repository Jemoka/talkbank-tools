# CHAT2TEXT — CHAT to Plain Text

**Status:** Current
## Purpose

Converts CHAT files into plain text lines suitable for downstream text analysis. The legacy manual describes `CHAT2TEXT` as converting a CHAT file to text lines for concordance programs such as AntConc, and says it is implemented as an alias of `FLO` with `+cr +t*`.

`clan-core` implements the same broad intent directly by extracting plain spoken text from the parsed CHAT AST.

## Usage

```bash
chatter clan chat2text file.cha
chatter clan chat2text --include-speaker file.cha
```

## Options

| Option | Description |
|--------|-------------|
| `--include-speaker` | Prefix each line with speaker code (e.g., "CHI: hello world") |

## Output

Plain text with one utterance per line. All CHAT annotations are stripped:
- Bracketed annotations (`[/]`, `[: replacement]`, `[*]`)
- Timing bullets
- Terminators (`.`, `?`, `!`)
- Fillers (`&-um`), fragments (`&+fr`), events (`&=laughs`)
- Untranscribed markers (`xxx`, `yyy`, `www`)
- Omitted words (`0word`)

## Differences from CLAN

- **Manual intent**: the
  [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) does have a
  dedicated `CHAT2TEXT` section, and it describes the command as a
  `FLO`-based alias rather than as a separate semantic engine.
- Uses AST-based content extraction for reliable annotation stripping.
- Does not model the legacy implementation literally as `flo +cr +t*`; it performs the plain-text extraction directly.
