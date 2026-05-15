//! LINKER-AUDIT — Cross-utterance linker and special terminator analysis.
//!
//! Analyzes usage of CHAT utterance linkers (`+<`, `++`, `+^`, `+"`, `+,`,
//! `+≋`, `+≈`) and special terminators (`+...`, `+/.`, `+//.`, `+"/.`, `+".`,
//! etc.) across an entire corpus.
//!
//! For each file, extracts:
//! - Linker and terminator frequency counts
//! - Cross-utterance pairing correctness (e.g., `++` must follow `+...` from
//!   different speaker)
//! - Anomalies: same-speaker `++`, `+,` without prior `+/.`, `+"` without
//!   `+"/.`, orphaned special terminators, `+<` overlap block patterns
//!
//! # Output
//!
//! Per-file anomaly details plus a corpus-wide summary with:
//! - Frequency tables for all linker and terminator types
//! - Pairing statistics and violation rates
//! - `+<` block analysis (block sizes, speaker counts)
//! - Orphaned terminator counts

use std::collections::HashMap;
use std::fmt;

use indexmap::IndexMap;
use serde::Serialize;
use talkbank_model::model::{Linker, Terminator};
use talkbank_model::{Line, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section,
};

// ── Configuration ──────────────────────────────────────────────────────

/// Configuration for the LINKER-AUDIT command.
#[derive(Debug, Clone, Default)]
pub struct LinkerAuditConfig {}

// ── Per-file tracking ──────────────────────────────────────────────────

/// Tracks linker/terminator statistics and anomalies for one file.
#[derive(Debug, Clone, Default, Serialize)]
struct FileStats {
    filename: String,
    total_utterances: usize,

    // Linker counts
    linker_lazy_overlap: usize,
    linker_other_completion: usize,
    linker_quick_uptake: usize,
    linker_quotation_follows: usize,
    linker_self_completion: usize,
    linker_tcu_continuation: usize,
    linker_no_break_tcu: usize,

    // Special terminator counts
    term_trailing_off: usize,
    term_trailing_off_question: usize,
    term_interruption: usize,
    term_interrupted_question: usize,
    term_self_interruption: usize,
    term_self_interrupted_question: usize,
    term_broken_question: usize,
    term_quotation_follows: usize,
    term_quotation_precedes: usize,
    term_break_for_coding: usize,
    term_ca_technical_break: usize,
    term_ca_no_break: usize,

    // ++ pairing analysis
    pp_correct: usize,          // ++ after different-speaker +...
    pp_same_speaker: usize,     // ++ after same speaker (should be +,)
    pp_wrong_terminator: usize, // ++ after different speaker but not +...
    pp_first_utterance: usize,  // ++ as first utterance in file

    // +, pairing analysis
    sc_correct: usize,          // +, after same-speaker +/.
    sc_wrong_terminator: usize, // +, after same speaker but not +/.
    sc_no_prior: usize,         // +, with no prior same-speaker utterance

    // +" pairing analysis
    qf_correct: usize,          // +" after same-speaker +"/.
    qf_chained: usize,          // +" after same-speaker +"
    qf_wrong_terminator: usize, // +" after same speaker but not +"/. or +"
    qf_no_prior: usize,         // +" with no prior same-speaker utterance

    // Quotation balance
    quot_follows_terms: usize, // +"/. terminators
    quot_follows_links: usize, // +" linkers

    // +< overlap block analysis
    lo_blocks: usize,                // Number of +< blocks
    lo_block_size_1: usize,          // Single +< (isolated)
    lo_block_size_2: usize,          // Pair of +<
    lo_block_size_3plus: usize,      // 3+ consecutive +<
    lo_same_speaker_start: usize,    // +< where speaker == previous speaker and not in block
    lo_max_speakers_in_block: usize, // Max distinct speakers in any block
    lo_combined_with_other: usize,   // +< combined with another linker

    // +^ analysis
    qu_same_speaker: usize,
    qu_diff_speaker: usize,

    // +≋/+≈ TCU analysis
    tcu_tech_same_speaker: usize,
    tcu_tech_diff_speaker: usize,
    tcu_nb_same_speaker: usize,
    tcu_nb_diff_speaker: usize,

    // Orphaned terminators
    trailing_off_total: usize,
    trailing_off_followed: usize,
    interruption_total: usize,
    interruption_followed: usize,
}

