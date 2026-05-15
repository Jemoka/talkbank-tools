//! V2 execute dispatch helpers — task mapping and engine override resolution.
//!
//! These pure functions bridge the V2 execute protocol (typed per-backend
//! requests) to the pool's worker-key abstraction (bootstrap target + lang +
//! engine overrides). Extracted from `mod.rs` for browsability.

use crate::api::WorkerLanguage;
use crate::types::worker_v2::{
    AsrBackendV2, ExecuteRequestV2, FaBackendV2, InferenceTaskV2, TaskRequestV2,
};
use crate::worker::error::WorkerError;
use crate::worker::{InferTask, WorkerBootstrapMode, WorkerTarget};

/// Map a V2 inference task enum to the pool's infer-task vocabulary.
pub(super) fn infer_task_for_execute_v2(task: InferenceTaskV2) -> Result<InferTask, WorkerError> {
    match task {
        InferenceTaskV2::Morphosyntax => Ok(InferTask::Morphosyntax),
        InferenceTaskV2::Utseg => Ok(InferTask::Utseg),
        InferenceTaskV2::Translate => Ok(InferTask::Translate),
        InferenceTaskV2::Coref => Ok(InferTask::Coref),
        InferenceTaskV2::Asr => Ok(InferTask::Asr),
        InferenceTaskV2::ForcedAlignment => Ok(InferTask::Fa),
        InferenceTaskV2::Speaker => Ok(InferTask::Speaker),
        InferenceTaskV2::Opensmile => Ok(InferTask::Opensmile),
        InferenceTaskV2::Avqi => Ok(InferTask::Avqi),
    }
}

/// Derive the worker-pool key (target, lang, engine overrides) for one V2
/// execute request.
pub(super) fn execute_v2_worker_key(
    lang: WorkerLanguage,
    request: &ExecuteRequestV2,
    default_engine_overrides: &str,
    bootstrap_mode: WorkerBootstrapMode,
) -> Result<(WorkerTarget, WorkerLanguage, String), WorkerError> {
    let infer_task = infer_task_for_execute_v2(request.task)?;
    let target = WorkerTarget::from_infer_task(infer_task, bootstrap_mode);

    // In LazyProfile mode, all GPU tasks for a language share ONE worker
    // process. Engine overrides are applied via ensure_task IPC, not by
    // creating separate workers per override. This prevents the memory guard
    // deadlock where pre-scale creates key "" and FA dispatch looks for
    // {"fa":"wave2vec"} (a user incident 2026-04-02).
    let engine_overrides = if bootstrap_mode == WorkerBootstrapMode::LazyProfile
        && target.is_concurrent()
    {
        String::new()
    } else {
        execute_v2_engine_overrides(request).unwrap_or_else(|| default_engine_overrides.to_owned())
    };

    Ok((target, lang.clone(), engine_overrides))
}

