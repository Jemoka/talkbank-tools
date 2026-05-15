//! Fragment-aware parsing methods on TreeSitterParser.
//!
//! These methods accept an `offset` parameter for span adjustment and an
//! `ErrorSink` for streaming error reporting. They wrap the raw parsing
//! methods and handle offset correction for embedded CHAT fragments.
//!
//! CHAT reference anchors:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Main_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//!
//! # Architecture
//!
//! Most tier parsing follows the "minimal wrapper" pattern:
//! 1. Wrap tier content in a minimal valid CHAT document
//! 2. Parse the wrapper with tree-sitter
//! 3. Extract the tier from the parsed file
//! 4. Adjust spans from wrapper-relative to document-absolute
//!
//! This pattern is encapsulated in `parser_impl::wrapper_parse_tier()`.

use talkbank_model::dependent_tier::DependentTier;
use talkbank_model::model::{
    ActTier, AddTier, ChatFile, CodTier, ComTier, ExpTier, GpxTier, GraTier, GrammaticalRelation,
    Header, IDHeader, IntTier, MainTier, MorTier, MorWord, ParticipantEntry, PhoTier, PhoWord,
    SinTier, SitTier, SpaTier, Utterance, WorTier, Word,
};
use talkbank_model::{
    ErrorCode, ErrorContext, ErrorSink, OffsetAdjustingErrorSink, ParseError, ParseErrors,
    Severity, SourceLocation, Span, SpanShift,
};
use talkbank_model::{FragmentSemanticContext, ParseOutcome};

use super::parser_impl::{wrapper_parse_generic_tier, wrapper_parse_tier};
use crate::parser::TreeSitterParser;
use crate::parser::chat_file_parser::MINIMAL_CHAT_PREFIX;

/// Report parse errors through an ErrorSink.
fn report_parse_errors(errors: ParseErrors, sink: &impl ErrorSink) {
    sink.report_vec(errors.into_error_vec());
}

const PARTICIPANTS_HEADER_PREFIX: &str = "@Participants:\t";

fn report_context_dependent_main_tier_error(
    errors: &impl ErrorSink,
    input: &str,
    offset: usize,
    span: Span,
) {
    let start = (span.start as usize)
        .saturating_sub(offset)
        .min(input.len());
    let end = (span.end as usize).saturating_sub(offset).min(input.len());
    let snippet = input.get(start..end).unwrap_or(input);

    errors.report(
        ParseError::new(
            ErrorCode::InvalidWordFormat,
            Severity::Error,
            SourceLocation::new(span),
            ErrorContext::new(input, start..end.max(start), snippet),
            "Main-tier fragment requires file context to interpret CA omission shorthand",
        )
        .with_suggestion(
            "Parse a full CHAT file with @Options: CA (or CA-Unicode), or rewrite the fragment using context-independent CHAT notation",
        ),
    );
}

/// Fragment-aware parsing methods.
///
/// These take an `offset` parameter for span adjustment and an `ErrorSink`
/// for streaming error reporting. Use these when parsing embedded CHAT
/// fragments where spans need to map back to positions in a larger document.
///
/// For simple full-file parsing, use `parse_chat_file()` or
/// `parse_chat_file_streaming()` directly — no offset needed.
impl TreeSitterParser {
    // =========================================================================
    // Word-Level Fragment Parsing
    // =========================================================================

    /// Parse an individual word with offset adjustment and streaming errors.
    pub fn parse_word_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
        match self.parse_word(input) {
            Ok(mut word) => {
                let wrapper_prefix_len = MINIMAL_CHAT_PREFIX.len() + "*CHI:\t".len();
                word.shift_spans_after(0, -(wrapper_prefix_len as i32) + offset as i32);
                ParseOutcome::parsed(word)
            }
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    // =========================================================================
    // Main Tier Fragment Parsing
    // =========================================================================

    /// Parse a main tier line with offset adjustment and streaming errors.
    pub fn parse_main_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        self.parse_main_tier_fragment_with_context(
            input,
            offset,
            &FragmentSemanticContext::default(),
            errors,
        )
    }

