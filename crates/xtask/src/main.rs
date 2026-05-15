use std::collections::{BTreeSet, HashMap, HashSet};
use std::env;
use std::error::Error;
use std::path::{Path, PathBuf};
use std::process::{self, Command, Stdio};

use serde::Deserialize;

type DynError = Box<dyn Error>;
type Result<T> = std::result::Result<T, DynError>;

const FULL_WORKSPACE_PATHS: &[&str] = &["Cargo.toml", "Cargo.lock"];
const FULL_WORKSPACE_PREFIXES: &[&str] = &[".github/"];
const DOC_PREFIXES: &[&str] = &["book/", "docs/"];
const SPECIAL_PATH_PREFIXES: &[(&str, &str)] = &[("spec/tools/", "spec-tools")];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AffectedMode {
    Packages,
    Check,
    Clippy,
    Test,
}

impl AffectedMode {
    fn parse(raw: &str) -> Option<Self> {
        match raw {
            "packages" => Some(Self::Packages),
            "check" => Some(Self::Check),
            "clippy" => Some(Self::Clippy),
            "test" => Some(Self::Test),
            _ => None,
        }
    }
}

#[derive(Debug, Deserialize)]
struct Metadata {
    packages: Vec<MetadataPackage>,
}

#[derive(Debug, Deserialize)]
struct MetadataPackage {
    name: String,
    manifest_path: PathBuf,
    #[serde(default)]
    dependencies: Vec<MetadataDependency>,
}

#[derive(Debug, Deserialize)]
struct MetadataDependency {
    name: String,
}

struct AffectedScope {
    full_workspace: bool,
    impacted_packages: Vec<String>,
    special_targets: BTreeSet<String>,
    base_ref: String,
}

fn main() {
    if let Err(error) = run_main() {
        eprintln!("xtask error: {error}");
        process::exit(1);
    }
}

mod audit_docs;
mod audit_prose_references;
mod ci_hygiene;
mod dead_variant_audit;
mod docs_sync;
mod gen_runtime_toml;
mod panic_audit;
mod rust_scan;
mod wide_struct_audit;

fn run_main() -> Result<()> {
    let mut args = env::args().skip(1);
    match args.next().as_deref() {
        Some("help") => {
            print_help();
            Ok(())
        }
        Some("affected-rust") => {
            let Some(mode_raw) = args.next() else {
                return Err(usage_error());
            };
            if args.next().is_some() {
                return Err(usage_error());
            }
            let Some(mode) = AffectedMode::parse(&mode_raw) else {
                return Err(usage_error());
            };
            run_affected_rust(mode)
        }
        Some("lint-wide-structs") => wide_struct_audit::run(repo_root()),
        Some("lint-ci-hygiene") => ci_hygiene::run(repo_root()),
        Some("lint-docs-sync") => docs_sync::run(repo_root()),
        Some("lint-dead-variants") => {
            let rest: Vec<String> = args.collect();
            dead_variant_audit::run(repo_root(), rest)
        }
        Some("panic-audit") => {
            let rest: Vec<String> = args.collect();
            panic_audit::run(repo_root(), rest)
        }
        Some("audit-docs") => {
            let rest: Vec<String> = args.collect();
            audit_docs::parse_and_run(rest)
        }
        Some("audit-prose-references") => {
            if args.next().is_some() {
                return Err(usage_error());
            }
            audit_prose_references::run(repo_root())
        }
        Some("gen-runtime-toml") => {
            let check = args.any(|a| a == "--check");
            gen_runtime_toml::run(check).map_err(|e| e.to_string().into())
        }
        _ => Err(usage_error()),
    }
}

fn usage_error() -> DynError {
    "usage: cargo run -q -p xtask -- {help|affected-rust {packages|check|clippy|test}|lint-wide-structs|lint-ci-hygiene|lint-docs-sync|lint-dead-variants ...|panic-audit [--json] [--crate <prefix>]|audit-docs <scan|flag-staleness|status|streak|vet> [args...]|audit-prose-references|gen-runtime-toml [--check]}".into()
}

