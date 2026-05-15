//! Shared domain and worker-boundary types for batchalign.
//!
//! This crate is the first step toward separating build/publish boundaries:
//! worker protocol, runtime language/domain scalars, and other wire-facing
//! identifiers should not live inside the server crate.

#[macro_use]
mod macros;

pub mod api {
    //! Backward-compatible re-export of domain types historically reached via
    //! `batchalign::api`.
    pub use crate::domain::*;
}

pub mod command_spec;
pub mod domain;
pub mod memory;
pub mod paths;
pub mod scheduling;
pub mod worker;
pub mod worker_profile;
pub mod worker_v2;
