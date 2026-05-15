//! Structural merge: combine primary structural info with secondary lexical output.
//!
//! The core POS resolution algorithm with priority-ordered rules:
//! 1. Copula predicate check
//! 2. Constraint agreement
//! 3. Closed-class function word override
//! 4. Content noun (NOUN/PROPN) override
//! 5. Primary POS structural fallback
//! 6. Best-guess from constraint

use talkbank_model::model::LanguageCode;
use talkbank_model::model::dependent_tier::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::{Mor, PosCategory};

use super::deprel::{
    UdDeprel, deprel_to_pos_constraint, infer_deprel_from_pos, refine_with_dependents,
};
use super::extract::PrimaryStructuralInfo;
use super::plan::{L2Attachment, L2SpanPlan};
use crate::morphosyntax::{
    MappingContext, MappingError, UdId, UdSentence, UdWord, UniversalPos, map_ud_sentence,
};

/// Sentence-level context from the secondary model for a single `@s`
/// word being merged.
///
/// The individual `Mor` passed to [`merge_primary_secondary`] carries
/// the secondary's UPOS and lemma for one word, but not the structural
/// evidence that identifies phrasal-verb constructions (the
/// `compound:prt` relation). Callers thread this context alongside the
/// `Mor` so [`resolve_merged_pos_with_context`] can detect phrasal-verb
/// heads and particles and override the POS priority chain accordingly.
///
/// `word_position` is the 0-based index into `sentence.words` of the
/// word whose POS is currently being resolved. It is NOT the 1-based
/// UD `id`; see [`current_ud_id`](Self::current_ud_id).
#[derive(Debug, Clone, Copy)]
pub struct SecondaryUdContext<'a> {
    /// The full UD sentence produced by the secondary model for the
    /// contiguous `@s` span.
    pub sentence: &'a UdSentence,
    /// Position of the current word within `sentence.words`.
    pub word_position: usize,
}

impl<'a> SecondaryUdContext<'a> {
    /// Returns the `UdWord` at `word_position`, or `None` if the
    /// position is out of range.
    pub fn current_word(&self) -> Option<&'a UdWord> {
        self.sentence.words.get(self.word_position)
    }

    /// 1-based UD id of the current word (for head/child matching).
    ///
    /// Returns `None` for Range or Decimal ids (MWT parent tokens /
    /// empty nodes), which never participate in phrasal-verb relations.
    pub fn current_ud_id(&self) -> Option<usize> {
        match self.current_word()?.id {
            UdId::Single(n) => Some(n),
            _ => None,
        }
    }

    /// Whether the current word is the head of a `compound:prt`
    /// relation (i.e., has a dependent whose deprel is `compound:prt`
    /// pointing back at this word).
    ///
    /// The `compound:prt` relation is Stanza's signal for a phrasal
    /// verb (`wake up`, `give up`, `figure out`). When it appears, the
    /// head is the verb and the dependent is the particle.
    pub fn is_phrasal_verb_head(&self) -> bool {
        let Some(my_id) = self.current_ud_id() else {
            return false;
        };
        self.sentence
            .words
            .iter()
            .any(|w| w.head == my_id && is_compound_prt(&w.deprel))
    }

    /// Whether the current word is itself a `compound:prt` particle.
    pub fn is_phrasal_verb_particle(&self) -> bool {
        self.current_word()
            .map(|w| is_compound_prt(&w.deprel))
            .unwrap_or(false)
    }
}

/// Deprel-base comparison against the UD phrasal-verb particle
/// relation. Matches `compound:prt` exactly; the only other
/// `compound:*` subtype we have seen in Stanza output is `compound:svc`
/// (serial verb construction), which has a different semantics and
/// must not trigger phrasal-verb promotion.
fn is_compound_prt(deprel: &str) -> bool {
    deprel == "compound:prt"
}

