//! FIXBULLETS -- fix timing bullet consistency.
//!
//! This reimplementation keeps bullet handling on the typed AST. The current
//! supported subset is:
//! - normalize overlapping utterance-terminal bullets on main tiers
//! - apply a global time offset to parsed bullet timings
//! - scope dependent-tier bullet fixing to selected tier kinds
//!
//! Unsupported legacy behavior such as old filename-bearing bullet syntax,
//! `@Media` insertion, multi-bullet line merging (`+b`), and `+l` language-tag
//! expansion is intentionally not emulated silently.

use talkbank_model::{
    BracketedItem, Bullet, BulletContent, BulletContentSegment, ChatFile, Header, Line,
    MediaTiming, UtteranceContent,
};

use crate::framework::{TransformCommand, TransformError};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
/// Configuration for the AST-safe subset of `FIXBULLETS`.
pub struct FixbulletsConfig {
    /// Global millisecond offset applied to parsed bullet timings.
    pub offset_ms: i64,
    /// Dependent-tier kinds to include (`mor`, `%cod`, `*` for main tiers).
    pub include_tiers: Vec<String>,
    /// Dependent-tier kinds to exclude (`gra`, `%com`, `*` for main tiers).
    pub exclude_tiers: Vec<String>,
}

/// FIXBULLETS transform: fix bullet timestamp ordering and offsets.
#[derive(Debug, Default)]
pub struct FixbulletsCommand {
    config: FixbulletsConfig,
}

impl FixbulletsCommand {
    /// Build a configured `FIXBULLETS` transform, validating tier selectors.
    pub fn new(config: FixbulletsConfig) -> Result<Self, TransformError> {
        validate_tier_selectors(&config.include_tiers)?;
        validate_tier_selectors(&config.exclude_tiers)?;
        Ok(Self { config })
    }
}

impl TransformCommand for FixbulletsCommand {
    type Config = FixbulletsConfig;

    /// Enforce non-overlapping, monotonic main-tier bullet timing windows and
    /// shift parsed bullet timings where requested.
    fn transform(&self, file: &mut ChatFile) -> Result<(), TransformError> {
        let mut prev_end_ms: Option<u64> = None;

        for line in file.lines.iter_mut() {
            match line {
                Line::Header { header, .. } => {
                    if self.config.offset_ms != 0 {
                        shift_header_bullets(header.as_mut(), self.config.offset_ms)?;
                    }
                }
                Line::Utterance(utterance) => {
                    if self.config.offset_ms != 0 {
                        for header in &mut utterance.preceding_headers {
                            shift_header_bullets(header, self.config.offset_ms)?;
                        }
                    }

                    if tier_selected("*", &self.config.include_tiers, &self.config.exclude_tiers)? {
                        shift_main_tier_bullets(
                            &mut utterance.main.content.content,
                            self.config.offset_ms,
                        )?;

                        if let Some(bullet) = &mut utterance.main.content.bullet {
                            shift_bullet(bullet, self.config.offset_ms)?;
                            if let Some(prev_end) = prev_end_ms
                                && bullet.timing.start_ms < prev_end
                            {
                                let duration =
                                    bullet.timing.end_ms.saturating_sub(bullet.timing.start_ms);
                                bullet.timing.start_ms = prev_end + 1;
                                bullet.timing.end_ms = bullet.timing.start_ms + duration.max(1);
                            }
                            prev_end_ms = Some(bullet.timing.end_ms);
                        }
                    } else if let Some(bullet) = &utterance.main.content.bullet {
                        prev_end_ms = Some(bullet.timing.end_ms);
                    }

                    for tier in &mut utterance.dependent_tiers {
                        if tier_selected(
                            tier.kind(),
                            &self.config.include_tiers,
                            &self.config.exclude_tiers,
                        )? {
                            shift_dependent_tier_bullets(tier, self.config.offset_ms)?;
                        }
                    }
                }
            }
        }

        Ok(())
    }
}

fn validate_tier_selectors(selectors: &[String]) -> Result<(), TransformError> {
    for selector in selectors {
        let normalized = selector.trim();
        if normalized.is_empty() {
            return Err(TransformError::Transform(
                "FIXBULLETS tier selectors must not be empty".into(),
            ));
        }
        if normalized.starts_with('*') && normalized != "*" {
            return Err(TransformError::Transform(format!(
                "FIXBULLETS does not support speaker-specific tier selector `{normalized}`; use `*` for all main tiers or dependent-tier codes like `mor`/`%mor`"
            )));
        }
    }
    Ok(())
}