/// Extract backend-specific engine override JSON from a V2 execute request.
pub(super) fn execute_v2_engine_overrides(request: &ExecuteRequestV2) -> Option<String> {
    match &request.payload {
        TaskRequestV2::Asr(request) => asr_backend_override_name(request.backend)
            .map(|backend| format!(r#"{{"asr":"{backend}"}}"#)),
        TaskRequestV2::ForcedAlignment(request) => Some(format!(
            r#"{{"fa":"{}"}}"#,
            fa_backend_override_name(request.backend)
        )),
        _ => None,
    }
}

/// Extract the ensure_task parameters (task name + engine overrides map) from a
/// V2 execute request, without the JSON round-trip.
///
/// Returns `None` for tasks that don't need model loading (e.g., text tasks in
/// eager-profile mode). Used by the LazyProfile dispatch path.
pub(super) fn ensure_task_params(
    request: &ExecuteRequestV2,
) -> Result<(String, Option<std::collections::BTreeMap<String, String>>), WorkerError> {
    let task = infer_task_for_execute_v2(request.task)?;
    let task_name = crate::worker::target::task_name(task).to_string();

    let overrides = match &request.payload {
        TaskRequestV2::Asr(req) => asr_backend_override_name(req.backend).map(|name| {
            let mut map = std::collections::BTreeMap::new();
            map.insert("asr".to_owned(), name.to_owned());
            map
        }),
        TaskRequestV2::ForcedAlignment(req) => {
            let mut map = std::collections::BTreeMap::new();
            map.insert(
                "fa".to_owned(),
                fa_backend_override_name(req.backend).to_owned(),
            );
            Some(map)
        }
        _ => None,
    };

    Ok((task_name, overrides))
}

fn asr_backend_override_name(backend: AsrBackendV2) -> Option<&'static str> {
    match backend {
        AsrBackendV2::LocalWhisper => Some("whisper"),
        AsrBackendV2::WhisperHub => Some("whisper_hub"),
        AsrBackendV2::HkTencent => Some("tencent"),
        AsrBackendV2::HkAliyun => Some("aliyun"),
        AsrBackendV2::HkFunaudio => Some("funaudio"),
        AsrBackendV2::Revai => None,
    }
}

fn fa_backend_override_name(backend: FaBackendV2) -> &'static str {
    match backend {
        FaBackendV2::Whisper => "whisper",
        FaBackendV2::Wave2vec => "wave2vec",
        FaBackendV2::Wav2vecCanto => "wav2vec_canto",
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::LanguageCode3;
    use crate::types::worker_v2::{
        AsrInputV2, AsrRequestV2, FaTextModeV2, ForcedAlignmentRequestV2, MorphosyntaxRequestV2,
        PreparedAudioInputV2, WorkerArtifactIdV2, WorkerRequestIdV2,
    };
    use crate::worker::{WorkerBootstrapMode, WorkerProfile, WorkerTarget};

    fn request_with_payload(task: InferenceTaskV2, payload: TaskRequestV2) -> ExecuteRequestV2 {
        ExecuteRequestV2 {
            request_id: WorkerRequestIdV2::from("req-1"),
            task,
            payload,
            attachments: Vec::new(),
        }
    }

    #[test]
    fn maps_forced_alignment_execute_v2_to_fa_worker_profile() {
        assert_eq!(
            infer_task_for_execute_v2(InferenceTaskV2::ForcedAlignment).unwrap(),
            InferTask::Fa
        );
    }

    #[test]
    fn execute_v2_asr_worker_key_uses_request_backend_override() {
        let request = request_with_payload(
            InferenceTaskV2::Asr,
            TaskRequestV2::Asr(AsrRequestV2 {
                lang: WorkerLanguage::from(LanguageCode3::fra()),
                backend: AsrBackendV2::LocalWhisper,
                input: AsrInputV2::PreparedAudio(PreparedAudioInputV2 {
                    audio_ref_id: WorkerArtifactIdV2::from("audio-1"),
                }),
            }),
        );

        let key = execute_v2_worker_key(
            WorkerLanguage::from(LanguageCode3::fra()),
            &request,
            r#"{"asr":"tencent"}"#,
            WorkerBootstrapMode::Profile,
        )
        .unwrap();

        assert_eq!(key.0, WorkerTarget::profile(WorkerProfile::Gpu));
        assert_eq!(key.1, WorkerLanguage::from(LanguageCode3::fra()));
        assert_eq!(key.2, r#"{"asr":"whisper"}"#);
    }

    #[test]
    fn execute_v2_fa_worker_key_uses_request_backend_override() {
        let request = request_with_payload(
            InferenceTaskV2::ForcedAlignment,
            TaskRequestV2::ForcedAlignment(ForcedAlignmentRequestV2 {
                backend: FaBackendV2::Wave2vec,
                payload_ref_id: WorkerArtifactIdV2::from("payload-1"),
                audio_ref_id: WorkerArtifactIdV2::from("audio-1"),
                text_mode: FaTextModeV2::SpaceJoined,
                pauses: false,
            }),
        );

        let key = execute_v2_worker_key(
            WorkerLanguage::from(LanguageCode3::eng()),
            &request,
            r#"{"fa":"whisper"}"#,
            WorkerBootstrapMode::Profile,
        )
        .unwrap();

        assert_eq!(key.0, WorkerTarget::profile(WorkerProfile::Gpu));
        assert_eq!(key.1, WorkerLanguage::from(LanguageCode3::eng()));
        assert_eq!(key.2, r#"{"fa":"wave2vec"}"#);
    }

    #[test]
    fn execute_v2_worker_key_uses_task_target_when_requested() {
        let request = request_with_payload(
            InferenceTaskV2::Morphosyntax,
            TaskRequestV2::Morphosyntax(MorphosyntaxRequestV2 {
                lang: LanguageCode3::eng(),
                payload_ref_id: WorkerArtifactIdV2::from("payload-1"),
                item_count: 1,
                retokenize: false,
            }),
        );

        let key = execute_v2_worker_key(
            WorkerLanguage::from(LanguageCode3::eng()),
            &request,
            "",
            WorkerBootstrapMode::Task,
        )
        .unwrap();

        assert_eq!(key.0, WorkerTarget::infer_task(InferTask::Morphosyntax));
    }

    #[test]
    fn lazy_profile_gpu_key_drops_engine_overrides() {
        // In LazyProfile mode, ALL GPU tasks for a language share one worker.
        // Engine overrides are loaded via ensure_task, not baked into the key.
        let fa_request = request_with_payload(
            InferenceTaskV2::ForcedAlignment,
            TaskRequestV2::ForcedAlignment(ForcedAlignmentRequestV2 {
                backend: FaBackendV2::Wave2vec,
                payload_ref_id: WorkerArtifactIdV2::from("payload-1"),
                audio_ref_id: WorkerArtifactIdV2::from("audio-1"),
                text_mode: FaTextModeV2::SpaceJoined,
                pauses: false,
            }),
        );

        let asr_request = request_with_payload(
            InferenceTaskV2::Asr,
            TaskRequestV2::Asr(AsrRequestV2 {
                lang: WorkerLanguage::from(LanguageCode3::eng()),
                backend: AsrBackendV2::LocalWhisper,
                input: AsrInputV2::PreparedAudio(PreparedAudioInputV2 {
                    audio_ref_id: WorkerArtifactIdV2::from("audio-1"),
                }),
            }),
        );

        let fa_key = execute_v2_worker_key(
            WorkerLanguage::from(LanguageCode3::eng()),
            &fa_request,
            "",
            WorkerBootstrapMode::LazyProfile,
        )
        .unwrap();

        let asr_key = execute_v2_worker_key(
            WorkerLanguage::from(LanguageCode3::eng()),
            &asr_request,
            "",
            WorkerBootstrapMode::LazyProfile,
        )
        .unwrap();

        // Both should use empty engine_overrides — same worker key.
        assert_eq!(fa_key.2, "");
        assert_eq!(asr_key.2, "");
        // Same target and language → same worker.
        assert_eq!(fa_key.0, asr_key.0);
        assert_eq!(fa_key.1, asr_key.1);
    }

    // -----------------------------------------------------------------------
    // Cross-check: dispatch_override_name() must agree with backend_override_name()
    // -----------------------------------------------------------------------

    #[test]
    fn fa_override_names_match_across_enum_families() {
        use crate::options::FaEngineName;

        assert_eq!(
            FaEngineName::Wave2Vec.dispatch_override_name(),
            fa_backend_override_name(FaBackendV2::Wave2vec),
        );
        assert_eq!(
            FaEngineName::Whisper.dispatch_override_name(),
            fa_backend_override_name(FaBackendV2::Whisper),
        );
        assert_eq!(
            FaEngineName::Wav2vecCanto.dispatch_override_name(),
            fa_backend_override_name(FaBackendV2::Wav2vecCanto),
        );
    }

    #[test]
    fn asr_override_names_match_across_enum_families() {
        use crate::options::AsrEngineName;

        assert_eq!(
            AsrEngineName::Whisper.dispatch_override_name(),
            asr_backend_override_name(AsrBackendV2::LocalWhisper),
        );
        assert_eq!(
            AsrEngineName::HkTencent.dispatch_override_name(),
            asr_backend_override_name(AsrBackendV2::HkTencent),
        );
        assert_eq!(
            AsrEngineName::HkAliyun.dispatch_override_name(),
            asr_backend_override_name(AsrBackendV2::HkAliyun),
        );
        assert_eq!(
            AsrEngineName::HkFunaudio.dispatch_override_name(),
            asr_backend_override_name(AsrBackendV2::HkFunaudio),
        );
        assert_eq!(
            AsrEngineName::RevAi.dispatch_override_name(),
            asr_backend_override_name(AsrBackendV2::Revai),
        );
    }
}
