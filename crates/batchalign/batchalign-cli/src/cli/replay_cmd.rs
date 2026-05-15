//! `batchalign3 replay` — replay a captured failed IPC request.
//!
//! Takes a `failed_ipc_*.json` dump from `~/.batchalign3/debug/` and sends
//! the exact request to a fresh worker, reporting the response.

use crate::cli::args::ReplayArgs;
use crate::cli::error::CliError;
use crate::cli::python::resolve_python_executable;

use std::io::{BufRead, Write};
use std::process::{Command, Stdio};
use std::time::Instant;

/// Parsed dump file structure.
#[derive(Debug, serde::Deserialize)]
struct FailedIpcDump {
    worker_label: String,
    request: String,
    error_message: String,
}

/// Run the replay command.
pub async fn run(args: &ReplayArgs) -> Result<(), CliError> {
    // 1. Load the dump file
    let dump_text = std::fs::read_to_string(&args.dump_file).map_err(|e| {
        CliError::InvalidArgument(format!(
            "Cannot read dump file {}: {e}",
            args.dump_file.display()
        ))
    })?;

    let dump: FailedIpcDump = serde_json::from_str(&dump_text)
        .map_err(|e| CliError::InvalidArgument(format!("Cannot parse dump file: {e}")))?;

    eprintln!(
        "Replaying failed IPC request from {}",
        args.dump_file.display()
    );
    eprintln!("  Worker label: {}", dump.worker_label);
    eprintln!("  Original error: {}", dump.error_message);

    // 2. Parse the original request to extract task and lang
    let request_val: serde_json::Value = serde_json::from_str(&dump.request).map_err(|e| {
        CliError::InvalidArgument(format!("Cannot parse request JSON from dump: {e}"))
    })?;

    // Extract task from the request envelope
    let task = request_val
        .pointer("/request/task")
        .and_then(|v| v.as_str())
        .unwrap_or("morphosyntax");

    let lang = args.lang.as_deref().unwrap_or_else(|| {
        // Try to extract from worker label (e.g., "profile:gpu:eng")
        dump.worker_label.split(':').nth(2).unwrap_or("eng")
    });

    let python = args
        .python
        .clone()
        .unwrap_or_else(resolve_python_executable);

    eprintln!("  Task: {task}");
    eprintln!("  Lang: {lang}");
    eprintln!("  Python: {python}");
    eprintln!();

    // 3. Spawn a fresh worker
    let mut cmd = Command::new(&python);
    cmd.args(["-m", "batchalign.worker", "--task", task, "--lang", lang]);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    let mut child = cmd
        .spawn()
        .map_err(|e| CliError::InvalidArgument(format!("Cannot spawn worker: {e}")))?;

    let stdout = child
        .stdout
        .take()
        .ok_or(CliError::InvalidArgument("No stdout".into()))?;
    let mut stdin = child
        .stdin
        .take()
        .ok_or(CliError::InvalidArgument("No stdin".into()))?;
    let reader = std::io::BufReader::new(stdout);
    let mut lines = reader.lines();

    // 4. Wait for ready
    let start = Instant::now();
    let mut ready = false;
    let deadline = Instant::now() + std::time::Duration::from_secs(120);
    while let Some(Ok(line)) = lines.next() {
        if Instant::now() > deadline {
            let _ = child.kill();
            return Err(CliError::InvalidArgument(
                "Timeout waiting for worker ready".into(),
            ));
        }
        if line.contains("\"ready\"") && line.contains("true") {
            ready = true;
            eprintln!("Worker ready ({:.1}s)", start.elapsed().as_secs_f64());
            break;
        }
    }
    if !ready {
        let _ = child.kill();
        return Err(CliError::InvalidArgument(
            "Worker exited without ready signal".into(),
        ));
    }

    // 5. Send the exact original request
    eprintln!("Sending original request...");
    writeln!(stdin, "{}", dump.request)
        .map_err(|e| CliError::InvalidArgument(format!("Write failed: {e}")))?;
    stdin
        .flush()
        .map_err(|e| CliError::InvalidArgument(format!("Flush failed: {e}")))?;

    // 6. Read and report the response
    let response_deadline = Instant::now() + std::time::Duration::from_secs(120);
    while let Some(Ok(line)) = lines.next() {
        if Instant::now() > response_deadline {
            let _ = child.kill();
            return Err(CliError::InvalidArgument(
                "Timeout waiting for response".into(),
            ));
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        match serde_json::from_str::<serde_json::Value>(trimmed) {
            Ok(val) => {
                let op = val.get("op").and_then(|v| v.as_str()).unwrap_or("?");
                eprintln!("Response op: {op}");

                if op == "error" {
                    let err = val.get("error").and_then(|v| v.as_str()).unwrap_or("?");
                    eprintln!("Worker returned error: {err}");
                } else {
                    eprintln!("Worker returned success.");
                }

                // Pretty-print the full response
                println!("{}", serde_json::to_string_pretty(&val).unwrap_or(line));

                // Shutdown and exit
                let _ = writeln!(stdin, r#"{{"op":"shutdown"}}"#);
                let _ = child.wait();
                return Ok(());
            }
            Err(_) => {
                eprintln!("(noise) {trimmed}");
            }
        }
    }

    let _ = child.kill();
    Err(CliError::InvalidArgument(
        "Worker exited without response".into(),
    ))
}