fn normalize_selector(selector: &str) -> String {
    let selector = selector.trim();
    if selector == "*" {
        return "*".into();
    }
    let selector = selector.strip_prefix('%').unwrap_or(selector);
    match selector.to_ascii_lowercase().as_str() {
        "trn" => "mor".into(),
        "grt" => "gra".into(),
        other => other.into(),
    }
}

fn tier_selected(
    kind: &str,
    include: &[String],
    exclude: &[String],
) -> Result<bool, TransformError> {
    let kind = normalize_selector(kind);
    let included = if include.is_empty() {
        true
    } else {
        include
            .iter()
            .map(|selector| normalize_selector(selector))
            .any(|selector| selector == "*" || selector == kind)
    };
    let excluded = exclude
        .iter()
        .map(|selector| normalize_selector(selector))
        .any(|selector| selector == "*" || selector == kind);

    if included && excluded {
        return Err(TransformError::Transform(format!(
            "FIXBULLETS tier selection is contradictory for `{kind}`"
        )));
    }

    Ok(included && !excluded)
}

fn shift_header_bullets(header: &mut Header, offset_ms: i64) -> Result<(), TransformError> {
    if let Header::Comment { content } = header {
        shift_bullet_content(content, offset_ms)?;
    }
    Ok(())
}

