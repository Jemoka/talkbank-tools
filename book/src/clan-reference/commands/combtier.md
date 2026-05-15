# COMBTIER -- Combine Duplicate Dependent Tiers

**Status:** Current
## Purpose

Combines duplicate dependent tiers within an utterance. The legacy manual describes `COMBTIER` narrowly: it corrects the case where transcribers create several `%com` lines, combining two `%com` lines into one by removing the second header and moving its material onto the first `%com` line.

`clan-core` generalizes that behavior to any selected dependent-tier label, while preserving the underlying tier variant whenever the selected tier has a supported AST model.

This is useful for cleaning up files where duplicate tiers were introduced during manual editing or automated annotation.

## Usage

```bash
chatter clan combtier --tier com file.cha
```

## Options

| Option | Type | Default | Description |
|--------|------|---------|-------------|
| `--tier` | string | *(required)* | The tier label to combine (e.g., `com` for `%com:`, `spa` for `%spa:`) |
| `--separator` | string | `" "` | Separator between combined tier contents |

## Behavior

For each utterance, the transform finds all dependent tiers matching the specified label. If two or more matching tiers exist, their text content is concatenated using the configured separator and the result replaces the first occurrence. All subsequent duplicates are removed.

Utterances with zero or one matching tier are left unchanged.

## Differences from CLAN

- **Manual intent is narrower**: the legacy manual only documents `%com` combination explicitly. `clan-core` supports other selected tier labels too.
- Operates on AST rather than raw text.
- Uses the framework transform pipeline (parse -> transform -> serialize -> write).
- **Tier-kind preservation**: Combining bullet/text dependent tiers such as `%cod:` and `%com:` preserves their actual tier variants instead of degrading them to user-defined tiers.
