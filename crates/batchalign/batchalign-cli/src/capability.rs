//! Worker capability discovery, validation, and snapshot resolution.
//!
//! This module owns the logic that determines which released commands a
//! batchalign3 instance can serve, based on the infer-task set reported by
//! the Python worker subsystem. It does NOT depend on axum, sqlx, or any
//! server-specific crate.
//!
//! Used by both the direct-mode host ([`crate::worker_setup`]) and the HTTP
//! server ([`crate::state`]).

use std::collections::BTreeMap;

use tracing::warn;

use crate::commands::released_command_definitions;
use crate::commands::spec::CommandCapabilityKind;
use crate::error;
use crate::worker::target::task_name as infer_task_capability_name;
use crate::worker::{InferTask, WorkerCapabilities};

// ---------------------------------------------------------------------------
// Capability snapshot
// ---------------------------------------------------------------------------

/// One resolved view of worker capability state used by execution-time callers.
///
/// The startup path may only know an optimistic command list while the worker
/// pool has not yet spawned a real backend. Once the pool has lazily probed a
/// live worker, callers should switch to that detected infer-task/engine view.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkerCapabilitySnapshot {
    pub capabilities: Vec<String>,
    pub infer_tasks: Vec<InferTask>,
    pub engine_versions: BTreeMap<String, String>,
}

// ---------------------------------------------------------------------------
// Capability validation
// ---------------------------------------------------------------------------

fn derive_command_capabilities(infer_tasks: &[InferTask]) -> Vec<String> {
    let mut derived = Vec::new();

    for descriptor in released_command_definitions()
        .iter()
        .map(|definition| definition.descriptor)
        .filter(|descriptor| descriptor.capability_kind == CommandCapabilityKind::DirectInfer)
    {
        if infer_tasks.contains(&descriptor.infer_task)
            && !derived
                .iter()
                .any(|cap: &String| descriptor.command.as_str() == cap.as_str())
        {
            derived.push(descriptor.command.to_string());
        }
    }

    for descriptor in released_command_definitions()
        .iter()
        .map(|definition| definition.descriptor)
        .filter(|descriptor| descriptor.capability_kind == CommandCapabilityKind::ServerComposed)
    {
        if infer_tasks.contains(&descriptor.infer_task)
            && !derived
                .iter()
                .any(|cap: &String| descriptor.command.as_str() == cap.as_str())
        {
            derived.push(descriptor.command.to_string());
        }
    }

    derived
}

/// Derive released command capabilities from infer tasks and validate engine versions.
///
/// Engine version entries for reported infer tasks must still be present and
/// non-empty. The worker-reported `commands` field is treated as compatibility
/// metadata only; released server command availability is derived entirely from
/// infer-task support.
pub(crate) fn validate_infer_capability_gate(
    infer_tasks: &[InferTask],
    engine_versions: &BTreeMap<String, String>,
    test_echo_mode: bool,
) -> Result<Vec<String>, error::ServerError> {
    if test_echo_mode {
        let mut commands: Vec<String> = batchalign_types::command_spec::COMMAND_SPECS
            .iter()
            .map(|s| s.name.as_str().to_string())
            .collect();
        commands.sort();
        commands.dedup();
        return Ok(commands);
    }

    // Validate engine versions for all reported infer tasks.
    for task in infer_tasks {
        let task_name = infer_task_capability_name(*task);
        let Some(version) = engine_versions.get(task_name) else {
            return Err(error::ServerError::Validation(format!(
                "worker capability gate failed: infer task '{task_name}' is reported but engine_versions['{task_name}'] is missing"
            )));
        };
        if version.trim().is_empty() {
            return Err(error::ServerError::Validation(format!(
                "worker capability gate failed: infer task '{task_name}' has empty engine_versions['{task_name}']"
            )));
        }
    }

    let derived = derive_command_capabilities(infer_tasks);
    if derived.is_empty() && !infer_tasks.is_empty() {
        warn!(infer_tasks = ?infer_tasks, "No released commands derived from infer-task set");
    }

    Ok(derived)
}

