//! Binary subprocess tests for `batchalign3`.
//!
//! Uses `assert_cmd` to run the binary and verify exit codes, stdout, stderr.
//! No server is required — these tests exercise the CLI argument parsing,
//! help output, hidden command redirects, and utility commands against a
//! HOME-isolated tempdir.

mod cli_common;
mod common;

use predicates::prelude::*;

use cli_common::{
    CliHarness, MINIMAL_CHAT, cli_cmd as cmd, ffmpeg_available, resolve_python, start_live_server,
    transcode_audio_to_mp4, write_silent_mp4, write_silent_wav,
};
use common::test_server_fixture::acquire_test_server_session;

// ---------------------------------------------------------------------------
// Version / help
// ---------------------------------------------------------------------------

#[test]
fn version() {
    cmd()
        .arg("version")
        .assert()
        .success()
        .stderr(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn help_lists_visible_commands() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("align"))
        .stdout(predicate::str::contains("transcribe"))
        .stdout(predicate::str::contains("compare"))
        .stdout(predicate::str::contains("benchmark"))
        .stdout(predicate::str::contains("morphotag"))
        .stdout(predicate::str::contains("cache"))
        .stdout(predicate::str::contains("logs"))
        .stdout(predicate::str::contains("setup"))
        .stdout(predicate::str::contains("models"))
        .stdout(predicate::str::contains("avqi"))
        .stdout(predicate::str::contains("bench"));
}

#[test]
fn help_per_subcommand() {
    for subcmd in &[
        "align",
        "transcribe",
        "translate",
        "morphotag",
        "coref",
        "utseg",
        "benchmark",
        "opensmile",
        "compare",
        "avqi",
        "setup",
        "serve",
        "jobs",
        "logs",
        "openapi",
        "cache",
        "models",
        "bench",
    ] {
        cmd()
            .args([subcmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::is_empty().not());
    }
}

#[test]
fn workflow_command_help_descriptions_match_narrative() {
    for (subcmd, expected) in [
        (
            "align",
            "Align transcripts against corresponding media files",
        ),
        ("transcribe", "Create a transcript from audio files"),
        (
            "morphotag",
            "Perform morphosyntactic analysis on transcripts",
        ),
        (
            "compare",
            "Compare transcripts against gold-standard references",
        ),
        ("benchmark", "Benchmark ASR word accuracy"),
    ] {
        cmd()
            .args([subcmd, "--help"])
            .assert()
            .success()
            .stdout(predicate::str::contains(expected));
    }
}

#[test]
fn verbose_flag() {
    cmd().args(["-vvv", "version"]).assert().success();
}

// ---------------------------------------------------------------------------
// Bench behavior
// ---------------------------------------------------------------------------

#[test]
fn bench_listed_in_help() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("\n  bench "));
}

#[test]
#[ignore = "integration test: requires running daemon or spawns one with 90s health timeout"]
fn bench_runs_single_iteration() {
    let tmp = tempfile::TempDir::new().unwrap();
    let in_dir = tmp.path().join("in");
    let out_dir = tmp.path().join("out");
    std::fs::create_dir_all(&in_dir).unwrap();
    std::fs::create_dir_all(&out_dir).unwrap();

    cmd()
        .args([
            "bench",
            "align",
            in_dir.to_str().unwrap(),
            out_dir.to_str().unwrap(),
            "--runs",
            "1",
        ])
        .assert()
        .success()
        .stdout(predicate::str::contains("BENCH_RESULT:"));
}

