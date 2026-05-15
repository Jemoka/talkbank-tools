//! Rust-side request builders for batched text worker-protocol V2 execution.
//!
//! Text tasks are performance-sensitive because Rust pools cache misses across
//! files into one worker call. The V2 transport must preserve that property, so
//! each request builder here freezes an entire batch into one prepared-text
//! artifact and returns one typed `execute_v2` envelope.

use std::sync::atomic::{AtomicU64, Ordering};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::api::LanguageCode3;
use crate::chat_ops::morphosyntax_ops::{MorphosyntaxBatchItem, MwtDict};
use crate::types::worker_v2::{
    ArtifactRefV2, CorefRequestV2, ExecuteRequestV2, InferenceTaskV2, MorphosyntaxRequestV2,
    TaskRequestV2, TranslateRequestV2, UtsegRequestV2, WorkerArtifactIdV2, WorkerRequestIdV2,
};
use talkbank_transform::coref::CorefBatchItem;
use talkbank_transform::translate::TranslateBatchItem;
use talkbank_transform::utseg::UtsegBatchItem;

use super::artifacts_v2::PreparedArtifactStoreV2;

/// Monotonic sequence used to make prepared-text request ids unique enough for
/// concurrent runtime use.
static TEXT_REQUEST_SEQUENCE_V2: AtomicU64 = AtomicU64::new(1);

/// Stable ids for one batched prepared-text V2 request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedTextRequestIdsV2 {
    /// Top-level request id for the worker envelope.
    pub request_id: WorkerRequestIdV2,
    /// Artifact id for the prepared text payload.
    pub payload_ref_id: WorkerArtifactIdV2,
}

impl PreparedTextRequestIdsV2 {
    /// Construct explicit ids for one prepared-text V2 request.
    pub fn new(
        request_id: impl Into<WorkerRequestIdV2>,
        payload_ref_id: impl Into<WorkerArtifactIdV2>,
    ) -> Self {
        Self {
            request_id: request_id.into(),
            payload_ref_id: payload_ref_id.into(),
        }
    }

    /// Construct unique-enough ids for one task-local prepared-text V2 request.
    pub fn for_task(task: &str) -> Self {
        let sequence = TEXT_REQUEST_SEQUENCE_V2.fetch_add(1, Ordering::Relaxed);
        Self::new(
            format!("{task}-v2-request-{sequence}"),
            format!("{task}-v2-payload-{sequence}"),
        )
    }
}

/// Prepared morphosyntax batch payload written by Rust.
#[derive(Clone, Serialize, Deserialize)]
pub struct PreparedMorphosyntaxBatchV2 {
    /// Batched utterance payloads in worker order.
    pub items: Vec<MorphosyntaxBatchItem>,
    /// Multi-word token lexicon shared across the batch.
    #[serde(default, skip_serializing_if = "std::collections::BTreeMap::is_empty")]
    pub mwt: MwtDict,
}

/// Prepared utterance-segmentation batch payload written by Rust.
#[derive(Clone, Serialize, Deserialize)]
pub struct PreparedUtsegBatchV2 {
    /// Batched utterance payloads in worker order.
    pub items: Vec<UtsegBatchItem>,
}

/// Prepared translation batch payload written by Rust.
#[derive(Clone, Serialize, Deserialize)]
pub struct PreparedTranslateBatchV2 {
    /// Batched utterance payloads in worker order.
    pub items: Vec<TranslateBatchItem>,
}

/// Prepared coreference batch payload written by Rust.
#[derive(Clone, Serialize, Deserialize)]
pub struct PreparedCorefBatchV2 {
    /// Batched document payloads in worker order.
    pub items: Vec<CorefBatchItem>,
}

/// Errors produced while building one batched prepared-text V2 request.
#[derive(Debug, Error)]
pub enum TextRequestBuildErrorV2 {
    /// The batch size exceeded the V2 request field range.
    #[error("worker protocol V2 text batch has {count} items, which exceeds the supported range")]
    ItemCountOverflow {
        /// Number of items Rust attempted to freeze into the prepared payload.
        count: usize,
    },

