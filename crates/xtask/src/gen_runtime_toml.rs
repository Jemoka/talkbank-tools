//! Codegen `python/batchalign/runtime_constants.toml` from
//! `batchalign_types::command_spec::COMMAND_SPECS`.
//!
//! Per the Phase β spec
//! (`docs/architecture/2026-05-10-phase-beta-command-spec.md`),
//! the Rust `CommandSpec` registry is the SOLE canonical source of
//! per-command metadata. The TOML is a generated mirror that Python
//! reads at import time. The `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code` target invokes
//! this with `--check` to fail CI on drift.

use batchalign_types::command_spec::{COMMAND_SPECS, GilProcessNeed};
use batchalign_types::worker_profile::WorkerProfile;
use std::path::PathBuf;

/// Header emitted at the top of the regenerated TOML.
const HEADER: &str = "\
# GENERATED — DO NOT EDIT.
# Source: crates/batchalign-types/src/command_spec.rs (COMMAND_SPECS)
# Regenerate: `cargo run -p xtask -- gen-runtime-toml`
# Drift gate: `cargo run -p xtask -- gen-runtime-toml --check`
#
# Read by:
#   - Rust: batchalign-types/src/command_spec.rs (compile-time const)
#   - Python: batchalign/runtime.py (read at import time)
";

/// Static template for the non-per-command sections. Verbatim from
/// the pre-Phase β TOML; values here are global, not per-command.
const NON_PER_COMMAND_SECTIONS: &str = "
[worker_caps]
max_gpu_workers = 8
max_process_workers = 8
max_thread_workers = 8

[memory]
default_base_mb = 4000
mb_per_file_mb = 25
loading_overhead = 1.5
";

const KNOWN_ENGINE_KEYS_SECTION: &str = "
[known_engine_keys]
keys = [\"asr\", \"batch_size\", \"fa\", \"feature_set\", \"utr\"]
";

/// Project `COMMAND_SPECS` into a TOML string.
///
/// Output sections, in order:
///   1. Header (GENERATED warning)
///   2. `[cmd2task]`                     (from `CommandSpec.task_label`)
///   3. `[worker_caps]`, `[memory]`      (verbatim from template)
///   4. `[gpu_heavy_commands]`           (filter `CommandSpec.profile == Gpu`)
///   5. `[process_commands]`             (filter `CommandSpec.gil_process_need`)
///   6. `[command_base_mb.process]`      (from `CommandSpec.base_mb_process`)
///   7. `[command_base_mb.threaded]`     (from `CommandSpec.base_mb_threaded`)
///   8. `[known_engine_keys]`            (verbatim)
pub fn generate_runtime_toml() -> String {
    let mut out = String::new();
    out.push_str(HEADER);

    // [cmd2task]
    out.push_str("\n[cmd2task]\n");
    for spec in COMMAND_SPECS {
        out.push_str(&format!(
            "{} = \"{}\"\n",
            spec.name.as_str(),
            spec.task_label
        ));
    }

    out.push_str(NON_PER_COMMAND_SECTIONS);

    // [gpu_heavy_commands]
    out.push_str("\n[gpu_heavy_commands]\n");
    let gpu_heavy: Vec<String> = COMMAND_SPECS
        .iter()
        .filter(|s| s.profile == WorkerProfile::Gpu)
        .map(|s| format!("\"{}\"", s.name.as_str()))
        .collect();
    out.push_str(&format!("commands = [{}]\n", gpu_heavy.join(", ")));

    // [process_commands]
    out.push_str("\n[process_commands]\n");
    out.push_str("# Non-free-threaded: CPU-bound commands needing process isolation.\n");
    let gil_list: Vec<String> = COMMAND_SPECS
        .iter()
        .filter(|s| {
            matches!(
                s.gil_process_need,
                GilProcessNeed::Always | GilProcessNeed::OnlyInGilRuntime
            )
        })
        .map(|s| format!("\"{}\"", s.name.as_str()))
        .collect();
    out.push_str(&format!("gil = [{}]\n", gil_list.join(", ")));

    out.push_str("# Free-threaded: only native-code commands need process isolation.\n");
    let ft_list: Vec<String> = COMMAND_SPECS
        .iter()
        .filter(|s| matches!(s.gil_process_need, GilProcessNeed::Always))
        .map(|s| format!("\"{}\"", s.name.as_str()))
        .collect();
    out.push_str(&format!("free_threaded = [{}]\n", ft_list.join(", ")));

    // [command_base_mb.process]
    out.push_str("\n# Per-command base memory (MB) for process workers (non-free-threaded).\n");
    out.push_str("[command_base_mb.process]\n");
    for spec in COMMAND_SPECS {
        out.push_str(&format!(
            "{} = {}\n",
            spec.name.as_str(),
            spec.base_mb_process.0
        ));
    }

    // [command_base_mb.threaded]
    out.push_str(
        "\n# Per-command base memory (MB) for thread workers (free-threaded, shared models).\n",
    );
    out.push_str("[command_base_mb.threaded]\n");
    for spec in COMMAND_SPECS {
        out.push_str(&format!(
            "{} = {}\n",
            spec.name.as_str(),
            spec.base_mb_threaded.0
        ));
    }

    out.push_str(KNOWN_ENGINE_KEYS_SECTION);

    out
}

/// Resolve the path to `python/batchalign/runtime_constants.toml` relative to
/// the xtask manifest dir (workspace root + "python/batchalign/runtime_constants.toml").
fn toml_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../python/batchalign/runtime_constants.toml")
}

/// Run the codegen.
///
/// `check=true` mode prints a diff if the regenerated TOML diverges
/// from the committed file and exits non-zero. Used by
/// `just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code` for CI drift detection.
///
/// `check=false` mode writes the regenerated TOML to disk.
pub fn run(check: bool) -> Result<(), Box<dyn std::error::Error>> {
    let path = toml_path();
    let regenerated = generate_runtime_toml();

    if check {
        let committed = std::fs::read_to_string(&path)?;
        if regenerated.trim() != committed.trim() {
            eprintln!("runtime_constants.toml is stale.");
            eprintln!("Run: cargo run -p xtask -- gen-runtime-toml");
            std::process::exit(1);
        }
    } else {
        std::fs::write(&path, &regenerated)?;
        println!("Wrote {}", path.display());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regeneration must be byte-deterministic.
    #[test]
    fn generate_runtime_toml_is_deterministic() {
        let first = generate_runtime_toml();
        let second = generate_runtime_toml();
        assert_eq!(first, second, "regeneration must be byte-identical");
    }

    /// The first regeneration must equal the committed file (modulo
    /// the new GENERATED header). Drift here means the codegen has
    /// a bug, not the TOML.
    #[test]
    fn generate_runtime_toml_matches_committed_file() {
        let regenerated = generate_runtime_toml();
        let committed_path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../python/batchalign/runtime_constants.toml");
        let committed =
            std::fs::read_to_string(&committed_path).expect("runtime_constants.toml must exist");
        // Trim is generous — we don't want a trailing-newline mismatch
        // to fail this test. The byte-equality on per-command content
        // is what matters.
        assert_eq!(
            regenerated.trim(),
            committed.trim(),
            "regenerated TOML diverges from committed copy. \
             If you changed CommandSpec, run \
             `cargo run -p xtask -- gen-runtime-toml` and commit the result."
        );
    }
}
