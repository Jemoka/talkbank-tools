# Validation Feature Flags — Future Design

**Status:** Draft
**Last updated:** 2026-05-01 09:47 EDT

## Current State

`chatter validate` runs a fixed set of validation rules. The only
customization is `--suppress E###` which suppresses specific error codes
from the output (they still run, just hidden).

`chatter clan check` accepts CLAN-compatible flags:
- `--bullets N` / `+cN` — bullet consistency mode (0=full, 1=missing only)
- `--check-target` / `+g2` — require CHI Target_Child
- `--check-unused` / `+g5` — flag unused speakers
- `--check-ud` / `+u` — validate UD features on %mor
- `--check-id` / `+g4` — require @ID tiers (on by default)

## Problem

CLAN CHECK allows users to enable/disable specific validation categories.
`chatter validate` does not. This creates two issues:

1. **Users can't opt in to stricter checks** (bullet consistency, CHI
   requirement, unused speakers) without switching to `chatter clan check`.

2. **Users can't opt out of noisy checks** without knowing the specific
   error code to suppress.

## Proposed: `--enable` and `--disable` Flags

```bash
# Enable stricter checks (off by default)
chatter validate --enable bullets        # CHECK +c0 equivalent
chatter validate --enable target-child   # CHECK +g2 equivalent
chatter validate --enable unused-speakers # CHECK +g5 equivalent
chatter validate --enable ud-features    # CHECK +u equivalent

# Disable always-on checks
chatter validate --disable cross-speaker-overlap  # suppress E729
chatter validate --disable bullet-timing          # suppress E701/E704/E729

# Combine
chatter validate --enable bullets --disable cross-speaker-overlap corpus/
```

### Validation Categories

| Category | Default | Errors | CHECK Flag |
|----------|---------|--------|------------|
| `structure` | ON | E5xx (headers, @Begin/@End) | always |
| `content` | ON | E2xx, E3xx (words, terminators) | always |
| `tiers` | ON | E6xx (dependent tier validation) | always |
| `alignment` | ON | E7xx (mor/gra/pho alignment) | always |
| `bullet-timing` | ON | E701, E704, E729 | always |
| `bullets` | OFF | E730, E732 (gap, missing) | `+c0`/`+c1` |
| `target-child` | OFF | CHECK 68 | `+g2` |
| `unused-speakers` | OFF | CHECK 0 | `+g5` |
| `ud-features` | OFF | UD validation | `+u` |

### vs `--suppress`

`--suppress` hides specific error codes from output.
`--disable` prevents the validation from running at all.

For most users, the difference doesn't matter. But for large corpus
runs, skipping expensive checks (`alignment`, `bullet-timing`) saves
time.

`--suppress` is the current workaround. `--enable`/`--disable` is the
principled long-term solution.

## Implementation Notes

The validation pipeline already has `ValidationContext` with flags like
`bullets_mode`, `ca_mode`, `enable_quotation_validation`. Adding more
flags is mechanical — the architecture supports it.

The work is:
1. Add flags to `ValidationContext` / `SharedValidationConfig`
2. Wire CLI `--enable`/`--disable` to the config
3. Gate each validation category on its config flag
4. Document which categories exist and what they control

## CLAN CHECK Behaviors We Deliberately Don't Implement

| CHECK | Behavior | Why Not |
|-------|----------|---------|
| 81 | Bullet position | Grammar enforces structurally |
| 89 | Wrong chars in bullet | Grammar validates format |
| 90 | Illegal time in bullet | Grammar validates format |
| 118 | Delimiter before bullet | Grammar enforces order |
| 11 | Symbol not in depfile | We don't use depfile.cut (yet) |
| 17 | Tier not in depfile | Same |
| 147 | Form marker not in depfile | Same |

These either can't occur due to our grammar being stricter than CLAN's
parser, or require depfile.cut integration which is a separate project.

## CLAN CHECK Behaviors We Haven't Implemented Yet

| CHECK | Behavior | Status |
|-------|----------|--------|
| 84 | Cross-speaker overlap | **DONE** (E729) |
| 85 | Gap between tiers | E730 defined, not implemented |
| 110 | Missing bullet (+c mode) | E732 defined, not implemented |
| 133 | Speaker self-overlap (timing) | **DONE** (E704 + E731) |
| 13 | Duplicate speaker declaration | Not implemented |
| 122 | @ID lang not in @Languages | Not implemented |
| 142 | Role mismatch @ID vs @Participants | Not implemented |
| 153 | Age format (missing zero pad) | Not implemented |
| 157 | Media filename match | Partially implemented (E531) |