#[test]
fn setup_non_interactive_writes_config() {
    let harness = CliHarness::new();
    harness
        .cmd()
        .args(["setup", "--non-interactive"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Saved configuration"));

    let cfg = harness.home_dir().join(".batchalign.ini");
    let content = std::fs::read_to_string(cfg).unwrap();
    assert!(content.contains("engine = whisper"));
}

#[test]
fn openapi_writes_output_file() {
    let tmp = tempfile::TempDir::new().unwrap();
    let schema = tmp.path().join("crates/batchalign/batchalign-cli/openapi.json");

    cmd()
        .args(["openapi", "--output", schema.to_str().unwrap()])
        .assert()
        .success();

    let content = std::fs::read_to_string(schema).unwrap();
    assert!(content.contains("\"openapi\": \"3.1.0\""));
}

#[test]
fn openapi_check_passes_when_fresh() {
    let tmp = tempfile::TempDir::new().unwrap();
    let schema = tmp.path().join("crates/batchalign/batchalign-cli/openapi.json");

    cmd()
        .args(["openapi", "--output", schema.to_str().unwrap()])
        .assert()
        .success();

    cmd()
        .args(["openapi", "--check", "--output", schema.to_str().unwrap()])
        .assert()
        .success()
        .stderr(predicate::str::contains("up to date"));
}

#[test]
fn openapi_check_fails_when_schema_is_stale() {
    let tmp = tempfile::TempDir::new().unwrap();
    let schema = tmp.path().join("crates/batchalign/batchalign-cli/openapi.json");
    std::fs::write(&schema, "{}").unwrap();

    cmd()
        .args(["openapi", "--check", "--output", schema.to_str().unwrap()])
        .assert()
        .failure()
        .code(6)
        .stderr(predicate::str::contains("OpenAPI schema is out of date"));
}

// ---------------------------------------------------------------------------
// Models command
// ---------------------------------------------------------------------------

#[test]
fn models_python_module_importable() {
    // The `models` command forwards to `python -m batchalign.models.training.run`.
    // Verify the Python module can at least be found (import check).
    let Some(python) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };

    let output = std::process::Command::new(&python)
        .args(["-c", "import batchalign.models.training.run"])
        .output()
        .unwrap_or_else(|e| panic!("Failed to run {python}: {e}"));

    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        output.status.success(),
        "`{python} -c 'import batchalign.models.training.run'` failed:\n{stderr}"
    );
}

// ---------------------------------------------------------------------------
// Argument validation
// ---------------------------------------------------------------------------

#[test]
fn unknown_subcommand() {
    cmd().arg("doesnotexist").assert().failure().code(2);
}

#[test]
fn processing_cmd_no_paths() {
    // morphotag with no paths is caught by resolve_inputs (not clap), exits usage(2)
    cmd()
        .arg("morphotag")
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("no input paths"));
}

#[test]
fn processing_cmd_nonexistent_path() {
    cmd()
        .args(["morphotag", "/nonexistent_path_abc123"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("does not exist").or(predicate::str::contains("error")));
}

#[test]
fn setup_non_interactive_rev_without_key_is_usage_error() {
    cmd()
        .args(["setup", "--non-interactive", "--engine", "rev"])
        .assert()
        .failure()
        .code(2)
        .stderr(predicate::str::contains("requires --rev-key"));
}

#[test]
fn jobs_unreachable_server_is_network_error() {
    cmd()
        .args(["jobs", "--server", "http://127.0.0.1:59999"])
        .assert()
        .failure()
        .code(4);
}

#[test]
fn serve_start_invalid_config_is_config_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bad_cfg = tmp.path().join("bad.yaml");
    std::fs::write(&bad_cfg, ":\n  - invalid").unwrap();

    cmd()
        .args([
            "serve",
            "start",
            "--foreground",
            "--config",
            bad_cfg.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("failed to parse config"));
}

#[test]
fn serve_start_unknown_config_field_is_config_error() {
    let tmp = tempfile::TempDir::new().unwrap();
    let bad_cfg = tmp.path().join("bad.yaml");
    std::fs::write(&bad_cfg, "port: 9123\nwarmup: false\n").unwrap();

    cmd()
        .args([
            "serve",
            "start",
            "--foreground",
            "--config",
            bad_cfg.to_str().unwrap(),
        ])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("unknown field `warmup`"));
}

// ---------------------------------------------------------------------------
// Cache commands (HOME-isolated)
// ---------------------------------------------------------------------------

#[test]
fn cache_stats_no_db() {
    let harness = CliHarness::new();
    harness
        .cmd()
        .args(["cache", "stats"])
        .assert()
        .success()
        .stderr(predicate::str::contains("Entries:"));
}

#[test]
fn cache_clear_no_db() {
    let harness = CliHarness::new();
    harness
        .cmd()
        .args(["cache", "clear", "-y"])
        .assert()
        .success();
}

// ---------------------------------------------------------------------------
// Serve status (no server running)
// ---------------------------------------------------------------------------

