//! Shared test infrastructure for CLI integration tests.
//!
//! Reuses the pattern from `batchalign-server/tests/integration.rs`:
//! resolve Python, spin up a test server, poll for job completion.

#![allow(dead_code)]

use std::ffi::OsStr;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use assert_cmd::cargo::cargo_bin_cmd;
use batchalign::api::{JobInfo, JobStatus, MemoryMb, NumSpeakers};
use batchalign::config::ServerConfig;
use batchalign::host_facts::PerProfile;
use batchalign::options::{
    AlignOptions, AvqiOptions, BenchmarkOptions, CommandOptions, CommonOptions, CompareOptions,
    CorefOptions, MorphotagOptions, OpensmileOptions, TranscribeOptions, TranslateOptions,
    UtsegOptions,
};
use batchalign::worker::InferTask;
use batchalign::worker::pool::PoolConfig;
use batchalign::{AppState, create_app};

/// Create a subprocess command for the published `batchalign3` binary.
///
/// Tests use a shared baseline environment so they do not accidentally pick up
/// a developer's configured remote server or auto-open browser behavior.
pub fn cli_cmd() -> assert_cmd::Command {
    // Ensure the per-process ledger override is in place before any
    // CLI subprocess inherits the parent env.
    isolate_host_memory_ledger();
    let mut command = cargo_bin_cmd!("batchalign3");
    command.env_remove("BATCHALIGN_SERVER");
    command.env("BATCHALIGN_NO_BROWSER", "1");
    command
}

/// Shared isolated filesystem layout for CLI subprocess tests.
pub struct CliHarness {
    _scratch: tempfile::TempDir,
    home_dir: PathBuf,
    state_dir: PathBuf,
}

impl CliHarness {
    /// Create a harness with an isolated `HOME` and `.batchalign3` state dir.
    ///
    /// Seeds a minimal `~/.batchalign.ini` so the first-run setup gate
    /// (which matches batchalign2 behavior) does not block test commands.
    pub fn new() -> Self {
        // Isolate the host-memory ledger before any CLI subprocess
        // inherits the env. The subprocess uses the env var to pick a
        // per-test-process ledger path, so cross-binary races on the
        // default ledger don't surface as test flakes.
        isolate_host_memory_ledger();

        let scratch = tempfile::TempDir::new().expect("tempdir");
        let home_dir = scratch.path().join("home");
        let state_dir = home_dir.join(".batchalign3");
        std::fs::create_dir_all(&state_dir).expect("mkdir state dir");

        // Seed config so processing commands don't trigger interactive setup.
        let config_path = home_dir.join(".batchalign.ini");
        std::fs::write(&config_path, "[asr]\nengine = whisper\n")
            .expect("write test .batchalign.ini");

        Self {
            _scratch: scratch,
            home_dir,
            state_dir,
        }
    }

    /// Create a command bound to this harness's isolated home directory.
    pub fn cmd(&self) -> assert_cmd::Command {
        let mut command = cli_cmd();
        command.env("HOME", &self.home_dir);
        command
    }

    /// Path to the isolated `HOME` directory.
    pub fn home_dir(&self) -> &Path {
        &self.home_dir
    }

    /// Path to the isolated `.batchalign3` state directory.
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Path to the default daemon/server config under this harness.
    pub fn server_config_path(&self) -> PathBuf {
        self.state_dir.join("server.yaml")
    }

    /// Write a `server.yaml` file under this harness.
    pub fn write_server_config(&self, yaml: &str) {
        std::fs::write(self.server_config_path(), yaml).expect("write server config");
    }

    /// Disable auto-daemon startup for subprocess tests.
    pub fn disable_auto_daemon(&self) {
        self.write_server_config("auto_daemon: false\n");
    }
}

/// Resolve the Python path for tests. Prefers the project venv
/// (which has all dependencies) over a bare PATH lookup.
pub fn resolve_python_for_module(module: &str) -> Option<String> {
    if let Ok(d) = std::env::current_dir() {
        let mut dir = d;
        loop {
            for venv in preferred_venv_pythons(&dir) {
                if venv.exists() && python_imports_module(&venv, module) {
                    return Some(venv.to_string_lossy().to_string());
                }
            }
            if !dir.pop() {
                break;
            }
        }
    }

    for candidate in preferred_path_pythons() {
        if python_imports_module(candidate, module) {
            return Some((*candidate).to_string());
        }
    }

    None
}