fn print_help() {
    println!("talkbank-tools xtask commands");
    println!();
    println!("  help");
    println!("      Show this command summary.");
    println!("  affected-rust packages");
    println!(
        "      Print affected workspace packages and special targets from current git changes."
    );
    println!("  affected-rust check");
    println!("      Run compile checks for the affected Rust surface.");
    println!("  affected-rust clippy");
    println!("      Run clippy for the affected Rust surface.");
    println!("  affected-rust test");
    println!("      Run tests for the affected Rust surface.");
    println!("  lint-wide-structs");
    println!("      Audit oversized structs against the repo's architecture rules.");
    println!("  lint-ci-hygiene");
    println!("      Verify merged Batchalign/CI retirement and version-consistency rules.");
    println!("  lint-docs-sync");
    println!("      Check docs that must stay synchronized with the command/runtime surface.");
    println!("  lint-dead-variants [args...]");
    println!("      Audit enum variants that appear unused; accepts module/path filters.");
    println!("  panic-audit [--json] [--crate <prefix>]");
    println!(
        "      Catalogue every panic-producing call site (.unwrap, .expect, panic!, todo!, unimplemented!, unreachable!) for the Phase B audit."
    );
    println!("  audit-docs scan [--db PATH] [--talkbank-tools PATH]");
    println!(
        "      Walk markdown across talkbank-tools; upsert per-section rows into the doc-to-code provenance audit catalog (talkbank-tools-only since v2 cutover)."
    );
    println!("  audit-docs flag-staleness [--db PATH] [--talkbank-tools PATH]");
    println!(
        "      Re-run regex surface-scan over every catalog section; populates `staleness_flags` so high-suspicion sections float to the top of the queue."
    );
    println!("  audit-docs status [--db PATH]");
    println!(
        "      Print Bucket A vetting progress + streak + the next 5 unvetted Bucket A sections. The default daily-cadence command."
    );
    println!("  audit-docs streak [--db PATH]");
    println!("      Print just the streak count (consecutive days with ≥1 vet).");
    println!(
        "  audit-docs vet --id <section_id> --verdict <unvetted|in-review|no-claims|vetted-accurate|needs-fix|fixed> [--reviewer <name>] [--notes <text>] [--fix-commit <hash>]"
    );
    println!("      Mark a section's verdict. `fixed` requires --fix-commit.");
    println!("  audit-prose-references");
    println!(
        "      Layer 1 CI gate. Walk all markdown under the repo, apply \
         high-severity prose-reference patterns (deleted crates, moved book \
         paths), exit non-zero on any non-allow-listed hit. \
         Catalog-independent — does not require audit.db."
    );
    println!("  gen-runtime-toml [--check]");
    println!(
        "      Regenerate python/batchalign/runtime_constants.toml from COMMAND_SPECS. \
         With --check, exit non-zero on drift (used by just spec gen-tree-sitter-tests && just spec gen-rust-tests && just spec gen-error-docs && git diff --exit-code)."
    );
}

fn run_affected_rust(mode: AffectedMode) -> Result<()> {
    let scope = inspect_changes()?;
    print_scope(&scope);

    if mode == AffectedMode::Packages {
        for package in &scope.impacted_packages {
            println!("{package}");
        }
        for target in &scope.special_targets {
            println!("+{target}");
        }
        return Ok(());
    }

    run_workspace_mode(mode, scope.full_workspace, &scope.impacted_packages)?;
    for target in &scope.special_targets {
        run_special(mode, target)?;
    }
    Ok(())
}

fn repo_root() -> &'static Path {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("xtask crate must live under repo root")
}

fn path_text(path: &Path) -> String {
    path.to_string_lossy().replace('\\', "/")
}

fn run_capture(args: &[&str], cwd: &Path, allow_failure: bool) -> Result<Option<String>> {
    let output = Command::new(args[0])
        .args(&args[1..])
        .current_dir(cwd)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .output()?;

    if output.status.success() {
        return Ok(Some(String::from_utf8_lossy(&output.stdout).into_owned()));
    }

    if allow_failure {
        return Ok(None);
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
    if stderr.is_empty() {
        Err(format!("command failed: {}", args.join(" ")).into())
    } else {
        Err(format!("command failed: {}: {stderr}", args.join(" ")).into())
    }
}

fn git_output(args: &[&str]) -> Result<Vec<String>> {
    let mut full_args = vec!["git"];
    full_args.extend_from_slice(args);
    let Some(stdout) = run_capture(&full_args, repo_root(), true)? else {
        return Ok(Vec::new());
    };
    Ok(stdout
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty())
        .map(ToOwned::to_owned)
        .collect())
}

fn detect_base_ref() -> Result<String> {
    let upstream = git_output(&[
        "rev-parse",
        "--abbrev-ref",
        "--symbolic-full-name",
        "@{upstream}",
    ])?;
    if let Some(upstream_ref) = upstream.first()
        && let Some(base) = git_output(&["merge-base", "HEAD", upstream_ref])?.first()
    {
        return Ok(base.clone());
    }

    if !git_output(&["rev-parse", "--verify", "origin/main"])?.is_empty()
        && let Some(base) = git_output(&["merge-base", "HEAD", "origin/main"])?.first()
    {
        return Ok(base.clone());
    }

    if let Some(fallback) = git_output(&["rev-parse", "HEAD~1"])?.first() {
        return Ok(fallback.clone());
    }

    Ok("HEAD".to_owned())
}

