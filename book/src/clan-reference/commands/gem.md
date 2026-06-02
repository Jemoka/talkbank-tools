# GEM -- Extract Gem Segments

**Status:** Current
**Last updated:** 2026-05-11 17:42 EDT

## Purpose

Extracts material within gem boundaries. The legacy manual gives `GEM` a dedicated section; in `clan-core`, it extracts utterances and their dependent tiers that fall within `@Bg`/`@Eg` gem boundaries, producing a new CHAT file containing only the gem-scoped content.

See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409206) for the original GEM command specification.

## Usage

```bash
chatter clan gem file.cha
chatter clan gem --gem story file.cha
```

## CLAN Equivalence

| CLAN command                    | Rust equivalent                            |
|---------------------------------|--------------------------------------------|
| `gem file.cha`                  | `chatter clan gem file.cha`                |
| `gem +g"story" file.cha`        | `chatter clan gem --gem story file.cha`    |

## Options

| Option | CLAN Flag | Description |
|--------|-----------|-------------|
| `--gem <LABEL>` | `+g"label"` | Extract only gem segments matching this label |

Without `--gem`, all gem segments in the file are extracted.

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.13 (GEM, in-section `+d` note) for
> PI review.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d0` | "Produces simple output that is in legal chat format." |
| `+d1` | "Adds information to the legal chat output regarding file names, line numbers, and `@ID` codes." |

### Open questions for PI review

1. GEM is a transform command in chatter (writes a new CHAT file).
   `+d0` "legal CHAT format" *is* GEM's default behavior in chatter
   — so `--display-mode 0` would be a no-op. Should the flag error
   on `--display-mode 0` (already-default), accept it silently, or
   simply not be plumbed for GEM at all?
2. `+d1` adds filenames/line numbers/@ID codes — that's annotation
   metadata not normally in a CHAT file. Map to a separate
   `--annotate` boolean rather than `--display-mode 1`?

## Behavior

The transform scans for `@Bg:` (begin gem) and `@Eg:` (end gem) header boundaries. All utterances between a matching `@Bg`/`@Eg` pair are included in the output, along with their dependent tiers. The gem boundary headers themselves are preserved. File-level headers and participant metadata are carried through unchanged.

## Differences from CLAN

- Gem boundary detection operates on parsed `Header` variants from the AST rather than raw text line matching for `@BG:`/`@EG:`.
- Handles both `@Bg:`/`@Eg:` (mixed case) and `@BG:`/`@EG:` (uppercase).
- Without `--gem` filter, extracts all gem segments. With `--gem`, extracts only matching labels.
