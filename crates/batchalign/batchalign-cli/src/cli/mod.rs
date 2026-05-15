//! CLI for batchalign3 direct and server-backed execution.
//!
//! This crate implements the `batchalign3` command-line interface. The CLI
//! does not own the command engine itself, but it can now route work through
//! either direct local execution or an explicit batchalign server
//! (see [`batchalign_server`]).
//!
//! # Dispatch flow
//!
//! When the user runs a processing command (e.g. `batchalign3 morphotag`),
//! the dispatch router chooses a host:
//!
//! ```text
//!   --server URL given?
//!        |
//!   yes  |   no
//!   |    |    |
//!   v    |    v
//! command | direct
//! needs   | local
//! local   |
//! media?  |
//!   |     |
//! yes|no  |
//!   |  |  |
//!   v  v  v
//! direct remote direct
//! local  server local
//! ```
//!
//! # Dispatch modes
//!
//! 1. **Single-server** (`--server URL`): The CLI reads local `.cha` files,
//!    POSTs their content (~2 KB each) to the server, polls for completion,
//!    and writes results to the output directory. Audio-dependent commands in
//!    this mode must use filesystem paths that are visible on the execution
//!    host rather than relying on client-local media fallback.
//!
//! 2. **Direct local execution**: When no explicit server is specified, the
//!    CLI prepares a local submission and runs it inline through the shared
//!    execution engine. No HTTP server, queue, or registry discovery is
//!    required for this path.
//!
//! # Usage
//!
//! ```text
//! # Process files with morphosyntactic tagging
//! batchalign3 morphotag corpus/ output/
//!
//! # Force-align transcripts against audio
//! batchalign3 align --lang eng corpus/ output/
//!
//! # Submit to a specific remote server
//! batchalign3 morphotag corpus/ output/ --server http://myserver:8000
//!
//! # Manage the server
//! batchalign3 serve start --foreground
//! batchalign3 serve status
//! batchalign3 serve stop
//!
//! # Inspect jobs
//! batchalign3 jobs --server http://myserver:8000
//! batchalign3 jobs <JOB_ID>
//! ```
//!
//! With `--server`, `jobs` queries the remote server. Without `--server`, a
//! positional `JOB_ID` inspects local debug artifacts under the runtime-owned
//! jobs directory.
//!
//! # Examples
//!
//! ## Health-check a running server
//!
//! ```rust,no_run
//! use crate::cli::client::BatchalignClient;
//!
//! # async fn example() -> Result<(), crate::cli::error::CliError> {
//! let client = BatchalignClient::new();
//! let health = client.health_check("http://localhost:8000").await?;
//! println!("server version: {}", health.version);
//! println!("capabilities:   {:?}", health.capabilities);
//! # Ok(())
//! # }
//! ```
//!
//! ## Resolve inputs and discover files
//!
//! ```rust,no_run
//! use std::path::Path;
//! use crate::cli::resolve::resolve_inputs;
//! use crate::cli::discover::discover_client_files;
//!
//! // Resolve CLI-style positional args into (inputs, output_dir)
//! let (inputs, out_dir) = resolve_inputs(
//!     &["corpus/".into(), "output/".into()],
//!     None,   // --output
//!     None,   // --file-list
//!     false,  // --in-place
//! ).unwrap();
//!
//! // Walk a directory and collect .cha files, sorted largest-first
//! let (files, outputs) = discover_client_files(
//!     Path::new(&inputs[0]),
//!     Path::new(out_dir.as_deref().unwrap()),
//!     &["cha"],
//! ).unwrap();
//! println!("found {} .cha files", files.len());
//! ```
//!
//! # Module map
//!
//! ## Core dispatch
//!
//! | Module        | Responsibility                                                     |
//! |---------------|--------------------------------------------------------------------|
//! | [`args`]      | Clap argument definitions: `Cli`, `GlobalOpts`, `Commands`, per-command structs |
//! | [`dispatch`]  | Top-level dispatch router (single server or daemon) and job lifecycle |
//! | [`client`]    | HTTP client wrapping `reqwest` with retry and adaptive polling     |
//! | [`resolve`]   | Input path resolution (`--file-list`, `--in-place`, legacy 2-arg)  |
//! | [`discover`]  | File discovery: walk directories, filter by extension, skip dummies |
//! | [`error`]     | Typed CLI errors with stable exit codes (2--6) for scripting       |
//!
//! ## Server and daemon management
//!
//! | Module        | Responsibility                                                     |
//! |---------------|--------------------------------------------------------------------|
//! | [`daemon`]    | Local daemon lifecycle: spawn, health-check, stale-binary restart  |
//! | [`serve_cmd`] | `batchalign3 serve` subcommands (start, stop, status)              |
//! | [`python`]    | Python runtime resolution (re-export from `batchalign_worker`)     |
//!
//! ## User interface
//!
//! | Module        | Responsibility                                                     |
//! |---------------|--------------------------------------------------------------------|
//! | [`output`]    | Write job results to local filesystem with path traversal protection |
//! | [`progress`]  | Terminal progress bars (indicatif) and `ProgressSink` trait        |
//! | [`tui`]       | Ratatui-based TUI dashboard for real-time job monitoring           |
//!
//! ## Subcommands
//!
//! | Module        | Responsibility                                                     |
//! |---------------|--------------------------------------------------------------------|
//! | [`jobs_cmd`]  | `batchalign3 jobs` -- inspect remote jobs or local job artifacts  |
//! | [`cache_cmd`] | `batchalign3 cache` -- cache statistics and clearing               |
//! | [`logs_cmd`]  | `batchalign3 logs` -- view, export, follow, clear run logs         |
//! | [`setup_cmd`] | `batchalign3 setup` -- initialize `~/.batchalign.ini`              |
//! | [`models_cmd`]| `batchalign3 models` -- forward to Python model training           |
//! | [`bench_cmd`] | `batchalign3 bench` -- repeated performance runs                   |
//!
//! ## Lifecycle
//!
//! | Module           | Responsibility                                                  |
//! |------------------|-----------------------------------------------------------------|
//! | [`update_check`] | Non-blocking PyPI version check with 24h file cache             |
//!
//! Binary-only concerns such as OTLP export and the update-check task are kept
//! behind the `binary-entry` feature. The embedded PyO3 CLI bridge depends on
//! this crate with `default-features = false`, so the native extension no
//! longer pulls the standalone binary's telemetry stack into its dependency
//! graph.

