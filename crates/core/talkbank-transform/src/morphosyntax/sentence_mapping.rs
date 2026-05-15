//! Generic sentence-level UD-to-CHAT mapping helpers.

use crate::morphosyntax::{
    ChunkHead, MappingContext, MappingError, MorProvenance, UdId, UdPunctable, UdSentence, UdWord,
    UniversalPos, assemble_mors, lang2, map_ud_word_to_mor, provenance_for_ud_word,
    try_handle_italian_range_override, try_handle_italian_single_override, validate_generated_gra,
};
use smallvec::smallvec;
use std::collections::HashMap;
use talkbank_model::model::GrammaticalRelation;
use talkbank_model::model::dependent_tier::mor::Mor;

/// How the caller handles the sentence-terminator PUNCT relation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TerminatorPolicy {
    /// Append a trailing terminator relation after chunk emission.
    AppendTrailingPunct,
    /// Terminators are already represented as ordinary chunks.
    AlreadyInChunks,
}

/// Map a UD sentence to MOR and GRA structures with Range tokens expanded into
/// per-component items instead of merged clitics.
pub fn map_ud_sentence_expanded(
    sentence: &UdSentence,
    ctx: &MappingContext,
) -> Result<(Vec<Mor>, Vec<GrammaticalRelation>), MappingError> {
    let cap = sentence.words.len();
    let mut mors: Vec<Mor> = Vec::with_capacity(cap);
    let mut provenance: Vec<MorProvenance> = Vec::with_capacity(cap);
    let mut i = 0;
    let push_ud = |ud: &UdWord,
                   mors: &mut Vec<Mor>,
                   provenance: &mut Vec<MorProvenance>,
                   ctx: &MappingContext|
     -> Result<(), MappingError> {
        mors.push(map_ud_word_to_mor(ud, ctx)?);
        provenance.push(smallvec![provenance_for_ud_word(ud)?]);
        Ok(())
    };
    while i < sentence.words.len() {
        let ud = &sentence.words[i];
        match &ud.id {
            UdId::Range(start, end) => {
                let count = end - start + 1;
                let next_idx = i + 1;
                if next_idx + count <= sentence.words.len() {
                    for comp in &sentence.words[next_idx..next_idx + count] {
                        push_ud(comp, &mut mors, &mut provenance, ctx)?;
                    }
                    i += 1 + count;
                } else {
                    push_ud(ud, &mut mors, &mut provenance, ctx)?;
                    i += 1;
                }
            }
            UdId::Single(_) => {
                // Terminator-punct is supplied separately via `Terminator`;
                // `AppendTrailingPunct` adds the matching PUNCT relation.
                if !is_terminator_punct(ud) {
                    push_ud(ud, &mut mors, &mut provenance, ctx)?;
                }
                i += 1;
            }
            UdId::Decimal(_) => {
                i += 1;
            }
        }
    }

    build_gra_and_validate(&mors, &provenance, TerminatorPolicy::AppendTrailingPunct)
        .map(|gras| (mors, gras))
}

/// Map a UD sentence to MOR and GRA structures using the canonical deterministic
/// language-specific overrides that belong in `talkbank-transform`.
///
/// Today that means the shared sentence loop plus the canonical Italian
/// reconciler hooks. Batchalign runtime code should call this entrypoint rather
/// than hosting its own sentence-level wrapper logic.
pub fn map_ud_sentence(
    sentence: &UdSentence,
    ctx: &MappingContext,
) -> Result<(Vec<Mor>, Vec<GrammaticalRelation>), MappingError> {
    let is_it = lang2(&ctx.lang) == "it";
    map_ud_sentence_with_overrides(
        sentence,
        ctx,
        TerminatorPolicy::AppendTrailingPunct,
        |ud, components, ctx| {
            if is_it {
                try_handle_italian_range_override(ud, components, ctx)
            } else {
                Ok(None)
            }
        },
        |ud, ctx| {
            if is_it {
                try_handle_italian_single_override(ud, ctx)
            } else {
                Ok(None)
            }
        },
    )
}

