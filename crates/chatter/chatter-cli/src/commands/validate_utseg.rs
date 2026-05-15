//! `chatter validate-utseg <input-dir> <output-dir>` — verify that an
//! utseg run preserved the invariants that matter.
//!
//! Background: `batchalign3 utseg` rewrites `*SPK:` lines by splitting
//! long utterances. The transformation must preserve some kinds of
//! data (main-tier terminal bullets, `@Media` linkage state, file
//! count) and is permitted to drop others (`%wor` per-word timing on
//! the specific utterances that get split — the BA3 architecture
//! deliberately decoupled `%wor` count validation from positional
//! alignment, see `talkbank-tools/resources/spec/errors/E341` and the
//! 2026-04-09 commits `3c178f49`/`ca18388f`/`f7d86537`).
//!
//! This subcommand walks two corpus directories — the pre-utseg input
//! and the post-utseg output — and reports per-file diffs along the
//! gated invariants. Exit code:
//!
//! - `0` — gate passed (no regressions).
//! - `1` — gate failed (file count diverged, main-tier bullet count
//!   dropped on any file, or `@Media` linkage state changed on any
//!   file). Use this exit code in pre-push CI / hooks.
//! - `2` — usage error.
//!
//! The `%wor` per-word bullet count is reported informationally and
//! does not affect the verdict. `%wor` drops on split utterances are
//! architecturally permitted; FA regenerates the data on the next
//! `align` pass.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use talkbank_model::model::dependent_tier::wor::WorItem;
use talkbank_model::model::{Line, MediaStatus};
use talkbank_parser::TreeSitterParser;
use walkdir::WalkDir;

/// `@Media` header linkage state. `LinkedDefault` is the implicit
/// linked state when no status token is present (the transcript
/// carries timing bullets aligned to media). `NoMediaHeader` is when
/// the file has no `@Media` line at all — distinct from any explicit
/// status because it changes the meaning of "linkage."
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LinkageState {
    NoMediaHeader,
    LinkedDefault,
    Unlinked,
    Missing,
    Notrans,
}

impl LinkageState {
    fn from_chat_file(file: &talkbank_model::model::ChatFile) -> Self {
        let Some(media) = file.media.as_ref() else {
            return Self::NoMediaHeader;
        };
        match &media.status {
            None => Self::LinkedDefault,
            Some(MediaStatus::Unlinked) => Self::Unlinked,
            Some(MediaStatus::Missing) => Self::Missing,
            Some(MediaStatus::Notrans) => Self::Notrans,
            // Unsupported tokens fall back to LinkedDefault: any change
            // between input and output linkage is what we care about,
            // not the specific token text.
            Some(MediaStatus::Unsupported(_)) => Self::LinkedDefault,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::NoMediaHeader => "no_media_header",
            Self::LinkedDefault => "linked",
            Self::Unlinked => "unlinked",
            Self::Missing => "missing",
            Self::Notrans => "notrans",
        }
    }
}

/// Per-file gate metrics. `main_bullets` is the gated invariant
/// (utseg must not drop main-tier terminal bullets); `wor_bullets`
/// is informational.
#[derive(Debug, Clone, Copy)]
struct FileMetrics {
    main_bullets: usize,
    wor_bullets: usize,
    linkage: LinkageState,
}

impl FileMetrics {
    fn from_chat_file(file: &talkbank_model::model::ChatFile) -> Self {
        let mut main_bullets = 0;
        let mut wor_bullets = 0;
        for line in &file.lines.0 {
            let Line::Utterance(utt) = line else { continue };
            if utt.main.content.bullet.is_some() {
                main_bullets += 1;
            }
            if let Some(wor) = utt.wor_tier() {
                for item in &wor.items {
                    if let WorItem::Word(word) = item
                        && word.inline_bullet.is_some()
                    {
                        wor_bullets += 1;
                    }
                }
            }
        }
        Self {
            main_bullets,
            wor_bullets,
            linkage: LinkageState::from_chat_file(file),
        }
    }
}

/// Per-file diff between input and output. The `is_regression`
/// predicate is what the gate's exit code is computed against.
#[derive(Debug, Clone)]
struct FileReport {
    relative_path: String,
    input: FileMetrics,
    output: FileMetrics,
}

impl FileReport {
    fn main_bullet_delta(&self) -> i64 {
        self.output.main_bullets as i64 - self.input.main_bullets as i64
    }

    fn lost_main_bullets(&self) -> bool {
        self.output.main_bullets < self.input.main_bullets
    }

    fn linkage_changed(&self) -> bool {
        self.input.linkage != self.output.linkage
    }
}

/// Walk a directory for `.cha` files; return the relative path → full
/// path map. `BTreeSet` for deterministic iteration order on output.
fn enumerate_cha(root: &Path) -> std::io::Result<BTreeSet<PathBuf>> {
    let mut out = BTreeSet::new();
    for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
        let path = entry.path();
        if path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("cha"))
        {
            let rel = path
                .strip_prefix(root)
                .map(PathBuf::from)
                .unwrap_or_else(|_| path.to_path_buf());
            out.insert(rel);
        }
    }
    Ok(out)
}

