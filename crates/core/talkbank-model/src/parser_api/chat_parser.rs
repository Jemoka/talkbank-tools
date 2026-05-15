use crate::ErrorSink;
use crate::{
    ActTier, AddTier, ChatFile, CodTier, ComTier, DependentTier, ExpTier, FragmentSemanticContext,
    GpxTier, GraTier, GrammaticalRelation, Header, IDHeader, IntTier, MainTier, MorTier, MorWord,
    ParseOutcome, ParticipantEntry, PhoTier, PhoWord, SinTier, SitTier, SpaTier, Utterance,
    WorTier, Word,
};

/// Shared CHAT parsing API for parsing at any granularity.
///
/// This trait provides methods for parsing CHAT format constructs at multiple levels:
/// - **File level**: Complete CHAT documents
/// - **Line level**: Headers, utterances, main tiers, dependent tiers
/// - **Token level**: Words, MOR words, GRA relations
/// - **Component level**: Participants, ID fields, etc.
///
/// # Offset Parameter
///
/// All parse methods accept an `offset: usize` parameter for parsing embedded CHAT content.
/// The offset is added to all spans in the returned model objects and error reports.
///
/// - **Standalone parsing**: Use `offset = 0`
/// - **Embedded CHAT**: Use the byte position where CHAT content starts in the larger document
///
/// # Error Reporting
///
/// All methods use streaming error reporting via `ErrorSink`. Parsers should:
/// - Return `ParseOutcome::Parsed(T)` if parsing produced semantic output
/// - Return `ParseOutcome::Rejected` only if parsing could not produce semantic output
/// - Stream all errors via `errors.report()` as discovered
/// - Ensure error spans are also adjusted by `offset`
pub trait ChatParser {
    /// Human-readable name of this parser implementation.
    fn parser_name(&self) -> &'static str;

