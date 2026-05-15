//! Helper functions for sentence-level UD-to-CHAT mapping.

use crate::morphosyntax::{
    ChunkHead, ChunkProvenance, MappingContext, MappingError, MorProvenance, UdId, UdWord,
    is_clitic, map_ud_word_to_mor,
};
use smallvec::{SmallVec, smallvec};
use std::borrow::Cow;
use talkbank_model::model::GrammaticalRelationType;
use talkbank_model::model::dependent_tier::mor::Mor;

/// Normalize a UD deprel to a validated CHAT `%gra` relation label.
pub fn normalize_deprel(
    raw: &str,
    context_for_error: impl FnOnce() -> String,
) -> Result<GrammaticalRelationType, MappingError> {
    let needs_transform = raw.bytes().any(|b| b.is_ascii_lowercase() || b == b':');
    let relation: Cow<'_, str> = if needs_transform {
        Cow::Owned(raw.to_uppercase().replace(':', "-"))
    } else {
        Cow::Borrowed(raw)
    };
    let bytes = relation.as_bytes();
    if bytes.is_empty()
        || !bytes[0].is_ascii_uppercase()
        || !bytes
            .iter()
            .all(|&b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-')
    {
        return Err(MappingError::InvalidDeprel {
            details: format!(
                "{}: deprel {:?} transforms to {:?} — not a valid CHAT %gra relation (must match [A-Z][A-Z0-9-]*)",
                context_for_error(),
                raw,
                relation.as_ref()
            ),
        });
    }
    Ok(GrammaticalRelationType::new(relation.as_ref()))
}

/// Build chunk provenance for a regular UD word that produced one chunk.
pub fn provenance_for_ud_word(ud: &UdWord) -> Result<ChunkProvenance, MappingError> {
    let source_ud_ids = match ud.id {
        UdId::Single(id) => smallvec![id],
        UdId::Range(start, _end) => smallvec![start],
        UdId::Decimal(_) => SmallVec::new(),
    };
    let head = ChunkHead::from_ud_head(ud.head);
    let deprel = normalize_deprel(&ud.deprel, || format!("word {:?}", ud.text))?;
    Ok(ChunkProvenance {
        source_ud_ids,
        head,
        deprel,
    })
}

/// Assemble multiple UD tokens into a single CHAT MOR with clitics, plus one
/// `ChunkProvenance` per emitted chunk.
pub fn assemble_mors(
    components: &[UdWord],
    ctx: &MappingContext,
) -> Result<(Mor, MorProvenance), MappingError> {
    if components.is_empty() {
        return Err(MappingError::EmptyRangeComponents);
    }

    let mut main_idx = 0;
    for (idx, comp) in components.iter().enumerate() {
        if !is_clitic(&comp.text, ctx) {
            main_idx = idx;
            break;
        }
    }

    let mut mor = map_ud_word_to_mor(&components[main_idx], ctx)?;
    for comp in &components[..main_idx] {
        let m = map_ud_word_to_mor(comp, ctx)?;
        mor = mor.with_post_clitic(m.main);
    }
    for comp in &components[main_idx + 1..] {
        let m = map_ud_word_to_mor(comp, ctx)?;
        mor = mor.with_post_clitic(m.main);
    }

    let mut prov: MorProvenance = SmallVec::new();
    prov.push(provenance_for_ud_word(&components[main_idx])?);
    for comp in &components[..main_idx] {
        prov.push(provenance_for_ud_word(comp)?);
    }
    for comp in &components[main_idx + 1..] {
        prov.push(provenance_for_ud_word(comp)?);
    }

    Ok((mor, prov))
}
