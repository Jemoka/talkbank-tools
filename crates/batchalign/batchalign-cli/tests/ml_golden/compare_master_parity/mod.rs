//! Compare parity tests backed by committed `batchalign2-master` outputs.
//!
//! These tests run the released BA3 compare command end-to-end in direct
//! paths mode, then compare both the emitted CHAT and the `.compare.csv`
//! sidecar against committed artifacts generated from live
//! `batchalign2-master` runs.

mod exact;
mod harness;
