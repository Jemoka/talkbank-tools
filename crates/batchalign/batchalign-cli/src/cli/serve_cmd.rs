//! `batchalign3 serve` -- manage the batchalign processing server.
//!
//! This module implements the three `serve` subcommands:
//!
//! - **`serve start`** -- Launch the HTTP server that accepts processing jobs.
//!   In foreground mode (`--foreground`) the server runs in the current process,
//!   blocking until shutdown. In background mode (the default) a detached child
//!   process is spawned in a new session (`setsid`) so it survives CLI exit, and
//!   a PID file is written for later cleanup. CLI flags (port, host, warmup,
//!   Python path, test-echo) override values from `server.yaml`.
//!
//! - **`serve stop`** -- Shut down any running server and local daemon. Reads the
//!   PID file, sends `SIGTERM` to the process group, and cleans up state files.
//!
//! - **`serve status`** -- Probe a running server's `/health` endpoint and print
//!   version, worker count, active jobs, and media root configuration. Discovers
//!   the server URL from `--server`, a local daemon info file, or falls back to
//!   the configured local server URL.

use crate::config::{self, RuntimeLayout, WARMUP_PRESET_FULL, WARMUP_PRESET_MINIMAL};
use crate::host_facts::EffectiveConfig;
use crate::host_memory::HostMemoryRuntimeConfig;
use crate::host_policy::HostExecutionPolicy;
use crate::worker::handle::WorkerRuntimeConfig;
use crate::worker::pool::PoolConfig;

use crate::cli::args::{ServeStartArgs, ServeStatusArgs};
use crate::cli::client::BatchalignClient;
use crate::cli::daemon;
use crate::cli::error::CliError;
use crate::cli::python::resolve_python_executable;
use crate::cli::self_exe::resolve_self_exe;

