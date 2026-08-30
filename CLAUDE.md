# Batchalign development guide

**Last modified:** 2026-08-30 12:03 EDT

Batchalign is the only product built from this repository. CHAT parsing,
validation, typed models, transforms, and the tree-sitter grammar are consumed
from the version-pinned TalkBank Chatter dependency.

## Build and run

Use Bazel through the repository recipes:

```bash
just batchalign build debug
just batchalign test debug
just batchalign pytest
just batchalign cli -- --help
```

`just batchalign cli` is the authoritative CLI integration path. Do not replace
it with an ad hoc Python or maturin invocation. Local Bazel work defaults to one
job so Rust compilation and ML model execution do not contend for memory.

## Design rules

1. Types document stable domain concepts; avoid primitive and tuple-packed
   seams.
2. Keep modules focused and public APIs narrow. Methods should make ownership
   and the intended action obvious.
3. Do not introduce one-use helper functions, god functions, or walls of
   unrelated functions, methods, or classes.
4. Similar operations use parallel semantic names.
5. Production failures return typed errors; do not use panics as control flow.
6. Construct and mutate CHAT through the typed Chatter APIs. Do not assemble
   `%mor`, `%gra`, or other CHAT tiers with string concatenation.
7. Backend submission must remain bounded. Model-bearing work is grouped by
   language so one cached Stanza pipeline serves compatible requests.
8. Work taking more than about one second must report progress through the
   Batchalign progress protocol.
9. Documentation edits update their timestamp using the actual system time.

## Verification

Run focused tests while iterating, then the Batchalign Bazel test targets.
Exercise corpus fixtures in small chunks and clear the Batchalign task cache
when comparing behavior across implementations. Never overlap a Rust/Bazel
compile with a Stanza runtime test on a memory-constrained host.
