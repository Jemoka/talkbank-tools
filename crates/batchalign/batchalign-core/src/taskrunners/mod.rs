//! Concrete `TaskRunner` impls. Day-zero these are stubs returning
//! `BAError::Internal("not yet implemented")`; later phases fill them.
//!
//! See `spec2.md` §24 for the phasing.

pub mod ai;
pub mod asr;
pub mod compare;
pub mod coref;
pub mod fa;
pub mod morphosyntax;
pub mod speaker;
pub mod translate;
pub mod utr;
pub mod utseg;

use crate::base::DynTaskRunner;
use crate::base::Task;

/// Return the canonical (default) runner for a given `Task`.
///
/// The engine crate uses this to populate `Pipeline::runners` when the user
/// hasn't supplied a custom runner. Stub implementations live in the per-task
/// submodules above — they currently return `BAError::Internal("not yet
/// implemented")` for tasks whose runners haven't landed. See spec2.md §24
/// for the phasing.
pub fn canonical(task: Task) -> Box<dyn DynTaskRunner> {
    match task {
        Task::Ai => Box::new(ai::AiTaskRunner::default()),
        Task::Asr => Box::new(asr::AsrTaskRunner),
        Task::Fa => Box::new(fa::FaTaskRunner),
        Task::Speaker => Box::new(speaker::SpeakerTaskRunner),
        Task::UtSeg => Box::new(utseg::UtSegTaskRunner),
        Task::Morphosyntax => Box::new(morphosyntax::MorphosyntaxTaskRunner),
        Task::Translate => Box::new(translate::TranslateTaskRunner),
        Task::Utr => Box::new(utr::UtrTaskRunner),
        Task::Coref => Box::new(coref::CorefTaskRunner),
        Task::Compare => Box::new(compare::CompareTaskRunner),
    }
}
