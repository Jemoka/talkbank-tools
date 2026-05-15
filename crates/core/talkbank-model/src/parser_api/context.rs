use crate::ChatOptionFlag;

/// Semantic context supplied to fragment parsers.
///
/// Whole-file parsing derives these semantics from headers. Fragment parsing does
/// not have that information unless the caller provides it explicitly.
///
/// The context starts conservative and only carries semantics that are already
/// known to affect fragment interpretation. More fields can be added as more
/// file-level dependencies are formalized.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FragmentSemanticContext {
    /// Effective `@Options` flags for the fragment.
    pub option_flags: Vec<ChatOptionFlag>,
}

impl FragmentSemanticContext {
    /// Construct an empty fragment context.
    #[inline]
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace the option flags carried by this context.
    #[inline]
    pub fn with_option_flags(mut self, option_flags: Vec<ChatOptionFlag>) -> Self {
        self.option_flags = option_flags;
        self
    }

    /// Append one effective `@Options` flag.
    #[inline]
    pub fn with_option_flag(mut self, option_flag: ChatOptionFlag) -> Self {
        self.option_flags.push(option_flag);
        self
    }

    /// Return whether CA mode is enabled for this fragment.
    #[inline]
    pub fn ca_mode(&self) -> bool {
        self.option_flags
            .iter()
            .any(ChatOptionFlag::enables_ca_mode)
    }

    /// Return whether bullets mode is enabled for this fragment.
    ///
    /// Note: the `bullets` option was removed from CHAT. This always returns
    /// `false`. Retained for API compatibility during migration.
    #[inline]
    pub fn bullets_mode(&self) -> bool {
        false
    }
}
