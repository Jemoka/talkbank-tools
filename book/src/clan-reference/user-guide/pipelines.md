# Pipelines and Scripting

**Status:** Current
**Last updated:** 2026-05-11 18:58 EDT

## JSON for Scripting

For programmatic consumption, use `--format json` and pipe to `jq`. The schemas are the typed result structs in `crates/clan/clan-core/src/commands/<cmd>.rs` (each implements `Serialize`).

```bash
# Total token count for the first speaker in a FREQ run
chatter clan freq --format json file.cha | jq '.speakers[0].total_tokens'

# Get MLU value for CHI (assuming CHI is the only speaker after filtering)
chatter clan mlu --speaker CHI --format json file.cha | jq '.speakers[0].mlu'
```

## Batch Processing

Process many files and collect results:

```bash
# Frequency analysis across an entire corpus
chatter clan freq --format csv corpus/ > results.csv

# Per-file MLU in JSON
for f in corpus/*.cha; do
    echo "=== $f ==="
    chatter clan mlu --speaker CHI --format json "$f"
done
```

## Combining with Other Tools

```bash
# Find files with low MLU
chatter clan mlu --format json corpus/ | jq 'select(.speakers[].mlu < 2.0)'

# Top 10 most frequent words per speaker (FreqEntry.count is sorted desc by default)
chatter clan freq --format json file.cha | jq '.speakers[].entries[0:10]'
```

## Transform Chains

Transforms write the rewritten CHAT to stdout by default, or to a path you pass with `--output`. To chain them, either redirect each stage to a file and feed it to the next, or pass `--output` explicitly:

```bash
# Clean up a file: lowercase, then compute ages — explicit intermediate files
chatter clan lowcase file.cha --output file.low.cha
chatter clan dates   file.low.cha --output file.final.cha

# Or with shell redirection
chatter clan lowcase file.cha > file.low.cha
chatter clan dates   file.low.cha > file.final.cha
```

Transforms do not modify the input file in place; see `crates/clan/clan-core/src/framework/transform.rs::run_transform`.