fn shift_dependent_tier_bullets(
    tier: &mut talkbank_model::dependent_tier::DependentTier,
    offset_ms: i64,
) -> Result<(), TransformError> {
    use talkbank_model::dependent_tier::DependentTier;

    match tier {
        DependentTier::Act(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Cod(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Add(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Com(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Exp(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Gpx(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Int(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Sit(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Spa(t) => shift_bullet_content(&mut t.content, offset_ms)?,
        DependentTier::Wor(t) => {
            for item in &mut t.items {
                if let talkbank_model::dependent_tier::WorItem::Word(word) = item
                    && let Some(bullet) = &mut word.inline_bullet
                {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            if let Some(bullet) = &mut t.bullet {
                shift_bullet(bullet, offset_ms)?;
            }
        }
        DependentTier::Mor(_)
        | DependentTier::Gra(_)
        | DependentTier::Pho(_)
        | DependentTier::Mod(_)
        | DependentTier::Sin(_)
        | DependentTier::Alt(_)
        | DependentTier::Coh(_)
        | DependentTier::Def(_)
        | DependentTier::Eng(_)
        | DependentTier::Err(_)
        | DependentTier::Fac(_)
        | DependentTier::Flo(_)
        | DependentTier::Modsyl(_)
        | DependentTier::Phosyl(_)
        | DependentTier::Phoaln(_)
        | DependentTier::Gls(_)
        | DependentTier::Ort(_)
        | DependentTier::Par(_)
        | DependentTier::Tim(_)
        | DependentTier::UserDefined(_)
        | DependentTier::Unsupported(_) => {}
    }
    Ok(())
}

fn shift_main_tier_bullets(
    content: &mut [UtteranceContent],
    offset_ms: i64,
) -> Result<(), TransformError> {
    for item in content {
        match item {
            UtteranceContent::Word(word) => {
                if let Some(bullet) = &mut word.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            UtteranceContent::AnnotatedWord(annotated) => {
                if let Some(bullet) = &mut annotated.inner.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            UtteranceContent::ReplacedWord(replaced) => {
                if let Some(bullet) = &mut replaced.word.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            UtteranceContent::Group(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            UtteranceContent::AnnotatedGroup(annotated) => {
                shift_bracketed_bullets(&mut annotated.inner.content.content, offset_ms)?;
            }
            UtteranceContent::Retrace(retrace) => {
                shift_bracketed_bullets(&mut retrace.content.content, offset_ms)?;
            }
            UtteranceContent::PhoGroup(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            UtteranceContent::SinGroup(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            UtteranceContent::Quotation(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            UtteranceContent::InternalBullet(bullet) => {
                shift_bullet(bullet, offset_ms)?;
            }
            UtteranceContent::Event(_)
            | UtteranceContent::AnnotatedEvent(_)
            | UtteranceContent::Pause(_)
            | UtteranceContent::AnnotatedAction(_)
            | UtteranceContent::Freecode(_)
            | UtteranceContent::Separator(_)
            | UtteranceContent::OverlapPoint(_)
            | UtteranceContent::LongFeatureBegin(_)
            | UtteranceContent::LongFeatureEnd(_)
            | UtteranceContent::UnderlineBegin(_)
            | UtteranceContent::UnderlineEnd(_)
            | UtteranceContent::NonvocalBegin(_)
            | UtteranceContent::NonvocalEnd(_)
            | UtteranceContent::NonvocalSimple(_)
            | UtteranceContent::OtherSpokenEvent(_) => {}
        }
    }
    Ok(())
}

fn shift_bracketed_bullets(
    content: &mut [BracketedItem],
    offset_ms: i64,
) -> Result<(), TransformError> {
    for item in content {
        match item {
            BracketedItem::Word(word) => {
                if let Some(bullet) = &mut word.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            BracketedItem::AnnotatedWord(annotated) => {
                if let Some(bullet) = &mut annotated.inner.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            BracketedItem::ReplacedWord(replaced) => {
                if let Some(bullet) = &mut replaced.word.inline_bullet {
                    shift_bullet(bullet, offset_ms)?;
                }
            }
            BracketedItem::AnnotatedGroup(annotated) => {
                shift_bracketed_bullets(&mut annotated.inner.content.content, offset_ms)?;
            }
            BracketedItem::Retrace(retrace) => {
                shift_bracketed_bullets(&mut retrace.content.content, offset_ms)?;
            }
            BracketedItem::PhoGroup(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            BracketedItem::SinGroup(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            BracketedItem::Quotation(group) => {
                shift_bracketed_bullets(&mut group.content.content, offset_ms)?;
            }
            BracketedItem::InternalBullet(bullet) => {
                shift_bullet(bullet, offset_ms)?;
            }
            BracketedItem::Event(_)
            | BracketedItem::AnnotatedEvent(_)
            | BracketedItem::Pause(_)
            | BracketedItem::Action(_)
            | BracketedItem::AnnotatedAction(_)
            | BracketedItem::OverlapPoint(_)
            | BracketedItem::Separator(_)
            | BracketedItem::Freecode(_)
            | BracketedItem::LongFeatureBegin(_)
            | BracketedItem::LongFeatureEnd(_)
            | BracketedItem::UnderlineBegin(_)
            | BracketedItem::UnderlineEnd(_)
            | BracketedItem::NonvocalBegin(_)
            | BracketedItem::NonvocalEnd(_)
            | BracketedItem::NonvocalSimple(_)
            | BracketedItem::OtherSpokenEvent(_) => {}
        }
    }
    Ok(())
}

fn shift_bullet_content(content: &mut BulletContent, offset_ms: i64) -> Result<(), TransformError> {
    for segment in &mut content.segments {
        if let BulletContentSegment::Bullet(timing) = segment {
            shift_media_timing(timing, offset_ms)?;
        }
    }
    Ok(())
}

fn shift_bullet(bullet: &mut Bullet, offset_ms: i64) -> Result<(), TransformError> {
    shift_media_timing(&mut bullet.timing, offset_ms)
}

fn shift_media_timing(timing: &mut MediaTiming, offset_ms: i64) -> Result<(), TransformError> {
    if offset_ms == 0 {
        return Ok(());
    }

    timing.start_ms = shift_u64(timing.start_ms, offset_ms)?;
    timing.end_ms = shift_u64(timing.end_ms, offset_ms)?;
    Ok(())
}

fn shift_u64(value: u64, offset_ms: i64) -> Result<u64, TransformError> {
    let shifted = i128::from(value) + i128::from(offset_ms);
    if shifted < 0 {
        return Err(TransformError::Transform(format!(
            "FIXBULLETS offset {offset_ms} would move bullet timing before 0 ms"
        )));
    }
    u64::try_from(shifted).map_err(|_| {
        TransformError::Transform(format!(
            "FIXBULLETS offset {offset_ms} overflows bullet timing"
        ))
    })
}

#[cfg(test)]
mod tests {
    use talkbank_model::ParseValidateOptions;
    use talkbank_model::{ChatFile, WriteChat};

    use super::{FixbulletsCommand, FixbulletsConfig};
    use crate::framework::TransformCommand;

    fn parse_chat(input: &str) -> ChatFile {
        talkbank_transform::parse_and_validate(input, ParseValidateOptions::default())
            .expect("test fixture should parse")
    }

    #[test]
    fn fixbullets_normalizes_main_tier_overlap() {
        let mut file = parse_chat(
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello . \u{0015}100_200\u{0015}\n*MOT:\thi . \u{0015}150_175\u{0015}\n@End\n",
        );

        FixbulletsCommand::default()
            .transform(&mut file)
            .expect("FIXBULLETS should succeed");

        let output = file.to_chat_string();
        assert!(output.contains("\u{0015}100_200\u{0015}"));
        assert!(output.contains("\u{0015}201_226\u{0015}"));
    }

    #[test]
    fn fixbullets_applies_offset_to_header_main_word_and_dependent_bullets() {
        let mut file = parse_chat(
            "@UTF8\n@Begin\n@Languages:\teng\n@Media:\ttest, video\n@Comment:\theader \u{0015}10_20\u{0015}\n*CHI:\thello \u{0015}30_40\u{0015} there . \u{0015}100_150\u{0015}\n%cod:\tcode \u{0015}200_250\u{0015}\n@End\n",
        );

        let command = FixbulletsCommand::new(FixbulletsConfig {
            offset_ms: 50,
            include_tiers: vec![],
            exclude_tiers: vec![],
        })
        .expect("config should be valid");
        command
            .transform(&mut file)
            .expect("FIXBULLETS should succeed");

        let output = file.to_chat_string();
        assert!(output.contains("\u{0015}60_70\u{0015}"));
        assert!(output.contains("\u{0015}80_90\u{0015}"));
        assert!(output.contains("\u{0015}150_200\u{0015}"));
        assert!(output.contains("\u{0015}250_300\u{0015}"));
        assert!(!output.contains("\u{0015}10_20\u{0015}"));
        assert!(!output.contains("\u{0015}30_40\u{0015}"));
        assert!(!output.contains("\u{0015}100_150\u{0015}"));
        assert!(!output.contains("\u{0015}200_250\u{0015}"));
    }

    #[test]
    fn fixbullets_respects_tier_filters() {
        let mut file = parse_chat(
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello . \u{0015}100_150\u{0015}\n%cod:\tcode \u{0015}200_250\u{0015}\n%com:\tcomment \u{0015}300_350\u{0015}\n@End\n",
        );

        let command = FixbulletsCommand::new(FixbulletsConfig {
            offset_ms: 25,
            include_tiers: vec!["%cod".into()],
            exclude_tiers: vec![],
        })
        .expect("config should be valid");
        command
            .transform(&mut file)
            .expect("FIXBULLETS should succeed");

        let output = file.to_chat_string();
        assert!(output.contains("*CHI:\thello . \u{0015}100_150\u{0015}"));
        assert!(output.contains("%cod:\tcode \u{0015}225_275\u{0015}"));
        assert!(output.contains("%com:\tcomment \u{0015}300_350\u{0015}"));
    }

    #[test]
    fn fixbullets_rejects_negative_shift_past_zero() {
        let mut file = parse_chat(
            "@UTF8\n@Begin\n@Languages:\teng\n*CHI:\thello . \u{0015}10_20\u{0015}\n@End\n",
        );

        let command = FixbulletsCommand::new(FixbulletsConfig {
            offset_ms: -15,
            include_tiers: vec![],
            exclude_tiers: vec![],
        })
        .expect("config should be valid");

        let err = command
            .transform(&mut file)
            .expect_err("negative shift past zero should fail");
        assert!(err.to_string().contains("before 0 ms"));
    }

    #[test]
    fn fixbullets_rejects_speaker_specific_selector() {
        let err = FixbulletsCommand::new(FixbulletsConfig {
            offset_ms: 0,
            include_tiers: vec!["*CHI".into()],
            exclude_tiers: vec![],
        })
        .expect_err("speaker-specific selector should fail");

        assert!(err.to_string().contains("speaker-specific"));
    }
}
