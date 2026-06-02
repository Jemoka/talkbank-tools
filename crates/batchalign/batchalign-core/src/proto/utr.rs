//! Utterance Timing Recovery (UTR) proto types.
//!
//! UTR is "basically just ASR + a Rust-side post-processing strategy":
//! audio + language in, a flat list of timed tokens out. So the wire
//! payload is structurally identical to ASR's. We use serde-transparent
//! newtypes so:
//!
//! * The JSON bytes on the worker boundary are AsrInput / AsrOutput
//!   shaped — Python backends that already pattern-match on
//!   `isinstance(x, AsrInput)` work for UTR inputs unchanged (with a
//!   thin Python-side alias).
//! * The Rust types are nominally distinct, so the `union_input_output!`
//!   macro can mint a `From<UtrInput> for TaskInput` without colliding
//!   with the existing `From<AsrInput>`.
//!
//! The taskrunner does cache namespacing locally (prefixing keys with
//! `"utr_asr"`), so UTR and ASR don't collide in the validation cache
//! even when they hit the same backend with the same audio.

use crate::cache::CacheKey;
use crate::proto::asr::{AsrInput, AsrOutput};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::ops::Deref;

/// What the UTR runner ships to its backend.
///
/// Structurally an `AsrInput` on the wire (serde-transparent); a Rust-side
/// newtype so the closed-union macro can mint a non-colliding `From`.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct UtrInput(pub AsrInput);

impl UtrInput {
    /// Borrow the underlying ASR input.
    pub fn as_asr(&self) -> &AsrInput {
        &self.0
    }

    /// Consume into the underlying ASR input.
    pub fn into_asr(self) -> AsrInput {
        self.0
    }
}

impl From<AsrInput> for UtrInput {
    fn from(i: AsrInput) -> Self {
        UtrInput(i)
    }
}

// Deref forwarding lets the closed-union macro's `i.source_id` field
// access resolve transparently to the wrapped `AsrInput` field.
impl Deref for UtrInput {
    type Target = AsrInput;
    fn deref(&self) -> &AsrInput {
        &self.0
    }
}

impl CacheKey for UtrInput {
    /// Namespaced cache key: prefixes the AsrInput hash with `"utr_asr"`
    /// so UTR runs don't collide with ASR runs over the same audio +
    /// language + options. Mirrors tbtbt's `utr_asr_cache_key`
    /// (`crates/batchalign/src/runner/dispatch/utr.rs`).
    fn hash(&self, hasher: &mut blake3::Hasher) {
        hasher.update(b"utr_asr");
        self.0.hash(hasher);
    }
}

/// What a UTR backend returns. Structurally an `AsrOutput` on the wire.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(transparent)]
pub struct UtrOutput(pub AsrOutput);

impl UtrOutput {
    /// Borrow the underlying ASR output.
    pub fn as_asr(&self) -> &AsrOutput {
        &self.0
    }

    /// Consume into the underlying ASR output.
    pub fn into_asr(self) -> AsrOutput {
        self.0
    }
}

impl From<AsrOutput> for UtrOutput {
    fn from(o: AsrOutput) -> Self {
        UtrOutput(o)
    }
}

impl Deref for UtrOutput {
    type Target = AsrOutput;
    fn deref(&self) -> &AsrOutput {
        &self.0
    }
}