/// Map a UD sentence to MOR and GRA structures while allowing caller-supplied
/// overrides for complete MWT ranges and non-terminator single tokens.
///
/// This is the canonical language-neutral sentence mapper. Callers can keep
/// language-specific hacks outside `talkbank-transform` by returning `Some`
/// from the override closures for the cases they need to intercept, while all
/// default sentence walking, MWT assembly, terminator filtering, and `%gra`
/// building stay shared here.
pub fn map_ud_sentence_with_overrides<RangeOverride, SingleOverride>(
    sentence: &UdSentence,
    ctx: &MappingContext,
    terminator_policy: TerminatorPolicy,
    mut range_override: RangeOverride,
    mut single_override: SingleOverride,
) -> Result<(Vec<Mor>, Vec<GrammaticalRelation>), MappingError>
where
    RangeOverride: FnMut(
        &UdWord,
        &[UdWord],
        &MappingContext,
    ) -> Result<Option<(Mor, MorProvenance)>, MappingError>,
    SingleOverride:
        FnMut(&UdWord, &MappingContext) -> Result<Option<(Mor, MorProvenance)>, MappingError>,
{
    let cap = sentence.words.len();
    let mut mors: Vec<Mor> = Vec::with_capacity(cap);
    let mut provenance: Vec<MorProvenance> = Vec::with_capacity(cap);
    let mut i = 0;

    while i < sentence.words.len() {
        let ud = &sentence.words[i];

        match &ud.id {
            UdId::Range(start, end) => {
                let count = end - start + 1;
                let next_idx = i + 1;
                if next_idx + count <= sentence.words.len() {
                    let components = &sentence.words[next_idx..next_idx + count];
                    if let Some((mor, prov)) = range_override(ud, components, ctx)? {
                        mors.push(mor);
                        provenance.push(prov);
                    } else {
                        let (mor, prov) = assemble_mors(components, ctx)?;
                        mors.push(mor);
                        provenance.push(prov);
                    }
                    i += 1 + count;
                } else {
                    let (mor, prov) = map_ud_word_with_provenance(ud, ctx)?;
                    mors.push(mor);
                    provenance.push(prov);
                    i += 1;
                }
            }
            UdId::Single(_) => {
                if !is_terminator_punct(ud) {
                    if let Some((mor, prov)) = single_override(ud, ctx)? {
                        mors.push(mor);
                        provenance.push(prov);
                    } else {
                        let (mor, prov) = map_ud_word_with_provenance(ud, ctx)?;
                        mors.push(mor);
                        provenance.push(prov);
                    }
                }
                i += 1;
            }
            UdId::Decimal(_) => {
                i += 1;
            }
        }
    }

    build_gra_and_validate(&mors, &provenance, terminator_policy).map(|gras| (mors, gras))
}

/// Language-neutral GRA builder + validator.
pub fn build_gra_and_validate(
    mors: &[Mor],
    provenance: &[MorProvenance],
    terminator_policy: TerminatorPolicy,
) -> Result<Vec<GrammaticalRelation>, MappingError> {
    if mors.len() != provenance.len() {
        return Err(MappingError::ChunkCountMismatch {
            mor_chunks: mors.len(),
            gra_count: provenance.len(),
        });
    }
    for (i, (mor, prov)) in mors.iter().zip(provenance.iter()).enumerate() {
        if mor.count_chunks() != prov.len() {
            return Err(MappingError::ChunkCountMismatch {
                mor_chunks: mor.count_chunks(),
                gra_count: prov.len() + i,
            });
        }
    }

    let total_chunks: usize = mors.iter().map(|m| m.count_chunks()).sum();
    let mut ud_to_chunk_idx: HashMap<usize, usize> = HashMap::with_capacity(total_chunks);
    {
        let mut ci = 1usize;
        for prov_list in provenance {
            for (offset, chunk_prov) in prov_list.iter().enumerate() {
                let chunk_ci = ci + offset;
                for &ud_id in &chunk_prov.source_ud_ids {
                    ud_to_chunk_idx.insert(ud_id, chunk_ci);
                }
            }
            ci += prov_list.len();
        }
    }

    let mut gras: Vec<GrammaticalRelation> = Vec::with_capacity(total_chunks + 1);
    let mut root_chunk_idx = 0usize;
    {
        let mut ci = 1usize;
        for prov_list in provenance {
            let main_ci = ci;
            for (offset, chunk_prov) in prov_list.iter().enumerate() {
                let chunk_ci = main_ci + offset;
                let (head_ci, relation) = match &chunk_prov.head {
                    ChunkHead::Root => {
                        root_chunk_idx = chunk_ci;
                        (0, "ROOT".into())
                    }
                    ChunkHead::FromUd(ud_head_id) => (
                        *ud_to_chunk_idx.get(ud_head_id).ok_or_else(|| {
                            MappingError::InvalidHeadReference {
                                details: format!(
                                    "chunk {} (deprel={}) has UD head {} not mapped to any chunk",
                                    chunk_ci, chunk_prov.deprel, ud_head_id
                                ),
                            }
                        })?,
                        chunk_prov.deprel.clone(),
                    ),
                    ChunkHead::OwningMorMain => (main_ci, chunk_prov.deprel.clone()),
                };
                gras.push(GrammaticalRelation {
                    index: chunk_ci,
                    head: head_ci,
                    relation,
                });
            }
            ci += prov_list.len();
        }
    }

    if root_chunk_idx == 0 && !gras.is_empty() {
        return Err(MappingError::InvalidRoot {
            details: format!(
                "no chunk with ChunkHead::Root in provenance (Stanza returned no root). GRA so far: {:?}",
                gras
            ),
        });
    }

    if terminator_policy == TerminatorPolicy::AppendTrailingPunct {
        gras.push(GrammaticalRelation {
            index: total_chunks + 1,
            head: root_chunk_idx,
            relation: "PUNCT".into(),
        });
    }

    validate_generated_gra(&gras)?;

    let expected_gra_count = total_chunks
        + match terminator_policy {
            TerminatorPolicy::AppendTrailingPunct => 1,
            TerminatorPolicy::AlreadyInChunks => 0,
        };
    if gras.len() != expected_gra_count {
        return Err(MappingError::ChunkCountMismatch {
            mor_chunks: expected_gra_count,
            gra_count: gras.len(),
        });
    }

    Ok(gras)
}

