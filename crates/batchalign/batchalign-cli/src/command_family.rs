//! Shared command family metadata.

/// High-level command family for one released command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WorkflowFamily {
    /// One file goes in, one primary output comes out.
    PerFileTransform,
    /// Many files go in, pooled work happens internally, then per-file outputs fan back out.
    CrossFileBatchTransform,
    /// Two artifacts are jointly primary and materialize from a comparison/projection bundle.
    ReferenceProjection,
    /// One command composes other command-owned flows rather than reimplementing them.
    Composite,
}