pub fn resolve_python() -> Option<String> {
    resolve_python_for_module("batchalign.worker")
}

fn python_imports_module(command: impl AsRef<OsStr>, module: &str) -> bool {
    let snippet = format!("import {module}");
    std::process::Command::new(command)
        .args(["-c", &snippet])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn preferred_venv_pythons(root: &Path) -> Vec<PathBuf> {
    if cfg!(windows) {
        vec![root.join(".venv").join("Scripts").join("python.exe")]
    } else {
        let bin = root.join(".venv").join("bin");
        vec![
            bin.join("python3.12"),
            bin.join("python3"),
            bin.join("python"),
        ]
    }
}

fn preferred_path_pythons() -> &'static [&'static str] {
    if cfg!(windows) {
        &["python"]
    } else {
        &["python3.12", "python3"]
    }
}

pub fn write_silent_wav(path: &Path) {
    let sample_rate = 16_000u32;
    let channels = 1u16;
    let bits_per_sample = 16u16;
    let sample_count = sample_rate as usize / 10;
    let data_len = (sample_count * std::mem::size_of::<i16>()) as u32;
    let byte_rate = sample_rate * channels as u32 * bits_per_sample as u32 / 8;
    let block_align = channels * bits_per_sample / 8;
    let chunk_size = 36 + data_len;

    let mut bytes = Vec::with_capacity(44 + data_len as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&chunk_size.to_le_bytes());
    bytes.extend_from_slice(b"WAVE");
    bytes.extend_from_slice(b"fmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&channels.to_le_bytes());
    bytes.extend_from_slice(&sample_rate.to_le_bytes());
    bytes.extend_from_slice(&byte_rate.to_le_bytes());
    bytes.extend_from_slice(&block_align.to_le_bytes());
    bytes.extend_from_slice(&bits_per_sample.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_len.to_le_bytes());
    bytes.resize(44 + data_len as usize, 0);

    std::fs::write(path, bytes).expect("write wav");
}

pub fn ffmpeg_available() -> bool {
    std::process::Command::new("ffmpeg")
        .arg("-version")
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

pub fn write_silent_mp4(path: &Path) {
    let output = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-f",
            "lavfi",
            "-i",
            "anullsrc=r=16000:cl=mono",
            "-t",
            "0.10",
            "-c:a",
            "aac",
            path.to_str().expect("mp4 path should be valid UTF-8"),
        ])
        .output()
        .expect("run ffmpeg");
    assert!(
        output.status.success(),
        "ffmpeg should generate the mp4 fixture: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

pub fn transcode_audio_to_mp4(input: &Path, output: &Path) {
    let ffmpeg_output = std::process::Command::new("ffmpeg")
        .args([
            "-y",
            "-i",
            input.to_str().expect("input path should be valid UTF-8"),
            "-c:a",
            "aac",
            output.to_str().expect("output path should be valid UTF-8"),
        ])
        .output()
        .expect("run ffmpeg");
    assert!(
        ffmpeg_output.status.success(),
        "ffmpeg should convert the audio fixture to mp4: {}",
        String::from_utf8_lossy(&ffmpeg_output.stderr)
    );
}

#[allow(unused_macros)]
macro_rules! require_python {
    () => {
        match $crate::cli_common::resolve_python() {
            Some(path) => path,
            None => {
                eprintln!("SKIP: Python 3 with batchalign not available");
                return;
            }
        }
    };
}

#[allow(unused_imports)]
pub(crate) use require_python;

pub struct LiveTestServer {
    base_url: String,
    infer_tasks: Vec<InferTask>,
    state: Arc<AppState>,
    server_task: tokio::task::JoinHandle<()>,
    _scratch: tempfile::TempDir,
}

impl LiveTestServer {
    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    pub fn has_infer_task(&self, task: InferTask) -> bool {
        self.infer_tasks.contains(&task)
    }

    pub async fn shutdown(self) {
        self.server_task.abort();
        let _ = self.server_task.await;
        match self.state.shutdown_for_reuse(Duration::from_secs(5)).await {
            Ok(shutdown) if shutdown.timed_out || shutdown.remaining_jobs > 0 => {
                eprintln!(
                    "WARN: live CLI test server shutdown left {} tracked jobs (timed_out={})",
                    shutdown.remaining_jobs, shutdown.timed_out
                );
            }
            Ok(_) => {}
            Err(error) => {
                eprintln!(
                    "WARN: live CLI test server shutdown failed to report runtime status: {error}"
                );
            }
        }
    }
}

/// Re-export of the library's per-process ledger isolator. Both the
/// test-server fixture and this CLI-subprocess harness need to set
/// `BATCHALIGN_HOST_MEMORY_LEDGER` before any worker spawns; routing
/// both through one `Once` in the library guarantees a single env-var
/// write per process.
pub use batchalign::host_memory::isolate_host_memory_ledger_for_test as isolate_host_memory_ledger;

pub async fn start_live_server(
    python_path: &str,
    media_roots: Vec<batchalign_types::paths::ServerPath>,
) -> Result<LiveTestServer, String> {
    isolate_host_memory_ledger();
    let tmp = tempfile::TempDir::new().map_err(|error| format!("tempdir failed: {error}"))?;
    let jobs_dir = tmp.path().join("jobs");
    std::fs::create_dir_all(&jobs_dir).map_err(|error| format!("mkdir jobs failed: {error}"))?;
    let db_dir = tmp.path().join("db");
    std::fs::create_dir_all(&db_dir).map_err(|error| format!("mkdir db failed: {error}"))?;

    let config = ServerConfig {
        host: "127.0.0.1".into(),
        port: 0,
        warmup_commands: vec![],
        media_roots,
        memory_gate_mb: Some(MemoryMb(0)),
        ..Default::default()
    };
    let pool_config = PoolConfig {
        python_path: python_path.into(),
        test_echo: false,
        health_check_interval_s: 3_600,
        ready_timeout_s: 120,
        max_workers_per_key: PerProfile::uniform(2),
        verbose: 0,
        engine_overrides: String::new(),
        runtime: Default::default(),
        ..Default::default()
    };

    let (router, state) = create_app(
        config,
        pool_config,
        Some(jobs_dir.to_string_lossy().into()),
        Some(db_dir),
        Some(batchalign::cli::build_hash().into()),
    )
    .await
    .map_err(|error| format!("could not create live server app: {error}"))?;

    let infer_tasks = state.infer_tasks().to_vec();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .map_err(|error| format!("bind failed: {error}"))?;
    let port = listener
        .local_addr()
        .map_err(|error| format!("local_addr failed: {error}"))?
        .port();
    let base_url = format!("http://127.0.0.1:{port}");

    let server_task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .ok();
    });

    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    Ok(LiveTestServer {
        base_url,
        infer_tasks,
        state,
        server_task,
        _scratch: tmp,
    })
}

