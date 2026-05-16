//! `CallbackSink` — concrete `ProgressSink` impl that routes events to
//! per-source Python callbacks.
//!
//! Core defines the `ProgressSink` trait (`batchalign_core::progress`); this
//! crate provides the only non-trivial impl: each event is dispatched to the
//! `Py<PyAny>` callback registered for its `source_id`. The trait stays in core
//! so runners can depend on the abstraction without dragging PyO3 into core's
//! non-`python` builds.

use std::collections::HashMap;

use batchalign_core::{ProgressEvent, ProgressSink, SourceId};
use pyo3::prelude::*;

#[derive(Default)]
pub struct CallbackSink {
    by_source: HashMap<SourceId, Py<PyAny>>,
}

impl CallbackSink {
    pub fn new() -> Self {
        Self { by_source: HashMap::new() }
    }

    pub fn from_pairs(pairs: Vec<(SourceId, Py<PyAny>)>) -> Self {
        let mut by_source = HashMap::with_capacity(pairs.len());
        for (sid, cb) in pairs {
            by_source.insert(sid, cb);
        }
        Self { by_source }
    }
}

impl ProgressSink for CallbackSink {
    fn emit(&self, event: ProgressEvent) {
        let Some(cb) = self.by_source.get(&event.source_id) else {
            return;
        };
        Python::attach(|py| {
            if let Err(err) = cb.bind(py).call1((event.clone(),)) {
                // A buggy progress callback must not poison the run.
                err.print_and_set_sys_last_vars(py);
            }
        });
    }
}
