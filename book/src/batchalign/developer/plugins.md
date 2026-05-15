# Plugin Architecture (Removed)

**Status:** Current
**Last updated:** 2026-03-26 14:05 EDT

> **Status: Removed (March 2026).** The plugin system (`batchalign.plugins`,
> `PluginDescriptor`, `InferenceProvider`, `discover_plugins()`) was deleted.
> The only plugin that existed (`batchalign-hk-plugin`) was folded into the
> core repository as built-in engines.
>
> The original detailed design history is preserved in maintainer archives; the
> public docs keep only the migration outcome and the current extension pattern.

## Why It Was Removed

1. **Single consumer** — `batchalign-hk-plugin` was the only plugin. The
   discovery machinery existed for one package.
2. **Entry-point fragility** — `importlib.metadata.entry_points()` failed
   silently on broken packages, loaded wrong versions across environments, and
   was difficult to debug.
3. **Enum dispatch is safer** — `AsrEngine` and `FaEngine` enums provide
   compile-time exhaustiveness checking and clear error messages for missing
   engines.
4. **Built-in providers are simpler** — we keep provider dependencies in the
   base package rather than recreating plugin-style install tiers.

## Current Engine Extension Pattern

To add a new inference engine, use the built-in engine pattern documented in
[Adding Inference Providers](adding-engines.md). The pattern is:

1. Create a `(load_*, infer_*)` function pair in `python/batchalign/inference/`
2. Add an enum variant to `AsrEngine`, `FaEngine`, or the relevant engine enum
3. Wire the loader into `worker/_model_loading/`
4. Register the runtime handler during bootstrap in `worker/_model_loading/`
   and keep `worker/_infer.py` thin
5. Add provider/runtime dependencies to the base package if the engine is part
   of the supported built-in surface

If you are adding a new command, do not reach for a plugin system. Put the
released command in `crates/batchalign/src/commands/` and keep any
algorithmic or orchestration logic in the owning Rust module (`compare.rs`,
`benchmark.rs`, `transcribe/`, `fa/`, `morphosyntax/`, etc.). Engines remain
providers for Rust-owned command flows; command composition does not happen
through late-bound plugin discovery.

For a real-world example of this pattern, see the Cantonese ASR engines in
`python/batchalign/inference/languages/cantonese/` and the
[Cantonese and CJK — Architecture](../../architecture/language-and-multilingual/cantonese-and-cjk.md).

## Migration Guide for Existing Plugins

If you have an existing `batchalign.plugins` plugin, migrate it to a built-in
engine:

1. Move your `load_*` and `infer_*` functions into `python/batchalign/inference/`
2. Add an enum variant for your engine in `worker/_types.py`
3. Remove `pyproject.toml` entry points and `PluginDescriptor`
4. Add your dependencies to batchalign's base package if the engine is a
   supported built-in provider
5. Update tests to use `monkeypatch` instead of mock-based plugin patching
