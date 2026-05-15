# COMPLEXITY — Syntactic Complexity Ratio

**Status:** Current
**Last updated:** 2026-05-12 10:07 EDT

Computes syntactic complexity by counting subordinating dependency relations in the `%gra` tier.

## Usage

```bash
chatter clan complexity file.cha
chatter clan complexity corpus/ --speaker PAR
chatter clan complexity file.cha --format json
```

## What It Measures

COMPLEXITY reads the `%gra` dependency tier and counts clause-embedding relations that indicate syntactic subordination. The **complexity ratio** is the number of subordinating relations divided by total tokens (non-punctuation).

### UD Relations (auto-detected)

Per `crates/clan-core/src/commands/complexity.rs:10`:

| Relation | Description |
|----------|-------------|
| CSUBJ | Clausal subject |
| CCOMP | Clausal complement |
| XCOMP | Open clausal complement |
| ACL | Adnominal clause |
| ADVCL | Adverbial clause modifier |
| APPOS | Apposition |
| EXPL | Expletive |

### Legacy CLAN Relations (auto-detected)

| Relation | Description |
|----------|-------------|
| CSUBJ | Clausal subject |
| COMP | Complement |
| CPRED | Clausal predicate |
| CPOBJ | Clausal object of preposition |
| COBJ | Clausal object |
| CJCT | Clausal adjunct |
| XJCT | Non-finite clausal adjunct |
| NJCT | Nominal adjunct |
| CMOD | Clausal modifier |
| XMOD | Non-finite clausal modifier |

## Output

Per speaker:
- Count of each subordinating relation
- **Tokens**: total matched complexity relations
- **TotalTokens**: all non-PUNCT tokens
- **Ratio**: Tokens / TotalTokens

## Requirements

Requires a `%gra` dependent tier. Utterances without `%gra` are skipped.

## Differences from CLAN

- Auto-detects UD vs legacy relation names (CLAN requires compile-time config)
- Uses typed AST `GraTier` with `GrammaticalRelation` entries
- Supports JSON and CSV output
- Sub-relations (e.g., `CSUBJ:pass`) are properly matched to their base relation
