//! Result injection, clearing, and alignment validation.

use talkbank_model::SpeakerCode;
use talkbank_model::model::{LanguageCode, Line};

use super::{
    BatchItemWithPosition, MappingContext, MisalignmentClass, MisalignmentDiagnostic, MorOutcome,
    MorOutcomeKind, MwtDict, TokenizationMode, UdId, UdResponse, apply_grammatical_invariants,
    map_ud_sentence, map_ud_sentence_expanded,
};
use crate::decisions::{DecisionRecord, DecisionStrategy, MorphosyntaxStrategy};

/// Context supplied by the caller to [`enrich_diagnostic`] so the
/// suspected misalignment class can be inferred more precisely.
enum RetokenizationContext {
    /// Non-retokenize path — Stanza was told to realign to CHAT
    /// boundaries. A mismatch here means realignment did not hold.
    Preserve,
    /// `StanzaRetokenize` mode was used; CHAT was rewritten to match
    /// Stanza's own tokenization, so a mismatch suggests a retokenize
    /// or MWT-expansion bug rather than a realignment issue.
    StanzaRetokenize,
}

/// Enrich an inner [`MisalignmentDiagnostic`] with caller-held context
/// that the inner validator didn't have access to: the Stanza tokens
/// that were sent, and a best-effort suspected-class classification.
fn enrich_diagnostic(
    mut diag: MisalignmentDiagnostic,
    stanza_tokens: &[String],
    context: RetokenizationContext,
) -> MisalignmentDiagnostic {
    if diag.stanza_tokens_after_mapping.is_empty() && !stanza_tokens.is_empty() {
        diag.stanza_tokens_after_mapping = stanza_tokens.to_vec();
    }

    // Infer the suspected class when the inner validator left it
    // `Unknown`. This is a heuristic — the real diagnosis requires a
    // developer looking at the logs — but it points them at the right
    // stage to investigate first.
    if matches!(diag.suspected_class, MisalignmentClass::Unknown) {
        diag.suspected_class = match context {
            RetokenizationContext::StanzaRetokenize => {
                // In retokenize mode the main tier is rewritten to match
                // Stanza's tokenization; a mismatch here usually means
                // the rebuild step dropped or duplicated tokens.
                MisalignmentClass::MwtReassemblyBug
            }
            RetokenizationContext::Preserve => {
                // Non-retokenize: Stanza was told to realign. A mismatch
                // implies either the realignment context wasn't set for
                // this call (RealignmentSkipped), a terminator-filter
                // regression (TerminatorFilterBug), or an MWT-reassembly
                // bug. Without more signal, Unknown is honest.
                MisalignmentClass::Unknown
            }
        };
    }

    diag
}

/// Result of morphosyntax injection: traces + provenance decisions.
#[derive(Debug, Clone)]
pub struct InjectionResult {
    /// Per-utterance retokenization traces for debugging.
    pub retokenization_traces: Vec<RetokenizationInfo>,
    /// Decision records for utterances that were skipped or degraded.
    pub decisions: Vec<DecisionRecord>,
}

/// Retokenization info collected during injection, for trace visualization.
#[derive(Debug, Clone)]
pub struct RetokenizationInfo {
    /// Utterance ordinal (0-based, among processed utterances).
    pub utterance_ordinal: usize,
    /// Original CHAT words.
    pub original_words: Vec<String>,
    /// Stanza tokens after retokenization.
    pub stanza_tokens: Vec<String>,
    /// Word→token index mapping: `mapping[word_idx]` = list of token indices.
    pub mapping: Vec<Vec<usize>>,
    /// Whether the fallback (length-proportional) mapping was used.
    pub used_fallback: bool,
}

// ---------------------------------------------------------------------------
// Result injection (from NLP callback)
// ---------------------------------------------------------------------------

