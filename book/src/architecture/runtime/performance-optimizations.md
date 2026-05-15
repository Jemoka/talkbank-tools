# Performance Optimizations

**Status:** Current
**Last updated:** 2026-05-01 17:07 EDT

Performance-critical patterns shipped in `talkbank-tools` and the
batchalign runtime. Each entry describes what runs in production and
where, not the alternatives that were rejected.

## `Word::cleaned_text()` — `OnceLock<SmolStr>` Cache

`Word` carries a lazy cache (`CachedStr`) wrapping
`OnceLock<SmolStr>`. The first call to `cleaned_text()` iterates the
word's content (concatenating `Text` and `Shortening` variants) and
stores the result; subsequent calls return `&str` from the cache.

Most CHAT words fit in `SmolStr`'s inline buffer (≤ 23 bytes), so
the common case has zero heap allocation. Without the cache,
`cleaned_text()` was the single largest allocation hotspot —
called 2–4× per word during validation and alignment, across
millions of words.

The `CachedStr` newtype:

- Has a custom `PartialEq` that always returns `true`, so the cache
  field never affects equality checks or test assertions.
- Carries `serde(skip)`, `schemars(skip)`, `semantic_eq(skip)`,
  `span_shift(skip)` attributes — excluded from serialization,
  JSON schema, semantic equality, and span-shift derive macros.

Return type is `&str` (not `String`); callers that need an owned
copy add `.to_string()` explicitly.

`crates/talkbank-model/src/model/content/word/types.rs`.

## `ValidationContext` — `Arc<SharedValidationData>` Split

`ValidationContext` is split into:

- **Seven file-level-constant fields** wrapped in
  `Arc<SharedValidationData>`: `participant_ids`,
  `default_language`, `declared_languages`, `ca_mode`,
  `enable_quotation_validation`, `bullets_mode`, `config`.
- **Five per-tier mutable overlay fields** kept inline:
  `tier_language`, `field_span`, `field_text`, `field_label`,
  `field_error_code`.

Cloning a `ValidationContext` now copies the `Arc` pointer (8
bytes) + 5 small overlay fields, instead of deep-cloning a
`HashSet`, two `Vec`s, and a `ValidationConfig`. The struct is
cloned 3+ times per utterance (main / word / dependent tiers) and
again per word.

Builder methods use `Arc::make_mut()` for shared fields
(copy-on-write if the `Arc` is shared, no-op if unique). The
builder API is unchanged; the `Arc` only matters once construction
is complete.

`crates/talkbank-model/src/validation/context.rs` plus the
validators that read shared fields (`context.shared.participant_ids`,
not `context.participant_ids`).

## LSP `DashMap<Url, Arc<ChatFile>>`

LSP request handlers (hover, completion, symbols, folding, inlay
hints, formatting — ~10 handlers) get `Arc::clone()` of the
`ChatFile` rather than holding a `DashMap` shard guard. The shard
lock is released immediately after the pointer copy.

The mutation path (validation orchestrator) needs an owned
`ChatFile` for alignment computation. It extracts the old file via
`ChatFile::clone(entry.value())` — a deep clone — but this only
happens on file change (debounced), not on every request. After
mutation, the new `ChatFile` is wrapped in `Arc::new()` and
inserted atomically.

`crates/chatter-lsp/src/backend/state.rs`,
`requests.rs`, `validation_orchestrator.rs`.

## Reusable `TreeSitterParser`

`TreeSitterParser` is expensive to create (~1 ms for tree-sitter
init). Callers create one instance and pass `&TreeSitterParser`
through every operation. For batch workflows,
`parse_and_validate_with_parser()` accepts `&TreeSitterParser` to
avoid per-call construction overhead.

## SQLite Pragmas (Validation Cache)

The validation cache opens connections with:

```text
synchronous = NORMAL
cache_size  = -8000  (8 MB)
busy_timeout = 5000
mmap_size   = 268435456  (256 MB)
```

Combined ~2–3× write-throughput improvement with no durability
risk — WAL mode is already crash-safe.

## Per-Thread SQLite Connections

`CachePool` uses `thread_local::ThreadLocal<CacheConnection>` for
its SQLite handles. Each worker thread lazily opens its own
connection. WAL mode handles concurrency natively — concurrent
readers, serialized writers with `busy_timeout` retry. No `Mutex`
anywhere; cache hits are contention-free.

## Pass/Fail-Only Cache Schema

The validation cache stores a single boolean per file (valid /
invalid). The legacy schema's `errors` table (10+ columns per
error including miette-rendered diagnostics) and `corpora` table
are gone. Cache writes no longer serialize `ParseError` payloads;
DB size on a 95k-file corpus dropped from ~100 MB to ~2 MB.

## Release Profile

Both Rust workspaces (`talkbank-tools`, `tb`) compile release
binaries with:

```toml
[profile.release]
lto = true              # Fat LTO — whole-program optimization
codegen-units = 1       # Best cross-crate inlining
strip = true            # Strip symbols from deployed binaries
```

Net ~15% throughput improvement for CLI validation. Deploy scripts
use `--release` so these settings apply to fleet binaries
automatically; dev builds are unaffected.

## LSP `LineIndex`

`offset_to_position` is O(log n) via a pre-computed line-offset
index and binary search, replacing the original O(n) scan. This
eliminates O(n²) behavior for semantic tokens on large files.

## LSP `did_save` Skip

LSP skips re-validation on save if the document hasn't changed
since the last debounced `did_change` validation.

## Audit-Mode Parallelization

`run_audit_mode()` uses crossbeam worker threads with a bounded
work-stealing channel, matching the regular validation path.
Workers report completed file results to a dedicated audit-writer
thread, which writes JSONL results to disk immediately and owns
the summary counters. No worker contends on a shared
`Mutex<BufWriter<File>>`.

## Roundtrip — Parse-Only

`run_roundtrip()` re-parses the serialized output with
`ParseValidateOptions::default()` (parse only, no validation).
Roundtrip checks serialization fidelity, not content validity —
the caller already ensures validation passed before invoking
roundtrip.

## Prefix/Suffix Stripping in DP Alignment

The Hirschberg `align()` strips matching prefixes and suffixes in
O(n) before entering the O(mn) DP core. For WER and transcript
comparison (typically 80–95% accuracy), this reduces effective DP
problem size 10–100×. Only the differing middle portion enters
recursion. See [Dynamic Programming](../parser-and-grammar/dynamic-programming.md).

## Lazy Worker Bootstrap (Batchalign)

The Batchalign daemon binds the TCP port immediately, with `/health`
available, but **no Python process** runs until the first job
arrives. The daemon starts in < 1 second and uses zero memory at
idle. Memory guards only fire when actual work is requested. See
[Batchalign Workers — Memory Check Flow](batchalign-workers.md#memory-check-flow).
