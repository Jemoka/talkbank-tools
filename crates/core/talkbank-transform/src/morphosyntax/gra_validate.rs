//! Validation of generated `%gra` structures.

use crate::morphosyntax::MappingError;
use std::collections::HashSet;
use talkbank_model::model::GrammaticalRelation;

/// Whether a self-headed relation (`head == index`) counts as a valid
/// structural root. Mirrors `talkbank_model`'s `is_valid_root_relation`:
/// a self-loop is a root only when the deprel is literally `ROOT`
/// (case-insensitive). Self-loops with any other deprel are cycles
/// per the wider validator's rule, and the L2 splice's post-validation
/// gate must agree to avoid the wild `chunk N|N|DISCOURSE` shape from
/// `~/talkbank/still-have-error-6.log` (Cantonese L2 splice cases at
/// `EACMC/long/Leo/Cantonese/020716.cha:3634, 3642`).
fn is_self_loop_a_valid_root(rel: &GrammaticalRelation) -> bool {
    rel.head == rel.index && rel.relation.as_str().eq_ignore_ascii_case("ROOT")
}

/// Validate that generated `%gra` relations form a valid dependency tree.
pub fn validate_generated_gra(gras: &[GrammaticalRelation]) -> Result<(), MappingError> {
    if gras.is_empty() {
        return Ok(());
    }

    let mut roots = Vec::new();
    for rel in gras {
        if rel.head == 0 || is_self_loop_a_valid_root(rel) {
            roots.push(rel.index);
        }
    }

    let non_terminator_roots: Vec<_> = roots
        .iter()
        .filter(|&&idx| idx != gras.len())
        .copied()
        .collect();

    if non_terminator_roots.is_empty() {
        return Err(MappingError::InvalidRoot {
            details: format!("no ROOT relation. GRA: {:?}", gras),
        });
    }

    if non_terminator_roots.len() > 1 {
        return Err(MappingError::InvalidRoot {
            details: format!(
                "multiple ROOT relations: {:?}. GRA: {:?}",
                non_terminator_roots, gras
            ),
        });
    }

    if let Some(word) = has_any_cycle_generated(gras) {
        return Err(MappingError::CircularDependency {
            details: format!("involving word {}. GRA: {:?}", word, gras),
        });
    }

    let max_index = gras.len();
    for rel in gras {
        if rel.head != 0 && rel.head > max_index {
            return Err(MappingError::InvalidHeadReference {
                details: format!(
                    "word {} points to non-existent word {}. GRA: {:?}",
                    rel.index, rel.head, gras
                ),
            });
        }
    }

    Ok(())
}

fn has_any_cycle_generated(gras: &[GrammaticalRelation]) -> Option<usize> {
    let mut safe: HashSet<usize> = HashSet::new();
    for rel in gras {
        if safe.contains(&rel.index) {
            continue;
        }
        let mut path: HashSet<usize> = HashSet::new();
        let mut current = rel.index;
        loop {
            if safe.contains(&current) {
                safe.extend(&path);
                break;
            }
            if path.contains(&current) {
                return Some(current);
            }
            path.insert(current);
            if let Some(r) = gras.iter().find(|r| r.index == current) {
                if r.head == 0 || is_self_loop_a_valid_root(r) {
                    safe.extend(&path);
                    break;
                }
                current = r.head;
            } else {
                safe.extend(&path);
                break;
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Wild shape from `~/talkbank/still-have-error-6.log`
    /// (`EACMC/long/Leo/Cantonese/020716.cha:3642`):
    /// `1|5|NSUBJ 2|5|PUNCT 3|4|CASE 4|5|NSUBJ 5|5|DISCOURSE 6|5|DISCOURSE 7|5|PUNCT`
    /// — chunk 5 is `head=5/DISCOURSE`, a self-loop with non-ROOT
    /// deprel. The wider `chatter validate` flags this as E724;
    /// `validate_generated_gra` must agree.
    #[test]
    fn self_loop_with_non_root_deprel_is_a_cycle() {
        let gras = vec![
            GrammaticalRelation::new(1, 5, "NSUBJ"),
            GrammaticalRelation::new(2, 5, "PUNCT"),
            GrammaticalRelation::new(3, 4, "CASE"),
            GrammaticalRelation::new(4, 5, "NSUBJ"),
            GrammaticalRelation::new(5, 5, "DISCOURSE"),
            GrammaticalRelation::new(6, 5, "DISCOURSE"),
            GrammaticalRelation::new(7, 5, "PUNCT"),
        ];
        let err = validate_generated_gra(&gras)
            .expect_err("self-loop with non-ROOT deprel must fail validation");
        assert!(
            matches!(
                err,
                MappingError::InvalidRoot { .. } | MappingError::CircularDependency { .. }
            ),
            "expected cycle/root error; got {err:?}",
        );
    }

    /// TalkBank variant: a self-headed relation explicitly labelled
    /// ROOT is the structural root (see `talkbank_model::GrammaticalRelation::is_root`).
    /// Must continue to validate.
    #[test]
    fn self_loop_with_root_deprel_is_accepted_as_structural_root() {
        let gras = vec![
            GrammaticalRelation::new(1, 2, "DET"),
            GrammaticalRelation::new(2, 2, "ROOT"),
            GrammaticalRelation::new(3, 2, "PUNCT"),
        ];
        validate_generated_gra(&gras)
            .expect("self-headed ROOT must be accepted as structural root");
    }

    /// Canonical UD: head=0 deprel=ROOT.
    #[test]
    fn canonical_head_zero_root_validates() {
        let gras = vec![
            GrammaticalRelation::new(1, 2, "DET"),
            GrammaticalRelation::new(2, 0, "ROOT"),
            GrammaticalRelation::new(3, 2, "PUNCT"),
        ];
        validate_generated_gra(&gras).expect("canonical head=0/ROOT must validate");
    }
}
