//! Parse → validate → publish diagnostics pipeline.
//!
//! Orchestrates the full lifecycle on each `didOpen` / `didChange`:
//!
//! 1. Apply text edit to the cached tree-sitter CST (incremental re-parse).
//! 2. Determine which utterances were affected via [`detect_utterance_splice`]
//!    and [`affected_utterance_indices`].
//! 3. Re-parse affected utterances into `ChatFile` lines, splice into the
//!    cached `ChatFile`, and re-validate only the changed utterances.
//! 4. Convert `ParseError`s to LSP `Diagnostic`s and publish them.
//!
//! Falls back to full re-parse when context-affecting headers change or when
//! the incremental path cannot handle the edit (e.g. multi-utterance deletes).

use super::super::incremental::{
    affected_utterance_indices, collect_utterance_line_indices,
    collect_utterances_and_header_changes, detect_utterance_splice,
};
use super::super::language_services::LanguageServices;
use super::super::state::BackendInitError;
use super::super::validation_cache::ValidationCache;
use super::cache_builder::{
    build_validation_cache, build_validation_cache_reuse_headers, compute_bullet_errors,
    compute_scoped_errors, validate_single_utterance,
};
use super::conversion::{to_diagnostics_batch, to_diagnostics_batch_with_context};
use super::text_diff::{compute_text_changed_range, compute_text_diff_span};
use dashmap::DashMap;
use std::sync::Arc;
use talkbank_model::model::{ChatFile, Line};
use talkbank_model::{ErrorCollector, Severity};
use tower_lsp::Client;
use tower_lsp::lsp_types::*;
use tracing::debug;
use tree_sitter::Tree;

/// Validate and publish diagnostics using incremental parsing when possible.
///
/// Also caches the parsed `ChatFile` for use by other features (hover, completion, etc.).
#[derive(Clone, Copy)]
pub(crate) struct ValidationResources<'a> {
    /// Shared LSP client for publishing diagnostics.
    pub client: &'a Client,
    /// Thread-local language services used during parsing and highlighting.
    pub language_services: &'a LanguageServices,
    /// Cached tree-sitter parse trees per document URI.
    pub parse_trees: &'a Arc<DashMap<Url, Tree>>,
    /// Cached `ChatFile` ASTs per document URI.
    pub chat_files: &'a Arc<DashMap<Url, Arc<ChatFile>>>,
    /// Whether each document parsed without errors.
    pub parse_clean: &'a Arc<DashMap<Url, bool>>,
    /// Cached validation artifacts per document URI.
    pub validation_cache: &'a Arc<DashMap<Url, ValidationCache>>,
    /// Cache of last-published diagnostics (for pull diagnostic model).
    pub last_diagnostics: &'a Arc<DashMap<Url, Vec<Diagnostic>>>,
}