impl FileStats {
    fn has_any_linker_or_special_terminator(&self) -> bool {
        self.linker_lazy_overlap > 0
            || self.linker_other_completion > 0
            || self.linker_quick_uptake > 0
            || self.linker_quotation_follows > 0
            || self.linker_self_completion > 0
            || self.linker_tcu_continuation > 0
            || self.linker_no_break_tcu > 0
            || self.term_trailing_off > 0
            || self.term_interruption > 0
            || self.term_self_interruption > 0
            || self.term_quotation_follows > 0
            || self.term_quotation_precedes > 0
    }

    fn total_anomalies(&self) -> usize {
        self.pp_same_speaker
            + self.pp_wrong_terminator
            + self.pp_first_utterance
            + self.sc_wrong_terminator
            + self.sc_no_prior
            + self.qf_wrong_terminator
            + self.qf_no_prior
    }
}

// ── Corpus-wide summary ────────────────────────────────────────────────

/// Corpus-wide aggregated statistics.
#[derive(Debug, Clone, Default, Serialize)]
struct CorpusSummary {
    files_total: usize,
    files_with_linkers: usize,
    files_with_anomalies: usize,

    // Linker totals
    total_lazy_overlap: usize,
    total_other_completion: usize,
    total_quick_uptake: usize,
    total_quotation_follows: usize,
    total_self_completion: usize,
    total_tcu_continuation: usize,
    total_no_break_tcu: usize,

    // Terminator totals
    total_trailing_off: usize,
    total_trailing_off_question: usize,
    total_interruption: usize,
    total_interrupted_question: usize,
    total_self_interruption: usize,
    total_self_interrupted_question: usize,
    total_broken_question: usize,
    total_quotation_follows_term: usize,
    total_quotation_precedes_term: usize,
    total_break_for_coding: usize,
    total_ca_technical_break: usize,
    total_ca_no_break: usize,

    // ++ pairing
    pp_correct: usize,
    pp_same_speaker: usize,
    pp_wrong_terminator: usize,
    pp_first_utterance: usize,

    // +, pairing
    sc_correct: usize,
    sc_wrong_terminator: usize,
    sc_no_prior: usize,

    // +" pairing
    qf_correct: usize,
    qf_chained: usize,
    qf_wrong_terminator: usize,
    qf_no_prior: usize,

    // +< blocks
    lo_blocks_total: usize,
    lo_isolated: usize,
    lo_pairs: usize,
    lo_large_blocks: usize,
    lo_same_speaker_start: usize,
    lo_combined_with_other: usize,

    // +^
    qu_same_speaker: usize,
    qu_diff_speaker: usize,

    // +≋/+≈
    tcu_tech_same: usize,
    tcu_tech_diff: usize,
    tcu_nb_same: usize,
    tcu_nb_diff: usize,

    // Orphans
    trailing_off_total: usize,
    trailing_off_followed: usize,
    interruption_total: usize,
    interruption_followed: usize,
}

// ── Output types ───────────────────────────────────────────────────────

/// Top-level result for the LINKER-AUDIT command.
#[derive(Debug, Clone, Serialize)]
pub struct LinkerAuditResult {
    files: Vec<FileStats>,
    summary: CorpusSummary,
}

