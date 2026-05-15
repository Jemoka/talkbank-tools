//! Worker process spawning — command construction and TCP daemon launch.
//!
//! Contains the free functions for building the Python worker command line
//! and spawning a detached TCP worker daemon. The stdio-based spawn is in
//! [`WorkerHandle::spawn`](super::WorkerHandle::spawn) since it constructs
//! the handle directly.

use std::process::{Command as StdCommand, Stdio};

use serde_json::Value;
use tokio::process::Command;
use tracing::info;

use super::config::WorkerConfig;
use super::protocol::TcpReadySignal;
use crate::worker::error::WorkerError;
use crate::worker::provider_credentials::HkAsrCredentialSources;
use crate::worker::target::task_name;
use crate::worker::{WorkerBootstrapMode, WorkerProfile, WorkerTarget};

/// Build the `std::process::Command` for a stdio-transport Python worker.
///
/// The caller converts this to a `tokio::process::Command` and spawns it.
/// Sets up stdin/stdout/stderr pipes, language, profile/task flags, engine
/// overrides, and process-group isolation (`setpgid` on Unix,
/// `CREATE_NEW_PROCESS_GROUP` on Windows).
///
/// When you add a new flag or env var here that the Python child reads at
/// startup, mirror the underlying `WorkerConfig` field in
/// `tests/common/test_worker_pool.rs::ConfigKey`. The shared test fixture
/// keys workers by that struct; missing a field there would silently let
/// it share workers across configs that should be distinct.
pub(super) fn build_worker_command(config: &WorkerConfig) -> StdCommand {
    let mut cmd = StdCommand::new(&config.python_path);
    cmd.arg("-c")
        .arg("import sys; sys.argv = ['batchalign-worker'] + sys.argv[1:]; from batchalign.worker import main; main()")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());

    if config.test_echo {
        cmd.arg("--test-echo");
    }
    match config.bootstrap_target() {
        WorkerTarget::Profile(profile) => {
            cmd.arg("--profile").arg(profile.name());
            // LazyProfile: start with no models, load on demand via ensure_task.
            if config.runtime.bootstrap_mode == WorkerBootstrapMode::LazyProfile {
                cmd.arg("--lazy");
            }
        }
        WorkerTarget::InferTask(task) => {
            cmd.arg("--task").arg(task_name(task));
        }
    }

    cmd.arg("--lang").arg(config.lang.as_worker_arg());
    cmd.arg("--num-speakers")
        .arg(config.num_speakers.0.to_string());

    if !config.engine_overrides.is_empty() {
        cmd.arg("--engine-overrides").arg(&config.engine_overrides);
    }

    if config.runtime.force_cpu {
        cmd.arg("--force-cpu");
    }

    if config.verbose > 0 {
        cmd.arg("--verbose").arg(config.verbose.to_string());
    }

    if config.profile == WorkerProfile::Gpu {
        cmd.arg("--gpu-thread-pool-size")
            .arg(config.runtime.gpu_thread_pool_size.to_string());
    }

    if config.test_delay_ms > 0 {
        cmd.arg("--test-delay-ms")
            .arg(config.test_delay_ms.to_string());
    }

    if let Some(api_key) = config.runtime.revai_api_key.as_deref() {
        cmd.env("BATCHALIGN_REV_API_KEY", api_key);
    }
    for (key, value) in worker_provider_envs(config, &HkAsrCredentialSources::from_env()) {
        cmd.env(key, value);
    }

    // Each worker becomes its own process group leader so that
    // killpg() in shutdown/Drop kills the worker AND all its children
    // (e.g. Stanza subprocesses).
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;

        unsafe {
            cmd.pre_exec(|| {
                libc::setpgid(0, 0);
                Ok(())
            });
        }
    }

    // On Windows, CREATE_NEW_PROCESS_GROUP places the worker in its own
    // console process group. This is the closest equivalent to Unix
    // setpgid(0,0) and allows targeted shutdown via GenerateConsoleCtrlEvent
    // or TerminateProcess.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // CREATE_NEW_PROCESS_GROUP = 0x00000200
        cmd.creation_flags(0x00000200);
    }

    cmd
}