fn changed_paths(base_ref: &str) -> Result<Vec<PathBuf>> {
    let diff_range = format!("{base_ref}...HEAD");
    let queries = [
        vec![
            "diff".to_owned(),
            "--name-only".to_owned(),
            "--diff-filter=ACMRTUXB".to_owned(),
            diff_range,
        ],
        vec![
            "diff".to_owned(),
            "--name-only".to_owned(),
            "--diff-filter=ACMRTUXB".to_owned(),
        ],
        vec![
            "diff".to_owned(),
            "--cached".to_owned(),
            "--name-only".to_owned(),
            "--diff-filter=ACMRTUXB".to_owned(),
        ],
        vec![
            "ls-files".to_owned(),
            "--others".to_owned(),
            "--exclude-standard".to_owned(),
        ],
    ];

    let mut paths = BTreeSet::new();
    for query in &queries {
        let borrowed: Vec<&str> = query.iter().map(String::as_str).collect();
        for line in git_output(&borrowed)? {
            paths.insert(PathBuf::from(line));
        }
    }

    Ok(paths.into_iter().collect())
}

fn load_workspace_metadata() -> Result<Metadata> {
    let args = ["cargo", "metadata", "--format-version", "1", "--no-deps"];
    let stdout = run_capture(&args, repo_root(), false)?
        .ok_or_else(|| "cargo metadata unexpectedly returned no output".to_string())?;
    Ok(serde_json::from_str(&stdout)?)
}

fn package_graph() -> Result<(HashMap<String, PathBuf>, HashMap<String, HashSet<String>>)> {
    let metadata = load_workspace_metadata()?;
    let workspace_names: HashSet<String> = metadata
        .packages
        .iter()
        .map(|pkg| pkg.name.clone())
        .collect();

    let mut package_dirs = HashMap::new();
    let mut reverse: HashMap<String, HashSet<String>> = HashMap::new();

    for package in &metadata.packages {
        let relative_manifest = package
            .manifest_path
            .strip_prefix(repo_root())?
            .to_path_buf();
        let package_dir = relative_manifest
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| PathBuf::from("."));
        package_dirs.insert(package.name.clone(), package_dir);
        reverse.entry(package.name.clone()).or_default();
    }

    for package in &metadata.packages {
        for dependency in &package.dependencies {
            if workspace_names.contains(&dependency.name) {
                reverse
                    .entry(dependency.name.clone())
                    .or_default()
                    .insert(package.name.clone());
            }
        }
    }

    Ok((package_dirs, reverse))
}

fn is_doc_only(path: &Path) -> bool {
    let path = path_text(path);
    DOC_PREFIXES.iter().any(|prefix| path.starts_with(prefix))
}

fn matches_full_workspace(path: &Path) -> bool {
    let path = path_text(path);
    FULL_WORKSPACE_PATHS.iter().any(|exact| path == *exact)
        || FULL_WORKSPACE_PREFIXES
            .iter()
            .any(|prefix| path.starts_with(prefix))
}

fn special_targets_for_path(path: &Path) -> BTreeSet<String> {
    let path = path_text(path);
    SPECIAL_PATH_PREFIXES
        .iter()
        .filter(|(prefix, _)| path.starts_with(prefix))
        .map(|(_, target)| (*target).to_owned())
        .collect()
}

fn package_for_path(path: &Path, package_dirs: &HashMap<String, PathBuf>) -> Option<String> {
    let path_str = path_text(path);
    let mut root_fallback: Option<String> = None;
    let mut best_match: Option<(usize, String)> = None;

    for (name, package_dir) in package_dirs {
        let prefix = path_text(package_dir);
        if prefix == "." {
            if path_str == "build.rs"
                || path_str.starts_with("src/")
                || path_str.starts_with("tests/")
                || path_str.starts_with("benches/")
                || path_str.starts_with("examples/")
            {
                root_fallback = Some(name.clone());
            }
            continue;
        }

        if path_str == prefix || path_str.starts_with(&format!("{prefix}/")) {
            let score = prefix.len();
            if best_match
                .as_ref()
                .map(|(best_score, _)| score > *best_score)
                .unwrap_or(true)
            {
                best_match = Some((score, name.clone()));
            }
        }
    }

    best_match.map(|(_, name)| name).or(root_fallback)
}