/// Resolve one capability snapshot, preferring live detected worker data when
/// the pool has already probed a real backend.
pub(crate) fn resolve_worker_capability_snapshot(
    startup_capabilities: &[String],
    startup_infer_tasks: &[InferTask],
    startup_engine_versions: &BTreeMap<String, String>,
    test_echo_mode: bool,
    detected: Option<&WorkerCapabilities>,
) -> Result<WorkerCapabilitySnapshot, error::ServerError> {
    if let Some(detected) = detected {
        let capabilities = validate_infer_capability_gate(
            &detected.infer_tasks,
            &detected.engine_versions,
            test_echo_mode,
        )?;
        return Ok(WorkerCapabilitySnapshot {
            capabilities,
            infer_tasks: detected.infer_tasks.clone(),
            engine_versions: detected.engine_versions.clone(),
        });
    }

    Ok(WorkerCapabilitySnapshot {
        capabilities: startup_capabilities.to_vec(),
        infer_tasks: startup_infer_tasks.to_vec(),
        engine_versions: startup_engine_versions.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::{resolve_worker_capability_snapshot, validate_infer_capability_gate};
    use crate::worker::{InferTask, WorkerCapabilities};
    use std::collections::BTreeMap;

    #[test]
    fn infer_gate_returns_no_commands_without_infer_tasks() {
        let filtered = validate_infer_capability_gate(&[], &BTreeMap::new(), false)
            .expect("empty infer task set should derive an empty command list");
        assert!(filtered.is_empty());
    }

    #[test]
    fn infer_gate_derives_released_commands_from_infer_tasks() {
        let infer_tasks = vec![
            InferTask::Morphosyntax,
            InferTask::Utseg,
            InferTask::Translate,
            InferTask::Coref,
            InferTask::Fa,
            InferTask::Opensmile,
            InferTask::Avqi,
        ];
        let versions = BTreeMap::from([
            ("morphosyntax".to_string(), "stanza-1.9.2".to_string()),
            ("utseg".to_string(), "stanza".to_string()),
            ("translate".to_string(), "seamless-v1".to_string()),
            ("coref".to_string(), "stanza-1.9.2".to_string()),
            ("fa".to_string(), "whisper".to_string()),
            ("opensmile".to_string(), "opensmile".to_string()),
            ("avqi".to_string(), "praat".to_string()),
        ]);
        let filtered = validate_infer_capability_gate(&infer_tasks, &versions, false)
            .expect("complete infer-task set should derive released commands");
        assert_eq!(
            filtered,
            vec![
                "morphotag".to_string(),
                "utseg".to_string(),
                "translate".to_string(),
                "coref".to_string(),
                "align".to_string(),
                "compare".to_string(),
                "opensmile".to_string(),
                "avqi".to_string(),
            ]
        );
    }

    #[test]
    fn infer_gate_rejects_missing_engine_version() {
        let infer_tasks = vec![InferTask::Morphosyntax];
        let err = validate_infer_capability_gate(&infer_tasks, &BTreeMap::new(), false)
            .expect_err("missing engine_versions entry should fail");
        assert!(
            err.to_string()
                .contains("engine_versions['morphosyntax'] is missing"),
            "actual: {}",
            err
        );
    }

    #[test]
    fn infer_gate_rejects_empty_engine_version() {
        let infer_tasks = vec![InferTask::Fa];
        let versions = BTreeMap::from([("fa".to_string(), " ".to_string())]);
        let err = validate_infer_capability_gate(&infer_tasks, &versions, false)
            .expect_err("empty engine version should fail");
        assert!(
            err.to_string().contains("empty engine_versions['fa']"),
            "actual: {}",
            err
        );
    }

    #[test]
    fn infer_gate_accepts_complete_capabilities() {
        let infer_tasks = vec![InferTask::Morphosyntax, InferTask::Fa];
        let versions = BTreeMap::from([
            ("morphosyntax".to_string(), "stanza-1.9.2".to_string()),
            ("fa".to_string(), "whisper-fa-large-v3".to_string()),
        ]);
        let filtered = validate_infer_capability_gate(&infer_tasks, &versions, false)
            .expect("complete infer capability data should pass");
        assert_eq!(
            filtered,
            vec![
                "morphotag".to_string(),
                "align".to_string(),
                "compare".to_string(),
            ]
        );
    }

    #[test]
    fn infer_gate_synthesizes_server_owned_asr_commands() {
        let infer_tasks = vec![InferTask::Asr];
        let versions = BTreeMap::from([("asr".to_string(), "whisper".to_string())]);
        let filtered = validate_infer_capability_gate(&infer_tasks, &versions, false)
            .expect("server-owned ASR commands should be synthesized when ASR is available");
        assert_eq!(
            filtered,
            vec![
                "transcribe".to_string(),
                "transcribe_s".to_string(),
                "benchmark".to_string(),
            ]
        );
    }

    #[test]
    fn infer_gate_skips_test_echo_mode() {
        let filtered = validate_infer_capability_gate(&[], &BTreeMap::new(), true)
            .expect("test-echo mode should bypass strict infer gate");
        assert!(filtered.iter().any(|command| command == "morphotag"));
        assert!(filtered.iter().any(|command| command == "transcribe"));
    }

    #[test]
    fn resolve_worker_capability_snapshot_prefers_live_detected_tasks() {
        let startup_capabilities = vec!["morphotag".to_string(), "utseg".to_string()];
        let startup_infer_tasks = Vec::new();
        let startup_engine_versions = BTreeMap::new();
        let detected = WorkerCapabilities {
            commands: Vec::new(),
            free_threaded: false,
            infer_tasks: vec![InferTask::Morphosyntax, InferTask::Utseg],
            engine_versions: BTreeMap::from([
                ("morphosyntax".to_string(), "stanza-1.10.1".to_string()),
                ("utseg".to_string(), "stanza-1.10.1".to_string()),
            ]),
            stanza_capabilities: BTreeMap::new(),
        };

        let snapshot = resolve_worker_capability_snapshot(
            &startup_capabilities,
            &startup_infer_tasks,
            &startup_engine_versions,
            false,
            Some(&detected),
        )
        .expect("live detected capabilities should override startup placeholder state");

        assert_eq!(
            snapshot.capabilities,
            vec![
                "morphotag".to_string(),
                "utseg".to_string(),
                "compare".to_string(),
            ]
        );
        assert_eq!(
            snapshot.infer_tasks,
            vec![InferTask::Morphosyntax, InferTask::Utseg]
        );
        assert_eq!(
            snapshot.engine_versions.get("morphosyntax"),
            Some(&"stanza-1.10.1".to_string())
        );
    }

    #[test]
    fn resolve_worker_capability_snapshot_falls_back_to_startup_when_no_live_data() {
        let startup_capabilities = vec!["morphotag".to_string()];
        let startup_infer_tasks = vec![InferTask::Morphosyntax];
        let startup_engine_versions =
            BTreeMap::from([("morphosyntax".to_string(), "stanza-1.9.2".to_string())]);

        let snapshot = resolve_worker_capability_snapshot(
            &startup_capabilities,
            &startup_infer_tasks,
            &startup_engine_versions,
            false,
            None,
        )
        .expect("startup snapshot should still be usable when no live worker was probed");

        assert_eq!(snapshot.capabilities, startup_capabilities);
        assert_eq!(snapshot.infer_tasks, startup_infer_tasks);
        assert_eq!(snapshot.engine_versions, startup_engine_versions);
    }

    #[test]
    fn resolve_worker_capability_snapshot_prefers_empty_live_probe_over_startup_guess() {
        let startup_capabilities = vec!["align".to_string(), "morphotag".to_string()];
        let startup_infer_tasks = vec![InferTask::Fa, InferTask::Morphosyntax];
        let startup_engine_versions = BTreeMap::from([
            ("fa".to_string(), "wave2vec".to_string()),
            ("morphosyntax".to_string(), "stanza-1.9.2".to_string()),
        ]);
        let detected = WorkerCapabilities {
            commands: Vec::new(),
            free_threaded: false,
            infer_tasks: Vec::new(),
            engine_versions: BTreeMap::new(),
            stanza_capabilities: BTreeMap::new(),
        };

        let snapshot = resolve_worker_capability_snapshot(
            &startup_capabilities,
            &startup_infer_tasks,
            &startup_engine_versions,
            false,
            Some(&detected),
        )
        .expect("live detected empty capabilities should override optimistic startup data");

        assert!(snapshot.capabilities.is_empty());
        assert!(snapshot.infer_tasks.is_empty());
        assert!(snapshot.engine_versions.is_empty());
    }
}
