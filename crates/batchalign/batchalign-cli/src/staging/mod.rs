//! Staged remote execution: stage inputs to a remote host, execute there,
//! copy results back.
//!
//! This module implements the "local submission + remote execution" pattern
//! for machines that are too weak to process heavy ML jobs locally (e.g.
//! a user's 32 GB Mac). The flow:
//!
//! 1. **Prepare** — resolve media files, build a local staging directory
//! 2. **Stage** — rsync the staging dir to the remote host's scratch space
//! 3. **Execute** — submit a job to the remote server's REST API
//! 4. **Copy back** — rsync results from remote scratch to local output paths
//!
//! The orchestrator ties these steps together and updates the local
//! `JobStore` with progress so the dashboard shows one cohesive job.

pub mod orchestrator;
pub mod prepare;
pub mod rsync;

pub use orchestrator::run_staged_remote_job;
pub use rsync::{RemoteStagingResult, StagingError, copy_back_results, stage_inputs};
