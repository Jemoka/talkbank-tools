# JSON Schema

**Status:** Current
**Last updated:** 2026-05-11 23:47 EDT

`talkbank-tools` now generates JSON Schema from Rust-owned types with
[schemars](https://docs.rs/schemars) for two different integration surfaces:

- the `ChatFile` transcript model used by `chatter to-json`
- the `talkbank/analyze` editor/server request contract shared by the VS Code extension and `chatter-lsp`

Keeping those schemas generated from the Rust source of truth lets cross-language
integrations consume stable contracts without re-deriving the shapes by hand.

## Available schemas

| Schema | Canonical URL | Repository | Generator |
|----------|------------|------------|------------|
| `ChatFile` transcript model | `https://talkbank.org/schemas/v0.1/chat-file.json` | `schemas/chat-file/chat-file.schema.json` | `cargo test --test generate_schema` |
| `AnalyzeCommandPayload` execute-command contract | `https://talkbank.org/schemas/v0.1/analyze-command.json` | `schemas/chat-file/analyze-command.schema.json` | `cargo test --test generate_analyze_command_schema` |

The generated schemas declare both `$schema` (JSON Schema 2020-12) and `$id`
(the canonical URL above). External consumers that want to track the
current transcript-model version should follow the `v0.1` URL; there is
no `/latest/` alias in the generated artifacts.

## Transcript schema: `ChatFile`

`chatter to-json` converts CHAT transcripts into a structured JSON form backed
by the same `ChatFile` model used by the parser, validator, and serializer.

### How `chatter to-json` uses it

By default, `chatter to-json`:

- validates the CHAT input,
- checks dependent-tier alignment unless `--skip-alignment` is passed, and
- validates the emitted JSON against the schema unless
  `--skip-schema-validation` is passed.

Useful flags:

```bash
chatter to-json input.cha --skip-validation
chatter to-json input.cha --skip-alignment
chatter to-json input.cha --skip-schema-validation
```

`chatter from-json` deserializes JSON back into the internal `ChatFile` model
and re-serializes it to CHAT format. The input should conform to this schema.

### Roundtrip expectations

The CHAT-to-JSON-to-CHAT pipeline is intended to preserve the `ChatFile` model:

```bash
chatter to-json input.cha -o intermediate.json
chatter from-json intermediate.json -o output.cha
diff input.cha output.cha
```

Both directions go through the same typed model. When changing the parser,
serializer, or schema generation, confirm roundtrip behavior with the existing
roundtrip test suites rather than assuming byte-for-byte identity.

### Using the schema externally

#### Validate JSON in Python

```python
import json
import jsonschema
import urllib.request

schema_url = "https://talkbank.org/schemas/v0.1/chat-file.json"
schema = json.loads(urllib.request.urlopen(schema_url).read())

with open("transcript.json") as f:
    data = json.load(f)

jsonschema.validate(data, schema)
```

#### IDE autocompletion

```json
{
  "$schema": "https://talkbank.org/schemas/v0.1/chat-file.json",
  "lines": [],
  "participants": {},
  "languages": [],
  "options": []
}
```

#### Generate types from the schema

Tools like [quicktype](https://quicktype.io),
[json-schema-to-typescript](https://github.com/bcherny/json-schema-to-typescript),
and [datamodel-code-generator](https://github.com/koxudaxi/datamodel-code-generator)
can generate typed structs or classes from the schema for TypeScript, Python,
Go, and other languages.

## Editor/server contract schema: `AnalyzeCommandPayload`

The `talkbank/analyze` LSP command still travels through
`workspace/executeCommand`, but its logical payload is now one typed object
described by `chatter_lsp::backend::contracts::AnalyzeCommandPayload`:

```json
{
  "commandName": "mlu",
  "targetUri": "file:///tmp/sample.cha",
  "options": {
    "words": true
  }
}
```

That schema is useful for:

- external editor integrations that want the exact request contract
- future TypeScript type generation or runtime validation
- documenting the stable boundary between the extension and `chatter-lsp`

The Rust contract module also reuses library-owned types such as
`AnalysisCommandName`, `Gender`, and `DatabaseFilter` so the transport schema
stays aligned with the typed CLAN execution boundary.

The repo also keeps one concrete shared fixture for this contract at
`apps/vscode-extension/src/test/fixtures/analyzeCommandPayload.json`. The TypeScript payload
tests import that fixture directly, and `tests/validate_analyze_command_fixture.rs`
validates the same JSON against `schemas/chat-file/analyze-command.schema.json` and then
deserializes it through `AnalyzeCommandPayload`. That gives the contract one
mechanically checked cross-language example in addition to the generated schema
artifact itself.

## Regenerating the schemas

After changing transcript-model types in `talkbank-model`:

```bash
cd talkbank-tools
cargo test --test generate_schema
```

After changing the editor/server analyze contract in
`crates/chatter/chatter-lsp/src/backend/contracts.rs`:

```bash
cd talkbank-tools
cargo test --test generate_analyze_command_schema
```

This writes the checked-in schema artifacts in `schema/`. CI already checks that
generated artifacts stay in sync.

## Code references

- `schemas/chat-file/chat-file.schema.json` — generated schema
- `schemas/chat-file/analyze-command.schema.json` — generated `talkbank/analyze` contract schema
- `apps/vscode-extension/src/test/fixtures/analyzeCommandPayload.json` — shared concrete analyze-command fixture
- `crates/core/talkbank-transform/src/json.rs` — schema loading and validation
- `crates/core/talkbank-model/src/model/` — Rust data model
- `crates/chatter/chatter-lsp/src/backend/contracts.rs` — Rust-owned editor/server transport contracts
- `tests/generate_schema/` — shared schema generation helpers
- `tests/generate_analyze_command_schema.rs` — analyze-contract schema generation test
- `tests/validate_analyze_command_fixture.rs` — fixture/schema validation test for the editor/server contract
