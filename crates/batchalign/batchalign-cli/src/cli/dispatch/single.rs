//! Single-server dispatch — submit files to one server, poll, write results.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use crate::ReleasedCommand;
use crate::api::JobSubmission;
use crate::options::CommandOptions;
use crate::released_command_supports_paths_mode;

use crate::cli::client::{BatchalignClient, server_label};
use crate::cli::discover::{build_server_names, copy_nonmatching, infer_base_dir};
use crate::cli::error::CliError;
use crate::cli::progress::BatchProgress;
use crate::cli::tui::TuiProgress;

/// Check if a server URL points to the local machine.
///
/// Returns `true` for localhost and 127.0.0.1 (the auto-daemon addresses).
/// Used to decide between paths mode (shared filesystem, for local daemons)
/// and content mode (HTTP upload, for explicit remote `--server`).
/// Map a `TuiCancelSignal` from the rendering thread into a wire-format
/// `CancellationRequest`. Captures source=Tui, the host machine name and
/// caller PID, plus the in-flight filename the TUI snapshot recorded at
/// the moment the user confirmed cancel ('y' after 'c').
///
/// Hostname falls back to `"unknown"` when the OS does not report one
/// (rare; tests, containers without `/etc/hostname`).
fn build_tui_cancel_provenance(
    signal: crate::cli::tui::TuiCancelSignal,
) -> crate::api::CancellationRequest {
    use crate::api::{
        CallerHost, CallerPid, CancelReason, CancelSource, CancellationRequest, DisplayPath,
    };

    let host = sysinfo::System::host_name().unwrap_or_else(|| "unknown".to_string());

    CancellationRequest {
        source: Some(CancelSource::Tui),
        host: Some(CallerHost::from(host)),
        pid: Some(CallerPid(std::process::id())),
        reason: Some(CancelReason::from("user-pressed-cancel")),
        correlation_id: None,
        in_flight_filename: signal.in_flight_filename.map(DisplayPath::from),
    }
}

fn is_local_server(url: &str) -> bool {
    let after_scheme = url
        .trim_start_matches("http://")
        .trim_start_matches("https://");

    // Handle IPv6 bracket notation: [::1]:8001
    let host = if after_scheme.starts_with('[') {
        after_scheme
            .find(']')
            .map(|i| &after_scheme[..=i])
            .unwrap_or(after_scheme)
    } else {
        after_scheme.split(':').next().unwrap_or("")
    };

    matches!(host, "localhost" | "127.0.0.1" | "::1" | "[::1]")
}

use super::helpers::{
    classify_files, filter_files_for_command, inject_lexicon, maybe_open_dashboard,
    poll_and_write_incrementally,
};
use super::paths::prepare_paths_submission;
use super::{server_supports_command, warn_stale_server};