/// `serve start` — start the processing server.
pub async fn start(
    args: &ServeStartArgs,
    verbose: u8,
    force_cpu: bool,
    engine_overrides: Option<&str>,
) -> Result<(), CliError> {
    let layout = RuntimeLayout::from_env();
    let mut cfg =
        config::load_config_from_layout(&layout, args.config.as_deref().map(std::path::Path::new))?;
    let worker_python = args
        .python
        .clone()
        .unwrap_or_else(resolve_python_executable);

    // Override config values only when explicitly passed via CLI.
    if let Some(port) = args.port {
        cfg.port = port;
    }
    if let Some(ref host) = args.host {
        cfg.host = host.clone();
    }
    if let Some(workers) = args.workers {
        // `--workers N` is always an explicit override.
        cfg.max_workers_per_job = Some(workers as u32);
    }
    if let Some(timeout) = args.timeout {
        cfg.audio_task_timeout_s = timeout;
    }

    if let Some(ref warmup) = args.warmup {
        apply_warmup_flag(warmup, &mut cfg);
    }

    let warnings = cfg.validate();
    for w in &warnings {
        eprintln!("warning: {w}");
    }

    if cfg.media_roots.is_empty() && cfg.media_mappings.is_empty() {
        eprintln!(
            "warning: no media_roots or media_mappings configured. \
             Align/transcribe commands will fail unless CHAT files reference \
             accessible media paths."
        );
    }

    if args.foreground {
        let tier = cfg.resolved_memory_tier();
        let host_policy = HostExecutionPolicy::from_server_config(&cfg);
        eprintln!("\nStarting server on {}:{}...", cfg.host, cfg.port);
        eprintln!(
            "Backend: {}",
            if cfg.use_temporal() {
                "temporal"
            } else {
                "local"
            }
        );
        eprintln!(
            "Memory tier: {}{} (total: {} GB, headroom: {} GB, stanza: {} GB, gpu: {} GB, bootstrap: {:?})\n",
            tier.kind,
            if cfg.memory_tier.is_some() {
                " (override)"
            } else {
                ""
            },
            tier.total_mb / 1000,
            tier.headroom_mb.0 / 1000,
            tier.stanza_startup_mb.0 / 1000,
            tier.gpu_startup_mb.0 / 1000,
            host_policy.bootstrap_mode,
        );

        // CLI `--force-cpu` is a presence-only switch; convert to
        // `Some(true)` so the host-facts pipeline treats it as an
        // explicit override. Absent CLI flag leaves the
        // `cfg.force_cpu` field at whatever server.yaml provides
        // (default `None`, which falls through to the
        // recommendation).
        if force_cpu {
            cfg.force_cpu = Some(true);
        }
        // Resolve operator overrides against the live host-facts
        // snapshot. The runtime forms below remain concrete `u32` /
        // `usize` values because every downstream consumer (worker
        // spawn args, dispatch_semaphore permit counts,
        // TcpWorkerInfo) already expects a single value, not an
        // override+recommendation pair.
        let effective = EffectiveConfig::resolve_from_server_config(&cfg);
        let worker_runtime = WorkerRuntimeConfig {
            force_cpu: effective.force_cpu,
            gpu_thread_pool_size: effective.gpu_thread_pool_size,
            host_memory: HostMemoryRuntimeConfig::from_server_config(&cfg),
            memory_tier: tier,
            bootstrap_mode: host_policy.bootstrap_mode,
            ..WorkerRuntimeConfig::default()
        };
        let pool_config = PoolConfig {
            python_path: worker_python.clone(),
            test_echo: args.test_echo,
            health_check_interval_s: if cfg.worker_health_interval_s > 0 {
                cfg.worker_health_interval_s
            } else {
                PoolConfig::default().health_check_interval_s
            },
            verbose,
            engine_overrides: engine_overrides.unwrap_or("").to_string(),
            runtime: worker_runtime,
            // Per-profile cap. `Some(n)` from server.yaml is the
            // operator's uniform override applied to all three
            // profiles; otherwise we use the host-facts per-profile
            // recommendation already resolved into `EffectiveConfig`.
            max_workers_per_key: match cfg.max_workers_per_key {
                Some(n) => crate::host_facts::PerProfile::uniform(n as usize),
                None => effective.max_workers_per_key_by_profile.map(|n| n as usize),
            },
            ready_timeout_s: if cfg.worker_ready_timeout_s > 0 {
                cfg.worker_ready_timeout_s
            } else {
                PoolConfig::default().ready_timeout_s
            },
            // `recommend_max_total_workers` clamps to `[2, 32]` so the
            // `as usize` cast is always well-defined.
            max_total_workers: effective.max_total_workers as usize,
            checkout_wait_timeout_s: 0, // 0 = use built-in default (300s)
            audio_task_timeout_s: cfg.audio_task_timeout_s,
            analysis_task_timeout_s: cfg.analysis_task_timeout_s,
            ensure_task_timeout_s: cfg.ensure_task_timeout_s,
            worker_registry_path: cfg.worker_registry_path.clone(),
            test_delay_ms: 0,
            // Production: live host CPU loadavg gate, not a test override.
            cpu_gate_threshold_override: None,
        };
        crate::serve_with_runtime(
            cfg,
            pool_config,
            layout,
            Some(crate::cli::build_hash().to_string()),
        )
        .await?;
    } else {
        // Background mode: spawn self with --foreground
        let exe = resolve_self_exe();

        std::fs::create_dir_all(layout.state_dir())?;

        // Stop any existing server
        let _ = stop_server(&layout);

        let log_path = layout.server_log_path();
        // Append mode: preserve previous server logs across restarts.
        let log_file = std::fs::OpenOptions::new()
            .append(true)
            .create(true)
            .open(&log_path)?;

        let mut cmd = std::process::Command::new(&exe);
        cmd.args([
            "serve",
            "start",
            "--foreground",
            "--port",
            &cfg.port.to_string(),
            "--host",
            &cfg.host,
        ]);
        if let Some(ref config_path) = args.config {
            cmd.args(["--config", config_path]);
        }
        cmd.args(["--python", &worker_python]);
        // Forward warmup configuration to the background server process.
        if let Some(ref warmup) = args.warmup {
            cmd.args(["--warmup", warmup]);
        }
        if args.test_echo {
            cmd.arg("--test-echo");
        }
        if force_cpu {
            cmd.arg("--force-cpu");
        }
        // Forward verbosity to the background server process.
        for _ in 0..verbose {
            cmd.arg("-v");
        }
        // Forward engine overrides to the background server process.
        if let Some(overrides) = engine_overrides {
            cmd.args(["--engine-overrides", overrides]);
        }
        // Forward workers to the background server process.
        if let Some(workers) = args.workers {
            cmd.args(["--workers", &workers.to_string()]);
        }
        // Forward timeout to the background server process.
        if let Some(timeout) = args.timeout {
            cmd.args(["--timeout", &timeout.to_string()]);
        }

        cmd.stdout(std::process::Stdio::null());
        cmd.stderr(log_file);

        // Start new session so it survives CLI exit
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

        // On Windows, CREATE_NEW_PROCESS_GROUP + DETACHED_PROCESS ensures
        // the server survives after the spawning CLI exits, analogous to
        // Unix setsid(). The server gets its own console group and is not
        // attached to the parent's console.
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            // CREATE_NEW_PROCESS_GROUP (0x200) | DETACHED_PROCESS (0x08)
            cmd.creation_flags(0x00000200 | 0x00000008);
        }

        let proc = cmd.spawn()?;
        let pid = proc.id();

        // Write PID file atomically (via temp + rename).
        let pid_path = layout.server_pid_path();
        let pid_tmp = pid_path.with_extension("pid.tmp");
        std::fs::write(&pid_tmp, pid.to_string())?;
        std::fs::rename(&pid_tmp, &pid_path)?;

        // Brief wait, then verify the spawned process is still alive.
        // If it crashed immediately (e.g. port conflict, missing Python),
        // report that rather than claiming success.
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        if !is_process_alive(pid) {
            let _ = std::fs::remove_file(&pid_path);
            eprintln!(
                "\nerror: server process (PID {pid}) exited immediately after spawn.\n\
                 Check the log file: {}\n\
                 hint: run `batchalign3 serve start --foreground` to see startup errors.",
                log_path.display()
            );
            return Err(CliError::DaemonStartFailed);
        }

        eprintln!("\nServer started (PID {pid})");
        eprintln!("Listening on http://{}:{}", cfg.host, cfg.port);
        eprintln!("\nPID file: {}", pid_path.display());
        eprintln!("Log file: {}", log_path.display());
        eprintln!(
            "\nClients can now use: batchalign3 <command> ... --server http://<this-machine>:{}",
            cfg.port
        );
    }

    Ok(())
}