use clap::Parser;

pub mod args;
pub mod bench_cmd;
pub mod cache_cmd;
pub mod client;
pub mod daemon;
pub mod discover;
pub mod dispatch;
pub mod doctor_cmd;
pub mod error;
pub mod eval_cmd;
pub mod ipc_schema;
pub mod jobs_cmd;
pub mod logs_cmd;
pub mod models_cmd;
pub mod output;
pub mod progress;
pub mod python;
pub mod replay_cmd;
pub mod resolve;
pub(crate) mod self_exe;
#[cfg(feature = "server")]
pub mod serve_cmd;
pub mod setup_cmd;
pub mod tui;
#[cfg(feature = "binary-entry")]
pub mod update_check;
pub mod worker_cmd;

/// Build fingerprint — changes on every rebuild, even when the version stays
/// the same.  Used for stale-binary detection during development.
pub fn build_hash() -> &'static str {
    env!("BUILD_HASH")
}

/// Run the embedded CLI entry path from an explicit `argv` vector.
///
/// This is used by the PyO3 console-script bridge. Unlike the standalone
/// binary entry, it does not spawn the background update check or initialize
/// OTLP tracing; it only sets up basic stderr logging and then delegates to
/// [`run_command`].
pub fn run_embedded_cli_from_argv(argv: Vec<String>) -> Result<(), i32> {
    let cli = args::Cli::parse_from(argv);
    init_embedded_cli_tracing(cli.global.verbose);

    let rt = tokio::runtime::Runtime::new().map_err(|error| {
        eprintln!("error: failed to create async runtime: {error}");
        6
    })?;

    rt.block_on(async {
        match run_command(cli).await {
            Ok(()) => Ok(()),
            Err(error) => {
                eprintln!("error: {error}");
                Err(error.exit_code())
            }
        }
    })
}

fn init_embedded_cli_tracing(verbose: u8) {
    use tracing_subscriber::EnvFilter;

    let filter = match verbose {
        0 => "warn",
        1 => "info",
        2 => "debug",
        _ => "trace",
    };

    let env_filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(filter));

    // The embedded PyO3 path uses only basic stderr logging. OTLP export
    // remains a standalone-binary concern.
    let _ = tracing_subscriber::fmt()
        .with_env_filter(env_filter)
        .with_target(false)
        .with_writer(std::io::stderr)
        .try_init();
}