    /// Prepared-text artifact creation failed.
    #[error("failed to write worker protocol V2 prepared text artifact: {0}")]
    Artifact(#[from] std::io::Error),
}

/// Build a batched morphosyntax V2 request.
pub fn build_morphosyntax_request_v2(
    store: &PreparedArtifactStoreV2,
    ids: &PreparedTextRequestIdsV2,
    lang: &LanguageCode3,
    items: &[MorphosyntaxBatchItem],
    mwt: &MwtDict,
    retokenize: bool,
) -> Result<ExecuteRequestV2, TextRequestBuildErrorV2> {
    let payload = PreparedMorphosyntaxBatchV2 {
        items: items.to_vec(),
        mwt: mwt.clone(),
    };
    let attachment = store.write_prepared_text_json(&ids.payload_ref_id, &payload)?;
    Ok(ExecuteRequestV2 {
        request_id: ids.request_id.clone(),
        task: InferenceTaskV2::Morphosyntax,
        payload: TaskRequestV2::Morphosyntax(MorphosyntaxRequestV2 {
            lang: lang.clone(),
            payload_ref_id: attachment.id.clone(),
            item_count: item_count(items.len())?,
            retokenize,
        }),
        attachments: vec![ArtifactRefV2::PreparedText(attachment)],
    })
}

/// Build a batched utterance-segmentation V2 request.
///
/// `allow_stanza_fallback` is the operator opt-in to the legacy Stanza
/// constituency-parser fallback for languages without a TalkBank BERT
/// utseg model. Surfaced as the `--utseg-fallback-stanza` CLI flag.
pub fn build_utseg_request_v2(
    store: &PreparedArtifactStoreV2,
    ids: &PreparedTextRequestIdsV2,
    lang: &LanguageCode3,
    items: &[UtsegBatchItem],
    allow_stanza_fallback: bool,
) -> Result<ExecuteRequestV2, TextRequestBuildErrorV2> {
    let payload = PreparedUtsegBatchV2 {
        items: items.to_vec(),
    };
    let attachment = store.write_prepared_text_json(&ids.payload_ref_id, &payload)?;
    Ok(ExecuteRequestV2 {
        request_id: ids.request_id.clone(),
        task: InferenceTaskV2::Utseg,
        payload: TaskRequestV2::Utseg(UtsegRequestV2 {
            lang: lang.clone(),
            payload_ref_id: attachment.id.clone(),
            item_count: item_count(items.len())?,
            allow_stanza_fallback,
        }),
        attachments: vec![ArtifactRefV2::PreparedText(attachment)],
    })
}

/// Build a batched translation V2 request.
pub fn build_translate_request_v2(
    store: &PreparedArtifactStoreV2,
    ids: &PreparedTextRequestIdsV2,
    source_lang: &LanguageCode3,
    target_lang: &LanguageCode3,
    items: &[TranslateBatchItem],
) -> Result<ExecuteRequestV2, TextRequestBuildErrorV2> {
    let payload = PreparedTranslateBatchV2 {
        items: items.to_vec(),
    };
    let attachment = store.write_prepared_text_json(&ids.payload_ref_id, &payload)?;
    Ok(ExecuteRequestV2 {
        request_id: ids.request_id.clone(),
        task: InferenceTaskV2::Translate,
        payload: TaskRequestV2::Translate(TranslateRequestV2 {
            source_lang: source_lang.clone(),
            target_lang: target_lang.clone(),
            payload_ref_id: attachment.id.clone(),
            item_count: item_count(items.len())?,
        }),
        attachments: vec![ArtifactRefV2::PreparedText(attachment)],
    })
}

/// Build a batched coreference V2 request.
pub fn build_coref_request_v2(
    store: &PreparedArtifactStoreV2,
    ids: &PreparedTextRequestIdsV2,
    lang: &LanguageCode3,
    items: &[CorefBatchItem],
) -> Result<ExecuteRequestV2, TextRequestBuildErrorV2> {
    let payload = PreparedCorefBatchV2 {
        items: items.to_vec(),
    };
    let attachment = store.write_prepared_text_json(&ids.payload_ref_id, &payload)?;
    Ok(ExecuteRequestV2 {
        request_id: ids.request_id.clone(),
        task: InferenceTaskV2::Coref,
        payload: TaskRequestV2::Coref(CorefRequestV2 {
            lang: lang.clone(),
            payload_ref_id: attachment.id.clone(),
            item_count: item_count(items.len())?,
        }),
        attachments: vec![ArtifactRefV2::PreparedText(attachment)],
    })
}

/// Convert one Rust batch length into the V2 request field range.
fn item_count(count: usize) -> Result<u32, TextRequestBuildErrorV2> {
    u32::try_from(count).map_err(|_| TextRequestBuildErrorV2::ItemCountOverflow { count })
}