#[test]
fn serve_status_unreachable() {
    cmd()
        .args(["serve", "status", "--server", "http://127.0.0.1:59999"])
        .assert()
        .success()
        .stderr(predicate::str::contains("cannot reach"));
}

#[test]
fn serve_status_unknown_config_field_is_config_error() {
    let harness = CliHarness::new();
    harness.write_server_config("port: 9123\nwarmup: false\n");

    harness
        .cmd()
        .args(["serve", "status"])
        .assert()
        .failure()
        .code(3)
        .stderr(predicate::str::contains("unknown field `warmup`"));
}

// ---------------------------------------------------------------------------
// CLI subprocess with real server (P0)
// ---------------------------------------------------------------------------

/// Start a live server and run `batchalign3 morphotag` as a CLI subprocess
/// against it, verifying end-to-end that the binary can talk to a real server,
/// process files, and produce output with %mor and %gra tiers.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_morphotag_real_server() {
    use batchalign::worker::InferTask;

    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3 with batchalign not available");
        return;
    };

    let tmp = tempfile::TempDir::new().expect("tempdir");
    // Set up input directory with a .cha file
    let in_dir = tmp.path().join("input");
    let out_dir = tmp.path().join("output");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    std::fs::write(
        in_dir.join("test.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tCHI Target_Child\n@ID:\teng|test|CHI||female|||Target_Child|||\n*CHI:\thello world .\n@End\n",
    ).expect("write input");

    let server = match start_live_server(
        &python_path,
        vec![batchalign_types::paths::ServerPath::new(in_dir.clone())],
    )
    .await
    {
        Ok(server) => server,
        Err(message) => {
            eprintln!("SKIP: {message}");
            return;
        }
    };
    if !server.has_infer_task(InferTask::Morphosyntax) {
        eprintln!("SKIP: live server does not advertise morphosyntax infer support");
        server.shutdown().await;
        return;
    }

    // Run CLI subprocess in a blocking task so it doesn't block the tokio
    // runtime (which is serving the axum server on the same runtime).
    let in_str = in_dir.to_str().unwrap().to_string();
    let out_str = out_dir.to_str().unwrap().to_string();
    let url = server.base_url().to_string();
    let cli_result = tokio::task::spawn_blocking(move || {
        let mut command = cmd();
        command
            .args(["morphotag", &in_str, &out_str, "--server", &url])
            .output()
            .expect("spawn CLI")
    })
    .await
    .expect("blocking task");

    server.shutdown().await;

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "CLI should succeed. stderr: {stderr}"
    );

    // The CLI should succeed and report "All done!" or "written".
    // With test-echo workers, the morphosyntax pipeline may not produce
    // valid output (test-echo echoes raw IPC, not NLP results), so we
    // only verify the CLI completes without error.
    assert!(
        stderr.contains("All done") || stderr.contains("written"),
        "CLI should report completion. stderr: {stderr}"
    );

    let output_chat =
        std::fs::read_to_string(out_dir.join("test.cha")).expect("read morphotag output");
    assert!(
        output_chat.contains("%mor:"),
        "morphotag output should contain a %mor tier. output:\n{output_chat}"
    );
    assert!(
        output_chat.contains("%gra:"),
        "morphotag output should contain a %gra tier. output:\n{output_chat}"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_align_real_server_live_fa_succeeds() {
    use batchalign::worker::InferTask;

    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };

    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir parent")
        .parent()
        .expect("repo root");
    let source_audio = repo.join("python/batchalign/tests/support/test.mp3");
    let source_chat = repo.join("batchalign/tests/formats/chat/support/test.cha");
    if !source_audio.is_file() || !source_chat.is_file() {
        eprintln!("SKIP: committed align fixture pair is unavailable");
        return;
    }

    let tmp = tempfile::TempDir::new().expect("tempdir");
    let in_dir = tmp.path().join("input");
    let out_dir = tmp.path().join("output");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    std::fs::copy(&source_audio, in_dir.join("test.mp3")).expect("copy audio");

    let stripped_chat = std::fs::read_to_string(&source_chat)
        .expect("read source chat")
        .lines()
        .filter(|line| {
            !(line.starts_with("%mor:") || line.starts_with("%gra:") || line.starts_with("%wor:"))
        })
        .collect::<Vec<_>>()
        .join("\n")
        + "\n";
    std::fs::write(in_dir.join("test.cha"), stripped_chat).expect("write stripped chat");

    let server = match start_live_server(
        &python_path,
        vec![batchalign_types::paths::ServerPath::new(in_dir.clone())],
    )
    .await
    {
        Ok(server) => server,
        Err(message) => {
            eprintln!("SKIP: {message}");
            return;
        }
    };
    if !server.has_infer_task(InferTask::Fa) {
        eprintln!("SKIP: live server does not advertise FA infer support");
        server.shutdown().await;
        return;
    }

    let in_str = in_dir.to_str().unwrap().to_string();
    let out_str = out_dir.to_str().unwrap().to_string();
    let url = server.base_url().to_string();
    let cli_result = tokio::task::spawn_blocking(move || {
        let mut command = cmd();
        command
            .args([
                "align",
                &in_str,
                "-o",
                &out_str,
                "--server",
                &url,
                "--whisper-fa",
                "--no-utr",
            ])
            .timeout(std::time::Duration::from_secs(300))
            .output()
            .expect("spawn CLI")
    })
    .await
    .expect("blocking task");

    server.shutdown().await;

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "CLI should succeed. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("All done! 1 file(s) written"),
        "CLI should report a completed write. stderr: {stderr}"
    );

    let output_chat =
        std::fs::read_to_string(out_dir.join("test.cha")).expect("read aligned output");
    assert!(
        output_chat.contains("%wor:"),
        "aligned output should contain a %wor tier. output:\n{output_chat}"
    );
}

