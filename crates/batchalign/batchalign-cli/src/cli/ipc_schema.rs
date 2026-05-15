//! Generate JSON Schema for all Rust types that cross the Python IPC boundary.
//!
//! This module is the Rust half of the Rust→Python type sync pipeline:
//!
//! 1. Rust types derive `schemars::JsonSchema`
//! 2. This module emits JSON Schema for all IPC-boundary types
//! 3. `scripts/generate_ipc_types.sh` runs `datamodel-code-generator` to
//!    produce Pydantic models from the schema
//! 4. `scripts/check_ipc_type_drift.sh` detects when Rust types change
//!    without regenerating the Python side
//!
//! ## Adding new IPC types
//!
//! When you add a new Rust type that will be serialized to JSON and
//! deserialized by Python Pydantic models:
//!
//! 1. Add `schemars::JsonSchema` to the derive block
//! 2. Add a `register!()` call in [`generate_ipc_schema()`] below
//! 3. Run `bash scripts/generate_ipc_types.sh`

use std::collections::BTreeMap;
use std::path::Path;

use schemars::Schema;

/// Helper macro to reduce boilerplate when registering types.
///
/// Uses the schema title (type name without module path) as the map key,
/// falling back to the last segment of `stringify!($ty)` if no title is set.
macro_rules! register {
    ($map:expr, $($ty:ty),+ $(,)?) => {
        $(
            let schema = schemars::schema_for!($ty);
            let name = schema
                .get("title")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
                .unwrap_or_else(|| {
                    let full = stringify!($ty);
                    full.rsplit("::").next().unwrap_or(full).to_string()
                });
            $map.insert(name, schema);
        )+
    };
}

/// Generate JSON Schema for all IPC-boundary types, grouped by protocol layer.
///
/// The `worker_v2` layer name is intentional: it matches the live typed worker
/// namespace across Rust, `schemas/ipc/worker_v2`, and
/// `python/batchalign/generated/worker_v2` while the frozen V1 worker surface still
/// exists beside it.
///
/// Returns a map of `layer_name → { type_name → Schema }`.
pub fn generate_ipc_schema() -> BTreeMap<String, BTreeMap<String, Schema>> {
    let mut layers: BTreeMap<String, BTreeMap<String, Schema>> = BTreeMap::new();

    // --- Live typed worker protocol types (layer name retained while V1 remains) ---
    {
        use crate::types::worker_v2::*;

        let mut v2 = BTreeMap::new();
        register!(
            v2,
            // Envelope
            ExecuteRequestV2,
            ExecuteResponseV2,
            ExecuteOutcomeV2,
            // Task requests (discriminated union)
            TaskRequestV2,
            ForcedAlignmentRequestV2,
            AsrRequestV2,
            SpeakerRequestV2,
            MorphosyntaxRequestV2,
            UtsegRequestV2,
            TranslateRequestV2,
            CorefRequestV2,
            OpenSmileRequestV2,
            AvqiRequestV2,
            // Task results (discriminated union)
            TaskResultV2,
            IndexedWordTimingResultV2,
            MonologueAsrResultV2,
            WhisperChunkResultV2,
            WhisperTokenTimingResultV2,
            SpeakerResultV2,
            MorphosyntaxResultV2,
            MorphosyntaxItemResultV2,
            UtsegResultV2,
            UtsegItemResultV2,
            TranslationResultV2,
            TranslationItemResultV2,
            CorefResultV2,
            CorefItemResultV2,
            CorefAnnotationV2,
            CorefChainRefV2,
            OpenSmileResultV2,
            AvqiResultV2,
            // Prepared batches
            InlineJsonRefV2,
            PreparedAudioRefV2,
            PreparedTextRefV2,
            PreparedAudioInputV2,
            SpeakerPreparedAudioInputV2,
            // Inputs
            AsrInputV2,
            SpeakerInputV2,
            ProviderMediaInputV2,
            SubmittedJobInputV2,
            // Artifacts
            ArtifactRefV2,
            // Sub-types
            AsrElementV2,
            AsrElementKindV2,
            AsrMonologueV2,
            IndexedWordTimingV2,
            WhisperChunkSpanV2,
            WhisperTokenTimingV2,
            SpeakerSegmentV2,
            ProgressEventV2,
            // Enums
            WorkerKindV2,
            InferenceTaskV2,
            AsrBackendV2,
            FaBackendV2,
            FaTextModeV2,
            SpeakerBackendV2,
            PreparedAudioEncodingV2,
            PreparedTextEncodingV2,
            ProtocolErrorCodeV2,
            WorkerAttachmentKindV2,
            // Handshake
            HelloRequestV2,
            HelloResponseV2,
            CapabilitiesRequestV2,
            CapabilitiesResponseV2,
            TaskCapabilityV2,
            ShutdownRequestV2,
            WorkerRuntimeInfoV2,
        );
        layers.insert("worker_v2".into(), v2);
    }

    // --- Batch item types (chat-ops) ---
    {
        let mut batch = BTreeMap::new();
        register!(
            batch,
            crate::chat_ops::morphosyntax_ops::MorphosyntaxBatchItem,
            talkbank_transform::utseg::UtsegBatchItem,
            talkbank_transform::translate::TranslateBatchItem,
            talkbank_transform::coref::CorefBatchItem,
            talkbank_transform::coref::CorefAnnotation,
            talkbank_transform::coref::ChainRef,
        );
        layers.insert("batch_items".into(), batch);
    }

    layers
}

/// Write IPC schemas to a directory structure: `{dir}/{layer}/{type}.json`.
pub fn write_ipc_schema(
    schemas: &BTreeMap<String, BTreeMap<String, Schema>>,
    dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(dir);
    for (layer, types) in schemas {
        let layer_dir = base.join(layer);
        std::fs::create_dir_all(&layer_dir)?;
        for (type_name, schema) in types {
            let path = layer_dir.join(format!("{type_name}.json"));
            let json = serde_json::to_string_pretty(schema)?;
            std::fs::write(&path, json)?;
        }
    }
    let total: usize = schemas.values().map(|v| v.len()).sum();
    eprintln!(
        "Wrote {} schemas across {} layers to {}",
        total,
        schemas.len(),
        dir
    );
    Ok(())
}

/// Check that existing schema files match the current Rust types.
pub fn check_ipc_schema(
    schemas: &BTreeMap<String, BTreeMap<String, Schema>>,
    dir: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let base = Path::new(dir);
    let mut drift = Vec::new();

    for (layer, types) in schemas {
        let layer_dir = base.join(layer);
        for (type_name, schema) in types {
            let path = layer_dir.join(format!("{type_name}.json"));
            let expected = serde_json::to_string_pretty(schema)?;
            match std::fs::read_to_string(&path) {
                Ok(actual) if actual.trim() == expected.trim() => {}
                Ok(_) => drift.push(format!("  CHANGED: {}", path.display())),
                Err(_) => drift.push(format!("  MISSING: {}", path.display())),
            }
        }
    }

    if drift.is_empty() {
        Ok(())
    } else {
        Err(format!(
            "IPC schema drift detected ({} files):\n{}\n\nRun: bash scripts/generate_ipc_types.sh",
            drift.len(),
            drift.join("\n")
        )
        .into())
    }
}
