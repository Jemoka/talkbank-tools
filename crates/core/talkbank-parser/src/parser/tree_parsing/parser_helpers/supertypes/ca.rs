//! Supertype matchers for CA marker and CA delimiter token kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Option>
//! - <https://talkbank.org/0info/manuals/CHAT.html#CA_Unicode_Option>

/// Check if a node kind is `ca_element`
///
/// After coarsening, ca_element is a single token leaf (no subtypes).
pub fn is_ca_element(kind: &str) -> bool {
    kind == "ca_element"
}

/// Check if a node kind is `ca_delimiter`
///
/// After coarsening, ca_delimiter is a single token leaf (no subtypes).
pub fn is_ca_delimiter(kind: &str) -> bool {
    kind == "ca_delimiter"
}