#[test]
fn cli_transcribe_explicit_server_falls_back_to_local_daemon() {
    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };

    let harness = CliHarness::new();
    let in_dir = harness.home_dir().join("input");
    let out_dir = harness.home_dir().join("output");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    write_silent_wav(&in_dir.join("sample.wav"));

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    harness.write_server_config(&format!(
        "host: 127.0.0.1\nport: {port}\nauto_daemon: true\nwarmup_commands: []\n"
    ));

    let start_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "start", "--test-echo"])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("start CLI test server");
    let start_stderr = String::from_utf8_lossy(&start_result.stderr);
    assert!(
        start_result.status.success(),
        "serve start should succeed. stderr: {start_stderr}"
    );

    let cli_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args([
            "transcribe",
            in_dir.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "--lang",
            "eng",
            "--server",
            "http://127.0.0.1:59999",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("spawn CLI");

    let _ = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "stop"])
        .timeout(std::time::Duration::from_secs(10))
        .output();

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "CLI should succeed. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains(
            "warning: transcribe uses local audio — ignoring --server and using local daemon."
        ),
        "CLI should explain the fallback. stderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Submitting to local daemon at http://127.0.0.1:{port}"
        )),
        "CLI should reuse the local daemon. stderr: {stderr}"
    );
    assert!(
        stderr.contains("All done! 1 file(s) written"),
        "CLI should report a completed write. stderr: {stderr}"
    );
    assert!(
        out_dir.join("sample.cha").is_file(),
        "fallback run should write a CHAT transcript"
    );
}

#[test]
fn cli_align_explicit_server_uses_remote_content_mode() {
    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };

    let harness = CliHarness::new();
    let media_dir = harness.home_dir().join("media");
    let in_dir = harness.home_dir().join("input");
    let out_dir = harness.home_dir().join("output");
    std::fs::create_dir_all(&media_dir).expect("mkdir media");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    write_silent_wav(&media_dir.join("sample.wav"));
    std::fs::write(
        in_dir.join("sample.cha"),
        "@UTF8\n@Begin\n@Languages:\teng\n@Participants:\tPAR Participant\n@ID:\teng|test|PAR|||||Participant|||\n@Media:\tsample, audio\n*PAR:\thello world .\n@End\n",
    )
    .expect("write input");

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    harness.write_server_config(&format!(
        "host: 127.0.0.1\nport: {port}\nauto_daemon: false\nwarmup_commands: []\nmedia_roots:\n  - {}\n",
        media_dir.display()
    ));

    let start_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "start", "--test-echo"])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("start CLI test server");
    let start_stderr = String::from_utf8_lossy(&start_result.stderr);
    assert!(
        start_result.status.success(),
        "serve start should succeed. stderr: {start_stderr}"
    );

    let cli_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args([
            "align",
            in_dir.to_str().unwrap(),
            "-o",
            out_dir.to_str().unwrap(),
            "--server",
            &format!("http://127.0.0.1:{port}"),
        ])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("spawn CLI");

    let _ = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "stop"])
        .timeout(std::time::Duration::from_secs(10))
        .output();

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "align should succeed against the explicit content-mode test server. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("Submitting to http://127.0.0.1:{port}")),
        "align should use the explicit remote server path. stderr: {stderr}"
    );
    assert!(
        !stderr.contains("ignoring --server and using local daemon"),
        "align should stay on the explicit remote content path. stderr: {stderr}"
    );
    assert!(
        out_dir.join("sample.cha").is_file(),
        "align should write the echoed output file via the explicit server path"
    );
    let output_chat =
        std::fs::read_to_string(out_dir.join("sample.cha")).expect("read aligned output");
    assert!(
        output_chat.contains("*PAR:\thello world ."),
        "align content-mode test output should preserve the uploaded CHAT content. output:\n{output_chat}"
    );
}

