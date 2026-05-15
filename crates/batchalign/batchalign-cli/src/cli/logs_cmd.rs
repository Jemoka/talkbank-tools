//! `batchalign3 logs` — view, export, clear, or follow run logs.

use std::fs::{self, File};
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Duration;

use crate::config::RuntimeLayout;
use serde_json::Value;
use zip::CompressionMethod;
use zip::write::SimpleFileOptions;

use crate::cli::args::LogsArgs;
use crate::cli::error::CliError;

const MAX_RUNS: usize = 50;

/// Execute the `logs` command.
pub fn run(args: &LogsArgs) -> Result<(), CliError> {
    let layout = RuntimeLayout::from_env();
    let dir = log_dir(&layout);
    fs::create_dir_all(&dir)?;

    if args.follow {
        return follow_latest(&dir);
    }

    if args.clear {
        let deleted = clear_logs(&dir)?;
        eprintln!(
            "Deleted {deleted} log file{}.",
            if deleted == 1 { "" } else { "s" }
        );
        return Ok(());
    }

    if args.export {
        let out = export_logs(&dir, None)?;
        eprintln!("Logs exported to: {}", out.display());
        return Ok(());
    }

    let runs = list_runs(&dir, args.count);
    if runs.is_empty() {
        eprintln!("No run logs found.");
        return Ok(());
    }

    if args.last {
        let events = read_run(&runs[0])?;
        if args.raw {
            for event in events {
                println!("{}", serde_json::to_string(&event)?);
            }
        } else {
            println!("{}", format_run(&events));
        }
        return Ok(());
    }

    eprintln!("Recent runs (from {}):\n", dir.display());
    for run_path in &runs {
        let events = read_run(run_path)?;
        let start = events
            .iter()
            .find(|e| e.get("event").and_then(Value::as_str) == Some("run_start"));
        let end = events
            .iter()
            .find(|e| e.get("event").and_then(Value::as_str) == Some("run_end"));

        let cmd = event_str(start, "command").unwrap_or("?");
        let ts = event_str(start, "ts")
            .map(ToOwned::to_owned)
            .unwrap_or_else(|| {
                run_path
                    .file_stem()
                    .map(|s| s.to_string_lossy().to_string())
                    .unwrap_or_else(|| "?".to_string())
            });
        let elapsed = event_num(end, "total_elapsed_s")
            .map(|x| format!("{x}s"))
            .unwrap_or_else(|| "incomplete".to_string());
        let file_count = events
            .iter()
            .find(|e| e.get("event").and_then(Value::as_str) == Some("files_discovered"))
            .and_then(|e| event_num(Some(e), "count"))
            .unwrap_or_else(|| "?".to_string());
        let errors = events
            .iter()
            .filter(|e| e.get("event").and_then(Value::as_str) == Some("file_error"))
            .count();
        let status = if errors == 0 {
            "ok".to_string()
        } else {
            format!("{errors} errors")
        };

        eprintln!("  {ts}  {cmd:<14}  {file_count:>3} files  {elapsed:>8}  {status}");
    }
    eprintln!();

    Ok(())
}

fn log_dir(layout: &RuntimeLayout) -> PathBuf {
    layout.logs_dir()
}

fn list_runs(dir: &Path, limit: usize) -> Vec<PathBuf> {
    let mut logs: Vec<PathBuf> = match fs::read_dir(dir) {
        Ok(rd) => rd
            .flatten()
            .map(|e| e.path())
            .filter(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.starts_with("run-") && n.ends_with(".jsonl"))
                    .unwrap_or(false)
            })
            .collect(),
        Err(_) => Vec::new(),
    };

    logs.sort();
    logs.reverse();
    logs.truncate(limit);
    logs
}

fn read_run(path: &Path) -> Result<Vec<Value>, CliError> {
    let file = File::open(path)?;
    let mut events = Vec::new();
    for line in BufReader::new(file).lines() {
        let line = line?;
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        events.push(serde_json::from_str::<Value>(line)?);
    }
    Ok(events)
}

