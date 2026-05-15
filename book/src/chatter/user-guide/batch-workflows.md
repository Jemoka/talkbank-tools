# Batch Workflows

**Status:** Current
**Last updated:** 2026-05-02 02:30 EDT

The `chatter` CLI is designed for processing large CHAT corpora efficiently. This page covers common batch workflows.

## Validating a Corpus

Validate all `.cha` files in a directory tree:

```bash
chatter validate /path/to/corpus/
```

The validator recursively discovers `.cha` files and processes them in parallel. Results are cached — subsequent runs skip unchanged files.

### Forcing Revalidation

To bypass the cache and revalidate everything:

```bash
chatter validate /path/to/corpus/ --force
```

### Filtering Output

Show only errors (hide warnings):

```bash
chatter validate /path/to/corpus/ --quiet
```

Stop after the first reported error:

```bash
chatter validate /path/to/corpus/ --max-errors 1
```

Write a JSONL audit file while validating:

```bash
chatter validate /path/to/corpus/ --audit validation.jsonl
```

## CHAT-JSON Roundtrip

Convert an entire corpus to JSON and back:

```bash
# CHAT → JSON
for f in corpus/**/*.cha; do
  chatter to-json "$f" > "${f%.cha}.json"
done

# JSON → CHAT
for f in corpus/**/*.json; do
  chatter from-json "$f" > "${f%.json}.roundtrip.cha"
done
```

The roundtrip is designed to preserve the `ChatFile` model. In regression
tests, compare normalized output rather than assuming byte-for-byte identity
after parser or serializer changes.

## Cache Management

The validation cache stores results for previously validated files
(keyed by content hash). The cache database file is named
`talkbank-cache.db` and lives in the OS cache directory:

- **macOS:** `~/Library/Caches/talkbank-chat/talkbank-cache.db`
- **Linux:** `~/.cache/talkbank-chat/talkbank-cache.db`
- **Windows:** `%LocalAppData%\talkbank-chat\talkbank-cache.db`

It can hold results for large file collections.

```bash
chatter cache stats    # Show hit rates and entry count
chatter cache clear --all
```

Do not delete the cache file manually while `chatter` is running.

## Reference Corpus Validation

The `talkbank-tools` repository includes a reference corpus at
`resources/corpus/reference/` (currently ~100 `.cha` files; verify by
`find resources/corpus/reference -name '*.cha' | wc -l`). The parser must
handle every file in this corpus at 100%:

```bash
cargo nextest run -p talkbank-parser-tests -E 'test(parser_equivalence)'
```

This runs the parser equivalence test — each `.cha` file is its own test, so nextest runs them in parallel and reports individual failures.

## Integration with batchalign

The Batchalign pipeline uses the same Rust core (via PyO3) for CHAT
parsing and serialization. Since the 2026-04-28 monorepo merge,
Batchalign source lives inside this repository under `crates/batchalign-*`
(the standalone `batchalign3` GitHub repo was archived). Files processed
by Batchalign produce valid CHAT that passes `chatter validate`.