/// Spawn a **detached** TCP worker daemon.
///
/// Unlike [`WorkerHandle::spawn`](super::WorkerHandle::spawn) which creates a
/// child process tied directly to the current Rust process, this launches a
/// standalone Python process with `--transport tcp` that:
/// 1. Loads models, binds a TCP port
/// 2. Registers itself in `workers.json`
/// 3. Prints a ready signal to stderr
/// 4. Continues running until explicitly shut down
///
/// If [`WorkerRuntimeConfig::server_instance_id`](super::config::WorkerRuntimeConfig::server_instance_id)
/// is present, the daemon registers itself as **server-owned** and should be
/// retired by that same server instance on shutdown. Otherwise it registers as
/// **external** and may be reused across server restarts.
///
/// Returns `(pid, port)` on success after waiting for the ready signal.
pub async fn spawn_tcp_daemon(config: &WorkerConfig, port: u16) -> Result<(u32, u16), WorkerError> {
    if config.task.is_some() {
        return Err(WorkerError::SpawnFailed(
            "persistent TCP workers only support profile bootstrap targets".into(),
        ));
    }
    // Memory guard — same as WorkerHandle::spawn().
    let spawn_permit = crate::worker::memory_guard::acquire_spawn_permit(config).await?;

    let mut cmd = StdCommand::new(&config.python_path);
    cmd.arg("-c")
        .arg("import sys; sys.argv = ['batchalign-worker'] + sys.argv[1:]; from batchalign.worker import main; main()")
        .arg("--transport")
        .arg("tcp")
        .arg("--profile")
        .arg(config.profile.name())
        .arg("--lang")
        .arg(config.lang.as_worker_arg())
        .arg("--num-speakers")
        .arg(config.num_speakers.0.to_string())
        .arg("--host")
        .arg("127.0.0.1");

    if port > 0 {
        cmd.arg("--port").arg(port.to_string());
    }

    if config.test_echo {
        cmd.arg("--test-echo");
    }

    if !config.engine_overrides.is_empty() {
        cmd.arg("--engine-overrides").arg(&config.engine_overrides);
    }

    if config.runtime.force_cpu {
        cmd.arg("--force-cpu");
    }

    if config.verbose > 0 {
        cmd.arg("--verbose").arg(config.verbose.to_string());
    }

    if config.profile == WorkerProfile::Gpu {
        cmd.arg("--gpu-thread-pool-size")
            .arg(config.runtime.gpu_thread_pool_size.to_string());
    }

    if config.test_delay_ms > 0 {
        cmd.arg("--test-delay-ms")
            .arg(config.test_delay_ms.to_string());
    }

    if let Some(api_key) = config.runtime.revai_api_key.as_deref() {
        cmd.env("BATCHALIGN_REV_API_KEY", api_key);
    }
    if let Some(server_instance_id) = config.runtime.server_instance_id.as_deref() {
        cmd.env("BATCHALIGN_SERVER_INSTANCE_ID", server_instance_id);
    }
    if let Some(server_process_id) = config.runtime.server_process_id {
        cmd.env("BATCHALIGN_SERVER_PID", server_process_id.to_string());
    }
    for (key, value) in worker_provider_envs(config, &HkAsrCredentialSources::from_env()) {
        cmd.env(key, value);
    }

    // Detach: stdin from /dev/null, stdout to /dev/null, stderr piped (for ready signal).
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());

    // On Unix, create a new session (setsid) so the worker is fully detached
    // from the server's process group and terminal.
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            cmd.pre_exec(|| {
                libc::setsid();
                Ok(())
            });
        }
    }

    info!(
        profile = %config.bootstrap_label(),
        lang = %config.lang,
        port = port,
        "Spawning TCP worker daemon"
    );

    let mut child: Command = cmd.into();
    let mut child = child
        .spawn()
        .map_err(|e| WorkerError::SpawnFailed(format!("failed to spawn TCP worker daemon: {e}")))?;

    // Tag the host-memory lease with the TCP-worker's PID — Bug 2 fix
    // (see WorkerHandle::spawn for rationale). Without this, the
    // lease's owner_pid is the daemon and ghost slots accumulate when
    // the worker dies. `child.id()` is `None` only after `wait`/`kill`,
    // which haven't run yet.
    if let Some(child_pid) = child.id() {
        spawn_permit.set_worker_pid(child_pid);
    }
    let _spawn_permit = spawn_permit;

    // Read stderr for the ready signal: {"ready": true, "pid": N, "transport": "tcp", "port": P}
    let stderr = child
        .stderr
        .take()
        .ok_or_else(|| WorkerError::SpawnFailed("TCP daemon stderr not captured".into()))?;
    let mut stderr_reader = tokio::io::BufReader::new(stderr);

    let ready = tokio::time::timeout(
        std::time::Duration::from_secs(config.ready_timeout_s),
        read_tcp_ready_signal(&mut stderr_reader),
    )
    .await
    .map_err(|_| WorkerError::ReadyTimeout {
        timeout_s: config.ready_timeout_s,
    })?
    .map_err(|e| WorkerError::ReadyParseFailed(format!("TCP daemon ready failed: {e}")))?;

    // Detach stderr reader — the daemon continues on its own.
    // We intentionally do NOT wait on the child or hold its handle.
    // The process is now a standalone daemon managed by the OS.
    drop(stderr_reader);

    info!(
        profile = %config.bootstrap_label(),
        lang = %config.lang,
        pid = ready.0,
        port = ready.1,
        "TCP worker daemon ready"
    );

    Ok(ready)
}

