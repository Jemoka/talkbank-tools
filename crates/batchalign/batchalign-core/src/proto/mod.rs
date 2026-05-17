//! Wire-protocol types crossing the worker boundary.
//!
//! !!!  THIS CONTRACT IS BRITTLE  !!!
//! Hand-mirrored with `python/batchalign/_core/proto.py`. Every edit MUST
//! happen in both places. The parity test in
//! `crates/batchalign-core/tests/proto_parity.rs` checks Python class
//! existence; field-level shape is on the contributor.
//!
//! See `spec2.md` §9 and §18.

pub mod asr;
pub mod avqi;
pub mod compare;
pub mod coref;
pub mod fa;
pub mod morphosyntax;
pub mod opensmile;
pub mod speaker;
pub mod translate;
pub mod utseg;

pub use asr::{AsrInput, AsrOptions, AsrOutput, AsrSegment, AsrWord, LanguageSpec};
pub use avqi::{AvqiInput, AvqiOutput};
pub use compare::{CompareInput, CompareOutput};
pub use coref::{CorefInput, CorefOutput};
pub use fa::{FaInput, FaOutput};
pub use morphosyntax::{
    MorphosyntaxInput, MorphosyntaxOutput, MorphosyntaxUtterance, MorphosyntaxToken,
    TaggedUtterance,
};
pub use opensmile::{OpenSmileInput, OpenSmileOutput};
pub use speaker::{Diarization, DiarizationSegment, SpeakerInput, SpeakerOutput};
pub use translate::{TranslateInput, TranslateOutput};
pub use utseg::{UtSegInput, UtSegOutput, UtteranceSpan};