/// Result of running the gate over an input/output corpus pair.
#[derive(Debug, Default)]
struct GateResult {
    files: Vec<FileReport>,
    only_in_input: Vec<PathBuf>,
    only_in_output: Vec<PathBuf>,
}

impl GateResult {
    fn passed(&self) -> bool {
        self.only_in_input.is_empty()
            && self.only_in_output.is_empty()
            && self.files.iter().all(|f| !f.lost_main_bullets())
            && self.files.iter().all(|f| !f.linkage_changed())
    }

    fn total_in_main_bullets(&self) -> usize {
        self.files.iter().map(|f| f.input.main_bullets).sum()
    }

    fn total_out_main_bullets(&self) -> usize {
        self.files.iter().map(|f| f.output.main_bullets).sum()
    }

    fn total_in_wor_bullets(&self) -> usize {
        self.files.iter().map(|f| f.input.wor_bullets).sum()
    }

    fn total_out_wor_bullets(&self) -> usize {
        self.files.iter().map(|f| f.output.wor_bullets).sum()
    }
}

/// Drives the gate. Returns the process exit code:
/// `0` = pass, `1` = fail, `2` = usage error.
pub fn run_validate_utseg(input: &Path, output: &Path, quiet: bool) -> i32 {
    if !input.is_dir() {
        eprintln!("error: not a directory: {}", input.display());
        return 2;
    }
    if !output.is_dir() {
        eprintln!("error: not a directory: {}", output.display());
        return 2;
    }

    let in_files = match enumerate_cha(input) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error walking {}: {err}", input.display());
            return 2;
        }
    };
    let out_files = match enumerate_cha(output) {
        Ok(v) => v,
        Err(err) => {
            eprintln!("error walking {}: {err}", output.display());
            return 2;
        }
    };

    let parser = match TreeSitterParser::new() {
        Ok(p) => p,
        Err(err) => {
            eprintln!("failed to initialize parser: {err}");
            return 2;
        }
    };

    let mut result = GateResult::default();
    for rel in in_files.difference(&out_files) {
        result.only_in_input.push(rel.clone());
    }
    for rel in out_files.difference(&in_files) {
        result.only_in_output.push(rel.clone());
    }

    for rel in in_files.intersection(&out_files) {
        let in_path = input.join(rel);
        let out_path = output.join(rel);
        let in_metrics = match parse_metrics(&parser, &in_path) {
            Ok(m) => m,
            Err(err) => {
                eprintln!("warn: {} (input): {err}", rel.display());
                continue;
            }
        };
        let out_metrics = match parse_metrics(&parser, &out_path) {
            Ok(m) => m,
            Err(err) => {
                eprintln!("warn: {} (output): {err}", rel.display());
                continue;
            }
        };
        result.files.push(FileReport {
            relative_path: rel.display().to_string(),
            input: in_metrics,
            output: out_metrics,
        });
    }

    if quiet {
        println!("{}", if result.passed() { "PASS" } else { "FAIL" });
    } else {
        emit_text_report(&result);
    }
    if result.passed() { 0 } else { 1 }
}

fn parse_metrics(parser: &TreeSitterParser, path: &Path) -> Result<FileMetrics, String> {
    let text = std::fs::read_to_string(path).map_err(|e| format!("read: {e}"))?;
    let file = parser
        .parse_chat_file(&text)
        .map_err(|e| format!("parse: {e}"))?;
    Ok(FileMetrics::from_chat_file(&file))
}