fn format_run(events: &[Value]) -> String {
    let mut out = String::new();
    out.push_str("=== Batchalign Run Log ===\n");

    let start = events
        .iter()
        .find(|e| e.get("event").and_then(Value::as_str) == Some("run_start"));
    let end = events
        .iter()
        .find(|e| e.get("event").and_then(Value::as_str) == Some("run_end"));

    if let Some(s) = start {
        out.push_str(&format!(
            "Command:    {}\n",
            event_str(Some(s), "command").unwrap_or("?")
        ));
        out.push_str(&format!(
            "Language:   {}\n",
            event_str(Some(s), "lang").unwrap_or("?")
        ));
        out.push_str(&format!(
            "Started:    {}\n",
            event_str(Some(s), "ts").unwrap_or("?")
        ));
    }

    let done_count = events
        .iter()
        .filter(|e| e.get("event").and_then(Value::as_str) == Some("file_done"))
        .count();
    let fail_count = events
        .iter()
        .filter(|e| e.get("event").and_then(Value::as_str) == Some("file_error"))
        .count();
    let total = done_count + fail_count;

    out.push('\n');
    out.push_str(&format!(
        "Result:     {done_count}/{total} succeeded, {fail_count} failed\n"
    ));
    if let Some(e) = end.and_then(|e| event_num(Some(e), "total_elapsed_s")) {
        out.push_str(&format!("Total time: {e}s\n"));
    }

    out
}

fn clear_logs(dir: &Path) -> Result<usize, CliError> {
    let runs = list_runs(dir, usize::MAX);
    let mut deleted = 0usize;
    for p in &runs {
        if fs::remove_file(p).is_ok() {
            deleted += 1;
        }
    }
    Ok(deleted)
}

fn export_logs(dir: &Path, output_path: Option<PathBuf>) -> Result<PathBuf, CliError> {
    let output_path = output_path.unwrap_or_else(|| dir.join("batchalign-logs.zip"));
    let runs = list_runs(dir, MAX_RUNS);

    let file = File::create(&output_path)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Deflated)
        .unix_permissions(0o644);

    for log_path in &runs {
        let Some(name) = log_path.file_name().and_then(|n| n.to_str()) else {
            continue;
        };
        zip.start_file(name, options)
            .map_err(std::io::Error::other)?;
        let mut f = File::open(log_path)?;
        std::io::copy(&mut f, &mut zip)?;
    }
    zip.finish().map_err(std::io::Error::other)?;
    Ok(output_path)
}

fn follow_latest(dir: &Path) -> Result<(), CliError> {
    eprintln!(
        "Watching {} for log events... (Ctrl-C to stop)\n",
        dir.display()
    );

    let mut current_path: Option<PathBuf> = None;
    let mut file_pos: u64 = 0;

    loop {
        let newest = list_runs(dir, 1).into_iter().next();

        if newest != current_path {
            current_path = newest;
            file_pos = 0;
            if let Some(path) = &current_path {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string());
                eprintln!("Following: {name}");
            }
        }

        if let Some(path) = &current_path
            && path.exists()
        {
            let mut f = File::open(path)?;
            f.seek(SeekFrom::Start(file_pos))?;

            let mut reader = BufReader::new(f);
            let mut line = String::new();
            loop {
                line.clear();
                let n = reader.read_line(&mut line)?;
                if n == 0 {
                    break;
                }
                file_pos += n as u64;
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                match serde_json::from_str::<Value>(trimmed) {
                    Ok(v) => eprintln!("{}", format_event_line(&v)),
                    Err(_) => eprintln!("{trimmed}"),
                }
            }
        }

        std::thread::sleep(Duration::from_millis(500));
    }
}