/// Result of merging primary structural info with secondary model output.
#[derive(Debug, Clone)]
pub struct MergedL2Morphology {
    /// Pre-mapped CHAT MOR item with resolved POS override applied.
    ///
    /// Produced by `map_ud_sentence` (which handles MWT Range tokens for
    /// contractions like `it's` → `pron|it~aux|be`) and then POS-overridden
    /// by the merge algorithm.
    pub mor: Mor,
    /// Corresponding GRA relations for the chunks in `mor`.
    pub gras: Vec<GrammaticalRelation>,
    /// Corrected deprel when the primary model's deprel was unreliable.
    /// `None` means keep the primary deprel as-is.
    pub corrected_deprel: Option<UdDeprel>,
    /// Host-attachment intent for any secondary ROOT in this merged item.
    ///
    /// The numeric host chunk anchor is resolved later from the current
    /// placeholder `%gra` relation during splice; this avoids carrying
    /// ambiguous raw indices across word-domain/chunk-domain seams.
    pub attachment: L2Attachment,
}

/// Resolve the merged POS from primary structural info and a secondary
/// UPOS, without sentence-level context. Thin wrapper over
/// [`resolve_merged_pos_with_context`]; see that function for the full
/// priority chain.
pub fn resolve_merged_pos(
    primary: &PrimaryStructuralInfo,
    secondary_upos: Option<UniversalPos>,
) -> UniversalPos {
    resolve_merged_pos_with_context(primary, secondary_upos, None)
}

/// Resolve the merged POS with optional secondary UD sentence context.
///
/// POS resolution priority for @s words:
///
/// 0. **Phrasal verb** (context required): a word with `compound:prt`
///    deprel is the particle → `Part`; a word whose UPOS is Verb and
///    which is the head of a `compound:prt` relation stays `Verb`
///    even if the primary constraint would reject it.
/// 1. **Copula predicate**: if `cop` dependent, reject VERB → NOUN/ADJ
/// 2. **Agreement**: secondary POS matches constraint → use it
/// 3. **Function word**: secondary is closed-class → trust it
/// 4. **Content noun**: secondary is NOUN/PROPN → trust it
/// 5. **Structural fallback**: primary POS matches constraint → use it
/// 6. **Best guess**: constraint's most likely POS
///
/// Priority 0 exists because the sentence-level evidence of a verb +
/// particle construction (`wake up`, `give up`, `figure out`) is more
/// reliable than either the primary's deprel constraint (which rejects
/// VERB when the primary parser tagged a foreign word as `advmod`) or
/// Priority 3's blind trust of any closed-class POS (which would lock
/// in `adp|up` instead of `part|up`).
pub fn resolve_merged_pos_with_context(
    primary: &PrimaryStructuralInfo,
    secondary_upos: Option<UniversalPos>,
    secondary_context: Option<&SecondaryUdContext<'_>>,
) -> UniversalPos {
    // Priority 0: phrasal-verb structural recognition. Runs first because
    // Stanza's compound:prt analysis is cross-linguistically reliable
    // for true verb + particle constructions.
    if let Some(ctx) = secondary_context {
        if ctx.is_phrasal_verb_particle() {
            return UniversalPos::Part;
        }
        if ctx.is_phrasal_verb_head() && secondary_upos == Some(UniversalPos::Verb) {
            return UniversalPos::Verb;
        }
    }

    let base_constraint = deprel_to_pos_constraint(&primary.deprel);
    let constraint = refine_with_dependents(&base_constraint, &primary.dependent_deprels);

    let has_copula = primary.dependent_deprels.iter().any(|d| d.base() == "cop");

    if let Some(sec_pos) = secondary_upos {
        // Priority 1: copula predicate — reject VERB
        if has_copula && sec_pos == UniversalPos::Verb {
            return if primary.upos == Some(UniversalPos::Noun)
                || primary.upos == Some(UniversalPos::Propn)
            {
                UniversalPos::Noun
            } else {
                UniversalPos::Adj
            };
        }

        // Priority 2: secondary agrees with structural constraint
        if constraint.contains(&sec_pos) {
            return sec_pos;
        }

        // Priority 3: closed-class function words are unambiguous
        if is_closed_class(sec_pos) {
            return sec_pos;
        }

        // Priority 4: NOUN/PROPN from the secondary model overrides wrong deprel
        if sec_pos == UniversalPos::Noun || sec_pos == UniversalPos::Propn {
            return sec_pos;
        }

        // Priority 5: primary POS within constraint
        if let Some(pri_pos) = primary.upos
            && constraint.contains(&pri_pos)
        {
            return pri_pos;
        }

        // Priority 6: best guess from constraint, or secondary as last resort
        constraint.most_likely().unwrap_or(sec_pos)
    } else if let Some(pri_pos) = primary.upos {
        if constraint.contains(&pri_pos) {
            pri_pos
        } else {
            constraint.most_likely().unwrap_or(pri_pos)
        }
    } else {
        constraint.most_likely().unwrap_or(UniversalPos::Noun)
    }
}