/// Minimal valid CHAT content for test-echo round-trips.
pub const MINIMAL_CHAT: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
*PAR:\thello world .
@End
";

/// CHAT content with @Options: dummy header.
pub const DUMMY_CHAT: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Options:\tdummy
*PAR:\thello .
@End
";

/// CHAT content with @Options: NoAlign header.
pub const NOALIGN_CHAT: &str = "\
@UTF8
@Begin
@Languages:\teng
@Participants:\tPAR Participant
@ID:\teng|test|PAR|||||Participant|||
@Options:\tNoAlign
*PAR:\thello .
@End
";

/// Build a default `CommandOptions` for the given command name.
pub fn default_options_for(command: &str) -> CommandOptions {
    match command {
        "align" => CommandOptions::Align(AlignOptions {
            common: CommonOptions::default(),
            fa_engine: batchalign::options::FaEngineName::Wave2Vec,
            utr_engine: None,
            utr_overlap_strategy: Default::default(),
            utr_two_pass: Default::default(),
            pauses: false,
            wor: true.into(),
            merge_abbrev: false.into(),
            media_dir: None,
            bullet_repair: false,
            review_level: Default::default(),
        }),
        "transcribe" => CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        "transcribe_s" => CommandOptions::TranscribeS(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: true,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
        "morphotag" => CommandOptions::Morphotag(MorphotagOptions {
            common: CommonOptions::default(),

            ..Default::default()
        }),
        "translate" => CommandOptions::Translate(TranslateOptions {
            common: CommonOptions::default(),
            merge_abbrev: false.into(),
        }),
        "coref" => CommandOptions::Coref(CorefOptions {
            common: CommonOptions::default(),
            merge_abbrev: false.into(),
        }),
        "utseg" => CommandOptions::Utseg(UtsegOptions {
            common: CommonOptions::default(),
            merge_abbrev: false.into(),
            utseg_fallback: false.into(),
        }),
        "benchmark" => CommandOptions::Benchmark(BenchmarkOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            wor: false.into(),
            merge_abbrev: false.into(),
        }),
        "opensmile" => CommandOptions::Opensmile(OpensmileOptions {
            common: CommonOptions::default(),
            feature_set: "eGeMAPSv02".into(),
        }),
        "compare" => CommandOptions::Compare(CompareOptions {
            common: CommonOptions::default(),
            merge_abbrev: false.into(),
        }),
        "avqi" => CommandOptions::Avqi(AvqiOptions {
            common: CommonOptions::default(),
        }),
        // Unknown commands: fall back to transcribe options so the struct
        // compiles; the server will reject the submission based on the
        // mismatched command string.
        _ => CommandOptions::Transcribe(TranscribeOptions {
            common: CommonOptions::default(),
            asr_engine: batchalign::options::AsrEngineName::RevAi,
            diarize: false,
            wor: false.into(),
            merge_abbrev: false.into(),
            batch_size: 8,
            utseg_fallback: false.into(),
        }),
    }
}

