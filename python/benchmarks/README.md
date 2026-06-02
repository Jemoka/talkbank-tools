# Benchmarks

**Status:** Reference
**Last updated:** 2026-06-01

Quick local benchmarks for core parsing and processing hot paths.

Run these from the repository root. If you are benchmarking a fresh source
checkout, prepare the dev environment first:

```bash
just batchalign build         # prepares the Bazel-native Python deps + `_core.so`
```

## 1) CHAT parser throughput

Run:

```bash
uv run python benchmarks/chat_parser_throughput.py --iterations 1000
```

Optional custom file:

```bash
uv run python benchmarks/chat_parser_throughput.py --chat-file /path/to/sample.cha --iterations 1000
```

## 2) In-process callback boundary

Measure typed callback overhead across the in-process `ParsedChat` entry points:

```bash
uv run python benchmarks/callback_boundary.py --iterations 200
```

## Baselines

Store baseline snapshots in `benchmarks/baselines.json` and compare after major changes.
