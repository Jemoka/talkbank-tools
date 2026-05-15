//! Supertype matcher for linker token kinds in main-tier structure.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#QuotationFollows_Linker>

/// Check if a node kind is a `linker` subtype
///
/// **Subtypes:** linker_quick_uptake, linker_lazy_overlap, etc.
pub fn is_linker(kind: &str) -> bool {
    matches!(
        kind,
        "linker" |  // Keep for backwards compatibility
        "ca_no_break_linker" |
        "ca_technical_break_linker" |
        "linker_lazy_overlap" |
        "linker_quick_uptake" |
        "linker_quick_uptake_overlap" |
        "linker_quotation_follows" |
        "linker_self_completion"
    )
}
