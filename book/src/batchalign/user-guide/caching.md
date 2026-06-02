# Caching

**Status:** Current
**Last updated:** 2026-04-27 10:28 EDT

## What gets cached

Batchalign caches **only audio-task results**:

| Analysis | Cached? |
|----------|---------|
| Forced alignment word timings (`align`) | Yes |
| ASR results for utterance timing recovery (`align`'s UTR pre-pass) | Yes |
| Media conversion (`.mp4`/`.m4a` → `.wav`) | Yes |
| Morphosyntax (`morphotag`) | **No** — always recomputed |
| Utterance segmentation (`utseg`) | **No** — always recomputed |
| Translation (`translate`) | **No** — always recomputed |
| Coreference (`coref`) | **No** — always recomputed |
| Speaker diarization | No |

The text-NLP cache that previously covered `morphotag`, `utseg`, and
`translate` was **removed** after a benchmark on a 15,748-file corpus
showed it was about 25× slower than just re-inferring (6–16% hit rate;
2,500 ms SQLite lookup beat ~100 ms inference savings). See the
architecture page on Caching for the detailed reasoning.

In practice: a re-run of `morphotag` on the same corpus takes the
same time as the first run. A re-run of `align` on the same audio
is much faster — that's where the cache pays for itself.

## What invalidates the cache

| What changed | What re-runs | What stays cached |
|---|---|---|
| Edited the transcript words | FA (per-group cache key includes text) | UTR ASR (only depends on audio) |
| Re-recorded or replaced the audio | FA, UTR ASR | (n/a — audio is the cache key) |
| Changed the language code | UTR ASR (key includes lang) | (other corpora's entries) |
| Upgraded batchalign (new ASR engine version) | Stale entries auto-invalidated | Entries from unchanged engines |

Cache keys are content-addressed: they hash the actual input (audio
identity, time spans, words, engine version). Changing any input
component produces a different key, so stale results are never
returned. Engine version strings are stored alongside each entry, so
upgrading a model (e.g., a new ASR release) automatically invalidates
old results without manual intervention.

## How to force fresh results

Use the `--override-media-cache` global flag:

```bash
batchalign3 --override-media-cache align corpus/ -o output/
```

This skips all cache lookups, forcing every audio span through fresh
inference. New results are still stored in the cache for future runs.

Use this when you suspect cached results are wrong, or after manually
updating model files outside of a normal batchalign upgrade.

## Where the caches are stored

| Cache | macOS default | Linux default |
|---|---|---|
| Analysis cache DB | `~/Library/Caches/batchalign3/cache.db` | `~/.cache/batchalign3/cache.db` |
| Media conversion cache | `~/Library/Application Support/batchalign3/media_cache/` | `~/.local/share/batchalign3/media_cache/` |

The analysis cache is a single SQLite database file. The media cache
stores converted WAV artifacts for inputs such as `.mp4` and `.m4a`.

For isolated runs or testing, you can relocate them with environment
variables:

```bash
export BATCHALIGN_ANALYSIS_CACHE_DIR=/tmp/ba-analysis-cache
export BATCHALIGN_MEDIA_CACHE_DIR=/tmp/ba-media-cache
```

## How to clear the cache

Use the built-in cache command:

```bash
batchalign3 cache stats          # See cache size and entry count
batchalign3 cache clear --yes    # Clear the cache
```

`cache stats` and `cache clear` operate on both the analysis cache and
the media conversion cache.

Or delete the `cache.db` file and/or the media-cache directory directly.

To selectively refresh without clearing everything, use
`--override-media-cache` on specific runs instead — old entries for
other corpora remain available.

## Old text-NLP cache entries

If you used batchalign before the text-NLP cache was removed, your
`cache.db` may still contain old `morphosyntax_v*`, `utseg_v*`, and
`translate_v*` rows. Those are dead weight — they're never read
anymore. Run `batchalign3 cache clear --yes` (or `rm -f
~/Library/Caches/batchalign3/cache.db*`) to reclaim the disk space.