/// Shared CLI dispatch — the canonical command router.
///
/// Both the standalone binary (`main.rs`) and the PyO3 console_scripts entry
/// point (`cli_entry.rs`) delegate here. This is the single source of truth
/// for subcommand dispatch, eliminating the duplication that previously caused
/// the two entry points to drift out of sync.
pub async fn run_command(cli: args::Cli) -> Result<(), error::CliError> {
    use args::{Commands, CommonOpts};
    use std::io::IsTerminal;

    match &cli.command {
        #[cfg(feature = "server")]
        Commands::Serve(args) => match &args.action {
            args::ServeAction::Start(start_args) => {
                serve_cmd::start(
                    start_args,
                    cli.global.verbose,
                    cli.global.force_cpu,
                    cli.global.engine_overrides.as_deref(),
                )
                .await
            }
            args::ServeAction::Stop => serve_cmd::stop().await,
            args::ServeAction::Status(status_args) => serve_cmd::status(status_args).await,
        },
        #[cfg(not(feature = "server"))]
        Commands::Serve(_) => {
            eprintln!("error: serve command requires the 'server' feature");
            std::process::exit(2);
        }
        Commands::Jobs(a) => jobs_cmd::run(a).await,
        Commands::Logs(a) => logs_cmd::run(a),
        Commands::Models(a) => match &a.action {
            args::ModelsAction::Prep(prep) => models_cmd::run_prep(prep),
            args::ModelsAction::Train(train) => models_cmd::run_train(train),
        },
        Commands::Bench(a) => bench_cmd::run(&cli.global, a).await,
        Commands::Setup(a) => setup_cmd::run(a),
        Commands::Openapi(a) => {
            #[cfg(feature = "server")]
            {
                if a.check {
                    let output = a.output.as_deref().unwrap_or("openapi.json").to_string();
                    let out = std::path::Path::new(&output);
                    crate::openapi::check_openapi_json(out)?;
                    eprintln!("OpenAPI schema is up to date: {}", out.display());
                } else if let Some(path) = &a.output {
                    let out = std::path::Path::new(path);
                    crate::openapi::write_openapi_json(out)?;
                } else {
                    let json = crate::openapi::openapi_json_pretty()?;
                    println!("{json}");
                }
                Ok(())
            }
            #[cfg(not(feature = "server"))]
            {
                let _ = a;
                {
                    eprintln!("error: openapi command requires the 'server' feature");
                    std::process::exit(2);
                }
            }
        }
        Commands::IpcSchema(a) => {
            let schema = ipc_schema::generate_ipc_schema();
            if a.check {
                let output = a.output.as_deref().unwrap_or("ipc-schema").to_string();
                ipc_schema::check_ipc_schema(&schema, &output).map_err(|e| {
                    error::CliError::Server(crate::error::ServerError::Validation(e.to_string()))
                })?;
                eprintln!("IPC schema is up to date: {output}");
            } else if let Some(dir) = &a.output {
                ipc_schema::write_ipc_schema(&schema, dir).map_err(|e| {
                    error::CliError::Server(crate::error::ServerError::Validation(e.to_string()))
                })?;
            } else {
                println!("{}", serde_json::to_string_pretty(&schema)?);
            }
            Ok(())
        }
        Commands::Cache(a) => cache_cmd::run(a).await,
        Commands::Worker(a) => worker_cmd::run(a, cli.global.verbose).await,
        Commands::Doctor(a) => doctor_cmd::run(a).await,
        Commands::Replay(a) => replay_cmd::run(a).await,
        Commands::Eval(a) => eval_cmd::run(a),
        Commands::Version => {
            eprintln!(
                "batchalign3 {} (build {})",
                env!("CARGO_PKG_VERSION"),
                build_hash()
            );
            Ok(())
        }

        cmd => {
            // First-run config gate: processing commands require ~/.batchalign.ini.
            // If missing and we're in an interactive terminal, auto-trigger setup
            // (matching batchalign2 behavior where config_read(interactive=True)
            // runs interactive_setup() on first invocation).
            if !setup_cmd::config_exists() {
                if std::io::stdin().is_terminal() {
                    eprintln!("No configuration found. Running first-time setup...\n");
                    setup_cmd::run(&args::SetupArgs {
                        engine: None,
                        rev_key: None,
                        non_interactive: false,
                    })?;
                    eprintln!();
                } else {
                    return Err(std::io::Error::new(
                        std::io::ErrorKind::InvalidInput,
                        "Batchalign cannot find a configuration file. Run \
                         'batchalign3 setup' in the command line to generate one, \
                         or write one yourself and place it at ~/.batchalign.ini.",
                    )
                    .into());
                }
            }

            let profile = CommonOpts::command_profile(cmd);
            let common = args::common_opts(cmd);

            let (inputs, out_dir) = match cmd {
                Commands::Opensmile(a) => (vec![a.input_dir.clone()], Some(a.output_dir.clone())),
                Commands::Avqi(a) => (vec![a.input_dir.clone()], Some(a.output_dir.clone())),
                _ => {
                    // Caller-contract invariant: this match arm only
                    // runs for processing commands (every variant
                    // matched by `command_profile()` above), all of
                    // which carry `CommonOpts` from clap.
                    #[allow(clippy::expect_used)]
                    let c = common.expect("processing command must have CommonOpts");
                    resolve::resolve_inputs(
                        &c.paths,
                        c.output.as_deref(),
                        c.file_list.as_deref(),
                        c.in_place,
                    )?
                }
            };

            if let Some(ref od) = out_dir {
                std::fs::create_dir_all(od)?;
            }

            let options = args::build_typed_options(cmd, &cli.global);
            let bank = args::extract_bank(cmd);
            let subdir = args::extract_subdir(cmd);
            let lexicon = args::extract_lexicon(cmd);

            let before = args::extract_before(cmd);

            dispatch::dispatch(dispatch::DispatchRequest {
                command: profile.command,
                lang: profile.lang,
                num_speakers: profile.num_speakers,
                extensions: profile.extensions,
                server_arg: cli.global.server.as_deref(),
                inputs: &inputs,
                out_dir: out_dir.as_deref(),
                options,
                bank,
                subdir,
                lexicon,
                use_tui: cli.global.use_tui(),
                open_dashboard: cli.global.use_open_dashboard(),
                force_cpu: cli.global.force_cpu,
                no_server: cli.global.no_server,
                before,
                workers: cli.global.workers,
                timeout: cli.global.timeout,
                sequential: cli.global.sequential,
                memory_tier: cli.global.memory_tier,
            })
            .await
        }
    }
}