/// `serve stop` — stop the server and daemon.
pub async fn stop() -> Result<(), CliError> {
    let layout = RuntimeLayout::from_env();

    // Stop daemon first
    if daemon::stop_daemon().await? {
        eprintln!("Local daemon stopped.");
    }
    if daemon::stop_sidecar_daemon().await? {
        eprintln!("Sidecar daemon stopped.");
    }

    let stopped = stop_server(&layout);
    if stopped {
        eprintln!("Server stopped.");
    } else {
        eprintln!("No server process found.");
    }

    Ok(())
}

/// `serve status` — check server health.
pub async fn status(args: &ServeStatusArgs) -> Result<(), CliError> {
    let client = BatchalignClient::new()?;
    let layout = RuntimeLayout::from_env();
    let (cfg, warnings) = config::load_validated_config_from_layout(&layout, None)?;
    for warning in warnings {
        eprintln!("warning: {warning}");
    }
    let configured_port = cfg.port;

    let server = if let Some(ref s) = args.server {
        s.trim_end_matches('/').to_string()
    } else {
        // Try local daemon first
        if let Some(info) = daemon::read_daemon_info() {
            if client
                .health_check(&format!("http://127.0.0.1:{}", info.port))
                .await
                .is_ok()
            {
                eprintln!("Using local daemon (PID {})", info.pid);
                format!("http://127.0.0.1:{}", info.port)
            } else {
                format!("http://localhost:{configured_port}")
            }
        } else {
            format!("http://localhost:{configured_port}")
        }
    };

    match client.health_check(&server).await {
        Ok(health) => {
            eprintln!();
            eprintln!("Batchalign Server Status");
            eprintln!("{}", "-".repeat(40));
            eprintln!("URL:              {server}");
            eprintln!("Status:           {}", health.status);
            eprintln!("Version:          {}", health.version);
            if !health.build_hash.is_empty() {
                eprintln!("Build:            {}", health.build_hash);
            }
            eprintln!("Workers free:     {}", health.workers_available);
            eprintln!("Active jobs:      {}", health.active_jobs);
            if !health.media_roots.is_empty() {
                eprintln!("Media:            {}", health.media_roots.join(", "));
            }
            eprintln!();
        }
        Err(e) => {
            eprintln!("error: cannot reach server at {server}: {e}");
        }
    }

    Ok(())
}

