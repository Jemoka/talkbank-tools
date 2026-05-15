# Align Regression Fixtures

This directory holds real-world `batchalign3 align` regression
fixtures. Each subdirectory under `regressions/` is one bug.

**Privacy boundary.** `regressions/` is gitignored in the public
batchalign3 repository — the fixture content (input CHAT, audio
clips, reporter metadata) lives in a separate private fixture
repository, and contributors without access to that private repo
see the align regression tests `SKIP` rather than `FAIL`. See the
top-level `resources/fixtures/README.md` for the runner's fixture
resolution policy and the `BATCHALIGN3_PRIVATE_FIXTURES_DIR`
environment variable.

## Adding a new align regression

1. Trim the source CHAT + audio into a minimal pair using a
   structured trim helper that rebases timing bullets to the
   trimmed audio start and rewrites `@Media`. Do not hand-roll
   with `ffmpeg` or ad-hoc scripts.

2. Stage the trimmed pair under the private fixture repository
   path:
   `$BATCHALIGN3_PRIVATE_FIXTURES_DIR/align/regressions/<bug-name>/`.
   The directory name follows
   `<YYYY-MM-DD>-<opaque-reporter-slug>-<short-slug>/`.

3. Write `source.json` capturing where the material came from
   (opaque references, not real paths or reporter names) and
   what assertion mode the bug needs.

4. In the public batchalign3 repo, add a new test function slot
   to `crates/batchalign/tests/ml_golden/regression_fixtures.rs`:

   ```rust
   #[tokio::test]
   async fn align_regression_NNN() {
       run_fixture("align", "<bug-name>").await
   }
   ```

5. Run `cargo nextest run --profile ml -E 'test(regression_fixtures)'`
   and confirm the fixture is RED (or GREEN if it is a
   regression-prevention snapshot). Commit the runner change to
   the public repo and the fixture content to the private repo.

## Common failure classes

The assertion catalog in `tests/common/regression_manifest.rs`
currently captures:

- **Missing `%wor` timings** — FA emits words without per-word
  bullets. Catch with `no_zero_duration_wor_words`.
- **DP collapse to end** — the tail of the word sequence is
  crammed into 40-100 ms per word. Catch with
  `min_wor_word_duration_ms`.
- **Last-word cutoff** — the closing word of an utterance is
  squished into a sliver. Catch with
  `min_last_wor_word_duration_ms`.
- **First-word dominance** — one word eats > N% of the utterance
  bullet. Catch with `max_wor_word_duration_proportion`.

Add a new assertion variant when an incoming bug does not fit any
of these classes. Do not force a wrong assertion onto a new bug.