/// Read the TCP ready signal from a daemon's stderr.
async fn read_tcp_ready_signal<R: tokio::io::AsyncBufRead + Unpin>(
    reader: &mut R,
) -> Result<(u32, u16), String> {
    use tokio::io::AsyncBufReadExt;

    let mut line = String::new();
    let mut attempts = 0;
    loop {
        line.clear();
        match reader.read_line(&mut line).await {
            Ok(0) => return Err("TCP daemon closed stderr without ready signal".into()),
            Ok(_) => {
                let trimmed = line.trim();
                if trimmed.is_empty() {
                    continue;
                }
                if let Ok(signal) = serde_json::from_str::<TcpReadySignal>(trimmed)
                    && signal.ready
                {
                    let port = signal.port.unwrap_or(0);
                    return Ok((signal.pid, port));
                }
                // Not the ready line — might be a log line, skip it.
                attempts += 1;
                if attempts > 100 {
                    return Err("Too many non-ready lines on stderr".into());
                }
            }
            Err(e) => return Err(format!("Failed to read TCP daemon stderr: {e}")),
        }
    }
}

/// Build environment variables for HK ASR provider credentials.
///
/// Only injects credentials for the GPU profile (which handles ASR requests)
/// and only for the selected ASR engine override (if any).
pub(super) fn worker_provider_envs(
    config: &WorkerConfig,
    sources: &HkAsrCredentialSources,
) -> Vec<(String, String)> {
    // GPU profile includes ASR — inject provider credentials when the profile
    // handles ASR requests or the engine overrides select an HK ASR backend.
    if config.profile != WorkerProfile::Gpu {
        return Vec::new();
    }
    sources
        .provider_envs_for_asr_override(selected_asr_override(&config.engine_overrides).as_deref())
        .into_iter()
        .collect()
}

/// Extract the `"asr"` key from an engine overrides JSON string.
pub(super) fn selected_asr_override(engine_overrides: &str) -> Option<String> {
    if engine_overrides.trim().is_empty() {
        return None;
    }
    let parsed = serde_json::from_str::<Value>(engine_overrides)
        .map_err(|e| tracing::warn!(overrides = engine_overrides, error = %e, "malformed engine override JSON, ignoring"))
        .ok()?;
    parsed.get("asr")?.as_str().map(str::to_string)
}