/// Submit a content-mode job, poll to completion, return all file results.
///
/// Uses the `transcribe` command (goes through `process` path, compatible
/// with test-echo workers that don't advertise infer_tasks).
pub async fn run_job_to_completion(
    client: &reqwest::Client,
    base_url: &str,
    command: batchalign::api::ReleasedCommand,
    lang: &str,
    files: Vec<batchalign::api::FilePayload>,
    options: CommandOptions,
) -> (JobInfo, Vec<batchalign::api::FileResult>) {
    let submission = batchalign::api::JobSubmission {
        command,
        lang: batchalign::api::LanguageSpec::try_from(lang).expect("test lang"),
        num_speakers: NumSpeakers(1),
        files,
        media_files: vec![],
        media_mapping: Default::default(),
        media_subdir: Default::default(),
        source_dir: Default::default(),
        options,
        paths_mode: false,
        source_paths: vec![],
        output_paths: vec![],
        display_names: vec![],
        debug_traces: false,
        before_paths: vec![],
    };

    let resp = client
        .post(format!("{base_url}/jobs"))
        .json(&submission)
        .send()
        .await
        .expect("POST /jobs");
    assert_eq!(resp.status(), 200, "Job submission should succeed");
    let info: JobInfo = resp.json().await.expect("parse initial JobInfo");

    let final_info = poll_job_done(client, base_url, &info.job_id).await;

    let results_resp: batchalign::api::JobResultResponse = client
        .get(format!("{base_url}/jobs/{}/results", info.job_id))
        .send()
        .await
        .expect("GET results")
        .json()
        .await
        .expect("parse results");

    (final_info, results_resp.files)
}

/// Poll until a job reaches a terminal state (60s timeout).
pub async fn poll_job_done(client: &reqwest::Client, base_url: &str, job_id: &str) -> JobInfo {
    let deadline = tokio::time::Instant::now() + tokio::time::Duration::from_secs(60);

    loop {
        let resp = client
            .get(format!("{base_url}/jobs/{job_id}"))
            .send()
            .await
            .expect("GET job");
        let info: JobInfo = resp.json().await.expect("parse job");

        if matches!(
            info.status,
            JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
        ) {
            return info;
        }

        assert!(
            tokio::time::Instant::now() < deadline,
            "Job {job_id} did not finish within 60s (status: {:?})",
            info.status
        );

        tokio::time::sleep(tokio::time::Duration::from_millis(200)).await;
    }
}