/// Whether a UPOS tag is a closed-class (function word) category.
fn is_closed_class(upos: UniversalPos) -> bool {
    matches!(
        upos,
        UniversalPos::Det
            | UniversalPos::Adp
            | UniversalPos::Sconj
            | UniversalPos::Cconj
            | UniversalPos::Aux
            | UniversalPos::Part
            | UniversalPos::Pron
    )
}

/// Merge primary structural info with a secondary `Mor` item, without
/// sentence-level context. Thin wrapper over
/// [`merge_primary_secondary_with_context`].
pub fn merge_primary_secondary(
    primary: &PrimaryStructuralInfo,
    secondary_mor: Mor,
    secondary_gras: Vec<GrammaticalRelation>,
    secondary_lang: &LanguageCode,
    attachment: L2Attachment,
) -> MergedL2Morphology {
    merge_primary_secondary_with_context(
        primary,
        secondary_mor,
        secondary_gras,
        secondary_lang,
        attachment,
        None,
    )
}

/// Whether the entry at `start` reaches a `head=0` row by following
/// head pointers within `chunk_count + 1` hops. Returns false on
/// cycles and self-loops — the conditions
/// [`repair_secondary_gras`]'s pass 4 needs to detect.
fn entry_reaches_root_via_heads(
    gras: &[GrammaticalRelation],
    start: usize,
    chunk_count: usize,
) -> bool {
    let mut current = start;
    for _ in 0..=chunk_count {
        let head = gras[current].head;
        if head == 0 {
            return true;
        }
        let next = head - 1; // 1-indexed → 0-indexed
        if next == current {
            return false; // self-loop
        }
        current = next;
    }
    false
}

/// Strict splice-contract root predicate. Distinct from
/// [`GrammaticalRelation::is_root`], which also accepts self-loops
/// and `INCROOT` — neither admissible at the splice boundary.
fn is_strict_root(rel: &GrammaticalRelation) -> bool {
    rel.head == 0 && rel.relation.eq_ignore_ascii_case("ROOT")
}

/// 0-indexed gras position carrying `head=0, ROOT`. Returned from
/// [`repair_secondary_gras`] so callers don't re-scan to drive
/// `apply_safe_root_rewrites`.
pub(super) type RootOffset = usize;

