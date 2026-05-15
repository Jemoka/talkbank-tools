# avqi — Developer Reference

**Status:** Current
**Last updated:** 2026-05-02 08:18 EDT

Implementation guide for the `avqi` command. For user-facing documentation,
see [User Guide: avqi](../../user-guide/commands/avqi.md).

---

## Implementation map

| Layer | Location | Responsibility |
|-------|----------|----------------|
| CLI args | `crates/batchalign/src/cli/args/commands.rs` — `AvqiArgs` | Positional input/output dirs, lang |
| Command definition | `crates/batchalign/src/commands/avqi.rs` | `CommandDefinition` impl, paired file discovery |
| Audio prep | Shared media prep | Converts `.cs.*` and `.sv.*` to typed PCM artifacts |
| Worker IPC | `python/batchalign/inference/avqi.py` — `calculate_avqi()` | parselmouth + torchaudio analysis |
| Output writer | `crates/batchalign/src/commands/avqi.rs` | Writes `.avqi.txt` from typed metrics struct |

---

## Positional I/O

Like `opensmile`, `avqi` uses positional `INPUT_DIR OUTPUT_DIR` rather than
`CommonOpts`. Inherited from BA2 for interface parity.

---

## Paired file matching

For each continuous speech file `STEM.cs.EXT` in `INPUT_DIR`, the command
looks for `STEM.sv.EXT` (any supported audio extension). Unpaired files are
reported as errors. Matching is case-insensitive on the `.cs.` / `.sv.`
fragment; the extension can differ between the two files of a pair.

---

## Worker IPC: avqi task (V2 protocol)

```text
execute_v2 request:
{
  "task": "avqi",
  "cs_audio": { path, start_ms, end_ms, sample_rate },
  "sv_audio": { path, start_ms, end_ms, sample_rate }
}

execute_v2 response:
{
  "avqi_score": 3.14,
  "hnr": 12.5,
  "jitter": 0.003,
  "shimmer": 0.04,
  ...
}
```

---

## Daemon preference

`avqi` prefers the local daemon (auto_daemon path) when available. Explicit
`--server` overrides this. The daemon preference exists because AVQI requires
access to both paired audio files on the same host, which is always true for
the local daemon but may not be true for a remote server.

---

## Related developer documentation

- [Command Flowcharts: avqi](../../architecture/command-flowcharts.md#avqi)
