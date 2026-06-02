# Flag Translation Guide

**Status:** Current
**Last updated:** 2026-05-12 04:20 EDT

Complete mapping between legacy CLAN `+flag`/`-flag` syntax and modern `--flag` equivalents. Both styles are accepted by `chatter clan`. Translation lives in `crates/clan/clan-core/src/clan_args.rs`.

## Speaker and tier filters

| CLAN | Modern | Description |
|------|--------|-------------|
| `+t*CHI` | `--speaker CHI` | Include speaker |
| `-t*CHI` | `--exclude-speaker CHI` | Exclude speaker |
| `+t%mor` | `--tier mor` | Include dependent tier |
| `-t%gra` | `--exclude-tier gra` | Exclude dependent tier |
| `+t@ID="..."` | `--id-filter "..."` | Filter by @ID fields |

## Word filters

| CLAN | Modern | Description |
|------|--------|-------------|
| `+s<word>` | `--include-word <word>` | Include word |
| `-s<word>` | `--exclude-word <word>` | Exclude word |
| `+k` | `--case-sensitive` | Case-sensitive matching |

## Gem and range

| CLAN | Modern | Description |
|------|--------|-------------|
| `+g<label>` | `--gem <label>` | Include gem segment |
| `-g<label>` | `--exclude-gem <label>` | Exclude gem segment |
| `+z25-125` | `--range 25-125` | Utterance range |

## Display and output

| CLAN | Modern | Description |
|------|--------|-------------|
| `+dN` | `--display-mode N` | Display mode |
| `+fEXT` | `--output-ext EXT` | Output file extension |
| `+u` (CHECK) | `--check-ud` | Validate UD features (CHECK-specific) |
| `+u` (other commands) | *(no-op)* | Legacy "merge speakers"; chatter always merges, so this is a no-op |
| `+r6` | `--include-retracings` | Count retraced material |

## Context

| CLAN | Modern | Description |
|------|--------|-------------|
| `+wN` | `--context-after N` | Context lines after match |
| `-wN` | `--context-before N` | Context lines before match |
