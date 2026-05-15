# CLAN Manual Audit

**Status:** Current
This appendix tracks how the
[CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) covers the
commands currently implemented in `clan-core`.

The purpose of this audit is to keep the book honest about command intent:

- when the manual has a command section, that section is the starting point for
  legacy semantics
- when the manual is silent or incomplete, the command chapter should say so
  explicitly
- GUI-oriented legacy material should move to the TalkBank VS Code extension docs, not this CLI book

## Status legend

| Status | Meaning |
|--------|---------|
| `Section` | The legacy manual has a recognizable standalone command section. |
| `Indirect` | The command is mentioned or implied, but not documented as a standalone section. |
| `Missing` | No clear standalone coverage was found in the legacy manual. |

## Validation and Analysis Commands

| Command | Manual status | Notes |
|--------|---------------|-------|
| `CHECK` | `Section` | Needs a dedicated modernization pass. |
| `FREQ` | `Section` | Present in `CLAN.html`. |
| `MLU` | `Section` | Present in `CLAN.html`. |
| `MLT` | `Section` | Present in `CLAN.html`. |
| `VOCD` | `Section` | Present in `CLAN.html`. |
| `DSS` | `Section` | Present in `CLAN.html`; manual also references the MOR pipeline. |
| `EVAL` | `Section` | Present in `CLAN.html`. |
| `EVAL-D` | `Missing` | No standalone section was found in the published CLAN manual. |
| `KIDEVAL` | `Section` | Present in `CLAN.html`. |
| `IPSYN` | `Section` | Present in `CLAN.html`. |
| `FLUCALC` | `Section` | Present in `CLAN.html`. |
| `SUGAR` | `Missing` | No standalone section was found in the published CLAN manual. |
| `MORTABLE` | `Section` | Present in `CLAN.html`. |
| `KWAL` | `Section` | Present in `CLAN.html`. |
| `COMBO` | `Missing` | No standalone section was found in the published CLAN manual. |
| `CODES` | `Missing` | No standalone section found; `codes.ord` is mentioned inside `CHAINS`. |
| `COMPLEXITY` | `Missing` | No standalone section was found in the published CLAN manual. |
| `CORELEX` | `Section` | Present in `CLAN.html`. |
| `CHAINS` | `Section` | Explicit coding-tier semantics in manual. |
| `CHIP` | `Section` | Present in `CLAN.html`. |
| `COOCCUR` | `Missing` | No standalone section was found in the published CLAN manual. |
| `DIST` | `Section` | Present in `CLAN.html`. |
| `FREQPOS` | `Section` | Present in `CLAN.html`. |
| `GEMLIST` | `Section` | Present in `CLAN.html`. |
| `KEYMAP` | `Section` | Explicit coding-tier semantics in manual; non-`$` strings ignored. |
| `MAXWD` | `Section` | Present in `CLAN.html`. |
| `MODREP` | `Section` | Present in `CLAN.html`. |
| `PHONFREQ` | `Missing` | No standalone section was found in the published CLAN manual. |
| `RELY` | `Section` | Explicit `%cod`/`%spa` coding-tier semantics and `+c1` code-prefix comparison. |
| `SCRIPT` | `Section` | Present in `CLAN.html`. |
| `TIMEDUR` | `Section` | Present in `CLAN.html`. |
| `TRNFIX` | `Missing` | No standalone section was found in the published CLAN manual. |
| `UNIQ` | `Missing` | No standalone section was found in the published CLAN manual. |
| `WDLEN` | `Section` | Present in `CLAN.html`. |
| `WDSIZE` | `Missing` | No standalone section was found in the published CLAN manual. |

## Transform Commands

| Command | Manual status | Notes |
|--------|---------------|-------|
| `CHSTRING` | `Section` | Present in `CLAN.html`. |
| `COMBTIER` | `Section` | Manual intent is narrow: combining duplicate `%com` lines. |
| `COMPOUND` | `Missing` | No standalone section was found in the published CLAN manual. |
| `DATACLEAN` | `Missing` | No standalone section was found in the published CLAN manual. |
| `DATES` | `Section` | Present in `CLAN.html`. |
| `DELIM` | `Section` | Present in `CLAN.html`. |
| `FIXIT` | `Section` | Present in `CLAN.html`. |
| `FIXBULLETS` | `Section` | Present in `CLAN.html`. |
| `FLO` | `Section` | Present in `CLAN.html`. |
| `GEM` | `Section` | Present in `CLAN.html`. |
| `INDENT` | `Section` | Manual confirms text/layout purpose. |
| `LINES` | `Section` | Manual confirms text/layout purpose. |
| `LONGTIER` | `Section` | Manual confirms text/layout purpose. |
| `LOWCASE` | `Section` | Present in `CLAN.html`. |
| `MAKEMOD` | `Missing` | No standalone section was found in the published CLAN manual. |
| `ORT` | `Missing` | No standalone section was found in the published CLAN manual. |
| `POSTMORTEM` | `Indirect` | Mentioned only through the `mor *.cha` pipeline in the published CLAN manual. |
| `QUOTES` | `Section` | Present in `CLAN.html`. |
| `REPEAT` | `Section` | Present in `CLAN.html`. |
| `RETRACE` | `Section` | Present in `CLAN.html`. |
| `ROLES` | `Missing` | No standalone section was found in the published CLAN manual. |
| `TIERORDER` | `Section` | Present in `CLAN.html`. |
| `TRIM` | `Section` | Present in `CLAN.html`. |

## Format Converters

| Command | Manual status | Notes |
|--------|---------------|-------|
| `CHAT2TEXT` | `Section` | Present in `CLAN.html`. |
| `CHAT2ELAN` | `Section` | Present in `CLAN.html`. |
| `CHAT2PRAAT` | `Section` | Present in `CLAN.html`. |
| `CHAT2SRT` | `Section` | Present in `CLAN.html`. |
| `ELAN2CHAT` | `Section` | Present in `CLAN.html`. |
| `LAB2CHAT` | `Section` | Present in `CLAN.html`. |
| `LENA2CHAT` | `Section` | Present in `CLAN.html`. |
| `LIPP2CHAT` | `Section` | Present in `CLAN.html`. |
| `PLAY2CHAT` | `Missing` | No standalone section was found in the published CLAN manual. |
| `PRAAT2CHAT` | `Section` | Present in `CLAN.html`. |
| `RTF2CHAT` | `Missing` | No standalone section was found in the published CLAN manual. |
| `SALT2CHAT` | `Missing` | No standalone section was found in the published CLAN manual. |
| `SRT2CHAT` | `Section` | Present in `CLAN.html`. |
| `TEXT2CHAT` | `Missing` | No standalone section was found in the published CLAN manual. |

## Priorities

The next documentation passes should focus on:

1. commands with strong manual semantics that should constrain implementation behavior, especially `CHAINS`, `KEYMAP`, and `RELY`
2. commands whose current docs over-generalize legacy intent, especially `COMBTIER`
3. commands with no standalone `CLAN.html` section, where the book needs to say what evidence is being used instead of pretending the manual is explicit
