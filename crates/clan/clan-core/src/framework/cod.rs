use talkbank_model::{BulletContentPicture, BulletContentSegment, CodTier, MediaTiming};

/// Clan-local semantic interpretation of a `%cod` tier.
///
/// This is intentionally a derived layer built on top of the current CHAT AST,
/// not a claim about the canonical TalkBank data model. It captures the minimum
/// item structure needed by CLAN commands that treat `%cod` as a sequence of
/// code-bearing units.
#[derive(Debug, Clone, PartialEq)]
pub struct CodSemanticTier {
    /// Semantic `%cod` elements in source order.
    pub elements: Vec<CodSemanticElement>,
}

/// One semantic element in a `%cod` tier.
#[derive(Debug, Clone, PartialEq)]
pub enum CodSemanticElement {
    /// A code-bearing item, optionally scoped to a target selector.
    Item(CodSemanticItem),
    /// A selector token with no following value token.
    BareTarget(String),
    /// An inline media timing bullet preserved from the parsed tier.
    Bullet(MediaTiming),
    /// A continuation marker preserved from the parsed tier.
    Continuation,
    /// An inline picture reference preserved from the parsed tier.
    Picture(BulletContentPicture),
}

/// One code-bearing `%cod` item.
#[derive(Debug, Clone, PartialEq)]
pub struct CodSemanticItem {
    /// Raw bracketed target selector, such as `<w4>` or `<w4-5>`.
    pub target_raw: Option<String>,
    /// Raw code value token, such as `$WR` or `PL`.
    pub value_raw: String,
}

/// Derive a conservative semantic `%cod` model from the parsed `CodTier`.
///
/// The interpretation is:
/// - each non-whitespace token is a code value item
/// - a bracketed selector token (`<...>`) applies to the next value token
/// - bullets/pictures/continuations are preserved as separate elements
/// - a selector with no following value is preserved as `BareTarget`
pub fn cod_semantic_tier(tier: &CodTier) -> CodSemanticTier {
    let mut elements = Vec::new();
    let mut pending_target: Option<String> = None;

    for segment in &tier.content.segments {
        match segment {
            BulletContentSegment::Text(text) => {
                for token in text.text.split_whitespace() {
                    if is_cod_target_token(token) {
                        if let Some(target) = pending_target.replace(token.to_owned()) {
                            elements.push(CodSemanticElement::BareTarget(target));
                        }
                    } else {
                        elements.push(CodSemanticElement::Item(CodSemanticItem {
                            target_raw: pending_target.take(),
                            value_raw: token.to_owned(),
                        }));
                    }
                }
            }
            BulletContentSegment::Bullet(bullet) => {
                if let Some(target) = pending_target.take() {
                    elements.push(CodSemanticElement::BareTarget(target));
                }
                elements.push(CodSemanticElement::Bullet(*bullet));
            }
            BulletContentSegment::Continuation => {
                if let Some(target) = pending_target.take() {
                    elements.push(CodSemanticElement::BareTarget(target));
                }
                elements.push(CodSemanticElement::Continuation);
            }
            BulletContentSegment::Picture(picture) => {
                if let Some(target) = pending_target.take() {
                    elements.push(CodSemanticElement::BareTarget(target));
                }
                elements.push(CodSemanticElement::Picture(picture.clone()));
            }
        }
    }

    if let Some(target) = pending_target {
        elements.push(CodSemanticElement::BareTarget(target));
    }

    CodSemanticTier { elements }
}

/// Return `%cod` code values in tier order, excluding bare target selectors and
/// punctuation-only terminators used by some corpora.
pub fn cod_item_values(tier: &CodTier) -> Vec<String> {
    cod_semantic_tier(tier)
        .elements
        .into_iter()
        .filter_map(|element| match element {
            CodSemanticElement::Item(item) if item.value_raw != "." => Some(item.value_raw),
            _ => None,
        })
        .collect()
}

fn is_cod_target_token(token: &str) -> bool {
    token.starts_with('<') && token.ends_with('>') && token.len() >= 2
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::{BulletContent, BulletContentSegment};

    #[test]
    fn cod_semantics_assign_selector_to_next_value() {
        let tier = CodTier::from_text("<w5> dawn <wl> $WR");
        let semantic = cod_semantic_tier(&tier);
        assert_eq!(
            semantic.elements,
            vec![
                CodSemanticElement::Item(CodSemanticItem {
                    target_raw: Some("<w5>".to_owned()),
                    value_raw: "dawn".to_owned(),
                }),
                CodSemanticElement::Item(CodSemanticItem {
                    target_raw: Some("<wl>".to_owned()),
                    value_raw: "$WR".to_owned(),
                }),
            ]
        );
    }

    #[test]
    fn cod_semantics_preserve_bare_target() {
        let tier = CodTier::from_text("<w4>");
        let semantic = cod_semantic_tier(&tier);
        assert_eq!(
            semantic.elements,
            vec![CodSemanticElement::BareTarget("<w4>".to_owned())]
        );
    }

    #[test]
    fn cod_semantics_preserve_bullets() {
        let tier = CodTier::new(BulletContent {
            segments: vec![
                BulletContentSegment::text("$UNK "),
                BulletContentSegment::bullet(10, 20),
                BulletContentSegment::text("$na"),
            ]
            .into(),
        });
        let semantic = cod_semantic_tier(&tier);
        assert_eq!(
            semantic.elements,
            vec![
                CodSemanticElement::Item(CodSemanticItem {
                    target_raw: None,
                    value_raw: "$UNK".to_owned(),
                }),
                CodSemanticElement::Bullet(MediaTiming::new(10, 20)),
                CodSemanticElement::Item(CodSemanticItem {
                    target_raw: None,
                    value_raw: "$na".to_owned(),
                }),
            ]
        );
    }
}
