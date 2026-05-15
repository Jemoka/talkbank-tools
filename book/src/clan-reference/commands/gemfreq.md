# GEMFREQ -- Word Frequency Within Gem Segments

**Status:** Current
**Last updated:** 2026-05-11 17:43 EDT

## Purpose

Computes word frequency restricted to utterances inside `@G:`-labeled
gem segments. `gemfreq` is a CLAN compatibility alias for the more
general [`freq --gem`](freq.md). The behavior is identical to running
`freq` with a required `--gem` filter; the alias exists so legacy
CLAN scripts that invoke `gemfreq` directly keep working.

The legacy CLAN manual entry is at
[GEMFREQ](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409273).

## Usage

`gemfreq` requires the `--gem`/`-g` option — there is no implicit
"all gems" mode (unlike `freq`, where `--gem` is optional).

```bash
chatter clan gemfreq --gem story file.cha
chatter clan gemfreq -g story file.cha               # short form
chatter clan gemfreq --gem story --gem retell file.cha   # multiple gems
```

## Options

The flag set is identical to [`freq`](freq.md): `--mor`,
`--speaker` / `--exclude-speaker`, `--include-word` / `--exclude-word`,
`--exclude-gem`, `--range`, `--per-file`, `--include-retracings`,
`--format`, plus the universal verbosity / TUI / theme flags.

The only behavioral difference is that `gemfreq` rejects invocations
without at least one `--gem`/`-g` argument; `freq` treats `--gem`
as an optional restriction.

For full per-flag descriptions, output formats, word-normalization
rules, and CLAN-equivalence tables, see [freq.md](freq.md).

## Display Modes (`+dN` / `--display-mode N`) — DRAFT, awaiting PI review

> **Status: drafted from CLAN manual; not yet implemented.** Rewriter
> at `crates/clan-core/src/clan_args.rs:101` translates
> `+dN` → `--display-mode N`; no `clap` field consumes it today.
> Drafted from CLAN manual §7.14 (`Unique Options`, GEMFREQ) for
> PI review.
>
> **Important divergence flag.** In CLAN, GEMFREQ's `+d` table is
> identical to **GEM's** (legal CHAT format + annotation), not
> identical to **FREQ's** (output format selector with 9 values).
> chatter's gemfreq currently inherits the freq common-args, so any
> `--display-mode N` implementation has to decide which semantics
> apply — and that's worth pulling apart before clap touches it.

| N | CLAN behavior (verbatim from manual) |
|---|---|
| `+d0` | "Produces simple output that is in legal chat format." |
| `+d1` | "Adds information to the legal chat output regarding file names, line numbers, and `@ID` codes." |

### Open questions for PI review

1. GEMFREQ in CLAN borrows GEM's `+d`, not FREQ's. Should chatter
   follow CLAN's CLAN-faithful behavior (treat `--display-mode 0` as
   the GEM-style legal-CHAT output), or should the chatter `gemfreq`
   inherit FREQ's `--display-mode` table (since they share
   common-args)?
2. If we honor CLAN's split, `gemfreq` is the *only* analyze command
   whose `+d` differs from its lexically-similar parent. That feels
   like a CLAN historical accident worth deviating from in
   chatter — but it's a deviation, not a faithful port.

## When to use `gemfreq` vs `freq --gem`

Functionally these are the same call. Pick `gemfreq` when:

- you are porting a legacy CLAN script that invokes `gemfreq` and want
  byte-compatible-looking command lines, or
- you want the command name to surface "gem-restricted" intent
  immediately to readers of the script.

Pick `freq --gem story` when:

- you are writing new scripts that may need to mix gem-restricted and
  unrestricted analysis under the same command, or
- you want to omit `--gem` to fall back to whole-file frequency.

## Reference

Implementation: `Gemfreq` is a separate `clap` subcommand variant in
the `ClanCommands` enum (not a `#[command(alias = "...")]`); the
required `--gem` constraint is enforced via a clap `ArgGroup` with
`required(true)`. The dispatcher at
`crates/chatter-cli/src/commands/clan/compatibility.rs::96` routes
the parsed arguments to `run_analysis_and_print` with
`AnalysisCommandName::Freq`, so behavior past the parse boundary is
identical to a `freq --gem …` invocation. See [freq.md](freq.md) for
the complete reference.