fn reverse_dependent_closure(
    start: &HashSet<String>,
    reverse: &HashMap<String, HashSet<String>>,
) -> Vec<String> {
    let mut impacted = start.clone();
    let mut queue: Vec<String> = start.iter().cloned().collect();

    while let Some(current) = queue.pop() {
        if let Some(dependents) = reverse.get(&current) {
            for dependent in dependents {
                if impacted.insert(dependent.clone()) {
                    queue.push(dependent.clone());
                }
            }
        }
    }

    let mut impacted_list: Vec<String> = impacted.into_iter().collect();
    impacted_list.sort();
    impacted_list
}

fn inspect_changes() -> Result<AffectedScope> {
    let base_ref = detect_base_ref()?;
    let (package_dirs, reverse) = package_graph()?;
    let changed = changed_paths(&base_ref)?;

    let mut full_workspace = false;
    let mut special_targets = BTreeSet::new();
    let mut changed_packages = HashSet::new();

    for path in changed {
        special_targets.extend(special_targets_for_path(&path));

        if matches_full_workspace(&path) {
            full_workspace = true;
            continue;
        }

        if is_doc_only(&path) {
            continue;
        }

        if let Some(package) = package_for_path(&path, &package_dirs) {
            changed_packages.insert(package);
        }
    }

    let impacted_packages = if !full_workspace && !changed_packages.is_empty() {
        reverse_dependent_closure(&changed_packages, &reverse)
    } else {
        Vec::new()
    };

    Ok(AffectedScope {
        full_workspace,
        impacted_packages,
        special_targets,
        base_ref,
    })
}

fn print_scope(scope: &AffectedScope) {
    println!("==> affected-rust: base ref {}", scope.base_ref);
    if scope.full_workspace {
        println!("==> affected-rust: running full workspace scope");
    } else if scope.impacted_packages.is_empty() {
        println!("==> affected-rust: no affected Rust workspace packages");
    } else {
        println!(
            "==> affected-rust: impacted packages: {}",
            scope.impacted_packages.join(", ")
        );
    }

    if !scope.special_targets.is_empty() {
        println!(
            "==> affected-rust: extra workspaces: {}",
            scope
                .special_targets
                .iter()
                .cloned()
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
}

fn cargo_nextest_available() -> bool {
    Command::new("cargo")
        .args(["nextest", "--version"])
        .current_dir(repo_root())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn run_command(command: &[String], cwd: &Path) -> Result<()> {
    println!("+ {}", command.join(" "));
    let status = Command::new(&command[0])
        .args(&command[1..])
        .current_dir(cwd)
        .status()?;

    if status.success() {
        Ok(())
    } else {
        process::exit(status.code().unwrap_or(1));
    }
}

fn run_workspace_mode(mode: AffectedMode, full_workspace: bool, impacted: &[String]) -> Result<()> {
    if !full_workspace && impacted.is_empty() {
        println!("==> affected-rust: skipping main workspace command");
        return Ok(());
    }

    let mut command: Vec<String> = match mode {
        AffectedMode::Packages => unreachable!(),
        AffectedMode::Check => vec!["cargo", "check", "--all-targets"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        AffectedMode::Clippy => vec!["cargo", "clippy", "--all-targets", "--", "-D", "warnings"]
            .into_iter()
            .map(str::to_owned)
            .collect(),
        AffectedMode::Test => if cargo_nextest_available() {
            vec!["cargo", "nextest", "run", "--no-fail-fast"]
        } else {
            vec!["cargo", "test", "--no-fail-fast"]
        }
        .into_iter()
        .map(str::to_owned)
        .collect(),
    };

    if full_workspace {
        if mode == AffectedMode::Test && command.get(1).map(String::as_str) == Some("nextest") {
            command.insert(3, "--workspace".to_owned());
        } else {
            command.insert(2, "--workspace".to_owned());
        }
    } else {
        for package in impacted {
            command.push("-p".to_owned());
            command.push(package.clone());
        }
    }

    run_command(&command, repo_root())
}

fn run_special(mode: AffectedMode, target: &str) -> Result<()> {
    if target != "spec-tools" {
        return Err(format!("unsupported special target: {target}").into());
    }

    let cwd = repo_root().join("spec").join("tools");
    let command: Vec<String> = match mode {
        AffectedMode::Packages => unreachable!(),
        AffectedMode::Check => vec!["cargo", "check", "--all-targets"],
        AffectedMode::Clippy => vec!["cargo", "clippy", "--all-targets", "--", "-D", "warnings"],
        AffectedMode::Test => vec!["cargo", "test", "--no-fail-fast"],
    }
    .into_iter()
    .map(str::to_owned)
    .collect();

    run_command(&command, &cwd)
}
