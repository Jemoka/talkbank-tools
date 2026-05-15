//! Server configuration — mirrors `batchalign/serve/config.py`.
//!
//! Deserializes from the runtime-owned `server.yaml` under the resolved state
//! directory using serde_yaml.
//! No OmegaConf interpolation needed — plain YAML is sufficient.
//!
//! # Sub-modules
//!
//! | Module    | Purpose |
//! |-----------|---------|
//! | [`layout`]  | `RuntimeLayout` — filesystem path resolution from env/home |
//! | [`server`]  | `ServerConfig` struct, `FleetTarget`, serde defaults, warmup presets |
//! | [`resolve`] | `ServerConfig` methods: validation, Temporal backend, memory-tier resolution |
//! | [`load`]    | YAML loading helpers and `ConfigError` |

mod layout;
mod load;
mod resolve;
mod server;

#[cfg(test)]
mod tests;

// Re-export everything at the `config` module level for backwards compatibility.
// Callers use `crate::config::ServerConfig`, `crate::config::RuntimeLayout`, etc.
pub use layout::*;
pub use load::*;
pub use resolve::*;
pub use server::*;
