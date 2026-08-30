# Batchalign maintenance scripts

Use the repository entrypoints first:

- `just batchalign cli` runs the authoritative Bazel-built CLI.
- `just batchalign test` runs the Rust and Python integration gates.
- `just batchalign pytest` forwards arguments to the Bazel Python test target.

The scripts in this directory support Batchalign packaging, generated API
surfaces, bounded parity checks, fixture preparation, and focused diagnostics.
They are not an alternative build system. Parser, CLAN, Chatter, tree-sitter,
and specification-generator scripts were removed with their product crates.

## Common tasks

| Task | Entrypoint |
|---|---|
| Install the local push guard | `ln -sf ../../scripts/pre-push.sh .git/hooks/pre-push` |
| Check generated dashboard API files | `bash scripts/check_dashboard_api_drift.sh` |
| Check IPC schema files | `bash scripts/check_ipc_type_drift.sh` |
| Check runtime constants | `python3 scripts/check_runtime_drift.py` |
| Run Stanza drift probes | `bash scripts/run-drift-probes.sh` |
| Prepare a minimal CHAT/media fixture | `python3 scripts/prepare_corpus_media_fixture.py ...` |
| Trim a CHAT/media fixture | `python3 scripts/trim_chat_audio.py ...` |
| Compare against a stock Batchalign install | `python3 scripts/compare_stock_batchalign.py ...` |

Scripts without an entry above are narrow support utilities. Read their module
or shell header before invoking them, and keep model-backed runs bounded.