impl LinkerAuditResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("linker-audit");
        let s = &self.summary;

        // ── Linker frequency table ─────────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert(
            "+< (lazy overlap)".to_owned(),
            s.total_lazy_overlap.to_string(),
        );
        fields.insert(
            "++ (other completion)".to_owned(),
            s.total_other_completion.to_string(),
        );
        fields.insert(
            "+^ (quick uptake)".to_owned(),
            s.total_quick_uptake.to_string(),
        );
        fields.insert(
            "+\" (quotation follows)".to_owned(),
            s.total_quotation_follows.to_string(),
        );
        fields.insert(
            "+, (self completion)".to_owned(),
            s.total_self_completion.to_string(),
        );
        fields.insert(
            "+≋ (TCU continuation)".to_owned(),
            s.total_tcu_continuation.to_string(),
        );
        fields.insert(
            "+≈ (no-break TCU)".to_owned(),
            s.total_no_break_tcu.to_string(),
        );
        result.add_section(Section::with_fields(
            "Linker Frequencies".to_owned(),
            fields,
        ));

        // ── Terminator frequency table ─────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert(
            "+... (trailing off)".to_owned(),
            s.total_trailing_off.to_string(),
        );
        fields.insert(
            "+..? (trailing off question)".to_owned(),
            s.total_trailing_off_question.to_string(),
        );
        fields.insert(
            "+/. (interruption)".to_owned(),
            s.total_interruption.to_string(),
        );
        fields.insert(
            "+/? (interrupted question)".to_owned(),
            s.total_interrupted_question.to_string(),
        );
        fields.insert(
            "+//. (self-interruption)".to_owned(),
            s.total_self_interruption.to_string(),
        );
        fields.insert(
            "+//? (self-interrupted question)".to_owned(),
            s.total_self_interrupted_question.to_string(),
        );
        fields.insert(
            "+!? (broken question)".to_owned(),
            s.total_broken_question.to_string(),
        );
        fields.insert(
            "+\"/. (quotation follows)".to_owned(),
            s.total_quotation_follows_term.to_string(),
        );
        fields.insert(
            "+\". (quotation precedes)".to_owned(),
            s.total_quotation_precedes_term.to_string(),
        );
        fields.insert(
            "+. (break for coding)".to_owned(),
            s.total_break_for_coding.to_string(),
        );
        result.add_section(Section::with_fields(
            "Special Terminator Frequencies".to_owned(),
            fields,
        ));

        // ── ++ pairing analysis ────────────────────────────────────
        let pp_total =
            s.pp_correct + s.pp_same_speaker + s.pp_wrong_terminator + s.pp_first_utterance;
        let mut fields = IndexMap::new();
        fields.insert("Total ++".to_owned(), pp_total.to_string());
        fields.insert(
            "Correct (diff speaker + +...)".to_owned(),
            pct_str(s.pp_correct, pp_total),
        );
        fields.insert(
            "ANOMALY: same speaker (should be +,)".to_owned(),
            pct_str(s.pp_same_speaker, pp_total),
        );
        fields.insert(
            "ANOMALY: wrong terminator".to_owned(),
            pct_str(s.pp_wrong_terminator, pp_total),
        );
        fields.insert(
            "ANOMALY: first utterance".to_owned(),
            pct_str(s.pp_first_utterance, pp_total),
        );
        result.add_section(Section::with_fields(
            "++ (Other Completion) Pairing".to_owned(),
            fields,
        ));

        // ── +, pairing analysis ────────────────────────────────────
        let sc_total = s.sc_correct + s.sc_wrong_terminator + s.sc_no_prior;
        let mut fields = IndexMap::new();
        fields.insert("Total +,".to_owned(), sc_total.to_string());
        fields.insert(
            "Correct (same speaker + +/.)".to_owned(),
            pct_str(s.sc_correct, sc_total),
        );
        fields.insert(
            "ANOMALY: wrong terminator".to_owned(),
            pct_str(s.sc_wrong_terminator, sc_total),
        );
        fields.insert(
            "ANOMALY: no prior same-speaker".to_owned(),
            pct_str(s.sc_no_prior, sc_total),
        );
        result.add_section(Section::with_fields(
            "+, (Self Completion) Pairing".to_owned(),
            fields,
        ));

        // ── +" pairing analysis ────────────────────────────────────
        let qf_total = s.qf_correct + s.qf_chained + s.qf_wrong_terminator + s.qf_no_prior;
        let mut fields = IndexMap::new();
        fields.insert("Total +\"".to_owned(), qf_total.to_string());
        fields.insert(
            "Correct (same speaker + +\"/.)".to_owned(),
            pct_str(s.qf_correct, qf_total),
        );
        fields.insert(
            "Chained (same speaker + +\")".to_owned(),
            pct_str(s.qf_chained, qf_total),
        );
        fields.insert(
            "ANOMALY: wrong terminator".to_owned(),
            pct_str(s.qf_wrong_terminator, qf_total),
        );
        fields.insert(
            "ANOMALY: no prior same-speaker".to_owned(),
            pct_str(s.qf_no_prior, qf_total),
        );
        result.add_section(Section::with_fields(
            "+\" (Quotation) Pairing".to_owned(),
            fields,
        ));

        // ── +< block analysis ──────────────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert("Total +< blocks".to_owned(), s.lo_blocks_total.to_string());
        fields.insert("Isolated (size 1)".to_owned(), s.lo_isolated.to_string());
        fields.insert("Pairs (size 2)".to_owned(), s.lo_pairs.to_string());
        fields.insert("Large (size 3+)".to_owned(), s.lo_large_blocks.to_string());
        fields.insert(
            "Same-speaker start (suspicious)".to_owned(),
            s.lo_same_speaker_start.to_string(),
        );
        fields.insert(
            "Combined with other linker".to_owned(),
            s.lo_combined_with_other.to_string(),
        );
        result.add_section(Section::with_fields(
            "+< (Lazy Overlap) Blocks".to_owned(),
            fields,
        ));

        // ── +^ analysis ────────────────────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert("Same speaker".to_owned(), s.qu_same_speaker.to_string());
        fields.insert(
            "Different speaker".to_owned(),
            s.qu_diff_speaker.to_string(),
        );
        result.add_section(Section::with_fields(
            "+^ (Quick Uptake) Speaker".to_owned(),
            fields,
        ));

        // ── TCU analysis ───────────────────────────────────────────
        if s.tcu_tech_same + s.tcu_tech_diff > 0 || s.tcu_nb_same + s.tcu_nb_diff > 0 {
            let mut fields = IndexMap::new();
            fields.insert("+≋ same speaker".to_owned(), s.tcu_tech_same.to_string());
            fields.insert("+≋ diff speaker".to_owned(), s.tcu_tech_diff.to_string());
            fields.insert("+≈ same speaker".to_owned(), s.tcu_nb_same.to_string());
            fields.insert("+≈ diff speaker".to_owned(), s.tcu_nb_diff.to_string());
            result.add_section(Section::with_fields("CA TCU Linkers".to_owned(), fields));
        }

        // ── Orphaned terminators ───────────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert("+... total".to_owned(), s.trailing_off_total.to_string());
        fields.insert(
            "+... followed by ++/+,".to_owned(),
            s.trailing_off_followed.to_string(),
        );
        fields.insert(
            "+... orphaned".to_owned(),
            (s.trailing_off_total - s.trailing_off_followed).to_string(),
        );
        fields.insert("+/. total".to_owned(), s.interruption_total.to_string());
        fields.insert(
            "+/. followed by +,".to_owned(),
            s.interruption_followed.to_string(),
        );
        fields.insert(
            "+/. orphaned".to_owned(),
            (s.interruption_total - s.interruption_followed).to_string(),
        );
        result.add_section(Section::with_fields(
            "Orphaned Special Terminators".to_owned(),
            fields,
        ));

        // ── Overall ────────────────────────────────────────────────
        let mut fields = IndexMap::new();
        fields.insert("Files analyzed".to_owned(), s.files_total.to_string());
        fields.insert(
            "Files with linkers/special terminators".to_owned(),
            s.files_with_linkers.to_string(),
        );
        fields.insert(
            "Files with anomalies".to_owned(),
            s.files_with_anomalies.to_string(),
        );
        result.add_section(Section::with_fields("Summary".to_owned(), fields));

        result
    }
}

