//! Thread-local parser and semantic-token services for the LSP backend.
//!
//! The backend previously shared a single parser and semantic-tokens provider
//! behind global mutexes. Both resources are inherently thread-confined
//! (`tree_sitter::Parser` is not `Sync`, and the highlighter keeps mutable
//! internal state), so this module replaces those mutexes with lazily
//! initialized thread-local instances and a small explicit service API.

use std::cell::RefCell;

use talkbank_parser::TreeSitterParser;

use crate::semantic_tokens::SemanticTokensProvider;

use super::LspBackendError;
use super::state::BackendInitError;

thread_local! {
    /// Thread-local tree-sitter parser used by synchronous parser entry points.
    static CHAT_PARSER: RefCell<Option<Result<TreeSitterParser, BackendInitError>>> = const {
        RefCell::new(None)
    };
    /// Thread-local semantic-tokens provider used by token requests.
    static SEMANTIC_TOKENS: RefCell<Option<Result<SemanticTokensProvider, BackendInitError>>> = const {
        RefCell::new(None)
    };
}

/// Lazily initialized thread-local language services used by the LSP backend.
#[derive(Clone, Default)]
pub(crate) struct LanguageServices;

impl LanguageServices {
    /// Create a new language-services façade.
    pub(crate) fn new() -> Self {
        Self
    }

    /// Execute a closure with the current thread's CHAT parser.
    pub(crate) fn with_parser<T>(
        &self,
        callback: impl FnOnce(&TreeSitterParser) -> T,
    ) -> Result<T, BackendInitError> {
        CHAT_PARSER.with(|slot| {
            initialize_parser(slot);
            let parser = slot.borrow();

            // `initialize_parser` writes `Some(...)` on first call;
            // subsequent calls observe `Some(...)` from the same slot.
            // The expect here is therefore infallible by construction —
            // dropping it would require a `OnceCell` or `LazyLock`
            // refactor of the thread-local slot, tracked as a
            // follow-up in `book/src/operations/panic-audit/chatter-lsp.md`.
            #[allow(clippy::expect_used)]
            match parser.as_ref().expect("parser slot initialized") {
                Ok(parser) => Ok(callback(parser)),
                Err(error) => Err(error.clone()),
            }
        })
    }

    /// Execute a closure with the current thread's semantic-tokens provider.
    ///
    /// The closure returns the typed `LspBackendError` so call sites can
    /// `match` on variants. The init-failure branch surfaces through the
    /// same error type via [`LspBackendError::HighlightFailed`] so callers
    /// see a single error surface regardless of whether the failure came
    /// from grammar init or token extraction.
    pub(crate) fn with_semantic_tokens_provider<T>(
        &self,
        callback: impl FnOnce(&mut SemanticTokensProvider) -> Result<T, LspBackendError>,
    ) -> Result<T, LspBackendError> {
        SEMANTIC_TOKENS.with(|slot| {
            initialize_semantic_tokens(slot);
            let mut provider = slot.borrow_mut();

            // Same invariant as `with_parser` above: the slot is
            // populated unconditionally by `initialize_semantic_tokens`
            // before this read. Tracked under `OnceCell` migration.
            #[allow(clippy::expect_used)]
            match provider.as_mut().expect("semantic-tokens slot initialized") {
                Ok(provider) => callback(provider),
                Err(init_error) => Err(LspBackendError::HighlightFailed {
                    reason: init_error.to_string(),
                }),
            }
        })
    }
}

/// Initialize the current thread's parser slot on first use.
fn initialize_parser(slot: &RefCell<Option<Result<TreeSitterParser, BackendInitError>>>) {
    if slot.borrow().is_none() {
        let parser =
            TreeSitterParser::new().map_err(|error| BackendInitError::Parser(format!("{error:?}")));
        *slot.borrow_mut() = Some(parser);
    }
}

/// Initialize the current thread's semantic-tokens slot on first use.
fn initialize_semantic_tokens(
    slot: &RefCell<Option<Result<SemanticTokensProvider, BackendInitError>>>,
) {
    if slot.borrow().is_none() {
        // `SemanticTokensProvider::new` now returns the typed
        // `LspBackendError`; stringify once at this boundary so the
        // existing `BackendInitError::SemanticTokens(String)` variant
        // keeps its wire shape.
        let provider = SemanticTokensProvider::new()
            .map_err(|err| BackendInitError::SemanticTokens(err.to_string()));
        *slot.borrow_mut() = Some(provider);
    }
}

#[cfg(test)]
mod tests {
    //! Unit tests for thread-local language services.

    use super::LanguageServices;

    /// Parser access should succeed repeatedly on the same thread.
    #[test]
    fn parser_service_parses_repeatedly() {
        let services = LanguageServices::new();
        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";

        let first = services
            .with_parser(|parser| parser.parse_chat_file(text).is_ok())
            .expect("parser should initialize");
        let second = services
            .with_parser(|parser| parser.parse_chat_file(text).is_ok())
            .expect("parser should remain available");

        assert!(first);
        assert!(second);
    }

    /// Semantic-token access should succeed repeatedly on the same thread.
    #[test]
    fn semantic_tokens_service_highlights_repeatedly() {
        let services = LanguageServices::new();
        let text = "@UTF8\n@Begin\n*CHI:\thello .\n@End\n";

        let first = services
            .with_semantic_tokens_provider(|provider| provider.semantic_tokens_full(text))
            .expect("semantic-tokens provider should initialize");
        let second = services
            .with_semantic_tokens_provider(|provider| {
                provider.semantic_tokens_range(text, 0, text.len())
            })
            .expect("semantic-tokens provider should remain available");

        assert!(!first.is_empty());
        assert!(!second.is_empty());
    }
}