/// Repair a span-aggregated secondary gras slice so the merged
/// result satisfies the splice invariants by construction. Returns
/// post-repair root offsets.
///
/// **Why repair runs at the splice layer, not per position in
/// [`merge_primary_secondary_with_attachment`].** Per-position gras
/// may carry span-relative head indices (e.g. position 1's gras
/// has `head=2` referencing the second position in the span);
/// `chunk_count = position's chunks` would falsely flag those as
/// OOB. Repair runs once per span, where `chunk_count = total span
/// chunks`, so cross-position references stay valid.
///
/// Four passes:
/// 1. Clamp OOB (`head > chunk_count` → `head=0, ROOT`).
///    Catches `secondary_head_oob`.
/// 2. Demote multi-roots to attach the survivor; deterministic by
///    position. Catches `secondary_multi_root`.
/// 3. Force `head=0, ROOT` on the first row when the attachment
///    is [`L2Attachment::InternalRoot`] and no root exists. No-op
///    for `ExternalRoot` (`apply_safe_root_rewrites` handles that).
///    Catches `secondary_no_root`.
/// 4. Break residual cycles by re-attaching cycle members to the
///    existing root, or promoting one if none exists. Catches
///    `secondary_cycle`.
pub(super) fn repair_secondary_gras(
    gras: &mut [GrammaticalRelation],
    attachment: &L2Attachment,
) -> Vec<RootOffset> {
    if gras.is_empty() {
        return Vec::new();
    }
    let chunk_count = gras.len();

    // Pass 1.
    for rel in gras.iter_mut() {
        if rel.head > chunk_count {
            rel.head = 0;
            rel.relation = "ROOT".into();
        }
    }

    // Pass 2.
    let mut root_indices: Vec<RootOffset> = gras
        .iter()
        .enumerate()
        .filter(|(_, r)| is_strict_root(r))
        .map(|(i, _)| i)
        .collect();
    if root_indices.len() > 1 {
        let primary_root_head = root_indices[0] + 1; // 1-indexed
        for &idx in root_indices.iter().skip(1) {
            gras[idx].head = primary_root_head;
            gras[idx].relation = "DEP".into();
        }
        root_indices.truncate(1);
    }

    // Pass 3.
    if root_indices.is_empty()
        && let L2Attachment::InternalRoot = attachment
    {
        gras[0].head = 0;
        gras[0].relation = "ROOT".into();
        root_indices.push(0);
    }

    // Pass 4.
    let existing_root = root_indices.first().copied();
    for i in 0..gras.len() {
        if gras[i].head == 0 || entry_reaches_root_via_heads(gras, i, chunk_count) {
            continue;
        }
        match existing_root {
            Some(root_idx) if root_idx != i => {
                gras[i].head = root_idx + 1;
                gras[i].relation = "DEP".into();
            }
            _ => {
                gras[i].head = 0;
                gras[i].relation = "ROOT".into();
                if existing_root.is_none() {
                    root_indices.push(i);
                }
            }
        }
    }

    root_indices
}

fn merge_primary_secondary_with_attachment(
    primary: &PrimaryStructuralInfo,
    mut secondary_mor: Mor,
    secondary_gras: Vec<GrammaticalRelation>,
    secondary_lang: &LanguageCode,
    attachment: &L2Attachment,
    secondary_context: Option<&SecondaryUdContext<'_>>,
) -> MergedL2Morphology {
    let _ = secondary_lang; // reserved for future language-specific overrides

    let secondary_upos = UniversalPos::from_pos_name(secondary_mor.main.pos.as_str());
    let resolved_pos = resolve_merged_pos_with_context(primary, secondary_upos, secondary_context);
    secondary_mor.main.pos = PosCategory::new(resolved_pos.to_chat_pos_name());

    // NOTE: per-position gras repair is NOT called here. The
    // per-position gras may carry span-relative head indices (e.g.
    // for a multi-position L2 span, position 1's gras may have
    // head=2 referencing the second position in the span). Calling
    // `repair_secondary_gras` here with chunk_count = position's
    // chunks would falsely flag those as OOB. Repair runs at the
    // splice layer (`splice_l2_into_chat` and `splice_one_position`)
    // where the span context is known.
    let mut gras = secondary_gras;
    if let Some(ctx) = secondary_context
        && ctx.is_phrasal_verb_particle()
    {
        let chat_deprel = UdDeprel::new("compound:prt").to_chat_gra();
        if let Some(rel) = gras.get_mut(0) {
            rel.relation = chat_deprel;
        }
        return MergedL2Morphology {
            mor: secondary_mor,
            gras,
            corrected_deprel: Some(UdDeprel::new("compound:prt")),
            attachment: attachment.with_host_deprel(UdDeprel::new("compound:prt")),
        };
    }

    let primary_constraint = deprel_to_pos_constraint(&primary.deprel);
    let needs_correction =
        primary.deprel.base() == "flat" || !primary_constraint.contains(&resolved_pos);

    let corrected_deprel = if needs_correction {
        let det = infer_deprel_from_pos(
            resolved_pos,
            primary.head_upos,
            primary.has_case_dependent(),
        );
        if let Some(ref d) = det {
            if let Some(rel) = gras.get_mut(0) {
                rel.relation = d.to_chat_gra().into();
            }
        }
        det
    } else {
        None
    };

    let attachment = if attachment.is_external_root() {
        attachment.with_host_deprel(
            corrected_deprel
                .clone()
                .or_else(|| attachment.external_root_deprel().cloned())
                .unwrap_or_else(|| primary.deprel.clone()),
        )
    } else {
        L2Attachment::InternalRoot
    };

    MergedL2Morphology {
        mor: secondary_mor,
        gras,
        corrected_deprel,
        attachment,
    }
}