impl CommandOutput for LinkerAuditResult {
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    fn render_clan(&self) -> String {
        self.render_text()
    }
}

fn pct_str(count: usize, total: usize) -> String {
    if total == 0 {
        format!("{count}")
    } else {
        format!("{count} ({:.1}%)", count as f64 / total as f64 * 100.0)
    }
}

// ── State ──────────────────────────────────────────────────────────────

/// Accumulated state across all files.
#[derive(Debug, Default)]
pub struct LinkerAuditState {
    files: Vec<FileStats>,
}

// ── Command implementation ─────────────────────────────────────────────

/// LINKER-AUDIT command.
#[derive(Debug, Clone, Default)]
pub struct LinkerAuditCommand;

/// Extract the linker kind(s) from an utterance.
fn get_linkers(utt: &Utterance) -> &[Linker] {
    utt.main.content.linkers.as_slice()
}

/// Extract the terminator from an utterance.
fn get_terminator(utt: &Utterance) -> Option<&Terminator> {
    utt.main.content.terminator.as_ref()
}

/// Check if a terminator is a trailing-off variant.
fn is_trailing_off(term: &Terminator) -> bool {
    matches!(
        term,
        Terminator::TrailingOff { .. } | Terminator::TrailingOffQuestion { .. }
    )
}

/// Check if a terminator is an interruption variant.
fn is_interruption(term: &Terminator) -> bool {
    matches!(
        term,
        Terminator::Interruption { .. } | Terminator::InterruptedQuestion { .. }
    )
}

/// Check if a terminator is the quotation-follows terminator.
fn _is_quotation_follows_term(term: &Terminator) -> bool {
    matches!(term, Terminator::QuotedNewLine { .. })
}

/// Classify a terminator for display/counting.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum TerminatorKind {
    Period,
    Question,
    Exclamation,
    TrailingOff,
    TrailingOffQuestion,
    Interruption,
    InterruptedQuestion,
    SelfInterruption,
    SelfInterruptedQuestion,
    BrokenQuestion,
    QuotationFollows,
    QuotationPrecedes,
    BreakForCoding,
    CaTechnicalBreak,
    CaTechnicalBreakLinker,
    CaNoBreak,
    CaNoBreakLinker,
    CaIntonation,
}

