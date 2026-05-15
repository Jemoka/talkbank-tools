//! Shared wire-format types for the batchalign3 Rust server and CLI.
//!
//! This module tree defines the data structures that cross serialization
//! boundaries -- REST API request/response models, server configuration,
//! worker IPC messages, and runtime constants -- as plain Rust structs with
//! `serde` `Serialize`/`Deserialize` derives.
//!
//! # Wire compatibility
//!
//! All types are **wire-compatible** with their Python counterparts
//! (`batchalign/serve/models.py`, `batchalign/serve/config.py`,
//! `batchalign/worker.py`).  Field names use
//! `snake_case` in both Rust and JSON, matching the Pydantic models exactly.
//! Enum variants serialize as lowercase strings via `#[serde(rename_all =
//! "lowercase")]` where the Python side expects it (e.g. [`api::JobStatus`]).
//!
//! This means a [`api::JobSubmission`] serialized by the Rust CLI can be
//! deserialized by the Python FastAPI server and vice versa, without any
//! translation layer.
//!
//! # Module position in the workspace
//!
//! ```text
//! batchalign-app::types     <-- this module tree
//!     |
//!     +-- worker IPC types
//!     +-- REST API models
//!     +-- runtime constants
//!     +-- scheduling/config types
//! ```
//!
//! Other modules in `batchalign-app` and the CLI depend on these definitions
//! for their wire contract surface.
//!
//! # Modules
//!
//! | Module      | Purpose |
//! |-------------|---------|
//! | [`api`]     | REST API request/response types (`JobSubmission`, `JobInfo`, `JobStatus`, `HealthResponse`, etc.) |
//! | [`config`]  | Server configuration (`ServerConfig`), YAML deserialization, warmup presets |
//! | [`runtime`] | Runtime constants: command-to-task mapping, memory budgets, worker caps |
//! | [`scheduling`] | Retry, attempt, failure-category, and work-unit domain types |
//! | [`worker`]  | Worker IPC types: `InferRequest`/`InferResponse`, batched infer requests, capabilities |
//! | [`worker_v2`] | Proposed next worker protocol schema with prepared-artifact descriptors |
//!
//! # Examples
//!
//! Deserializing a job submission from JSON (e.g. received by the server
//! from a CLI client):
//!
//! ```
//! use crate::api::JobSubmission;
//!
//! let json = r#"{
//!     "command": "morphotag",
//!     "lang": "eng",
//!     "files": [
//!         {"filename": "01DM_18.cha", "content": "@UTF8\n@Begin\n@End"}
//!     ],
//!     "options": {
//!         "command": "morphotag",
//!         "retokenize": false,
//!         "skipmultilang": false,
//!         "merge_abbrev": false
//!     }
//! }"#;
//!
//! let submission: JobSubmission = serde_json::from_str(json).unwrap();
//! assert_eq!(submission.command, "morphotag");
//! assert_eq!(submission.lang, "eng");
//! assert_eq!(submission.num_speakers, 1); // default
//! assert_eq!(submission.files.len(), 1);
//! assert!(submission.validate().is_ok());
//! ```
//!
//! Checking job status predicates:
//!
//! ```
//! use crate::api::JobStatus;
//!
//! let status = JobStatus::Running;
//! assert!(status.is_active());
//! assert!(status.can_cancel());
//! assert!(!status.is_terminal());
//!
//! let done = JobStatus::Completed;
//! assert!(done.is_terminal());
//! assert!(!done.can_restart());
//! ```
//!
//! Loading server configuration from a YAML string:
//!
//! ```
//! use crate::config::ServerConfig;
//!
//! let yaml = r#"
//! port: 9000
//! default_lang: spa
//! max_concurrent_jobs: 4
//! "#;
//!
//! let config: ServerConfig = serde_yaml::from_str(yaml).unwrap();
//! assert_eq!(config.port, 9000);
//! assert_eq!(config.default_lang, "spa");
//! ```

pub mod api;
pub mod cancellation;
pub mod config;
pub mod domain;
pub mod engines;
pub mod execution_plan;
pub mod options;
pub mod params;
pub mod request;
pub mod response;
pub mod results;
pub mod runtime;
pub mod scheduling;
pub mod status;
pub mod traces;
pub mod worker;
pub mod worker_v2;