/// Stop a server whose PID is recorded in the state directory.
///
/// Validates that the recorded PID actually belongs to a live process before
/// sending signals. Always cleans up the PID file, even if the process is
/// already dead (stale PID file from a previous crash).
fn stop_server(layout: &RuntimeLayout) -> bool {
    let pid_path = layout.server_pid_path();
    let pid_str = match std::fs::read_to_string(&pid_path) {
        Ok(s) => s,
        Err(_) => return false,
    };
    let pid: u32 = match pid_str.trim().parse() {
        Ok(p) => p,
        Err(_) => {
            // Corrupt PID file -- clean it up.
            let _ = std::fs::remove_file(&pid_path);
            return false;
        }
    };

    // Check if the process is actually alive before signalling.
    // Avoids sending signals to an unrelated process that reused the PID.
    if !is_process_alive(pid) {
        // Stale PID file -- clean it up.
        let _ = std::fs::remove_file(&pid_path);
        return false;
    }

    let killed = kill_pid(pid);
    let _ = std::fs::remove_file(&pid_path);
    killed
}

/// Check if a process is alive via `kill(pid, 0)`.
#[cfg(unix)]
fn is_process_alive(pid: u32) -> bool {
    unsafe { libc::kill(pid as libc::pid_t, 0) == 0 }
}

#[cfg(not(unix))]
fn is_process_alive(_pid: u32) -> bool {
    false
}

/// Parse the `--warmup` CLI flag value and apply it to the server config.
///
/// Accepts preset names (`off`, `minimal`, `full`) or a comma-separated list
/// of command names (e.g. `align,morphotag`).  The resolved list is written
/// to `warmup_commands`.
fn apply_warmup_flag(value: &str, cfg: &mut crate::config::ServerConfig) {
    cfg.warmup_commands = match value.to_ascii_lowercase().as_str() {
        "off" => Vec::new(),
        "minimal" => WARMUP_PRESET_MINIMAL
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        "full" => WARMUP_PRESET_FULL
            .iter()
            .map(|s| (*s).to_string())
            .collect(),
        _ => value
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect(),
    };
}

/// Kill a server process: SIGTERM the process group, wait up to 3 seconds,
/// then escalate to SIGKILL if still alive.
fn kill_pid(pid: u32) -> bool {
    #[cfg(unix)]
    {
        let pgid_ok = unsafe { libc::killpg(pid as libc::pid_t, libc::SIGTERM) == 0 };
        let pid_ok = unsafe { libc::kill(pid as libc::pid_t, libc::SIGTERM) == 0 };

        if !pgid_ok && !pid_ok {
            return false;
        }

        // Wait for the process to exit so the port is released.
        for _ in 0..6 {
            std::thread::sleep(std::time::Duration::from_millis(500));
            if !is_process_alive(pid) {
                return true;
            }
        }

        // Still alive after 3 seconds -- escalate to SIGKILL.
        unsafe {
            libc::killpg(pid as libc::pid_t, libc::SIGKILL);
            libc::kill(pid as libc::pid_t, libc::SIGKILL);
        }
        std::thread::sleep(std::time::Duration::from_millis(200));
        true
    }
    #[cfg(not(unix))]
    {
        let _ = pid;
        false
    }
}
