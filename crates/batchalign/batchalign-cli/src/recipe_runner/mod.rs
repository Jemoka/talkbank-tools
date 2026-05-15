//! Recipe-driven command architecture spike.
//!
//! This namespace is intentionally parallel to the legacy `workflow/` +
//! `runner/dispatch/` stack. It captures the new organizing model without
//! forcing an all-at-once migration.

pub(crate) mod catalog;
pub(crate) mod command_spec;
pub(crate) mod materialize;
pub(crate) mod planner;
pub(crate) mod recipe;
pub(crate) mod runtime;
pub(crate) mod work_unit;