/// Return whether a UD word is a CHAT utterance terminator.
pub fn is_terminator_punct(ud: &UdWord) -> bool {
    if !matches!(ud.id, UdId::Single(_)) {
        return false;
    }
    let is_punct_pos = matches!(
        ud.upos,
        UdPunctable::Value(UniversalPos::Punct) | UdPunctable::Punct(_)
    );
    if !is_punct_pos {
        return false;
    }
    use talkbank_model::model::content::Terminator;
    Terminator::is_chat_terminator(ud.lemma.trim())
        || Terminator::is_chat_terminator(ud.text.trim())
}

fn map_ud_word_with_provenance(
    ud: &UdWord,
    ctx: &MappingContext,
) -> Result<(Mor, MorProvenance), MappingError> {
    let mor = map_ud_word_to_mor(ud, ctx)?;
    let provenance = smallvec![provenance_for_ud_word(ud)?];
    Ok((mor, provenance))
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::model::LanguageCode;

    fn ud_word(
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
            upos: UdPunctable::Value(upos),
            xpos: None,
            feats: None,
            head,
            deprel: deprel.to_string(),
            deps: None,
            misc: None,
        }
    }

    #[test]
    fn map_ud_sentence_normalizes_compound_prt_to_chat_gra() {
        let sentence = UdSentence {
            words: vec![
                ud_word(1, "wake", "wake", UniversalPos::Verb, 0, "root"),
                ud_word(2, "up", "up", UniversalPos::Adp, 1, "compound:prt"),
                ud_word(3, ".", ".", UniversalPos::Punct, 1, "punct"),
            ],
        };
        let ctx = MappingContext {
            lang: LanguageCode::new("eng"),
        };

        let (_mors, gras) = map_ud_sentence(&sentence, &ctx).expect("map UD sentence");

        let actual: Vec<String> = gras.iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            vec![
                "1|0|ROOT".to_string(),
                "2|1|COMPOUND-PRT".to_string(),
                "3|1|PUNCT".to_string(),
            ]
        );
    }

    #[test]
    fn map_ud_sentence_rewrites_head_zero_dep_to_root() {
        let sentence = UdSentence {
            words: vec![
                ud_word(1, "uh", "uh", UniversalPos::Intj, 0, "dep"),
                ud_word(2, ".", ".", UniversalPos::Punct, 1, "punct"),
            ],
        };
        let ctx = MappingContext {
            lang: LanguageCode::new("eng"),
        };

        let (_mors, gras) = map_ud_sentence(&sentence, &ctx).expect("map UD sentence");

        let actual: Vec<String> = gras.iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            vec!["1|0|ROOT".to_string(), "2|1|PUNCT".to_string()]
        );
    }

    #[test]
    fn map_ud_sentence_rewrites_head_zero_discourse_to_root() {
        let sentence = UdSentence {
            words: vec![
                ud_word(1, "well", "well", UniversalPos::Intj, 0, "discourse"),
                ud_word(2, ".", ".", UniversalPos::Punct, 1, "punct"),
            ],
        };
        let ctx = MappingContext {
            lang: LanguageCode::new("eng"),
        };

        let (_mors, gras) = map_ud_sentence(&sentence, &ctx).expect("map UD sentence");

        let actual: Vec<String> = gras.iter().map(ToString::to_string).collect();
        assert_eq!(
            actual,
            vec!["1|0|ROOT".to_string(), "2|1|PUNCT".to_string()]
        );
    }
}