impl fmt::Display for TerminatorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Period => write!(f, "."),
            Self::Question => write!(f, "?"),
            Self::Exclamation => write!(f, "!"),
            Self::TrailingOff => write!(f, "+..."),
            Self::TrailingOffQuestion => write!(f, "+..?"),
            Self::Interruption => write!(f, "+/."),
            Self::InterruptedQuestion => write!(f, "+/?"),
            Self::SelfInterruption => write!(f, "+//."),
            Self::SelfInterruptedQuestion => write!(f, "+//?"),
            Self::BrokenQuestion => write!(f, "+!?"),
            Self::QuotationFollows => write!(f, "+\"/."),
            Self::QuotationPrecedes => write!(f, "+\"."),
            Self::BreakForCoding => write!(f, "+."),
            Self::CaTechnicalBreak => write!(f, "≋"),
            Self::CaTechnicalBreakLinker => write!(f, "+≋"),
            Self::CaNoBreak => write!(f, "≈"),
            Self::CaNoBreakLinker => write!(f, "+≈"),
            Self::CaIntonation => write!(f, "(CA intonation)"),
        }
    }
}

fn classify_terminator(term: &Terminator) -> TerminatorKind {
    match term {
        Terminator::Period { .. } => TerminatorKind::Period,
        Terminator::Question { .. } => TerminatorKind::Question,
        Terminator::Exclamation { .. } => TerminatorKind::Exclamation,
        Terminator::TrailingOff { .. } => TerminatorKind::TrailingOff,
        Terminator::TrailingOffQuestion { .. } => TerminatorKind::TrailingOffQuestion,
        Terminator::Interruption { .. } => TerminatorKind::Interruption,
        Terminator::InterruptedQuestion { .. } => TerminatorKind::InterruptedQuestion,
        Terminator::SelfInterruption { .. } => TerminatorKind::SelfInterruption,
        Terminator::SelfInterruptedQuestion { .. } => TerminatorKind::SelfInterruptedQuestion,
        Terminator::BrokenQuestion { .. } => TerminatorKind::BrokenQuestion,
        Terminator::QuotedNewLine { .. } => TerminatorKind::QuotationFollows,
        Terminator::QuotedPeriodSimple { .. } => TerminatorKind::QuotationPrecedes,
        Terminator::BreakForCoding { .. } => TerminatorKind::BreakForCoding,
        Terminator::CaTechnicalBreak { .. } => TerminatorKind::CaTechnicalBreak,
        Terminator::CaTechnicalBreakLinker { .. } => TerminatorKind::CaTechnicalBreakLinker,
        Terminator::CaNoBreak { .. } => TerminatorKind::CaNoBreak,
        Terminator::CaNoBreakLinker { .. } => TerminatorKind::CaNoBreakLinker,
        _ => TerminatorKind::CaIntonation,
    }
}