#[test]
fn cli_transcribe_in_place_mp4_succeeds_via_local_daemon() {
    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not installed");
        return;
    }

    let harness = CliHarness::new();
    let media_file = harness.home_dir().join("input").join("clip.mp4");
    std::fs::create_dir_all(media_file.parent().expect("media parent")).expect("mkdir input");
    write_silent_mp4(&media_file);

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    harness.write_server_config(&format!(
        "host: 127.0.0.1\nport: {port}\nauto_daemon: true\nwarmup_commands: []\n"
    ));

    let start_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "start", "--test-echo"])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("start CLI test server");
    let start_stderr = String::from_utf8_lossy(&start_result.stderr);
    assert!(
        start_result.status.success(),
        "serve start should succeed. stderr: {start_stderr}"
    );

    let cli_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args([
            "transcribe",
            "--in-place",
            media_file.to_str().unwrap(),
            "--lang",
            "eng",
        ])
        .timeout(std::time::Duration::from_secs(60))
        .output()
        .expect("spawn CLI");

    let _ = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "stop"])
        .timeout(std::time::Duration::from_secs(10))
        .output();

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "CLI should succeed. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("All done! 1 file(s) written"),
        "CLI should report a completed write. stderr: {stderr}"
    );
    assert!(
        media_file.with_extension("cha").is_file(),
        "in-place mp4 transcription should write a sibling CHAT file"
    );
}

#[test]
fn cli_transcribe_in_place_mp4_populates_injected_media_cache_live() {
    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3.12 with batchalign not available");
        return;
    };
    if !ffmpeg_available() {
        eprintln!("SKIP: ffmpeg not installed");
        return;
    }

    let repo = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("crate dir parent")
        .parent()
        .expect("repo root");
    let source_audio = repo.join("python/batchalign/tests/support/test.mp3");
    if !source_audio.is_file() {
        eprintln!("SKIP: committed speech fixture is unavailable");
        return;
    }

    let harness = CliHarness::new();
    let media_file = harness.home_dir().join("input").join("clip.mp4");
    let cache_dir = harness.home_dir().join("media-cache");
    std::fs::create_dir_all(media_file.parent().expect("media parent")).expect("mkdir input");
    std::fs::create_dir_all(&cache_dir).expect("mkdir cache");
    transcode_audio_to_mp4(&source_audio, &media_file);

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    harness.write_server_config(&format!(
        "host: 127.0.0.1\nport: {port}\nauto_daemon: true\nwarmup_commands: []\n"
    ));

    let cli_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .env("BATCHALIGN_MEDIA_CACHE_DIR", &cache_dir)
        .args([
            "transcribe",
            "--in-place",
            media_file.to_str().unwrap(),
            "--lang",
            "eng",
            "--whisper",
        ])
        .timeout(std::time::Duration::from_secs(300))
        .output()
        .expect("spawn CLI");

    let _ = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "stop"])
        .timeout(std::time::Duration::from_secs(10))
        .output();

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "CLI should succeed. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!(
            "Submitting to local daemon at http://127.0.0.1:{port}"
        )),
        "CLI should use the isolated local daemon. stderr: {stderr}"
    );
    assert!(
        stderr.contains("All done! 1 file(s) written"),
        "CLI should report a completed write. stderr: {stderr}"
    );

    let output_chat = std::fs::read_to_string(media_file.with_extension("cha"))
        .expect("read in-place transcribe output");
    let cached_wavs = std::fs::read_dir(&cache_dir)
        .expect("read cache dir")
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|ext| ext == "wav"))
        .collect::<Vec<_>>();
    assert!(
        !cached_wavs.is_empty(),
        "real mp4 transcribe should populate the injected media cache. stderr: {stderr}"
    );
    assert!(
        output_chat.contains("@Media:\tclip, audio"),
        "output should preserve the original media basename even when a cached WAV is used internally. output:\n{output_chat}"
    );
}

