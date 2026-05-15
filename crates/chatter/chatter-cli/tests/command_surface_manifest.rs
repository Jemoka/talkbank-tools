//! Release-facing manifest for the published `chatter` command surface.
//!
//! This keeps the current top-level and `clan` subcommand vocabulary explicit so
//! deeper refactors can tighten internals without accidentally drifting the CLI.

use std::collections::BTreeSet;

mod common;

use common::{
    CliHarness,
    command_surface::{CoverageExpectation, SURFACE_GROUPS, SurfaceFamily, SurfaceScope},
};

fn listed_commands(help: &str) -> BTreeSet<String> {
    let mut commands = BTreeSet::new();
    let mut in_commands = false;

    for line in help.lines() {
        let trimmed = line.trim();
        if trimmed == "Commands:"
            || trimmed.ends_with("Commands:")
            || trimmed.ends_with("Converters:")
            || trimmed.ends_with("Aliases:")
            || trimmed.starts_with("Not Available")
        {
            in_commands = true;
            continue;
        }

        if !in_commands {
            continue;
        }

        if trimmed == "Options:" {
            break;
        }

        if line.starts_with("  ") && !trimmed.is_empty() {
            if let Some(command) = trimmed.split_whitespace().next() {
                commands.insert(command.to_string());
            }
        }
    }

    commands
}

fn help_output(args: &[&str]) -> String {
    let harness =
        CliHarness::new().expect("command-surface help should get an isolated CLI harness");
    let output = harness
        .chatter_cmd()
        .args(args)
        .assert()
        .success()
        .get_output()
        .stdout
        .clone();
    String::from_utf8_lossy(&output).into_owned()
}

fn manifest_commands(scope: SurfaceScope) -> BTreeSet<&'static str> {
    SURFACE_GROUPS
        .iter()
        .filter(|group| group.scope == scope)
        .flat_map(|group| group.commands.iter().copied())
        .collect()
}

#[test]
fn command_surface_manifest_has_unique_command_names_per_scope() {
    for scope in [SurfaceScope::TopLevel, SurfaceScope::Clan] {
        let mut seen = BTreeSet::new();
        for command in manifest_commands(scope) {
            assert!(
                seen.insert(command),
                "duplicate command `{command}` in {:?} surface manifest",
                scope
            );
        }
    }
}

#[test]
fn top_level_help_lists_all_manifested_commands() {
    let commands = listed_commands(&help_output(&["--help"]));
    for command in manifest_commands(SurfaceScope::TopLevel) {
        assert!(
            commands.contains(command),
            "top-level help is missing manifested command `{command}`"
        );
    }
    assert!(
        !commands.contains("analyze"),
        "stale removed command `analyze` reappeared in top-level help"
    );
}

#[test]
fn clan_help_lists_all_manifested_commands() {
    let commands = listed_commands(&help_output(&["clan", "--help"]));
    for command in manifest_commands(SurfaceScope::Clan) {
        assert!(
            commands.contains(command),
            "clan help is missing manifested command `{command}`"
        );
    }
}

#[test]
fn every_surface_group_declares_coverage_and_rationale() {
    for group in SURFACE_GROUPS {
        assert!(
            !group.commands.is_empty(),
            "{:?} group {:?} has no commands",
            group.scope,
            group.family
        );
        assert!(
            !group.coverage.is_empty(),
            "{:?} group {:?} has no coverage expectations",
            group.scope,
            group.family
        );
        assert!(
            !group.note.is_empty(),
            "{:?} group {:?} has no rationale",
            group.scope,
            group.family
        );
    }
}

#[test]
fn legacy_clan_surface_groups_explicitly_require_compatibility_coverage() {
    for group in SURFACE_GROUPS {
        if matches!(
            group.family,
            SurfaceFamily::ClanCompatibilityShim | SurfaceFamily::ClanCompatibilityPlaceholder
        ) {
            assert!(
                group
                    .coverage
                    .contains(&CoverageExpectation::LegacyCompatibility),
                "legacy CLAN group {:?} must require compatibility coverage",
                group.family
            );
        }
    }
}