    /// Parse a complete CHAT file with streaming diagnostics.
    fn parse_chat_file(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ChatFile>;

    /// Parse any header line with streaming diagnostics.
    fn parse_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header>;

    /// Parse any header line with explicit semantic context.
    fn parse_header_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Header> {
        self.parse_header(input, offset, errors)
    }

    /// Parse an @ID header with streaming diagnostics.
    fn parse_id_header(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader>;

    /// Parse an `@ID` header with explicit semantic context.
    fn parse_id_header_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IDHeader> {
        self.parse_id_header(input, offset, errors)
    }

    /// Parse a participant entry (used in @Participants header).
    fn parse_participant_entry(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry>;

    /// Parse a participant entry with explicit semantic context.
    fn parse_participant_entry_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ParticipantEntry> {
        self.parse_participant_entry(input, offset, errors)
    }

    /// Parse a single utterance (main tier + dependent tiers) with streaming diagnostics.
    fn parse_utterance(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance>;

    /// Parse a single utterance with explicit semantic context.
    fn parse_utterance_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Utterance> {
        self.parse_utterance(input, offset, errors)
    }

    /// Parse a main tier line (`*CHI: ...`) with streaming diagnostics.
    fn parse_main_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier>;

    /// Parse a main-tier line with explicit semantic context.
    fn parse_main_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MainTier> {
        self.parse_main_tier(input, offset, errors)
    }

    /// Parse an individual word (main tier token) with streaming diagnostics.
    fn parse_word(&self, input: &str, offset: usize, errors: &impl ErrorSink)
    -> ParseOutcome<Word>;

    /// Parse an individual word with explicit semantic context.
    fn parse_word_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<Word> {
        self.parse_word(input, offset, errors)
    }

    /// Parse a `%mor` tier line with streaming diagnostics.
    fn parse_mor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorTier>;

    /// Parse a `%mor` tier with explicit semantic context.
    fn parse_mor_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorTier> {
        self.parse_mor_tier(input, offset, errors)
    }

    /// Parse a single MOR word with streaming diagnostics.
    fn parse_mor_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord>;

    /// Parse one `%mor` word with explicit semantic context.
    fn parse_mor_word_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<MorWord> {
        self.parse_mor_word(input, offset, errors)
    }

    /// Parse a `%gra` tier line with streaming diagnostics.
    fn parse_gra_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GraTier>;

    /// Parse a `%gra` tier with explicit semantic context.
    fn parse_gra_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GraTier> {
        self.parse_gra_tier(input, offset, errors)
    }

    /// Parse a single grammatical relation with streaming diagnostics.
    fn parse_gra_relation(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation>;

    /// Parse one `%gra` relation with explicit semantic context.
    fn parse_gra_relation_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GrammaticalRelation> {
        self.parse_gra_relation(input, offset, errors)
    }

    /// Parse a `%pho` tier line with streaming diagnostics.
    fn parse_pho_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoTier>;

    /// Parse a `%pho` tier with explicit semantic context.
    fn parse_pho_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoTier> {
        self.parse_pho_tier(input, offset, errors)
    }

    /// Parse a single phonological word with streaming diagnostics.
    fn parse_pho_word(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord>;

    /// Parse one phonological word with explicit semantic context.
    fn parse_pho_word_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<PhoWord> {
        self.parse_pho_word(input, offset, errors)
    }

    /// Parse a `%sin` (gesture) tier line with streaming diagnostics.
    fn parse_sin_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SinTier>;

    /// Parse a `%sin` tier with explicit semantic context.
    fn parse_sin_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SinTier> {
        self.parse_sin_tier(input, offset, errors)
    }

    /// Parse an `%act` (action) tier line with streaming diagnostics.
    fn parse_act_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier>;

    /// Parse an `%act` tier with explicit semantic context.
    fn parse_act_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ActTier> {
        self.parse_act_tier(input, offset, errors)
    }

    /// Parse a `%cod` (coding) tier line with streaming diagnostics.
    fn parse_cod_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier>;

    /// Parse a `%cod` tier with explicit semantic context.
    fn parse_cod_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<CodTier> {
        self.parse_cod_tier(input, offset, errors)
    }

    /// Parse a `%com` (comment) tier line with streaming diagnostics.
    fn parse_com_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier>;

    /// Parse a `%com` tier with explicit semantic context.
    fn parse_com_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ComTier> {
        self.parse_com_tier(input, offset, errors)
    }

    /// Parse an `%exp` (explanation) tier line with streaming diagnostics.
    fn parse_exp_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier>;

    /// Parse a `%exp` tier with explicit semantic context.
    fn parse_exp_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<ExpTier> {
        self.parse_exp_tier(input, offset, errors)
    }

    /// Parse an `%add` (addressee) tier line with streaming diagnostics.
    fn parse_add_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier>;

    /// Parse a `%add` tier with explicit semantic context.
    fn parse_add_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<AddTier> {
        self.parse_add_tier(input, offset, errors)
    }

    /// Parse a `%gpx` (gestural/physical context) tier line with streaming diagnostics.
    fn parse_gpx_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier>;

    /// Parse a `%gpx` tier with explicit semantic context.
    fn parse_gpx_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<GpxTier> {
        self.parse_gpx_tier(input, offset, errors)
    }

    /// Parse an `%int` (intonation) tier line with streaming diagnostics.
    fn parse_int_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier>;

    /// Parse an `%int` tier with explicit semantic context.
    fn parse_int_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<IntTier> {
        self.parse_int_tier(input, offset, errors)
    }

    /// Parse a `%spa` (speech act) tier line with streaming diagnostics.
    fn parse_spa_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier>;

    /// Parse a `%spa` tier with explicit semantic context.
    fn parse_spa_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SpaTier> {
        self.parse_spa_tier(input, offset, errors)
    }

    /// Parse a `%sit` (situation) tier line with streaming diagnostics.
    fn parse_sit_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier>;

    /// Parse a `%sit` tier with explicit semantic context.
    fn parse_sit_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<SitTier> {
        self.parse_sit_tier(input, offset, errors)
    }

    /// Parse a `%wor` (words) tier line with streaming diagnostics.
    fn parse_wor_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier>;

    /// Parse a `%wor` tier with explicit semantic context.
    fn parse_wor_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<WorTier> {
        self.parse_wor_tier(input, offset, errors)
    }

    /// Parse any dependent tier line with streaming diagnostics.
    ///
    /// Unlike the specific tier parsers above (which expect content *without*
    /// the `%tier:` prefix), this method accepts the **complete** dependent
    /// tier line including the prefix and dispatches to the appropriate
    /// specific parser.
    fn parse_dependent_tier(
        &self,
        input: &str,
        offset: usize,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<DependentTier>;

    /// Parse any dependent tier with explicit semantic context.
    fn parse_dependent_tier_with_context(
        &self,
        input: &str,
        offset: usize,
        _context: &FragmentSemanticContext,
        errors: &impl ErrorSink,
    ) -> ParseOutcome<DependentTier> {
        self.parse_dependent_tier(input, offset, errors)
    }
}
