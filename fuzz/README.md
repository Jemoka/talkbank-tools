# `fuzz/` — cargo-fuzz workspace

This is a **separate Cargo workspace** from the root, because `cargo-fuzz`
requires its own nightly toolchain with sanitizer flags that would otherwise
contaminate the main workspace's build.

## Targets

| Target | What it fuzzes |
|---|---|
| `fuzz_parse_chat_file` | The full `talkbank-parser` entry point against arbitrary byte input |
| `fuzz_parse_word` | Word-internal parsing (the smallest unit of CHAT syntax) |
| `fuzz_parse_main_tier` | Main-tier (`*SPK:`) parsing |
| `fuzz_validate` | The validation pass (`Validate` trait) against arbitrary AST input |

## Running

```bash
cd fuzz
cargo fuzz run fuzz_parse_chat_file
```

`cargo-fuzz` is invoked from this directory specifically so it doesn't
conflict with the root workspace's stable toolchain. Targets emit corpus
files under `fuzz/corpus/<target>/` and crash seeds under
`fuzz/artifacts/<target>/`.

## Why it's at root, not under `crates/`

The separate-workspace requirement is structural — `cargo-fuzz` discovers
`fuzz/Cargo.toml` relative to where it's invoked, and the workspace must
not be a member of the parent workspace. Keeping it at the top level
matches `cargo-fuzz`'s own conventions and makes the discovery rule
obvious to a naive reader.
