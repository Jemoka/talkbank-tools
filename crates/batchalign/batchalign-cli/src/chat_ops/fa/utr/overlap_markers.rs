//! Overlap marker extraction for CA-aware UTR windowing.
//!
//! Re-exports [`OverlapMarkerInfo`] and [`extract_overlap_info`] from
//! `talkbank-model`, where the generic CHAT traversal logic lives.
//! This module exists for backward compatibility with callers in
//! batchalign-chat-ops.

pub use talkbank_model::alignment::helpers::overlap::{OverlapMarkerInfo, extract_overlap_info};
