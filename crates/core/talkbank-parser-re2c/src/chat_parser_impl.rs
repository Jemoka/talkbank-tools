//! Implementation of the `ChatParser` trait for the re2c-based parser.
//!
//! This makes our parser a drop-in replacement for `TreeSitterParser`
//! via the shared `ChatParser` trait. Shared tests verify both parsers
//! produce semantically equivalent output.

use talkbank_model::model::{
    ActTier, AddTier, ChatFile as ModelChatFile, CodTier, ComTier,
    DependentTier as ModelDependentTier, ExpTier, GpxTier, GraTier as ModelGraTier,
    GrammaticalRelation, Header, IDHeader, IntTier, MainTier as ModelMainTier,
    MorTier as ModelMorTier, MorWord, ParticipantEntry, PhoTier as ModelPhoTier, PhoWord, SitTier,
    SpaTier, Utterance as ModelUtterance, WorTier, Word,
};
use talkbank_model::{ChatParser, ErrorSink, ParseOutcome, SpanShift};

/// Re2c-based CHAT parser implementing the shared `ChatParser` trait.
///
/// Unlike `TreeSitterParser`, this parser uses re2c for lexing and
/// a handwritten recursive-descent parser. It does not require
/// tree-sitter to be installed or configured.
pub struct Re2cParser;

impl Re2cParser {
    /// Create a new re2c parser instance.
    ///
    /// Unlike `TreeSitterParser`, this is zero-cost — no grammar loading,
    /// no internal buffers. The parser is stateless and `Send + Sync`.
    pub fn new() -> Self {
        Self
    }
}

impl Default for Re2cParser {
    fn default() -> Self {
        Self::new()
    }
}

/// Apply offset-based span shifting to a parsed result.
///
/// When `offset > 0`, all `Span` fields in the model type are shifted
/// forward by `offset` bytes. This supports embedded CHAT fragments where
/// spans must map back to positions in a larger document.
///
/// Note: Currently all re2c parser spans are `Span::DUMMY` (0,0).
/// `SpanShift::shift_spans_after` skips dummy spans by design, so this
/// is a no-op until the re2c parser produces real byte-offset spans.
fn shifted<T: SpanShift>(mut value: T, offset: usize) -> T {
    if offset > 0 {
        value.shift_spans_after(0, offset as i32);
    }
    value
}

impl ChatParser for Re2cParser {
    fn parser_name(&self) -> &'static str {
        "Re2cParser"
    }

    fn parse_chat_file(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelChatFile> {
        let mut file = crate::parser::parse_chat_file_to_model(input, errors);
        if offset > 0 {
            // ChatFile<S> derives SpanShift but requires S: SpanShift, and
            // NotValidated is a zero-size marker without that impl.
            // Shift each line individually instead.
            for line in file.lines.iter_mut() {
                line.shift_spans_after(0, offset as i32);
            }
        }
        ParseOutcome::parsed(file)
    }

    fn parse_header(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        let parsed = crate::parser::parse_chat_file(input);
        for line in &parsed.lines {
            if let crate::ast::Line::Header(h) = line {
                return ParseOutcome::parsed(shifted(
                    crate::convert::header_parsed_to_model(h),
                    offset,
                ));
            }
        }
        ParseOutcome::rejected()
    }

    fn parse_id_header(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        match crate::parser::parse_id_header(input) {
            Some(parsed) => ParseOutcome::parsed(shifted(IDHeader::from(&parsed), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_participant_entry(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        let parsed = crate::parser::parse_participants_header(input);
        match parsed.entries.first() {
            Some(entry) => ParseOutcome::parsed(shifted(ParticipantEntry::from(entry), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_utterance(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelUtterance> {
        let parsed = crate::parser::parse_chat_file(input);
        for line in &parsed.lines {
            if let crate::ast::Line::Utterance(u) = line {
                return ParseOutcome::parsed(shifted(ModelUtterance::from(u.as_ref()), offset));
            }
        }
        ParseOutcome::rejected()
    }

    fn parse_main_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMainTier> {
        match crate::parser::parse_main_tier(input) {
            Some(parsed) => ParseOutcome::parsed(shifted(ModelMainTier::from(&parsed), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_word(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        match crate::parser::parse_word(input) {
            Some(parsed) => ParseOutcome::parsed(shifted(Word::from(&parsed), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_mor_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelMorTier> {
        let parsed = crate::parser::parse_mor_tier(input);
        match ModelMorTier::try_from(&parsed) {
            Ok(tier) => ParseOutcome::parsed(shifted(tier, offset)),
            // AST-to-model conversion failure (missing or unrecognized
            // terminator). Caller pattern-matches on Rejected.
            Err(_) => ParseOutcome::rejected(),
        }
    }

    fn parse_mor_word(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        match crate::parser::parse_mor_word(input) {
            Some(parsed) => ParseOutcome::parsed(shifted(MorWord::from(&parsed), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_gra_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelGraTier> {
        let parsed = crate::parser::parse_gra_tier(input);
        ParseOutcome::parsed(shifted(ModelGraTier::from(&parsed), offset))
    }

    fn parse_gra_relation(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        match crate::parser::parse_gra_relation(input) {
            Some(parsed) => {
                ParseOutcome::parsed(shifted(GrammaticalRelation::from(&parsed), offset))
            }
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_pho_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelPhoTier> {
        let parsed = crate::parser::parse_pho_tier(input);
        ParseOutcome::parsed(shifted(ModelPhoTier::from(&parsed), offset))
    }

    fn parse_pho_word(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        let parsed = crate::parser::parse_pho_tier(input);
        let first_word = parsed.items.iter().find_map(|item| match item {
            crate::ast::PhoItemParsed::Word(w) => Some(w),
            _ => None,
        });
        match first_word {
            Some(w) => ParseOutcome::parsed(shifted(PhoWord::from(w), offset)),
            None => ParseOutcome::rejected(),
        }
    }

    fn parse_sin_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<talkbank_model::model::SinTier> {
        ParseOutcome::parsed(shifted(crate::convert::sin_tier_from_text(input), offset))
    }

    fn parse_act_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_act_tier(&parsed), offset))
    }

    fn parse_cod_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_cod_tier(&parsed), offset))
    }

    fn parse_com_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_com_tier(&parsed), offset))
    }

    fn parse_exp_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_exp_tier(&parsed), offset))
    }

    fn parse_add_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_add_tier(&parsed), offset))
    }

    fn parse_gpx_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_gpx_tier(&parsed), offset))
    }

    fn parse_int_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_int_tier(&parsed), offset))
    }

    fn parse_spa_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_spa_tier(&parsed), offset))
    }

    fn parse_sit_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        let parsed = crate::parser::parse_text_tier(input);
        ParseOutcome::parsed(shifted(crate::convert::to_sit_tier(&parsed), offset))
    }

    fn parse_wor_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        ParseOutcome::parsed(shifted(crate::convert::wor_tier_from_input(input), offset))
    }

    fn parse_dependent_tier(
        &self,
        input: &str,
        offset: usize,
        _errors: &impl ErrorSink,
    ) -> ParseOutcome<ModelDependentTier> {
        let parsed = crate::parser::parse_chat_file(input);
        for line in &parsed.lines {
            if let crate::ast::Line::Utterance(u) = line
                && let Some(tier) = u.dependent_tiers.first()
                && let Some(model_tier) = crate::convert::dependent_tier_to_model(tier)
            {
                return ParseOutcome::parsed(shifted(model_tier, offset));
            }
        }
        ParseOutcome::rejected()
    }
}
