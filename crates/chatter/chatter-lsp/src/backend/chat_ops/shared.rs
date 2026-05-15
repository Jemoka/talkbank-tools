//! Document-loading adapter for `chat_ops/*` handlers.
//!
//! Re-exports the shared [`chat_file_cache`] loader under the name
//! historically used by chat_ops submodules. Kept as a one-line
//! `pub(super) use` rather than collapsed so the four handler files
//! (`filter_document`, `scoped_find`, `speakers`, `utterances`) can
//! reference `super::shared::get_document_and_chat_file` uniformly;
//! callers elsewhere in the backend should use
//! [`chat_file_cache::load_document_and_chat_file`] directly.

pub(super) use crate::backend::chat_file_cache::load_document_and_chat_file as get_document_and_chat_file;