/// Merge one planned secondary-dispatch span back into per-word L2 morphology.
///
/// This is the transform-layer assembly seam: callers hand over the planned
/// span and the secondary UD sentence, and receive per-position merged
/// morphology ready for final lowering.
pub fn merge_planned_secondary_span(
    span: &L2SpanPlan,
    deferred: &[super::extract::L2DeferredPosition],
    sentence: &UdSentence,
) -> Result<Vec<(usize, MergedL2Morphology)>, MappingError> {
    let mapping_ctx = MappingContext {
        lang: span.target_lang.clone(),
    };
    let (mors, gra_relations) = map_ud_sentence(sentence, &mapping_ctx)?;

    let total_chunks: usize = mors.iter().map(Mor::count_chunks).sum();
    if total_chunks > gra_relations.len() {
        return Err(MappingError::ChunkCountMismatch {
            mor_chunks: total_chunks,
            gra_count: gra_relations.len(),
        });
    }

    if mors.len() < span.deferred_indices.len() {
        tracing::warn!(
            planned_words = span.deferred_indices.len(),
            mapped_words = mors.len(),
            lang = %span.target_lang,
            "L2 planned merge: secondary mapping returned fewer mors than planned words"
        );
    }

    let pass_context = sentence.words.len() == mors.len();
    let mut merged = Vec::new();
    let mut chunk_offset = 0usize;

    for (word_position, global_idx) in span.deferred_indices.iter().copied().enumerate() {
        let Some(primary) = deferred.get(global_idx).map(|item| &item.primary) else {
            tracing::warn!(
                global_idx,
                "L2 planned merge: deferred index out of range; skipping position"
            );
            continue;
        };
        let Some(mor) = mors.get(word_position).cloned() else {
            break;
        };

        let chunk_count = mor.count_chunks();
        let item_gras = gra_relations[chunk_offset..chunk_offset + chunk_count].to_vec();
        chunk_offset += chunk_count;
        let secondary_context = pass_context.then_some(SecondaryUdContext {
            sentence,
            word_position,
        });

        merged.push((
            global_idx,
            merge_primary_secondary_with_attachment(
                primary,
                mor,
                item_gras,
                &span.target_lang,
                &span.attachment,
                secondary_context.as_ref(),
            ),
        ));
    }

    Ok(merged)
}