/// Parse, validate, and publish diagnostics for one document revision.
pub(crate) async fn validate_and_publish(
    resources: ValidationResources<'_>,
    uri: Url,
    text: &str,
    old_text: Option<&str>,
) {
    let ValidationResources {
        client,
        language_services,
        parse_trees,
        chat_files,
        parse_clean,
        validation_cache,
        last_diagnostics,
    } = resources;

    let diagnostics = match language_services.with_parser(|parser| {
        'diagnostics: {
            // Extract filename from URI for E531 validation
            let filename = uri
                .path_segments()
                .and_then(|mut segments| segments.next_back())
                .and_then(|f| f.strip_suffix(".cha"));

            let old_tree = parse_trees.get(&uri).map(|entry| entry.clone());
            let old_chat_file = chat_files
                .get(&uri)
                .map(|entry| ChatFile::clone(entry.value()));
            // Try incremental parse (based on the previous tree) before falling back.
            let new_tree = if old_tree.is_some() {
                match parser.parse_tree_incremental(text, old_tree.as_ref()) {
                    Ok(tree) => Some(tree),
                    Err(parse_errors) => {
                        parse_trees.remove(&uri);
                        // Keep old ChatFile and validation cache — stale but needed
                        // as baseline for incremental diffing on next keystroke.
                        parse_clean.insert(uri.clone(), false);
                        break 'diagnostics to_diagnostics_batch(
                            &parse_errors.errors.iter().collect::<Vec<_>>(),
                            text,
                        );
                    }
                }
            } else {
                None
            };

        // Track whether fallback can reuse header validation from old cache.
        let mut fallback_reuse_headers = false;

        if let (Some(old_tree_ref), Some(new_tree)) = (old_tree.as_ref(), new_tree.as_ref()) {
            let mut changed_ranges: Vec<tree_sitter::Range> =
                old_tree_ref.changed_ranges(new_tree).collect();
            if changed_ranges.is_empty()
                && let Some(old_text) = old_text
                && old_text != text
                && let Some(range) = compute_text_changed_range(old_text, text)
            {
                changed_ranges.push(range);
            }
            parse_trees.insert(uri.clone(), new_tree.clone());

            let diff_span = old_text.and_then(|old| compute_text_diff_span(old, text));
            let delta = match diff_span {
                Some((_, old_end, new_end)) => new_end as i64 - old_end as i64,
                None => 0,
            };

            if let Some(mut chat_file) = old_chat_file {
                let (utterance_nodes, context_header_changed, any_header_changed) =
                    collect_utterances_and_header_changes(new_tree, &changed_ranges);
                let utterance_count = chat_file.utterances().count();

                // When falling through to full fallback, headers can be reused
                // if no header was changed at all.
                if !any_header_changed {
                    fallback_reuse_headers = true;
                }

                if !context_header_changed && utterance_nodes.len() == utterance_count {
                    let line_indices = collect_utterance_line_indices(&chat_file);
                    if line_indices.len() == utterance_count {
                        let affected_indices =
                            affected_utterance_indices(&utterance_nodes, &changed_ranges);
                        debug!(
                            path = "incremental",
                            affected = affected_indices.len(),
                            total = utterance_count,
                            "LSP: reparsing affected utterances"
                        );

                        let mut parse_failed = false;
                        let parse_errors = ErrorCollector::new();

                        for idx in &affected_indices {
                            let Some(line_idx) = line_indices.get(*idx) else {
                                parse_failed = true;
                                break;
                            };

                            // Try to parse even if CST has errors — tree-sitter
                            // is error-recovering so we may still get a usable
                            // utterance. If not, keep the old utterance as stale
                            // baseline and record the failure.
                            let Some(utterance) = parser
                                .parse_utterance_cst(utterance_nodes[*idx], text, &parse_errors)
                                .into_option()
                            else {
                                parse_failed = true;
                                break;
                            };

                            chat_file.lines[*line_idx] = Line::utterance(utterance);
                        }

                        let parse_errors = parse_errors.into_vec();
                        let has_parse_errors = parse_failed
                            || parse_errors
                                .iter()
                                .any(|e| matches!(e.severity, Severity::Error));

                        if !has_parse_errors {
                            let mut chat_file = chat_file;
                            if let Some(mut cache_entry) = validation_cache.get_mut(&uri) {
                                if delta != 0 {
                                    cache_entry.shift_spans_after(
                                        match diff_span {
                                            Some((_, old_end, _)) => old_end as u32,
                                            None => 0,
                                        },
                                        delta as i32,
                                        text.len(),
                                    );
                                }
                                if cache_entry.utterance_errors.len() == utterance_count {
                                    for idx in &affected_indices {
                                        if let Some(line_idx) = line_indices.get(*idx)
                                            && let Line::Utterance(utterance) =
                                                &mut chat_file.lines[*line_idx]
                                        {
                                            cache_entry.utterance_errors[*idx] =
                                                validate_single_utterance(
                                                    utterance,
                                                    &cache_entry.context,
                                                );
                                        }
                                    }

                                    let mut scoped_changed = false;
                                    let mut bullet_changed = false;

                                    for idx in &affected_indices {
                                        if let Some(line_idx) = line_indices.get(*idx)
                                            && let Line::Utterance(utterance) =
                                                &chat_file.lines[*line_idx]
                                        {
                                            let scoped_sig =
                                                super::cache_builder::scoped_marker_signature(
                                                    utterance,
                                                );
                                            if let Some(prev) =
                                                cache_entry.utterance_scoped_signature.get_mut(*idx)
                                                && *prev != scoped_sig
                                            {
                                                *prev = scoped_sig;
                                                scoped_changed = true;
                                            }

                                            let bullet_sig =
                                                super::cache_builder::bullet_signature(utterance)
                                                    // DEFAULT: Utterances without bullets use signature 0.
                                                    .unwrap_or_default();
                                            if let Some(prev) =
                                                cache_entry.utterance_bullet_signature.get_mut(*idx)
                                                && *prev != bullet_sig
                                            {
                                                *prev = bullet_sig;
                                                bullet_changed = true;
                                            }
                                        }
                                    }

                                    if scoped_changed {
                                        cache_entry.scoped_errors = compute_scoped_errors(
                                            &chat_file,
                                            &cache_entry.context,
                                            cache_entry
                                                .utterance_scoped_signature
                                                .iter()
                                                .any(|sig| *sig != 0),
                                        );
                                    }

                                    if bullet_changed {
                                        cache_entry.bullet_errors = compute_bullet_errors(
                                            &chat_file,
                                            &cache_entry.context,
                                            cache_entry
                                                .utterance_bullet_signature
                                                .iter()
                                                .any(|sig| *sig != 0),
                                        );
                                    }

                                    debug!(
                                        path = "incremental",
                                        scoped_changed,
                                        bullet_changed,
                                        "LSP: updated per-utterance validations"
                                    );

                                    let errors = cache_entry.all_errors();

                                    // Create diagnostics with related information
                                    let diagnostics = to_diagnostics_batch_with_context(
                                        &errors.iter().collect::<Vec<_>>(),
                                        text,
                                        Some(&uri),
                                        Some(&chat_file),
                                    );

                                    chat_files.insert(uri.clone(), Arc::new(chat_file));
                                    parse_clean.insert(uri.clone(), true);

                                    break 'diagnostics diagnostics;
                                }
                            }

                            debug!(
                                path = "incremental-rebuild",
                                "LSP: cache miss or size mismatch, rebuilding validation cache"
                            );
                            let cache = if !any_header_changed {
                                if let Some(old_cache) = validation_cache.get(&uri) {
                                    debug!(
                                        path = "incremental-rebuild",
                                        "LSP: reusing header validation (headers unchanged)"
                                    );
                                    build_validation_cache_reuse_headers(
                                        &mut chat_file,
                                        &old_cache,
                                        filename,
                                    )
                                } else {
                                    build_validation_cache(&mut chat_file, filename)
                                }
                            } else {
                                build_validation_cache(&mut chat_file, filename)
                            };
                            let errors = cache.all_errors();

                            // Create diagnostics with related information
                            let diagnostics = to_diagnostics_batch_with_context(
                                &errors.iter().collect::<Vec<_>>(),
                                text,
                                Some(&uri),
                                Some(&chat_file),
                            );

                            chat_files.insert(uri.clone(), Arc::new(chat_file));
                            validation_cache.insert(uri.clone(), cache);
                            parse_clean.insert(uri.clone(), true);

                            break 'diagnostics diagnostics;
                        }

                        // Keep old ChatFile and validation cache as baseline.
                        parse_clean.insert(uri.clone(), false);
                        debug!(
                            path = "incremental-error",
                            "LSP: parse errors in updated utterances, emitting parse diagnostics"
                        );
                        break 'diagnostics to_diagnostics_batch(
                            &parse_errors.iter().collect::<Vec<_>>(),
                            text,
                        );
                    }
                } else if !context_header_changed
                    && let Some(diff_span_val) = diff_span
                    && let Some((splice_idx, is_insertion)) =
                        detect_utterance_splice(&utterance_nodes, diff_span_val.0, utterance_count)
                {
                    // Utterance count changed by ±1 — splice the ChatFile and rebuild cache.
                    let line_indices = collect_utterance_line_indices(&chat_file);
                    if line_indices.len() == utterance_count {
                        let parse_errors = ErrorCollector::new();
                        let mut splice_ok = true;

                        if is_insertion {
                            debug!(
                                path = "splice-insert",
                                splice_idx,
                                old_count = utterance_count,
                                new_count = utterance_nodes.len(),
                                "LSP: inserting utterance"
                            );
                            match parser
                                .parse_utterance_cst(
                                    utterance_nodes[splice_idx],
                                    text,
                                    &parse_errors,
                                )
                                .into_option()
                            {
                                Some(utt) => {
                                    let insert_pos = if splice_idx == 0 {
                                        line_indices
                                            .first()
                                            .copied()
                                            .unwrap_or(chat_file.lines.len())
                                    } else {
                                        line_indices[splice_idx - 1] + 1
                                    };
                                    chat_file.lines.insert(insert_pos, Line::utterance(utt));
                                }
                                None => splice_ok = false,
                            }
                        } else {
                            debug!(
                                path = "splice-delete",
                                splice_idx,
                                old_count = utterance_count,
                                new_count = utterance_nodes.len(),
                                "LSP: deleting utterance"
                            );
                            if splice_idx < line_indices.len() {
                                chat_file.lines.remove(line_indices[splice_idx]);
                            } else {
                                splice_ok = false;
                            }
                        }

                        let splice_parse_errors = parse_errors.into_vec();
                        let has_splice_errors = !splice_ok
                            || splice_parse_errors
                                .iter()
                                .any(|e| matches!(e.severity, Severity::Error));

                        if !has_splice_errors {
                            // Try targeted cache splice instead of full rebuild.
                            if let Some(mut cache_entry) = validation_cache.get_mut(&uri) {
                                // Shift spans to account for text position changes.
                                if delta != 0 {
                                    cache_entry.shift_spans_after(
                                        diff_span_val.1 as u32,
                                        delta as i32,
                                        text.len(),
                                    );
                                }

                                if is_insertion {
                                    cache_entry.insert_utterance_at(splice_idx);

                                    // Validate only the new utterance.
                                    let new_line_indices =
                                        collect_utterance_line_indices(&chat_file);
                                    if let Some(&line_idx) = new_line_indices.get(splice_idx)
                                        && let Line::Utterance(utterance) =
                                            &mut chat_file.lines[line_idx]
                                    {
                                        cache_entry.utterance_errors[splice_idx] =
                                            validate_single_utterance(
                                                utterance,
                                                &cache_entry.context,
                                            );
                                        cache_entry.utterance_scoped_signature[splice_idx] =
                                            super::cache_builder::scoped_marker_signature(
                                                utterance,
                                            );
                                        cache_entry.utterance_bullet_signature[splice_idx] =
                                            super::cache_builder::bullet_signature(utterance)
                                                .unwrap_or_default();
                                    }
                                } else {
                                    cache_entry.remove_utterance_at(splice_idx);
                                }

                                // Recompute cross-utterance validations since the
                                // utterance set structurally changed.
                                let has_scoped = cache_entry
                                    .utterance_scoped_signature
                                    .iter()
                                    .any(|sig| *sig != 0);
                                let has_bullets = cache_entry
                                    .utterance_bullet_signature
                                    .iter()
                                    .any(|sig| *sig != 0);
                                cache_entry.scoped_errors = compute_scoped_errors(
                                    &chat_file,
                                    &cache_entry.context,
                                    has_scoped,
                                );
                                cache_entry.bullet_errors = compute_bullet_errors(
                                    &chat_file,
                                    &cache_entry.context,
                                    has_bullets,
                                );

                                debug!(
                                    path = "splice",
                                    is_insertion,
                                    splice_idx,
                                    "LSP: cache spliced, validated splice target only"
                                );

                                let errors = cache_entry.all_errors();
                                let diagnostics = to_diagnostics_batch_with_context(
                                    &errors.iter().collect::<Vec<_>>(),
                                    text,
                                    Some(&uri),
                                    Some(&chat_file),
                                );

                                chat_files.insert(uri.clone(), Arc::new(chat_file));
                                parse_clean.insert(uri.clone(), true);

                                break 'diagnostics diagnostics;
                            }

                            // No existing cache — full build.
                            debug!(
                                path = "splice",
                                "LSP: no existing cache, falling back to full build"
                            );
                            let cache = build_validation_cache(&mut chat_file, filename);
                            let errors = cache.all_errors();
                            let diagnostics = to_diagnostics_batch_with_context(
                                &errors.iter().collect::<Vec<_>>(),
                                text,
                                Some(&uri),
                                Some(&chat_file),
                            );

                            chat_files.insert(uri.clone(), Arc::new(chat_file));
                            validation_cache.insert(uri.clone(), cache);
                            parse_clean.insert(uri.clone(), true);

                            break 'diagnostics diagnostics;
                        }

                        // Splice had parse errors — publish them, keep old baseline.
                        debug!(
                            path = "splice",
                            "LSP: splice had parse errors, falling back to parse diagnostics"
                        );
                        parse_clean.insert(uri.clone(), false);
                        break 'diagnostics to_diagnostics_batch(
                            &splice_parse_errors.iter().collect::<Vec<_>>(),
                            text,
                        );
                    }
                }
            }
        }
        debug!(
            path = "fallback-full",
            "LSP: falling back to full parse/validate"
        );

        let parse_errors_sink = ErrorCollector::new();
        let (mut chat_file, new_tree) = parser.parse_chat_file_streaming_incremental(
            text,
            old_tree.as_ref(),
            &parse_errors_sink,
        );

        if let Some(tree) = new_tree {
            parse_trees.insert(uri.clone(), tree);
        }

        let parse_errors = parse_errors_sink.into_vec();
        let has_severity_errors = parse_errors
            .iter()
            .any(|e| matches!(e.severity, Severity::Error));

            break 'diagnostics if has_severity_errors {
                // Store ChatFile anyway for incremental baseline + features (hover, etc.)
                chat_files.insert(uri.clone(), Arc::new(chat_file));
                parse_clean.insert(uri.clone(), false);
                to_diagnostics_batch(&parse_errors.iter().collect::<Vec<_>>(), text)
            } else {
                // Reuse header validation from old cache when headers haven't changed.
                let cache = if fallback_reuse_headers {
                    if let Some(old_cache) = validation_cache.get(&uri) {
                        debug!(
                            path = "fallback-full",
                            "LSP: reusing header validation (headers unchanged)"
                        );
                        build_validation_cache_reuse_headers(&mut chat_file, &old_cache, filename)
                    } else {
                        build_validation_cache(&mut chat_file, filename)
                    }
                } else {
                    build_validation_cache(&mut chat_file, filename)
                };
                let errors = cache.all_errors();

                let diagnostics = to_diagnostics_batch_with_context(
                    &errors.iter().collect::<Vec<_>>(),
                    text,
                    Some(&uri),
                    Some(&chat_file),
                );

                chat_files.insert(uri.clone(), Arc::new(chat_file));
                validation_cache.insert(uri.clone(), cache);
                parse_clean.insert(uri.clone(), true);

                diagnostics
            };
        }
    }) {
        Ok(diagnostics) => diagnostics,
        Err(error) => {
            parse_trees.remove(&uri);
            chat_files.remove(&uri);
            validation_cache.remove(&uri);
            parse_clean.insert(uri.clone(), false);
            vec![initialization_diagnostic(&error)]
        }
    };

    last_diagnostics.insert(uri.clone(), diagnostics.clone());
    client.publish_diagnostics(uri, diagnostics, None).await;
}

/// Convert a backend service initialization failure into a single LSP diagnostic.
fn initialization_diagnostic(error: &BackendInitError) -> Diagnostic {
    Diagnostic {
        range: Range::new(Position::new(0, 0), Position::new(0, 0)),
        severity: Some(DiagnosticSeverity::ERROR),
        code: None,
        code_description: None,
        source: Some("chatter-lsp".to_string()),
        message: error.to_string(),
        related_information: None,
        tags: None,
        data: None,
    }
}
