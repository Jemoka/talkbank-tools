# Speaker Filtering

**Status:** Current
**Last updated:** 2026-05-11 18:58 EDT

Speaker filters restrict which speakers' utterances are analyzed. This is one of the most frequently used filters — most CLAN analyses target a specific participant (e.g., the child in a child language study).

## Include speakers

Analyze only specific speakers:

```bash
chatter clan freq --speaker CHI file.cha
chatter clan mlu --speaker CHI --speaker MOT file.cha
```

CLAN equivalent: `+t*CHI`, `+t*CHI +t*MOT`

Multiple `--speaker` flags use OR logic: utterances from *any* listed speaker are included.

## Exclude speakers

Remove specific speakers from analysis:

```bash
chatter clan freq --exclude-speaker INV file.cha
```

CLAN equivalent: `-t*INV`

## @ID filtering

Filter speakers by metadata fields in the `@ID` header. The filter is pipe-delimited, in CHAT `@ID` column order:

```text
lang|corpus|speaker|age|sex|group|ses|role|education|custom
```

Each field is either `*` (wildcard), empty (also wildcard), or a literal exact match. Trailing fields may be omitted entirely.

```bash
# All children (role = Target_Child, at position 8)
chatter clan freq --id-filter "*|*|*|*|*|*|*|Target_Child" file.cha

# English-language speakers only
chatter clan freq --id-filter "eng|" file.cha

# A specific speaker code (position 3)
chatter clan freq --id-filter "*|*|CHI" file.cha
```

CLAN equivalent: `+t@ID="*|*|*|*|*|*|*|Target_Child"`.

Authoritative parser: `crates/clan-core/src/framework/id_filter.rs`. The language field is a comma-separated list in the underlying `@ID`; the filter matches if any language in the `@ID` matches the pattern.

## Interaction with other filters

- Speaker filtering is applied first, before range or word filters
- Include and exclude can be combined; excludes are applied after includes
- Speaker codes are case-sensitive and must match the `@Participants` header exactly (e.g., `CHI`, not `chi`)