/// If every word in the utterance is a special-form (FormType) or a
/// code-switch (resolved language), synthesize the morphology
/// directly from FormType and write a star-shaped %gra (first chunk
/// = head=0/ROOT; rest depend on it; terminator = head=1/PUNCT).
/// Returns `true` if synthesis was performed (caller should skip
/// the Stanza-derived path); `false` if the utterance doesn't qualify.
///
/// Stanza's analysis is bypassed here because:
/// - For all-placeholder input (`xbxxx .`), Stanza may return empty
///   (no parseable sentence).
/// - For all-placeholder input, Stanza may tokenize `xbxxx` into
///   multiple sub-tokens (e.g. `xb`+`xxx`), producing a mor count
///   that doesn't align with the utterance's word count, which trips
///   `inject_morphosyntax`'s `word_count == mor_count` check and
///   leaves the utterance untouched.
/// - In all cases, the morphology of these words is fully determined
///   by FormType; whatever Stanza says about them is irrelevant.
fn synthesize_all_special_form_utterance(
    chat_file: &mut talkbank_model::model::ChatFile,
    line_idx: usize,
    item: &super::payload::MorphosyntaxBatchItem,
    words: &[crate::extract::ExtractedWord],
    root_label: &str,
    dep_label: &str,
    punct_label: &str,
    decisions: &mut Vec<DecisionRecord>,
) -> bool {
    use super::synthesis::synthesize_special_form_mor;
    use talkbank_model::model::dependent_tier::GrammaticalRelation;
    use talkbank_model::model::dependent_tier::mor::{Mor, MorWord};

    let all_synthesizable = !item.special_forms.is_empty()
        && item
            .special_forms
            .iter()
            .all(|(form_type, resolved_lang)| form_type.is_some() || resolved_lang.is_some());
    if !all_synthesizable {
        return false;
    }

    let synth_mors: Vec<Mor> = item
        .special_forms
        .iter()
        .zip(words.iter())
        .map(|((form_type, resolved_lang), word)| {
            // Code-switched first: an `@s` word can carry both a
            // form_type and a resolved_lang in theory, and the L2
            // splice path (which fills in the placeholder later)
            // wins by precedent.
            if resolved_lang.is_some() {
                Mor::new(MorWord::l2_placeholder())
            } else if let Some(ft) = form_type {
                synthesize_special_form_mor(ft, word.text.as_str())
            } else {
                unreachable!(
                    "all_synthesizable invariant violated: word has \
                     neither form_type nor resolved_lang"
                )
            }
        })
        .collect();

    let chunk_count: usize = synth_mors.iter().map(Mor::count_chunks).sum();
    let mut synth_gras: Vec<GrammaticalRelation> = Vec::with_capacity(chunk_count + 1);
    for chunk_idx in 1..=chunk_count {
        let (head, label) = if chunk_idx == 1 {
            (0_usize, root_label)
        } else {
            (1_usize, dep_label)
        };
        synth_gras.push(GrammaticalRelation::new(chunk_idx, head, label));
    }
    synth_gras.push(GrammaticalRelation::new(chunk_count + 1, 1, punct_label));

    let utt = match &mut chat_file.lines[line_idx] {
        Line::Utterance(u) => u,
        _ => return false,
    };
    if let Err(diag) =
        crate::inject::inject_morphosyntax(utt, synth_mors, item.terminator.clone(), synth_gras)
    {
        let enriched = enrich_diagnostic(diag, &[], RetokenizationContext::Preserve);
        let outcome = MorOutcome {
            line_idx,
            speaker: SpeakerCode::new(utt.main.speaker.as_str()),
            kind: MorOutcomeKind::MisalignmentBug(enriched),
        };
        if let Some(record) = outcome.to_decision_record() {
            record.trace();
            decisions.push(record);
        }
    }
    true
}

