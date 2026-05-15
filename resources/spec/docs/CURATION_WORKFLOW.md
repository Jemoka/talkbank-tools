# Curating Construct Specs from Mined Data

This project uses a strict two-stage pipeline:

1. Mine candidate files from corpus data directory (staging only).
2. Curate small constructed examples into `resources/spec/constructs/` (source of truth).

Do not copy mined `.cha` files directly into release corpus outputs.

## Why

- Mined files are useful for discovery, not for stable publishable tests.
- Curated specs stay small, understandable, and reviewable.
- Generated tree-sitter tests are reproducible from specs only.

## Workflow

1. Mine candidates:

```bash
cargo run --bin extract_corpus_candidates --manifest-path crates/spec/talkbank-spec-testrun/Cargo.toml -- \
  --data-dir ../data \
  --languages eng \
  --node-types grammar/src/node-types.json \
  --max-lines 200 \
  --max-files 20000 \
  --top 50 \
  --require-rust-parse=true \
  --require-rust-validation=true \
  --validate-alignment=true \
  --json \
  --output spec/tmp/mined/candidates.eng.json
```

2. Curate by hand from those candidates:
- Identify one minimal representative pattern per construct.
- Write or update markdown files in `resources/spec/constructs/*`.
- Prefer minimal examples over raw corpus copies.

3. Regenerate tests:

```bash
just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs
```

4. Verify:

```bash
cd grammar
tree-sitter test --overview-only
```

## Staging vs release

- Staging artifacts: `spec/tmp/mined/*` (ephemeral, non-release).
- Source of truth: `resources/spec/constructs/*` and `resources/spec/errors/*`.
- Generated release artifacts: `grammar/test/corpus/*`.