/// Submit files to a single server, poll for completion, write results.
#[allow(clippy::too_many_arguments)]
pub(super) async fn dispatch_single_server(
    client: &BatchalignClient,
    server_url: &str,
    allow_paths_mode: bool,
    command: ReleasedCommand,
    lang: &str,
    num_speakers: u32,
    extensions: &[&str],
    inputs: &[std::path::PathBuf],
    out_dir: Option<&std::path::Path>,
    options: Option<&CommandOptions>,
    lexicon: Option<&str>,
    before: Option<&std::path::Path>,
    use_tui: bool,
    open_dashboard: bool,
) -> Result<(), CliError> {
    // Health check
    let health = match client.health_check(server_url).await {
        Ok(h) => h,
        Err(e) => {
            return Err(e);
        }
    };
    warn_stale_server(server_url, &health);

    // Check capabilities
    if !server_supports_command(&health.capabilities, command) {
        return Err(CliError::UnsupportedCommand {
            server: server_label(server_url).to_string(),
            command,
        });
    }

    // Paths mode is only valid when client and server share a filesystem and
    // the caller explicitly opted into local-daemon/shared-filesystem routing.
    // Explicit `--server` stays on content mode even if it points at localhost.
    let server_is_local = is_local_server(server_url);
    let use_paths_mode =
        allow_paths_mode && released_command_supports_paths_mode(command) && server_is_local;

    let (submission, effective_out, result_map, paths_mode) = if use_paths_mode {
        let Some(prepared) = prepare_paths_submission(
            command,
            lang,
            num_speakers,
            extensions,
            inputs,
            out_dir,
            options,
            lexicon,
            before,
            &health.media_mapping_keys,
        )?
        else {
            eprintln!("warning: no files found with extensions {extensions:?}");
            return Ok(());
        };

        eprintln!("Found {} file(s) to submit.\n", prepared.total_files);
        eprintln!("Submitting shared-filesystem job to {server_url}...");
        eprintln!(
            "note: the server must be able to read these input paths. Successful outputs will also be copied back to this machine.\n"
        );

        (
            prepared.submission,
            prepared.effective_out,
            HashMap::new(),
            true,
        )
    } else {
        let (files, outputs) =
            crate::cli::discover::discover_server_inputs(inputs, out_dir, extensions)?;
        let (files, outputs) = filter_files_for_command(command, files, outputs);

        if let Some(od) = out_dir {
            for inp in inputs {
                if Path::new(inp).is_dir() {
                    copy_nonmatching(Path::new(inp), Path::new(od), extensions, command)?;
                }
            }
        }

        let base_dir = infer_base_dir(inputs)?;
        let (server_names, result_map) = build_server_names(&files, &outputs, inputs)?;
        let (file_payloads, media_file_names) = classify_files(&files, &server_names)?;
        if file_payloads.is_empty() && media_file_names.is_empty() {
            eprintln!("warning: no files found with extensions {extensions:?}");
            return Ok(());
        }

        let total_count = file_payloads.len() + media_file_names.len();
        eprintln!("Found {total_count} file(s) to submit.\n");

        let mut opts = options.cloned().unwrap_or_else(|| {
            CommandOptions::Morphotag(crate::options::MorphotagOptions {
                common: Default::default(),

                ..Default::default()
            })
        });
        inject_lexicon(&mut opts, lexicon)?;
        let debug_traces = opts.common().debug_dir.is_some();

        let effective_out = out_dir
            .map(PathBuf::from)
            .unwrap_or_else(|| base_dir.clone());

        (
            JobSubmission {
                command,
                lang: crate::api::LanguageSpec::try_from(lang)
                    .map_err(|e| CliError::InvalidArgument(format!("invalid language: {e}")))?,
                num_speakers: num_speakers.into(),
                files: file_payloads,
                media_files: media_file_names,
                media_mapping: Default::default(),
                media_subdir: Default::default(),
                source_dir: base_dir.to_string_lossy().to_string().into(),
                options: opts,
                paths_mode: false,
                source_paths: vec![],
                output_paths: vec![],
                display_names: vec![],
                debug_traces,
                before_paths: vec![],
            },
            effective_out,
            result_map,
            false,
        )
    };

    if !paths_mode {
        eprintln!("Submitting to {server_url}...");
    }
    let info = client.submit_job(server_url, &submission).await?;
    let job_id = &info.job_id;
    let total_files = info.total_files;
    eprintln!("Job {job_id} submitted ({total_files} file(s))");

    let dashboard_url = format!("{server_url}/dashboard/jobs/{job_id}");
    eprintln!("Dashboard: {dashboard_url}\n");

    maybe_open_dashboard(&dashboard_url, open_dashboard);

    // Poll and write incrementally
    if !info.status.is_terminal() {
        if use_tui && std::io::IsTerminal::is_terminal(&std::io::stdout()) {
            let (tui_progress, tui_runtime) =
                TuiProgress::new(total_files as u64, command.as_wire_name());
            let (cancel_tx, cancel_rx) =
                tokio::sync::oneshot::channel::<crate::cli::tui::TuiCancelSignal>();

            // Cancel task — awaits signal from TUI, posts cancel with full
            // caller provenance (source=Tui + hostname + PID + the in-flight
            // filename the TUI captured at confirm time). Server persists
            // these to the `cancellations` audit table so we can attribute
            // every cancel to a specific user gesture.
            let cc = client.clone();
            let cu = server_url.to_string();
            let cj = job_id.clone();
            tokio::spawn(async move {
                if let Ok(signal) = cancel_rx.await {
                    let provenance = build_tui_cancel_provenance(signal);
                    let _ = cc.cancel_job(&cu, &cj, provenance).await;
                }
            });

            // TUI on blocking thread
            let mut tui_handle = tokio::task::spawn_blocking(move || {
                crate::cli::tui::run_tui_loop(tui_runtime, Some(cancel_tx))
            });

            // Poll on current task — pinned so it survives TUI exit
            let poll_fut = poll_and_write_incrementally(
                client,
                server_url,
                job_id,
                total_files as u64,
                &result_map,
                &effective_out,
                command.as_wire_name(),
                &tui_progress,
            );
            tokio::pin!(poll_fut);

            tokio::select! {
                result = &mut poll_fut => {
                    result?;
                    // Job finished — wait for TUI to exit
                    let _ = tui_handle.await;
                }
                _ = &mut tui_handle => {
                    // User closed TUI — continue writing results to disk
                    eprintln!("\nDashboard closed — still writing results...");
                    poll_fut.await?;
                }
            }
        } else {
            let progress = BatchProgress::new(total_files as u64, command.as_wire_name());
            poll_and_write_incrementally(
                client,
                server_url,
                job_id,
                total_files as u64,
                &result_map,
                &effective_out,
                command.as_wire_name(),
                &progress,
            )
            .await?;
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::is_local_server;
    use crate::ReleasedCommand;
    use crate::released_command_supports_paths_mode;

    /// Commands whose descriptors are expected to stay on content mode.
    /// Kept as a tiny allowlist so a new command that forgets to pick a
    /// paths-mode-capable I/O profile trips an explicit review.
    ///
    /// Currently empty: all released commands use `PathsModeAudio` or
    /// `PathsModeText` in `io_profile_for()`. `Opensmile` was previously
    /// listed here based on the stale `opensmile.rs` macro definition,
    /// but the authoritative `command_model/catalog.rs` correctly maps it
    /// to `PathsModeAudio` (it processes audio files).
    const CONTENT_ONLY_COMMANDS: &[ReleasedCommand] = &[];

    fn expected_supports_paths_mode(command: ReleasedCommand) -> bool {
        !CONTENT_ONLY_COMMANDS.contains(&command)
    }

    #[test]
    fn localhost_is_local() {
        assert!(is_local_server("http://localhost:8001"));
        assert!(is_local_server("http://127.0.0.1:8001"));
        assert!(is_local_server("http://[::1]:8001"));
    }

    #[test]
    fn remote_hosts_are_not_local() {
        assert!(!is_local_server("http://server-01:8001"));
        assert!(!is_local_server("http://worker-machine:8001"));
        assert!(!is_local_server("http://192.168.1.100:8001"));
        assert!(!is_local_server("http://talkbank.org:8001"));
    }

    /// Every released command must advertise the expected paths-mode
    /// eligibility. Driven off `ReleasedCommand::ALL` so a new variant
    /// forces an explicit decision via `CONTENT_ONLY_COMMANDS` rather
    /// than silently inheriting the default.
    #[test]
    fn every_released_command_advertises_expected_paths_mode_eligibility() {
        for command in ReleasedCommand::ALL {
            assert_eq!(
                released_command_supports_paths_mode(command),
                expected_supports_paths_mode(command),
                "{command:?} paths-mode eligibility diverged from the allowlist"
            );
        }
    }
}