/// Inject UD NLP results back into utterances.
///
/// Applies special form overrides (@c -> c|, @s -> L2|xxx) and
/// optionally retokenizes the main tier based on the [`TokenizationMode`].
///
/// # Errors
///
/// Returns `Err` if a `line_idx` no longer points to an utterance, or if
/// retokenization or morphosyntax injection fails for any utterance.
pub fn inject_results(
    parser: &talkbank_parser::TreeSitterParser,
    chat_file: &mut talkbank_model::model::ChatFile,
    batch_items: Vec<BatchItemWithPosition>,
    responses: Vec<UdResponse>,
    _lang: &LanguageCode,
    tokenization_mode: TokenizationMode,
    mwt: &MwtDict,
) -> Result<InjectionResult, String> {
    use talkbank_model::model::GrammaticalRelationType;

    use super::synthesis::synthesize_special_form_mor;

    /// Deprel label written to `%gra` for non-analyzable special-form
    /// positions whose Stanza-side head is non-zero (i.e. the form-marker
    /// token is a dependent of some other chunk). UD `dep` = "no specific
    /// role applies." The head=0 case must keep `ROOT` instead — see the
    /// joint invariant `(head == 0) ⟺ (deprel == "ROOT")` enforced by
    /// the validator (E722/E723).
    const DEP_RELATION_LABEL: &str = "DEP";
    /// Deprel label written when the form-marker token is the syntactic
    /// root of the utterance (Stanza returned `head=0`). Required by the
    /// joint root invariant; the prior unconditional overwrite to DEP
    /// produced 3,378 wild E722 occurrences across the corpus on
    /// 2026-05-06.
    const ROOT_RELATION_LABEL: &str = "ROOT";
    /// Deprel label written for the terminator's relation in synthetic
    /// (no-Stanza) gras. Same string CHAT validation expects for the
    /// terminator's punct relation.
    const PUNCT_RELATION_LABEL: &str = "PUNCT";

    let mut retokenization_traces: Vec<RetokenizationInfo> = Vec::new();
    let mut decisions: Vec<DecisionRecord> = Vec::new();

    for (ud_resp, (line_idx, utt_ordinal, item, words)) in responses.into_iter().zip(batch_items) {
        // Pre-flight: utterances whose every word is a special-form
        // or code-switch placeholder don't need Stanza. Stanza's
        // analysis of `xbxxx`-only input is fundamentally untrustworthy
        // — its English tokenizer may split `xbxxx` into `xb`+`xxx`,
        // which then trips the count-mismatch check in
        // `inject_morphosyntax` and leaves the utterance with no
        // tiers (preserving any pre-existing buggy %gra). Per the
        // 2026-05-07 reproducer in
        // `synthesis_stanza_tokenizes_xbxxx_into_two_at_q_root_keeps_root_deprel`,
        // the morphology is fully determined by FormType — synthesize
        // it directly and skip the Stanza-derived path entirely.
        if synthesize_all_special_form_utterance(
            chat_file,
            line_idx,
            &item,
            &words,
            ROOT_RELATION_LABEL,
            DEP_RELATION_LABEL,
            PUNCT_RELATION_LABEL,
            &mut decisions,
        ) {
            continue;
        }

        if let Some(ud_sentence) = ud_resp.sentences.first() {
            let utt = match &mut chat_file.lines[line_idx] {
                Line::Utterance(u) => u,
                _ => {
                    return Err(format!(
                        "Line at index {line_idx} is no longer an utterance"
                    ));
                }
            };

            let ctx = MappingContext {
                lang: item.lang.clone(),
            };

            // Apply grammatical-invariant rewrites to correct known
            // Stanza defects (e.g., English copula 's + progressive
            // misanalyzed as possessive-gerund). The rewrite returns
            // the input unchanged when no rule fires, so this is safe
            // to call unconditionally. See
            // `crate::morphosyntax` for the current rule set and
            // `book/src/reference/stanza-limitations.md` for the
            // versioned defect registry.
            let ud_sentence_rescued = apply_grammatical_invariants(ud_sentence, &ctx);

            // Choose mapping strategy based on tokenization mode:
            //
            // - Preserve: merge Range token components into one clitic MOR
            //   (verb|go~part|to) so MOR count == CHAT word count.
            // - StanzaRetokenize: produce one MOR per component word so
            //   MOR count == Stanza token count (after filtering Range parents).
            //   The retokenize path rewrites the main tier with expanded tokens.
            let (mut mors, mut gra_relations) = {
                let map_result = if tokenization_mode == TokenizationMode::StanzaRetokenize {
                    map_ud_sentence_expanded(&ud_sentence_rescued, &ctx)
                } else {
                    map_ud_sentence(&ud_sentence_rescued, &ctx)
                };
                match map_result {
                    Ok(result) => result,
                    Err(e) => {
                        // Same policy as injection failures below: record and
                        // continue. Stanza occasionally returns structurally
                        // invalid UD (e.g., multiple heads=0) for specific
                        // utterances; we log those loudly and proceed.
                        decisions.push(DecisionRecord::new_and_trace(
                            line_idx,
                            utt.main.speaker.as_str().to_string(),
                            DecisionStrategy::Morphosyntax(MorphosyntaxStrategy::MappingFailed),
                            format!("ud_to_chat_error={e}"),
                            true,
                        ));
                        continue;
                    }
                }
            };

            // Synthesize %mor and %gra for non-analyzable special-form
            // positions; see `morphosyntax/synthesis/` for the policy
            // table and the per-FormType scat assignments.
            //
            // The @s family (resolved_lang.is_some()) gets the L2|xxx
            // placeholder; the L2 splice path overwrites it later.
            //
            // The four-way zip auto-truncates if lengths disagree;
            // downstream `inject_morphosyntax` / `retokenize_utterance`
            // emit a typed `MisalignmentBug` outcome in that case.
            for (((mor, (form_type, resolved_lang)), word), gra) in mors
                .iter_mut()
                .zip(item.special_forms.iter())
                .zip(words.iter())
                .zip(gra_relations.iter_mut())
            {
                if resolved_lang.is_some() {
                    mor.main.reset_to_l2_placeholder();
                    continue;
                }

                if let Some(ft) = form_type {
                    *mor = synthesize_special_form_mor(ft, word.text.as_str());
                    // Preserve the joint invariant `(head == 0) ⟺ (deprel
                    // == "ROOT")`: when the form-marker token is the
                    // utterance root (Stanza returned head=0), the deprel
                    // must remain ROOT so the validator's E722 check
                    // passes. For non-root positions the BA2-equivalent
                    // convention is the generic UD `dep`.
                    let label = if gra.head == 0 {
                        ROOT_RELATION_LABEL
                    } else {
                        DEP_RELATION_LABEL
                    };
                    gra.relation = GrammaticalRelationType::new(label);
                }
            }

            if tokenization_mode == TokenizationMode::StanzaRetokenize {
                // Range parents would double-count alongside their components;
                // terminator-punct singles are supplied separately via the
                // typed `Terminator` and already excluded from
                // `mors`/`gra_relations` by `map_ud_sentence_expanded`.
                let mut tokens: Vec<String> = ud_sentence
                    .words
                    .iter()
                    .filter(|w| {
                        !matches!(&w.id, UdId::Range(_, _))
                            && !crate::morphosyntax::is_terminator_punct(w)
                    })
                    .map(|w| {
                        if w.text.contains(char::is_whitespace) {
                            w.text.chars().filter(|c| !c.is_whitespace()).collect()
                        } else {
                            w.text.clone()
                        }
                    })
                    .collect();

                // Apply MWT lexicon overrides: when a Stanza token matches an
                // MWT entry, splice in the expansion tokens (and duplicate the
                // corresponding Mor/GRA items so counts stay aligned).
                if !mwt.is_empty() {
                    let mut expanded_tokens = Vec::with_capacity(tokens.len());
                    let mut expanded_mors = Vec::with_capacity(mors.len());
                    let mut expanded_gra = Vec::with_capacity(gra_relations.len());

                    for (tok_idx, tok) in tokens.iter().enumerate() {
                        let tok_lower = tok.to_lowercase();
                        if let Some(expansion) =
                            mwt.get(&tok_lower).or_else(|| mwt.get(tok.as_str()))
                        {
                            // Replace this token with the expansion tokens.
                            expanded_tokens.extend(expansion.iter().cloned());

                            // For the first expansion token, keep the original
                            // Mor/GRA. For subsequent tokens, duplicate so the
                            // alignment stays correct.
                            if tok_idx < mors.len() {
                                expanded_mors.push(mors[tok_idx].clone());
                                for _ in 1..expansion.len() {
                                    expanded_mors.push(mors[tok_idx].clone());
                                }
                            }
                            if tok_idx < gra_relations.len() {
                                expanded_gra.push(gra_relations[tok_idx].clone());
                                for _ in 1..expansion.len() {
                                    expanded_gra.push(gra_relations[tok_idx].clone());
                                }
                            }
                        } else {
                            expanded_tokens.push(tok.clone());
                            if tok_idx < mors.len() {
                                expanded_mors.push(mors[tok_idx].clone());
                            }
                            if tok_idx < gra_relations.len() {
                                expanded_gra.push(gra_relations[tok_idx].clone());
                            }
                        }
                    }

                    tokens = expanded_tokens;
                    mors = expanded_mors;
                    gra_relations = expanded_gra;
                }

                // Collect retokenization trace info before modifying the AST.
                {
                    use crate::retokenize::{
                        build_word_token_mapping, try_deterministic_word_token_mapping,
                    };
                    let mapping = build_word_token_mapping(&words, &tokens);
                    let used_fallback =
                        try_deterministic_word_token_mapping(&words, &tokens).is_none();
                    retokenization_traces.push(RetokenizationInfo {
                        utterance_ordinal: utt_ordinal,
                        original_words: words.iter().map(|w| w.text.as_str().to_string()).collect(),
                        stanza_tokens: tokens.clone(),
                        mapping: (0..words.len())
                            .map(|i| mapping.tokens_for_word(i).to_vec())
                            .collect(),
                        used_fallback,
                    });
                }

                if let Err(diag) = crate::retokenize::retokenize_utterance(
                    parser,
                    utt,
                    &words,
                    &tokens,
                    mors,
                    item.terminator.clone(),
                    gra_relations,
                ) {
                    // File-level absorption: convert the typed diagnostic
                    // to a MorOutcome::MisalignmentBug, emit its
                    // DecisionRecord, continue. The diagnostic is loud
                    // because it surfaces through `%xalign`; never silent.
                    let enriched =
                        enrich_diagnostic(diag, &tokens, RetokenizationContext::StanzaRetokenize);
                    let outcome = MorOutcome {
                        line_idx,
                        speaker: SpeakerCode::new(utt.main.speaker.as_str()),
                        kind: MorOutcomeKind::MisalignmentBug(enriched),
                    };
                    if let Some(mut record) = outcome.to_decision_record() {
                        // Retokenization-path failures share the
                        // MisalignmentBug outcome class but have their
                        // own strategy label for %xalign output.
                        record.strategy = DecisionStrategy::Morphosyntax(
                            MorphosyntaxStrategy::RetokenizationFailed,
                        );
                        record.trace();
                        decisions.push(record);
                    }
                    continue;
                }
            } else if let Err(diag) = crate::inject::inject_morphosyntax(
                utt,
                mors,
                item.terminator.clone(),
                gra_relations,
            ) {
                // Per-utterance injection failure: the 1-to-1 invariant
                // (CHAT alignable-word count == Mor count after mapping)
                // was violated. This is always a bug — the pipeline was
                // supposed to produce exactly as many Mors as there were
                // CHAT words, and did not. File-level absorption: log
                // loudly via DecisionRecord, continue with the next
                // utterance rather than killing the whole file. Every
                // mismatch produces a record, so a systemic regression
                // (e.g. 2026-04-17 comma-drop) is visible as a corpus-wide
                // warning spike instead of a silent quality drop.
                let enriched = enrich_diagnostic(diag, &[], RetokenizationContext::Preserve);
                let outcome = MorOutcome {
                    line_idx,
                    speaker: SpeakerCode::new(utt.main.speaker.as_str()),
                    kind: MorOutcomeKind::MisalignmentBug(enriched),
                };
                if let Some(record) = outcome.to_decision_record() {
                    record.trace();
                    decisions.push(record);
                }
                continue;
            }
        } else if let Line::Utterance(utt) = &chat_file.lines[line_idx] {
            // Stanza returned empty for an utterance that's NOT
            // all-synthesizable (the all-synthesizable case was
            // handled at the top of the iteration). This is a
            // genuine "Stanza had nothing to say" — record the
            // decision and leave the utterance untouched.
            decisions.push(DecisionRecord::new_and_trace(
                line_idx,
                utt.main.speaker.as_str().to_string(),
                DecisionStrategy::Morphosyntax(MorphosyntaxStrategy::NlpNoSentences),
                "stanza_returned_empty_response".into(),
                true,
            ));
        }
    }

    Ok(InjectionResult {
        retokenization_traces,
        decisions,
    })
}