fn emit_text_report(result: &GateResult) {
    let main_in = result.total_in_main_bullets();
    let main_out = result.total_out_main_bullets();
    let wor_in = result.total_in_wor_bullets();
    let wor_out = result.total_out_wor_bullets();
    println!("Files compared:          {}", result.files.len());
    let main_delta = main_out as i64 - main_in as i64;
    let wor_delta = wor_out as i64 - wor_in as i64;
    let main_pct = if main_in == 0 {
        0.0
    } else {
        (main_delta as f64) / (main_in as f64) * 100.0
    };
    let wor_pct = if wor_in == 0 {
        0.0
    } else {
        (wor_delta as f64) / (wor_in as f64) * 100.0
    };
    println!("Main-tier bullets in:    {main_in:>10}  (gated invariant)");
    println!("Main-tier bullets out:   {main_out:>10}");
    println!("Main-tier bullet delta:  {main_delta:>+10} ({main_pct:+.2}%)");
    println!("%wor bullets in:         {wor_in:>10}  (informational)");
    println!("%wor bullets out:        {wor_out:>10}");
    println!("%wor bullet delta:       {wor_delta:>+10} ({wor_pct:+.2}%)");
    println!();

    if !result.only_in_input.is_empty() {
        println!(
            "FAIL: {} files in input but missing from output:",
            result.only_in_input.len()
        );
        for f in result.only_in_input.iter().take(10) {
            println!("  - {}", f.display());
        }
        if result.only_in_input.len() > 10 {
            println!("  ... and {} more", result.only_in_input.len() - 10);
        }
    }
    if !result.only_in_output.is_empty() {
        println!(
            "FAIL: {} files in output but missing from input:",
            result.only_in_output.len()
        );
        for f in result.only_in_output.iter().take(10) {
            println!("  - {}", f.display());
        }
    }

    let losers: Vec<_> = result
        .files
        .iter()
        .filter(|f| f.lost_main_bullets())
        .collect();
    if !losers.is_empty() {
        println!("FAIL: {} files lost MAIN-TIER bullets:", losers.len());
        let mut sorted = losers.clone();
        sorted.sort_by_key(|f| f.main_bullet_delta());
        for f in sorted.iter().take(10) {
            println!(
                "  - {}: {} -> {} ({:+})",
                f.relative_path,
                f.input.main_bullets,
                f.output.main_bullets,
                f.main_bullet_delta()
            );
        }
        if losers.len() > 10 {
            println!("  ... and {} more", losers.len() - 10);
        }
    }

    let linkage_changes: Vec<_> = result
        .files
        .iter()
        .filter(|f| f.linkage_changed())
        .collect();
    if !linkage_changes.is_empty() {
        println!(
            "FAIL: {} files changed @Media linkage state:",
            linkage_changes.len()
        );
        for f in linkage_changes.iter().take(10) {
            println!(
                "  - {}: {} -> {}",
                f.relative_path,
                f.input.linkage.as_str(),
                f.output.linkage.as_str(),
            );
        }
    }

    println!();
    println!("VERDICT: {}", if result.passed() { "PASS" } else { "FAIL" });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    const BASE: &str = "@UTF8\n@Begin\n@Languages:\teng\n\
        @Participants:\tCHI Child\n\
        @ID:\teng|test|CHI|||||Child|||\n\
        @Media:\tsample, audio\n";
    const FOOTER: &str = "@End\n";

    fn write_file(dir: &Path, rel: &str, body: &str) {
        let path = dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        let mut text = String::from(BASE);
        text.push_str(body);
        text.push_str(FOOTER);
        fs::write(path, text).unwrap();
    }

    #[test]
    fn gate_passes_when_main_bullets_preserved() {
        let dir_in = tempdir().unwrap();
        let dir_out = tempdir().unwrap();
        write_file(
            dir_in.path(),
            "a.cha",
            "*CHI:\tone two three . \u{15}0_1000\u{15}\n",
        );
        write_file(
            dir_out.path(),
            "a.cha",
            "*CHI:\tone two .\n*CHI:\tthree . \u{15}0_1000\u{15}\n",
        );
        let rc = run_validate_utseg(dir_in.path(), dir_out.path(), true);
        assert_eq!(
            rc, 0,
            "main-tier bullet preserved on last child should pass"
        );
    }

    #[test]
    fn gate_fails_on_main_bullet_loss() {
        let dir_in = tempdir().unwrap();
        let dir_out = tempdir().unwrap();
        write_file(
            dir_in.path(),
            "a.cha",
            "*CHI:\tone two three . \u{15}0_1000\u{15}\n",
        );
        write_file(
            dir_out.path(),
            "a.cha",
            "*CHI:\tone two .\n*CHI:\tthree .\n",
        );
        let rc = run_validate_utseg(dir_in.path(), dir_out.path(), true);
        assert_eq!(rc, 1, "main-tier bullet drop must fail the gate");
    }

    #[test]
    fn gate_passes_when_only_wor_drops() {
        // Wor-only drop on a split utterance is architecturally permitted
        // (per the 2026-04-09 rename).
        let dir_in = tempdir().unwrap();
        let dir_out = tempdir().unwrap();
        write_file(
            dir_in.path(),
            "a.cha",
            "*CHI:\tone two three . \u{15}0_1000\u{15}\n\
             %wor:\tone \u{15}0_300\u{15} two \u{15}300_700\u{15} three \u{15}700_1000\u{15} .\n",
        );
        write_file(
            dir_out.path(),
            "a.cha",
            "*CHI:\tone two .\n*CHI:\tthree . \u{15}0_1000\u{15}\n",
        );
        let rc = run_validate_utseg(dir_in.path(), dir_out.path(), true);
        assert_eq!(rc, 0, "%wor drop alone is informational, gate passes");
    }

    #[test]
    fn gate_fails_on_missing_output_file() {
        let dir_in = tempdir().unwrap();
        let dir_out = tempdir().unwrap();
        write_file(dir_in.path(), "a.cha", "*CHI:\tone . \u{15}0_500\u{15}\n");
        write_file(
            dir_in.path(),
            "b.cha",
            "*CHI:\ttwo . \u{15}500_1000\u{15}\n",
        );
        write_file(dir_out.path(), "a.cha", "*CHI:\tone . \u{15}0_500\u{15}\n");
        let rc = run_validate_utseg(dir_in.path(), dir_out.path(), true);
        assert_eq!(rc, 1, "missing output file must fail the gate");
    }
}
