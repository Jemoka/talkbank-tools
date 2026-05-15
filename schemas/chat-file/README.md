# JSON Schemas

**Status:** Current
**Last updated:** 2026-03-14

This directory contains generated JSON Schema artifacts for both the TalkBank
transcript model and stable editor/server contracts.

## Files

- **`chat-file.schema.generated.json`** -- generated transcript-model schema artifact
- **`chat-file.schema.json`** -- checked-in transcript-model schema
- **`analyze-command.schema.generated.json`** -- generated `talkbank/analyze` contract schema artifact
- **`analyze-command.schema.json`** -- checked-in `talkbank/analyze` contract schema

The `.generated.json` and canonical `.json` files currently contain the same
content. Keeping both matches the shared schema-generation harness and makes it
clear which files are machine-written outputs.

## Canonical URLs

```
https://talkbank.org/schemas/v0.1/chat-file.json
https://talkbank.org/schemas/v0.1/analyze-command.json
```

The `$id` in each schema resolves to its canonical URL. The transcript-model
schema also has a `/latest/` alias that always points to the current version.

## How it's generated

The schemas are auto-generated from Rust type definitions using
[schemars](https://docs.rs/schemars). A shared test harness adds metadata
(`$schema`, `$id`, `$comment`, `description`) and writes the files.

**Never edit these schema files by hand.** To regenerate:

```bash
cargo test --test generate_schema
cargo test --test generate_analyze_command_schema
```

The transcript schema comes from `talkbank-model`. The analyze-command schema
comes from `crates/chatter-lsp/src/backend/contracts.rs`.

## Versioning

These schema URLs currently follow the workspace contract version under
`/v0.1/`. Patch releases that don't change a schema shape can reuse the same
versioned URL.

## Deployment

See `deploy.sh` in this directory for instructions on publishing the schema to
`talkbank.org`.

## Further reading

- [JSON Schema book chapter](../book/src/integrating/json-schema.md) -- usage
  guide, external validation examples, transcript roundtrip guarantee, and
  editor/server contract notes
- `crates/talkbank-transform/src/json/mod.rs` -- Rust API for schema-validated
  serialization
- `crates/chatter-lsp/src/backend/contracts.rs` -- Rust-owned analyze-command contract
- `tests/generate_schema/` -- shared generation helpers, metadata injection, schema transforms
- `tests/generate_analyze_command_schema.rs` -- analyze-contract generation test