/// Analyze one file's linker and terminator usage.
fn analyze_file(utterances: &[&Utterance], filename: &str) -> FileStats {
    let mut stats = FileStats {
        filename: filename.to_owned(),
        total_utterances: utterances.len(),
        ..Default::default()
    };

    // Per-speaker: last terminator seen
    let mut last_term_by_speaker: HashMap<&str, TerminatorKind> = HashMap::new();
    // Per-speaker: last linker seen
    let mut last_linker_by_speaker: HashMap<&str, Option<Linker>> = HashMap::new();

    // Previous utterance state
    let mut prev_speaker: Option<&str> = None;
    let mut prev_terminator: Option<TerminatorKind> = None;
    let mut _prev_had_lazy_overlap = false;

    // +< block tracking
    let mut in_lazy_block = false;
    let mut lazy_block_size: usize = 0;
    let mut lazy_block_speakers: Vec<&str> = Vec::new();

    for (idx, utt) in utterances.iter().enumerate() {
        let speaker = utt.main.speaker.as_str();
        let linkers = get_linkers(utt);
        let terminator = get_terminator(utt);
        let term_kind = terminator.map(classify_terminator);

        // Count linkers
        let mut has_lazy_overlap = false;
        let mut has_other_linker = false;
        for linker in linkers {
            match linker {
                Linker::LazyOverlapPrecedes => {
                    stats.linker_lazy_overlap += 1;
                    has_lazy_overlap = true;
                }
                Linker::OtherCompletion => stats.linker_other_completion += 1,
                Linker::QuickUptakeOverlap => stats.linker_quick_uptake += 1,
                Linker::QuotationFollows => stats.linker_quotation_follows += 1,
                Linker::SelfCompletion => stats.linker_self_completion += 1,
                Linker::TcuContinuation => stats.linker_tcu_continuation += 1,
                Linker::NoBreakTcuContinuation => stats.linker_no_break_tcu += 1,
            }
            if !matches!(linker, Linker::LazyOverlapPrecedes) {
                has_other_linker = true;
            }
        }

        // Track +< combined with other linker
        if has_lazy_overlap && has_other_linker {
            stats.lo_combined_with_other += 1;
        }

        // Count special terminators
        if let Some(term) = terminator {
            match classify_terminator(term) {
                TerminatorKind::TrailingOff => stats.term_trailing_off += 1,
                TerminatorKind::TrailingOffQuestion => stats.term_trailing_off_question += 1,
                TerminatorKind::Interruption => stats.term_interruption += 1,
                TerminatorKind::InterruptedQuestion => stats.term_interrupted_question += 1,
                TerminatorKind::SelfInterruption => stats.term_self_interruption += 1,
                TerminatorKind::SelfInterruptedQuestion => {
                    stats.term_self_interrupted_question += 1
                }
                TerminatorKind::BrokenQuestion => stats.term_broken_question += 1,
                TerminatorKind::QuotationFollows => stats.term_quotation_follows += 1,
                TerminatorKind::QuotationPrecedes => stats.term_quotation_precedes += 1,
                TerminatorKind::BreakForCoding => stats.term_break_for_coding += 1,
                TerminatorKind::CaTechnicalBreak => stats.term_ca_technical_break += 1,
                TerminatorKind::CaNoBreak => stats.term_ca_no_break += 1,
                _ => {}
            }
        }

        // ── ++ (Other Completion) pairing ──────────────────────────
        if linkers.iter().any(|l| matches!(l, Linker::OtherCompletion)) {
            if idx == 0 {
                stats.pp_first_utterance += 1;
            } else if let Some(ps) = prev_speaker {
                if ps == speaker {
                    stats.pp_same_speaker += 1;
                } else if prev_terminator.is_some_and(|t| {
                    matches!(
                        t,
                        TerminatorKind::TrailingOff | TerminatorKind::TrailingOffQuestion
                    )
                }) {
                    stats.pp_correct += 1;
                } else {
                    stats.pp_wrong_terminator += 1;
                }
            }
        }

        // ── +, (Self Completion) pairing ───────────────────────────
        if linkers.iter().any(|l| matches!(l, Linker::SelfCompletion)) {
            match last_term_by_speaker.get(speaker) {
                None => stats.sc_no_prior += 1,
                Some(TerminatorKind::Interruption | TerminatorKind::InterruptedQuestion) => {
                    stats.sc_correct += 1;
                }
                Some(_) => stats.sc_wrong_terminator += 1,
            }
        }

        // ── +" (Quotation Follows) pairing ─────────────────────────
        if linkers
            .iter()
            .any(|l| matches!(l, Linker::QuotationFollows))
        {
            stats.quot_follows_links += 1;
            match last_term_by_speaker.get(speaker) {
                None => stats.qf_no_prior += 1,
                Some(TerminatorKind::QuotationFollows) => stats.qf_correct += 1,
                _ => {
                    // Check if previous same-speaker had +" linker (chaining)
                    if last_linker_by_speaker
                        .get(speaker)
                        .and_then(|l| l.as_ref())
                        .is_some_and(|l| matches!(l, Linker::QuotationFollows))
                    {
                        stats.qf_chained += 1;
                    } else {
                        stats.qf_wrong_terminator += 1;
                    }
                }
            }
        }

        // ── Quotation follows terminator count ─────────────────────
        if term_kind == Some(TerminatorKind::QuotationFollows) {
            stats.quot_follows_terms += 1;
        }

        // ── +< block analysis ──────────────────────────────────────
        if has_lazy_overlap {
            if in_lazy_block {
                lazy_block_size += 1;
                if !lazy_block_speakers.contains(&speaker) {
                    lazy_block_speakers.push(speaker);
                }
            } else {
                // Start new block — flush previous if any
                flush_lazy_block(&mut stats, lazy_block_size, &lazy_block_speakers);
                in_lazy_block = true;
                lazy_block_size = 1;
                lazy_block_speakers.clear();
                lazy_block_speakers.push(speaker);

                // Check if +< starts with same speaker as previous utterance
                // (when not continuing a block)
                if let Some(ps) = prev_speaker
                    && ps == speaker
                {
                    stats.lo_same_speaker_start += 1;
                }
            }
        } else if in_lazy_block {
            flush_lazy_block(&mut stats, lazy_block_size, &lazy_block_speakers);
            in_lazy_block = false;
            lazy_block_size = 0;
            lazy_block_speakers.clear();
        }

        // ── +^ analysis ───────────────────────────────────────────
        if linkers
            .iter()
            .any(|l| matches!(l, Linker::QuickUptakeOverlap))
        {
            if prev_speaker.is_some_and(|ps| ps == speaker) {
                stats.qu_same_speaker += 1;
            } else {
                stats.qu_diff_speaker += 1;
            }
        }

        // ── +≋/+≈ TCU analysis ────────────────────────────────────
        if linkers.iter().any(|l| matches!(l, Linker::TcuContinuation)) {
            if prev_speaker.is_some_and(|ps| ps == speaker) {
                stats.tcu_tech_same_speaker += 1;
            } else {
                stats.tcu_tech_diff_speaker += 1;
            }
        }
        if linkers
            .iter()
            .any(|l| matches!(l, Linker::NoBreakTcuContinuation))
        {
            if prev_speaker.is_some_and(|ps| ps == speaker) {
                stats.tcu_nb_same_speaker += 1;
            } else {
                stats.tcu_nb_diff_speaker += 1;
            }
        }

        // ── Orphaned terminator tracking ───────────────────────────
        if let Some(term) = terminator {
            if is_trailing_off(term) {
                stats.trailing_off_total += 1;
            }
            if is_interruption(term) {
                stats.interruption_total += 1;
            }
        }
        // Check if previous terminator was "followed"
        if let Some(pt) = prev_terminator {
            if matches!(
                pt,
                TerminatorKind::TrailingOff | TerminatorKind::TrailingOffQuestion
            ) && linkers
                .iter()
                .any(|l| matches!(l, Linker::OtherCompletion | Linker::SelfCompletion))
            {
                stats.trailing_off_followed += 1;
            }
            if matches!(
                pt,
                TerminatorKind::Interruption | TerminatorKind::InterruptedQuestion
            ) && linkers.iter().any(|l| matches!(l, Linker::SelfCompletion))
            {
                stats.interruption_followed += 1;
            }
        }

        // Update state
        prev_speaker = Some(speaker);
        prev_terminator = term_kind;
        _prev_had_lazy_overlap = has_lazy_overlap;
        if let Some(tk) = term_kind {
            last_term_by_speaker.insert(speaker, tk);
        }
        // Track the primary non-+< linker for quotation chaining
        let primary_linker = linkers
            .iter()
            .find(|l| !matches!(l, Linker::LazyOverlapPrecedes))
            .cloned();
        last_linker_by_speaker.insert(speaker, primary_linker);
    }

    // Flush final +< block
    if in_lazy_block {
        flush_lazy_block(&mut stats, lazy_block_size, &lazy_block_speakers);
    }

    stats
}

