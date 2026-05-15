//! L2 code-switching helpers for `@s`-marked words.

mod deprel;
mod extract;
mod merge;
mod plan;
mod spans;
mod splice;

pub use deprel::{
    PosConstraint, UdDeprel, deprel_to_pos_constraint, infer_deprel_from_pos,
    refine_with_dependents,
};
pub use extract::{L2DeferredPosition, PrimaryStructuralInfo, extract_l2_deferred_positions};
pub use merge::{
    MergedL2Morphology, SecondaryUdContext, merge_planned_secondary_span, merge_primary_secondary,
    merge_primary_secondary_with_context, resolve_merged_pos, resolve_merged_pos_with_context,
};
pub use plan::{
    L2Attachment, L2DispatchPlan, L2SpanPlan, build_l2_word_text_cache, plan_dispatch_spans,
    plan_secondary_dispatch,
};
pub use spans::{DispatchSpan, L2Span, group_deferred_into_dispatch_spans, group_l2_spans};
pub use splice::{SpliceOutcome, apply_l2_fallback, splice_l2_into_chat};