    /// Parse a main tier line with explicit semantic context.
    pub fn parse_main_tier_fragment_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
        match self.parse_main_tier(input) {
            Ok(mut main) => {
                let wrapper_prefix_len = MINIMAL_CHAT_PREFIX.len();
                main.shift_spans_after(0, -(wrapper_prefix_len as i32) + offset as i32);
                if context.ca_mode() {
                    crate::parser::chat_file_parser::chat_file::normalize::normalize_ca_omissions_main_tier(&mut main);
                    ParseOutcome::parsed(main)
                } else if let Some(span) = main.find_context_dependent_ca_omission_span() {
                    report_context_dependent_main_tier_error(errors, input, offset, span);
                    ParseOutcome::rejected()
                } else {
                    ParseOutcome::parsed(main)
                }
            }
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse a single utterance with offset adjustment and streaming errors.
    pub fn parse_utterance_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        self.parse_utterance_fragment_with_context(
            input,
            offset,
            &FragmentSemanticContext::default(),
            errors,
        )
    }

    /// Parse a single utterance with explicit semantic context.
    pub fn parse_utterance_fragment_with_context(
        &self,
        input: &str,
        offset: usize,
        context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
        match self.parse_utterance(input) {
            Ok(mut utterance) => {
                utterance.shift_spans_after(0, offset as i32);
                if context.ca_mode() {
                    crate::parser::chat_file_parser::chat_file::normalize::normalize_ca_omissions_main_tier(&mut utterance.main);
                    ParseOutcome::parsed(utterance)
                } else if let Some(span) = utterance.main.find_context_dependent_ca_omission_span()
                {
                    report_context_dependent_main_tier_error(errors, input, offset, span);
                    ParseOutcome::rejected()
                } else {
                    ParseOutcome::parsed(utterance)
                }
            }
            Err(errs) => {
                report_parse_errors(errs, &adjusting_sink);
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse a complete CHAT file with offset adjustment and streaming errors.
    pub fn parse_chat_file_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ChatFile> {
        let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
        let chat = self.parse_chat_file_streaming(input, &adjusting_sink);
        ParseOutcome::parsed(chat)
    }

    // =========================================================================
    // Headers
    // =========================================================================

    /// Parse any header line with offset adjustment and streaming errors.
    pub fn parse_header_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        let adjusting_sink = OffsetAdjustingErrorSink::new(errors, offset, input);
        match self.parse_header(input) {
            Ok(mut header) => {
                header.shift_spans_after(0, offset as i32);
                ParseOutcome::parsed(header)
            }
            Err(parse_errors) => {
                for error in parse_errors.errors {
                    adjusting_sink.report(error);
                }
                ParseOutcome::rejected()
            }
        }
    }

    /// Parse an @ID header with offset adjustment and streaming errors.
    pub fn parse_id_header_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        match self.parse_header_fragment(input, offset, errors) {
            ParseOutcome::Parsed(Header::ID(id)) => ParseOutcome::parsed(id),
            _ => ParseOutcome::rejected(),
        }
    }

    /// Parse a participant entry with offset adjustment and streaming errors.
    pub fn parse_participant_entry_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        let wrapper = format!("{PARTICIPANTS_HEADER_PREFIX}{}\n", input);
        let Some(header) = self
            .parse_header_fragment(&wrapper, 0, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        match header {
            Header::Participants { entries } => {
                let Some(mut entry) = entries.into_iter().next() else {
                    return ParseOutcome::rejected();
                };
                entry.shift_spans_after(
                    0,
                    -(PARTICIPANTS_HEADER_PREFIX.len() as i32) + offset as i32,
                );
                ParseOutcome::parsed(entry)
            }
            _ => ParseOutcome::rejected(),
        }
    }

    // =========================================================================
    // Morphology Tiers
    // =========================================================================

    /// Parse a %mor tier line with offset adjustment and streaming errors.
    pub fn parse_mor_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorTier> {
        wrapper_parse_tier(self, "%mor:\t", input, offset, errors, |tier| match tier {
            DependentTier::Mor(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single MOR word with offset adjustment and streaming errors.
    pub fn parse_mor_word_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        let Some(tier) = self
            .parse_mor_tier_fragment(&format!("{} .", input), offset, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        let Some(mor) = tier.into_items().into_iter().next() else {
            return ParseOutcome::rejected();
        };
        ParseOutcome::parsed(mor.main)
    }

    // =========================================================================
    // Grammar Tiers
    // =========================================================================

    /// Parse a %gra tier line with offset adjustment and streaming errors.
    pub fn parse_gra_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GraTier> {
        wrapper_parse_tier(self, "%gra:\t", input, offset, errors, |tier| match tier {
            DependentTier::Gra(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single grammatical relation with offset adjustment and streaming errors.
    pub fn parse_gra_relation_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        let Some(tier) = self
            .parse_gra_tier_fragment(&format!("{} 0|0|PUNCT", input), offset, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        tier.into_relations().into_iter().next().into()
    }

    // =========================================================================
    // Phonology Tiers
    // =========================================================================

    /// Parse a %pho tier line with offset adjustment and streaming errors.
    pub fn parse_pho_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoTier> {
        wrapper_parse_tier(self, "%pho:\t", input, offset, errors, |tier| match tier {
            DependentTier::Pho(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a single phonological word with offset adjustment and streaming errors.
    pub fn parse_pho_word_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        use talkbank_model::model::PhoItem;

        let Some(tier) = self
            .parse_pho_tier_fragment(&format!("{} .", input), offset, errors)
            .into_option()
        else {
            return ParseOutcome::rejected();
        };
        let Some(item) = tier.items.0.into_iter().next() else {
            return ParseOutcome::rejected();
        };
        match item {
            PhoItem::Word(word) => ParseOutcome::parsed(word),
            PhoItem::Group(_) => ParseOutcome::rejected(),
        }
    }

    // =========================================================================
    // Other Tiers
    // =========================================================================

    /// Parse a %sin tier with offset adjustment and streaming errors.
    pub fn parse_sin_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SinTier> {
        wrapper_parse_tier(self, "%sin:\t", input, offset, errors, |tier| match tier {
            DependentTier::Sin(tier) => Some(tier),
            _ => None,
        })
    }

    /// Parse a %act tier with offset adjustment and streaming errors.
    pub fn parse_act_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        wrapper_parse_tier(self, "%act:\t", input, offset, errors, |tier| match tier {
            DependentTier::Act(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %cod tier with offset adjustment and streaming errors.
    pub fn parse_cod_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        wrapper_parse_tier(self, "%cod:\t", input, offset, errors, |tier| match tier {
            DependentTier::Cod(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %com tier with offset adjustment and streaming errors.
    pub fn parse_com_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        wrapper_parse_tier(self, "%com:\t", input, offset, errors, |tier| match tier {
            DependentTier::Com(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %exp tier with offset adjustment and streaming errors.
    pub fn parse_exp_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        wrapper_parse_tier(self, "%exp:\t", input, offset, errors, |tier| match tier {
            DependentTier::Exp(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %add tier with offset adjustment and streaming errors.
    pub fn parse_add_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        wrapper_parse_tier(self, "%add:\t", input, offset, errors, |tier| match tier {
            DependentTier::Add(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %gpx tier with offset adjustment and streaming errors.
    pub fn parse_gpx_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        wrapper_parse_tier(self, "%gpx:\t", input, offset, errors, |tier| match tier {
            DependentTier::Gpx(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %int tier with offset adjustment and streaming errors.
    pub fn parse_int_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        wrapper_parse_tier(self, "%int:\t", input, offset, errors, |tier| match tier {
            DependentTier::Int(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %spa tier with offset adjustment and streaming errors.
    pub fn parse_spa_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        wrapper_parse_tier(self, "%spa:\t", input, offset, errors, |tier| match tier {
            DependentTier::Spa(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %sit tier with offset adjustment and streaming errors.
    pub fn parse_sit_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        wrapper_parse_tier(self, "%sit:\t", input, offset, errors, |tier| match tier {
            DependentTier::Sit(t) => Some(t),
            _ => None,
        })
    }

    /// Parse a %wor tier with offset adjustment and streaming errors.
    pub fn parse_wor_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        wrapper_parse_tier(self, "%wor:\t", input, offset, errors, |tier| match tier {
            DependentTier::Wor(t) => Some(t),
            _ => None,
        })
    }

    /// Parse any dependent tier line (including prefix) with offset adjustment and streaming errors.
    pub fn parse_dependent_tier_fragment(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<DependentTier> {
        wrapper_parse_generic_tier(self, input, offset, errors)
    }
}