/// Explicit `--server` compare jobs should stay on the remote content-mode path.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn cli_compare_explicit_server_uses_remote_content_mode() {
    let Some(_python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3 with batchalign not available");
        return;
    };

    let Some(session) = acquire_test_server_session().await else {
        return;
    };
    let server_url = session.base_url().to_owned();
    let harness = CliHarness::new();
    let in_dir = harness.home_dir().join("input");
    let out_dir = harness.home_dir().join("output");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    std::fs::write(in_dir.join("test.cha"), MINIMAL_CHAT).expect("write input");

    let home = harness.home_dir().to_path_buf();
    let in_str = in_dir.to_str().unwrap().to_string();
    let out_str = out_dir.to_str().unwrap().to_string();
    let url = server_url.clone();
    let cli_result = tokio::task::spawn_blocking(move || {
        let mut command = cmd();
        command.env("HOME", &home);
        command
            .args(["compare", &in_str, &out_str, "--server", &url])
            .output()
            .expect("spawn CLI")
    })
    .await
    .expect("blocking task");

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert!(
        cli_result.status.success(),
        "compare should succeed against the explicit content-mode test server. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains(&format!("Submitting to {server_url}")),
        "compare should use the explicit remote server path. stderr: {stderr}"
    );
    assert!(
        !stderr.contains("Submitting to local daemon"),
        "compare explicit server path should not route through the local daemon. stderr: {stderr}"
    );
    assert!(
        out_dir.join("test.cha").is_file(),
        "compare should write the echoed output file via the explicit server path"
    );
}

#[test]
fn cli_compare_failed_auto_daemon_job_returns_server_exit_code() {
    let Some(python_path) = resolve_python() else {
        eprintln!("SKIP: Python 3 with batchalign not available");
        return;
    };

    let harness = CliHarness::new();
    let in_dir = harness.home_dir().join("input");
    let out_dir = harness.home_dir().join("output");
    std::fs::create_dir_all(&in_dir).expect("mkdir input");
    std::fs::create_dir_all(&out_dir).expect("mkdir output");
    std::fs::write(in_dir.join("test.cha"), MINIMAL_CHAT).expect("write input");

    let port = std::net::TcpListener::bind("127.0.0.1:0")
        .expect("bind ephemeral port")
        .local_addr()
        .expect("local addr")
        .port();
    harness.write_server_config(&format!(
        "host: 127.0.0.1\nport: {port}\nauto_daemon: true\nwarmup_commands: []\n"
    ));

    let cli_result = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args([
            "compare",
            in_dir.to_str().unwrap(),
            out_dir.to_str().unwrap(),
        ])
        .timeout(std::time::Duration::from_secs(120))
        .output()
        .expect("spawn CLI");

    let _ = harness
        .cmd()
        .env("BATCHALIGN_PYTHON", &python_path)
        .args(["serve", "stop"])
        .timeout(std::time::Duration::from_secs(10))
        .output();

    let stderr = String::from_utf8_lossy(&cli_result.stderr);
    let stdout = String::from_utf8_lossy(&cli_result.stdout);
    eprintln!("CLI stdout: {stdout}");
    eprintln!("CLI stderr: {stderr}");
    assert_eq!(
        cli_result.status.code(),
        Some(5),
        "failed auto-daemon job should map to exit code 5. stdout: {stdout}\nstderr: {stderr}"
    );
    assert!(
        stderr.contains("status failed") || stderr.contains("gold") || stderr.contains("error:"),
        "CLI stderr should describe the failed job. stderr: {stderr}"
    );
}