fn flush_lazy_block(stats: &mut FileStats, size: usize, speakers: &[&str]) {
    if size == 0 {
        return;
    }
    stats.lo_blocks += 1;
    match size {
        1 => stats.lo_block_size_1 += 1,
        2 => stats.lo_block_size_2 += 1,
        _ => stats.lo_block_size_3plus += 1,
    }
    let distinct_speakers = speakers.len();
    if distinct_speakers > stats.lo_max_speakers_in_block {
        stats.lo_max_speakers_in_block = distinct_speakers;
    }
}

impl AnalysisCommand for LinkerAuditCommand {
    type Config = LinkerAuditConfig;
    type State = LinkerAuditState;
    type Output = LinkerAuditResult;

    fn process_utterance(
        &self,
        _utterance: &Utterance,
        _file_context: &FileContext<'_>,
        _state: &mut Self::State,
    ) {
        // All work done in end_file for cross-utterance analysis
    }

    fn end_file(&self, file_context: &FileContext<'_>, state: &mut Self::State) {
        let utterances: Vec<&Utterance> = file_context
            .chat_file
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Utterance(u) => Some(u.as_ref()),
                _ => None,
            })
            .collect();

        let stats = analyze_file(&utterances, file_context.filename);
        state.files.push(stats);
    }

    fn finalize(&self, state: Self::State) -> LinkerAuditResult {
        let files_with_linkers = state
            .files
            .iter()
            .filter(|f| f.has_any_linker_or_special_terminator())
            .count();
        let files_with_anomalies = state
            .files
            .iter()
            .filter(|f| f.total_anomalies() > 0)
            .count();

        let summary = CorpusSummary {
            files_total: state.files.len(),
            files_with_linkers,
            files_with_anomalies,
            total_lazy_overlap: state.files.iter().map(|f| f.linker_lazy_overlap).sum(),
            total_other_completion: state.files.iter().map(|f| f.linker_other_completion).sum(),
            total_quick_uptake: state.files.iter().map(|f| f.linker_quick_uptake).sum(),
            total_quotation_follows: state.files.iter().map(|f| f.linker_quotation_follows).sum(),
            total_self_completion: state.files.iter().map(|f| f.linker_self_completion).sum(),
            total_tcu_continuation: state.files.iter().map(|f| f.linker_tcu_continuation).sum(),
            total_no_break_tcu: state.files.iter().map(|f| f.linker_no_break_tcu).sum(),
            total_trailing_off: state.files.iter().map(|f| f.term_trailing_off).sum(),
            total_trailing_off_question: state
                .files
                .iter()
                .map(|f| f.term_trailing_off_question)
                .sum(),
            total_interruption: state.files.iter().map(|f| f.term_interruption).sum(),
            total_interrupted_question: state
                .files
                .iter()
                .map(|f| f.term_interrupted_question)
                .sum(),
            total_self_interruption: state.files.iter().map(|f| f.term_self_interruption).sum(),
            total_self_interrupted_question: state
                .files
                .iter()
                .map(|f| f.term_self_interrupted_question)
                .sum(),
            total_broken_question: state.files.iter().map(|f| f.term_broken_question).sum(),
            total_quotation_follows_term: state
                .files
                .iter()
                .map(|f| f.term_quotation_follows)
                .sum(),
            total_quotation_precedes_term: state
                .files
                .iter()
                .map(|f| f.term_quotation_precedes)
                .sum(),
            total_break_for_coding: state.files.iter().map(|f| f.term_break_for_coding).sum(),
            total_ca_technical_break: state.files.iter().map(|f| f.term_ca_technical_break).sum(),
            total_ca_no_break: state.files.iter().map(|f| f.term_ca_no_break).sum(),
            pp_correct: state.files.iter().map(|f| f.pp_correct).sum(),
            pp_same_speaker: state.files.iter().map(|f| f.pp_same_speaker).sum(),
            pp_wrong_terminator: state.files.iter().map(|f| f.pp_wrong_terminator).sum(),
            pp_first_utterance: state.files.iter().map(|f| f.pp_first_utterance).sum(),
            sc_correct: state.files.iter().map(|f| f.sc_correct).sum(),
            sc_wrong_terminator: state.files.iter().map(|f| f.sc_wrong_terminator).sum(),
            sc_no_prior: state.files.iter().map(|f| f.sc_no_prior).sum(),
            qf_correct: state.files.iter().map(|f| f.qf_correct).sum(),
            qf_chained: state.files.iter().map(|f| f.qf_chained).sum(),
            qf_wrong_terminator: state.files.iter().map(|f| f.qf_wrong_terminator).sum(),
            qf_no_prior: state.files.iter().map(|f| f.qf_no_prior).sum(),
            lo_blocks_total: state.files.iter().map(|f| f.lo_blocks).sum(),
            lo_isolated: state.files.iter().map(|f| f.lo_block_size_1).sum(),
            lo_pairs: state.files.iter().map(|f| f.lo_block_size_2).sum(),
            lo_large_blocks: state.files.iter().map(|f| f.lo_block_size_3plus).sum(),
            lo_same_speaker_start: state.files.iter().map(|f| f.lo_same_speaker_start).sum(),
            lo_combined_with_other: state.files.iter().map(|f| f.lo_combined_with_other).sum(),
            qu_same_speaker: state.files.iter().map(|f| f.qu_same_speaker).sum(),
            qu_diff_speaker: state.files.iter().map(|f| f.qu_diff_speaker).sum(),
            tcu_tech_same: state.files.iter().map(|f| f.tcu_tech_same_speaker).sum(),
            tcu_tech_diff: state.files.iter().map(|f| f.tcu_tech_diff_speaker).sum(),
            tcu_nb_same: state.files.iter().map(|f| f.tcu_nb_same_speaker).sum(),
            tcu_nb_diff: state.files.iter().map(|f| f.tcu_nb_diff_speaker).sum(),
            trailing_off_total: state.files.iter().map(|f| f.trailing_off_total).sum(),
            trailing_off_followed: state.files.iter().map(|f| f.trailing_off_followed).sum(),
            interruption_total: state.files.iter().map(|f| f.interruption_total).sum(),
            interruption_followed: state.files.iter().map(|f| f.interruption_followed).sum(),
        };

        LinkerAuditResult {
            files: state.files,
            summary,
        }
    }
}