fn format_event_line(event: &Value) -> String {
    let ts = event_str(Some(event), "ts")
        .map(|raw| {
            if let Some(idx) = raw.find('T') {
                raw[idx + 1..].chars().take(12).collect::<String>()
            } else {
                raw.to_string()
            }
        })
        .unwrap_or_default();
    let ev = event_str(Some(event), "event").unwrap_or("?");

    match ev {
        "cli_startup" => format!(
            "[{ts}] CLI startup: {}s",
            event_num(Some(event), "elapsed_s").unwrap_or_else(|| "?".into())
        ),
        "run_start" => format!(
            "[{ts}] START {} lang={}",
            event_str(Some(event), "command").unwrap_or("?"),
            event_str(Some(event), "lang").unwrap_or("?")
        ),
        "files_discovered" => format!(
            "[{ts}] Found {} files ({} MB)",
            event_num(Some(event), "count").unwrap_or_else(|| "?".into()),
            event_num(Some(event), "total_size_mb").unwrap_or_else(|| "?".into())
        ),
        "model_loading" => format!(
            "[{ts}] Loading models (tasks={})...",
            event_str(Some(event), "tasks").unwrap_or("?")
        ),
        "model_ready" => format!(
            "[{ts}] Models ready in {}s",
            event_num(Some(event), "elapsed_s").unwrap_or_else(|| "?".into())
        ),
        "workers_configured" => format!(
            "[{ts}] {} workers configured",
            event_num(Some(event), "count").unwrap_or_else(|| "?".into())
        ),
        "file_start" => format!(
            "[{ts}] >> {} ({} MB)",
            event_str(Some(event), "file").unwrap_or("?"),
            event_num(Some(event), "size_mb").unwrap_or_else(|| "?".into())
        ),
        "file_done" => format!(
            "[{ts}] << {} done ({}s)",
            event_str(Some(event), "file").unwrap_or("?"),
            event_num(Some(event), "elapsed_s").unwrap_or_else(|| "?".into())
        ),
        "file_error" => format!(
            "[{ts}] !! {} FAILED: {}",
            event_str(Some(event), "file").unwrap_or("?"),
            event_str(Some(event), "error").unwrap_or("?")
        ),
        "run_end" => format!(
            "[{ts}] END total={}s",
            event_num(Some(event), "total_elapsed_s").unwrap_or_else(|| "?".into())
        ),
        _ => format!("[{ts}] {ev}"),
    }
}

fn event_str<'a>(event: Option<&'a Value>, key: &str) -> Option<&'a str> {
    event
        .and_then(|e| e.get(key))
        .and_then(|v| if v.is_string() { v.as_str() } else { None })
}

fn event_num(event: Option<&Value>, key: &str) -> Option<String> {
    let v = event?.get(key)?;
    if v.is_number() {
        if let Some(i) = v.as_i64() {
            return Some(i.to_string());
        }
        if let Some(u) = v.as_u64() {
            return Some(u.to_string());
        }
        if let Some(f) = v.as_f64() {
            return Some(format!("{f:.1}"));
        }
    }
    if v.is_string() {
        return v.as_str().map(ToOwned::to_owned);
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_event_line_basic() {
        let ev = serde_json::json!({
            "ts": "2026-02-24T10:11:12.000Z",
            "event": "run_start",
            "command": "morphotag",
            "lang": "eng",
        });
        let line = format_event_line(&ev);
        assert!(line.contains("START morphotag lang=eng"));
    }

    #[test]
    fn list_runs_sorts_desc() {
        let td = tempfile::TempDir::new().unwrap();
        let dir = td.path();
        fs::write(dir.join("run-2026-02-24T10-00-00.jsonl"), "{}\n").unwrap();
        fs::write(dir.join("run-2026-02-24T11-00-00.jsonl"), "{}\n").unwrap();
        let runs = list_runs(dir, 10);
        assert_eq!(runs.len(), 2);
        assert!(runs[0].file_name().unwrap() > runs[1].file_name().unwrap());
    }
}