/// Merge primary structural info with a secondary `Mor` item, with
/// optional secondary UD sentence context for phrasal-verb recognition.
///
/// The `Mor` is pre-mapped from the secondary model's UD response via
/// `map_ud_sentence` (which handles MWT Range tokens for contractions).
/// This function resolves POS via the priority algorithm, overrides the
/// POS in the `Mor`, and computes deprel correction.
///
/// When `secondary_context` is supplied and identifies the current word
/// as a phrasal-verb particle, the `corrected_deprel` is set to
/// `compound:prt` so the CHAT %gra tier reflects the verb-particle
/// structure.
pub fn merge_primary_secondary_with_context(
    primary: &PrimaryStructuralInfo,
    secondary_mor: Mor,
    secondary_gras: Vec<GrammaticalRelation>,
    secondary_lang: &LanguageCode,
    attachment: L2Attachment,
    secondary_context: Option<&SecondaryUdContext<'_>>,
) -> MergedL2Morphology {
    merge_primary_secondary_with_attachment(
        primary,
        secondary_mor,
        secondary_gras,
        secondary_lang,
        &attachment,
        secondary_context,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::morphosyntax::l2::{L2SpanPlan, PrimaryStructuralInfo};

    fn make_primary(deprel: &str, head: usize) -> PrimaryStructuralInfo {
        PrimaryStructuralInfo {
            deprel: UdDeprel::new(deprel),
            upos: Some(UniversalPos::Noun),
            head,
            dependent_deprels: Vec::new(),
            head_upos: Some(UniversalPos::Verb),
        }
    }

    fn make_word(
        id: usize,
        text: &str,
        lemma: &str,
        upos: UniversalPos,
        head: usize,
        deprel: &str,
    ) -> UdWord {
        UdWord {
            id: UdId::Single(id),
            text: text.to_string(),
            lemma: lemma.to_string(),
            upos: crate::morphosyntax::UdPunctable::Value(upos),
            xpos: None,
            feats: None,
            head,
            deprel: deprel.to_string(),
            deps: None,
            misc: None,
        }
    }

    #[test]
    fn phrasal_verb_particle_uses_chat_gra_label() {
        let primary = make_primary("advmod", 1);
        let secondary_mor = Mor::new(talkbank_model::model::dependent_tier::mor::MorWord::new(
            PosCategory::new("adp"),
            "up",
        ));
        let secondary_gras = vec![GrammaticalRelation::new(1, 0, "ROOT")];
        let sentence = UdSentence {
            words: vec![
                make_word(1, "wake", "wake", UniversalPos::Verb, 0, "root"),
                make_word(2, "up", "up", UniversalPos::Adp, 1, "compound:prt"),
            ],
        };
        let ctx = SecondaryUdContext {
            sentence: &sentence,
            word_position: 1,
        };

        let merged = merge_primary_secondary_with_context(
            &primary,
            secondary_mor,
            secondary_gras,
            &LanguageCode::new("eng"),
            L2Attachment::ExternalRoot {
                host_deprel: UdDeprel::new("advmod"),
                root_anchor: crate::morphosyntax::l2::plan::L2RootAnchor::HostGovernor {
                    source_deferred_index: 0,
                },
            },
            Some(&ctx),
        );

        assert_eq!(
            merged.gras.first().map(|rel| rel.relation.as_str()),
            Some("COMPOUND-PRT"),
            "L2 phrasal-particle path must emit CHAT-normalized %gra labels"
        );
        assert_eq!(merged.corrected_deprel, Some(UdDeprel::new("compound:prt")));
    }

    #[test]
    fn merge_planned_secondary_span_carries_attachment_to_root_rewrite() {
        let span = L2SpanPlan {
            deferred_indices: vec![0],
            line_idx: 3,
            target_lang: LanguageCode::new("spa"),
            words: vec![talkbank_model::ChatCleanedText::test_unchecked(
                "extranjero",
            )],
            attachment: L2Attachment::ExternalRoot {
                host_deprel: UdDeprel::new("obj"),
                root_anchor: crate::morphosyntax::l2::plan::L2RootAnchor::HostGovernor {
                    source_deferred_index: 0,
                },
            },
        };
        let deferred = vec![super::super::extract::L2DeferredPosition {
            line_idx: 3,
            word_idx: 1,
            target_lang: LanguageCode::new("spa"),
            primary: make_primary("obj", 1),
        }];
        let sentence = UdSentence {
            words: vec![make_word(
                1,
                "extranjero",
                "extranjero",
                UniversalPos::Noun,
                0,
                "root",
            )],
        };

        let merged = merge_planned_secondary_span(&span, &deferred, &sentence).unwrap();
        assert_eq!(merged.len(), 1);
        assert_eq!(merged[0].0, 0);
        assert_eq!(
            merged[0]
                .1
                .attachment
                .external_root_deprel()
                .map(UdDeprel::as_str),
            Some("obj")
        );
        assert_eq!(merged[0].1.attachment.source_deferred_index(), Some(0));
    }
}
