//! VOCD — Vocabulary diversity (D statistic).
//!
//! Computes the D statistic for lexical diversity using bootstrap
//! sampling of type-token ratios (TTR). The D statistic provides a
//! more stable measure of vocabulary diversity than raw TTR because
//! it accounts for sample size effects.
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html#_Toc220409241)
//! for the original VOCD command specification.
//!
//! # Algorithm
//!
//! 1. Collect all countable word tokens per speaker from the main tier.
//! 2. For each of 3 independent trials:
//!    - For each sample size N in \[35..50\], draw 100 random samples of N
//!      tokens (without replacement) and compute mean TTR across samples.
//!    - Fit the empirical (N, TTR) curve to the theoretical D-curve using
//!      gradient-descent least-squares optimization.
//!    - Record the optimal D value.
//! 3. Report per-trial D values and their average.
//!
//! # Theoretical TTR Curve
//!
//! `TTR(N) = (D/N) * [sqrt(1 + 2*N/D) - 1]`
//!
//! This models the expected type-token ratio for a sample of size N given
//! a lexical diversity parameter D. Higher D means greater diversity.
//!
//! # CLAN Equivalence
//!
//! | CLAN command              | Rust equivalent                        |
//! |---------------------------|----------------------------------------|
//! | `vocd file.cha`           | `chatter analyze vocd file.cha`        |
//! | `vocd +t*CHI file.cha`    | `chatter analyze vocd file.cha -s CHI` |
//!
//! # Output
//!
//! Per-speaker D statistic with per-trial breakdown tables showing
//! (N, samples, TTR, std_dev, D) for each sample size.
//!
//! # Differences from CLAN
//!
//! - Word identification uses AST-based `is_countable_word()` instead of
//!   CLAN's string-prefix matching (`word[0] == '&'`, etc.).
//! - Token collection operates on parsed AST content rather than raw text.
//! - Output supports text, JSON, and CSV formats (CLAN produces text only).
//! - Deterministic output ordering via sorted collections.

use std::collections::HashSet;
use std::fmt::Write;

use indexmap::IndexMap;
use rand::prelude::*;
use rand::rngs::StdRng;
use serde::Serialize;
use talkbank_model::{SpeakerCode, Utterance, WriteChat};

use crate::framework::word_filter::countable_words;
use crate::framework::{
    AnalysisCommand, AnalysisScore, CommandOutput, FileContext, TypeCount, WordCount,
};

/// Strip fusional feature from a %mor lemma (e.g., "be&PRES" → "be").
///
/// CLAN echoes only the base lemma for insufficient-token speakers.
fn strip_fusional(lemma: &str) -> String {
    lemma.split('&').next().unwrap_or(lemma).to_owned()
}

/// Default range of sample sizes for VOCD.
const DEFAULT_SAMPLE_FROM: usize = 35;
const DEFAULT_SAMPLE_TO: usize = 50;
/// Number of random samples drawn per sample size N.
const DEFAULT_NUM_SAMPLES: usize = 100;
/// Number of independent trials to average.
const NUM_TRIALS: usize = 3;
/// Step size for gradient-descent D optimization (matches CLAN's 0.001).
const OPTIMIZATION_STEP: f64 = 0.001;

/// Configuration for the VOCD command.
#[derive(Debug, Clone)]
pub struct VocdConfig {
    /// Lower bound of sample size range (default: 35).
    pub sample_from: usize,
    /// Upper bound of sample size range (default: 50).
    pub sample_to: usize,
    /// Number of random samples per sample size (default: 100).
    pub num_samples: usize,
}

impl Default for VocdConfig {
    /// Use CLAN-style sampling defaults (N=35..50, 100 samples each).
    fn default() -> Self {
        Self {
            sample_from: DEFAULT_SAMPLE_FROM,
            sample_to: DEFAULT_SAMPLE_TO,
            num_samples: DEFAULT_NUM_SAMPLES,
        }
    }
}

/// Per-speaker accumulated token sequence.
#[derive(Debug, Default)]
struct SpeakerTokens {
    /// All countable word tokens (lowercased) in encounter order.
    tokens: Vec<String>,
    /// Per-utterance %mor lemma strings (for CLAN echo of insufficient-token speakers).
    mor_lemma_lines: Vec<String>,
}

/// Accumulated state for VOCD across all files.
#[derive(Debug, Default)]
pub struct VocdState {
    /// Per-speaker token sequences, keyed by speaker code.
    by_speaker: IndexMap<SpeakerCode, SpeakerTokens>,
}

/// A single (N, TTR) data point with statistics.
#[derive(Debug, Clone, Serialize)]
pub struct NtEntry {
    /// Sample size (number of tokens drawn).
    pub n: usize,
    /// Number of samples drawn at this N.
    pub samples: usize,
    /// Mean TTR across all samples.
    pub mean_ttr: f64,
    /// Standard deviation of TTR across samples.
    pub std_dev: f64,
    /// D value computed from this single (N, TTR) pair via the inverse equation.
    pub d_value: f64,
}

/// Results from a single VOCD trial.
#[derive(Debug, Clone, Serialize)]
pub struct VocdTrial {
    /// Per-N sampling results.
    pub entries: Vec<NtEntry>,
    /// Average D across all N values in this trial.
    pub d_average: f64,
    /// Standard deviation of D values across N values.
    pub d_std_dev: f64,
    /// Optimal D found by least-squares curve fitting.
    pub d_optimum: f64,
    /// Minimum least-squares error at d_optimum.
    pub min_least_sq: f64,
}

/// Per-speaker VOCD result.
#[derive(Debug, Clone, Serialize)]
pub struct VocdSpeakerResult {
    /// Speaker code.
    pub speaker: String,
    /// Total unique word types.
    pub types: TypeCount,
    /// Total word tokens.
    pub tokens: WordCount,
    /// Overall TTR (types/tokens).
    pub ttr: AnalysisScore,
    /// Individual trial results.
    pub trials: Vec<VocdTrial>,
    /// D_optimum values from each trial.
    pub d_optimum_values: Vec<f64>,
    /// Final averaged D_optimum across all trials.
    pub d_optimum_average: AnalysisScore,
}

/// Warning for speakers with insufficient tokens.
#[derive(Debug, Clone, Serialize)]
pub struct VocdWarning {
    /// Speaker code.
    pub speaker: String,
    /// Number of tokens available.
    pub token_count: WordCount,
    /// Minimum required.
    pub minimum_required: WordCount,
    /// Per-utterance %mor lemma strings (CLAN echoes these for low-token speakers).
    pub mor_lemma_lines: Vec<String>,
}

/// Typed output from the VOCD command.
#[derive(Debug, Clone, Serialize)]
pub struct VocdResult {
    /// Per-speaker VOCD results (only for speakers with enough tokens).
    pub speakers: Vec<VocdSpeakerResult>,
    /// Warnings for speakers without enough tokens.
    pub warnings: Vec<VocdWarning>,
}

/// VOCD command: compute vocabulary diversity D statistic.
///
/// Collects per-speaker token sequences during utterance processing.
/// At finalization, runs bootstrap trials per speaker (or emits warnings
/// for speakers with insufficient tokens). Requires at least
/// `sample_to` tokens (default: 50) per speaker.
#[derive(Default)]
pub struct VocdCommand {
    config: VocdConfig,
}

impl VocdCommand {
    /// Create a new `VocdCommand` with the given configuration.
    pub fn new(config: VocdConfig) -> Self {
        Self { config }
    }
}

impl AnalysisCommand for VocdCommand {
    type Config = VocdConfig;
    type State = VocdState;
    type Output = VocdResult;

    /// Append countable lexical tokens from one utterance to the speaker sequence.
    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker_data = state
            .by_speaker
            .entry(utterance.main.speaker.clone())
            .or_default();

        // Collect lowercased tokens from main tier countable words
        for word in countable_words(&utterance.main.content.content) {
            let text = word.to_chat_string();
            if !text.is_empty() {
                speaker_data.tokens.push(text.to_lowercase());
            }
        }

        // Collect %mor lemmas for CLAN echo (used when speaker has insufficient tokens).
        // Strip fusional features (&PRES, &INF, etc.) — CLAN echoes base lemmas only.
        if let Some(mor_tier) = utterance.mor_tier() {
            let lemmas: Vec<String> = mor_tier
                .items()
                .iter()
                .flat_map(|mor| {
                    let mut words = vec![strip_fusional(&mor.main.lemma)];
                    for clitic in &mor.post_clitics {
                        words.push(strip_fusional(&clitic.lemma));
                    }
                    words
                })
                .collect();
            if !lemmas.is_empty() {
                speaker_data.mor_lemma_lines.push(lemmas.join(" "));
            }
        }
    }

    /// Run VOCD trials per speaker or emit warnings when token counts are insufficient.
    fn finalize(&self, state: Self::State) -> Self::Output {
        let min_required = self.config.sample_to;
        let mut speakers = Vec::new();
        let mut warnings = Vec::new();

        for (speaker_code, speaker_tokens) in state.by_speaker {
            let token_count = speaker_tokens.tokens.len();

            if token_count < min_required {
                warnings.push(VocdWarning {
                    speaker: speaker_code.to_string(),
                    token_count: token_count as WordCount,
                    minimum_required: min_required as WordCount,
                    mor_lemma_lines: speaker_tokens.mor_lemma_lines,
                });
                continue;
            }

            // Compute overall types and TTR
            let unique_types: HashSet<&str> =
                speaker_tokens.tokens.iter().map(|s| s.as_str()).collect();
            let types = unique_types.len() as TypeCount;
            let tokens = token_count as WordCount;
            let ttr = types as f64 / tokens as f64;

            // Run trials — seed from system RNG for each speaker
            let mut rng = StdRng::from_rng(&mut rand::rng());
            let mut trials = Vec::with_capacity(NUM_TRIALS);
            let mut d_optimum_values = Vec::with_capacity(NUM_TRIALS);

            for _ in 0..NUM_TRIALS {
                let trial = run_trial(
                    &speaker_tokens.tokens,
                    self.config.sample_from,
                    self.config.sample_to,
                    self.config.num_samples,
                    &mut rng,
                );
                d_optimum_values.push(trial.d_optimum);
                trials.push(trial);
            }

            let d_optimum_average =
                d_optimum_values.iter().sum::<f64>() / d_optimum_values.len() as f64;

            speakers.push(VocdSpeakerResult {
                speaker: speaker_code.to_string(),
                types,
                tokens,
                ttr,
                trials,
                d_optimum_values,
                d_optimum_average,
            });
        }

        VocdResult { speakers, warnings }
    }
}

/// Run a single VOCD trial: sample at each N, compute TTR, fit D.
///
/// # Precondition
///
/// `tokens.len() >= sample_to` — caller must verify sufficient tokens.
///
/// # Postcondition
///
/// Produces one `NtEntry` for each sample size in `[from..=to]` plus fitted D statistics.
fn run_trial(
    tokens: &[String],
    sample_from: usize,
    sample_to: usize,
    num_samples: usize,
    rng: &mut StdRng,
) -> VocdTrial {
    let mut entries = Vec::with_capacity(sample_to - sample_from + 1);

    for n in sample_from..=sample_to {
        let (mean_ttr, std_dev) = average_ttr(tokens, n, num_samples, rng);
        let d_value = d_from_ttr(n, mean_ttr);

        entries.push(NtEntry {
            n,
            samples: num_samples,
            mean_ttr,
            std_dev,
            d_value,
        });
    }

    // Compute average D across all N values
    let d_sum: f64 = entries.iter().map(|e| e.d_value).sum();
    let d_count = entries.len() as f64;
    let d_average = d_sum / d_count;

    let d_variance: f64 = entries
        .iter()
        .map(|e| (e.d_value - d_average).powi(2))
        .sum::<f64>()
        / d_count;
    let d_std_dev = d_variance.sqrt();

    // Find optimal D via least-squares curve fitting
    let (d_optimum, min_least_sq) = find_min_d(d_average, &entries);

    VocdTrial {
        entries,
        d_average,
        d_std_dev,
        d_optimum,
        min_least_sq,
    }
}

/// Compute average TTR by random sampling without replacement.
///
/// Draws `num_samples` random subsets of size `n` from `tokens` (without
/// replacement within each sample), computes TTR for each, and returns
/// the (mean, std_dev) of TTR values.
///
/// # Precondition
///
/// `n <= tokens.len()` — sample size must not exceed available tokens.
///
/// # Postcondition
///
/// Produces `(mean_ttr, std_dev)` with mean TTR bounded to `[0.0, 1.0]`.
fn average_ttr(tokens: &[String], n: usize, num_samples: usize, rng: &mut StdRng) -> (f64, f64) {
    let total_tokens = tokens.len();
    let mut ttr_values = Vec::with_capacity(num_samples);

    for _ in 0..num_samples {
        // Sample n indices without replacement
        let mut selected = HashSet::with_capacity(n);
        let mut sample_types = HashSet::new();

        while selected.len() < n {
            let idx = rng.random_range(0..total_tokens);
            if selected.insert(idx) {
                sample_types.insert(tokens[idx].as_str());
            }
        }

        let ttr = sample_types.len() as f64 / n as f64;
        ttr_values.push(ttr);
    }

    let mean = ttr_values.iter().sum::<f64>() / ttr_values.len() as f64;
    let variance =
        ttr_values.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / ttr_values.len() as f64;
    let std_dev = variance.sqrt();

    (mean, std_dev)
}

/// Theoretical TTR as a function of D and N.
///
/// `TTR(N) = (D/N) * [sqrt(1 + 2*N/D) - 1]`
///
/// # Precondition
///
/// `d > 0.0` and `n > 0` — D must be positive, N must be at least 1.
///
/// # Postcondition
///
/// Output is theoretically bounded near `[0, 1]`.
fn ttr_equation(d: f64, n: usize) -> f64 {
    let n_f = n as f64;
    (d / n_f) * ((1.0 + 2.0 * n_f / d).sqrt() - 1.0)
}

/// Compute D from an observed (N, TTR) pair using the inverse equation.
///
/// `D = 0.5 * (N * T^2) / (1 - T)` where T is the observed TTR.
///
/// # Precondition
///
/// `ttr < 1.0` — a TTR of exactly 1.0 would divide by zero.
///
/// # Postcondition
///
/// Produces `D >= 0.0`; for degenerate `TTR >= 1.0` the function returns `0.0`.
fn d_from_ttr(n: usize, ttr: f64) -> f64 {
    let tmp = 1.0 - ttr;
    if tmp == 0.0 {
        return 0.0;
    }
    0.5 * (n as f64 * ttr * ttr) / tmp
}

/// Compute sum of squared errors between observed TTR values and
/// predicted TTR values for a given D.
///
/// # Precondition
///
/// `d > 0.0` — D must be positive for the TTR equation to be valid.
///
/// # Postcondition
///
/// Produces a non-negative least-squares error value.
fn d_least_squares(d: f64, entries: &[NtEntry]) -> f64 {
    entries
        .iter()
        .map(|e| {
            let predicted = ttr_equation(d, e.n);
            (e.mean_ttr - predicted).powi(2)
        })
        .sum()
}

/// Find the D value that minimizes the least-squares error.
///
/// Uses CLAN's gradient-descent approach: determine direction from
/// the initial estimate (`d_avg`), then walk in steps of 0.001 until
/// the error starts increasing.
///
/// # Precondition
///
/// `d_avg > 0.0` and `entries` is non-empty.
///
/// # Postcondition
///
/// Produces `(d_optimum, min_least_sq_error)`.
fn find_min_d(d_avg: f64, entries: &[NtEntry]) -> (f64, f64) {
    if d_avg <= 0.0 {
        return (0.0, f64::MAX);
    }

    let current_ls = d_least_squares(d_avg, entries);
    let slightly_lower_ls = d_least_squares(d_avg - OPTIMIZATION_STEP, entries);

    // Determine search direction
    let diff = current_ls - slightly_lower_ls;
    let direction: f64 = if diff > 0.0 {
        -1.0 // Decrease D (lower D had lower error)
    } else if diff < 0.0 {
        1.0 // Increase D (higher D has lower error)
    } else {
        // Already at minimum
        return (d_avg, current_ls);
    };

    // Walk in the chosen direction until error starts increasing
    let mut prev_ls = current_ls;
    let mut d = d_avg;
    let upper_bound = 2.0 * d_avg;

    loop {
        d += direction * OPTIMIZATION_STEP;
        if d <= 0.0 || d >= upper_bound {
            break;
        }

        let next_ls = d_least_squares(d, entries);
        if prev_ls < next_ls {
            // Error started increasing, back up one step
            d -= direction * OPTIMIZATION_STEP;
            break;
        }
        prev_ls = next_ls;
    }

    (d, prev_ls)
}

impl CommandOutput for VocdResult {
    /// Render warnings and per-trial VOCD tables in CLAN-compatible text.
    fn render_text(&self) -> String {
        let mut out = String::new();

        for warning in &self.warnings {
            writeln!(out, "****** Speaker: *{}:", warning.speaker).ok();
            writeln!(
                out,
                "WARNING: Not enough tokens for random sampling without replacement."
            )
            .ok();
            writeln!(
                out,
                "  ({} tokens available, {} required)\n",
                warning.token_count, warning.minimum_required
            )
            .ok();
        }

        for speaker in &self.speakers {
            writeln!(out, "****** Speaker: *{}:", speaker.speaker).ok();

            for (i, trial) in speaker.trials.iter().enumerate() {
                if i > 0 {
                    writeln!(out).ok();
                }

                writeln!(
                    out,
                    "D_optimum     <{:.2}; min least sq val = {:.3}>\n",
                    trial.d_optimum, trial.min_least_sq
                )
                .ok();

                writeln!(out, "tokens  samples    ttr     st.dev      D").ok();
                for entry in &trial.entries {
                    writeln!(
                        out,
                        "  {:>2}      {:>3}    {:.4}    {:.3}     {:.3}",
                        entry.n, entry.samples, entry.mean_ttr, entry.std_dev, entry.d_value,
                    )
                    .ok();
                }

                writeln!(
                    out,
                    "
D: average = {:.3}; std dev. = {:.3}",
                    trial.d_average, trial.d_std_dev,
                )
                .ok();
            }

            writeln!(out, "\nVOCD RESULTS SUMMARY").ok();
            writeln!(out, "====================").ok();
            writeln!(
                out,
                "   Types,Tokens,TTR:  <{},{},{:.6}>",
                speaker.types, speaker.tokens, speaker.ttr,
            )
            .ok();

            let d_strs: Vec<String> = speaker
                .d_optimum_values
                .iter()
                .map(|d| format!("{d:.2}"))
                .collect();
            writeln!(out, "  D_optimum  values:  <{}>", d_strs.join(", ")).ok();
            writeln!(
                out,
                "  D_optimum average:  {:.2}",
                speaker.d_optimum_average
            )
            .ok();
            writeln!(out).ok();
        }

        out
    }

    /// CLAN-compatible output: warnings show only the speaker header (no warning text).
    fn render_clan(&self) -> String {
        let mut out = String::new();

        for warning in &self.warnings {
            writeln!(out, "****** Speaker: *{}:", warning.speaker).ok();
            // CLAN echoes %mor lemmas for speakers with insufficient tokens.
            for line in &warning.mor_lemma_lines {
                writeln!(out, "{line} ").ok();
            }
        }

        for speaker in &self.speakers {
            writeln!(out, "****** Speaker: *{}:", speaker.speaker).ok();

            for (i, trial) in speaker.trials.iter().enumerate() {
                if i > 0 {
                    writeln!(out).ok();
                }

                writeln!(
                    out,
                    "D_optimum     <{:.2}; min least sq val = {:.3}>\n",
                    trial.d_optimum, trial.min_least_sq
                )
                .ok();

                writeln!(out, "tokens  samples    ttr     st.dev      D").ok();
                for entry in &trial.entries {
                    writeln!(
                        out,
                        "  {:>2}      {:>3}    {:.4}    {:.3}     {:.3}",
                        entry.n, entry.samples, entry.mean_ttr, entry.std_dev, entry.d_value,
                    )
                    .ok();
                }

                writeln!(
                    out,
                    "
D: average = {:.3}; std dev. = {:.3}",
                    trial.d_average, trial.d_std_dev,
                )
                .ok();
            }

            writeln!(out, "\nVOCD RESULTS SUMMARY").ok();
            writeln!(out, "====================").ok();
            writeln!(
                out,
                "   Types,Tokens,TTR:  <{},{},{:.6}>",
                speaker.types, speaker.tokens, speaker.ttr,
            )
            .ok();

            let d_strs: Vec<String> = speaker
                .d_optimum_values
                .iter()
                .map(|d| format!("{d:.2}"))
                .collect();
            writeln!(out, "  D_optimum  values:  <{}>", d_strs.join(", ")).ok();
            writeln!(
                out,
                "  D_optimum average:  {:.2}",
                speaker.d_optimum_average
            )
            .ok();
            writeln!(out).ok();
        }

        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Typical D/N values should produce a mid-range TTR.
    #[test]
    fn ttr_equation_basic() {
        // For D=100, N=50: TTR should be around 0.83
        let ttr = ttr_equation(100.0, 50);
        assert!(ttr > 0.8 && ttr < 0.9, "TTR={ttr}");
    }

    /// Very high D should push the theoretical TTR close to 1.
    #[test]
    fn ttr_equation_high_d() {
        // Very high D → TTR approaches 1.0
        let ttr = ttr_equation(10000.0, 50);
        assert!(ttr > 0.99, "Expected TTR near 1.0, got {ttr}");
    }

    /// Very low D should push the theoretical TTR close to 0.
    #[test]
    fn ttr_equation_low_d() {
        // Very low D → TTR approaches 0
        let ttr = ttr_equation(0.1, 50);
        assert!(ttr < 0.1, "Expected TTR near 0, got {ttr}");
    }

    /// Inverse equation should approximately recover the original D.
    #[test]
    fn d_from_ttr_inverse() {
        // d_from_ttr should approximately invert ttr_equation
        let d_original = 50.0;
        let n = 40;
        let ttr = ttr_equation(d_original, n);
        let d_recovered = d_from_ttr(n, ttr);
        assert!(
            (d_original - d_recovered).abs() < 0.01,
            "Expected ~{d_original}, got {d_recovered}"
        );
    }

    /// Degenerate `TTR == 1.0` should map to the guarded zero-D branch.
    #[test]
    fn d_from_ttr_handles_ttr_one() {
        // TTR=1.0 is a degenerate case (would divide by zero)
        let d = d_from_ttr(50, 1.0);
        assert_eq!(d, 0.0);
    }

    /// Bootstrap averaging should keep TTR means in a valid numeric range.
    #[test]
    fn average_ttr_produces_valid_range() {
        let tokens: Vec<String> = (0..100).map(|i| format!("word{}", i % 30)).collect();
        let mut rng = StdRng::seed_from_u64(42);
        let (mean, std_dev) = average_ttr(&tokens, 35, 50, &mut rng);

        assert!(mean > 0.0 && mean <= 1.0, "Mean TTR={mean}");
        assert!(std_dev >= 0.0, "Std dev should be non-negative: {std_dev}");
    }

    /// Finds min d converges.
    #[test]
    fn find_min_d_converges() {
        // Create synthetic (N, TTR) data from a known D
        let known_d = 60.0;
        let entries: Vec<NtEntry> = (35..=50)
            .map(|n| NtEntry {
                n,
                samples: 100,
                mean_ttr: ttr_equation(known_d, n),
                std_dev: 0.0,
                d_value: d_from_ttr(n, ttr_equation(known_d, n)),
            })
            .collect();

        let (d_opt, min_ls) = find_min_d(known_d, &entries);

        // Should converge very close to the known D
        assert!(
            (d_opt - known_d).abs() < 0.1,
            "Expected ~{known_d}, got {d_opt}"
        );
        assert!(min_ls < 0.001, "Expected very small LS error, got {min_ls}");
    }

    /// Speakers below the sample ceiling should emit warnings, not results.
    #[test]
    fn vocd_insufficient_tokens_warning() {
        use talkbank_model::Span;
        use talkbank_model::{MainTier, Terminator, Utterance, UtteranceContent, Word};

        let cmd = VocdCommand::default();
        let mut state = VocdState::default();

        // Create an utterance with only 3 words — far fewer than the 50 required
        let content: Vec<UtteranceContent> = ["hello", "world", "test"]
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let utt = Utterance::new(main);

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let leaked: &'static talkbank_model::ChatFile = Box::leak(Box::new(chat_file));
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: leaked,
            filename: "test",
            line_map: None,
        };

        cmd.process_utterance(&utt, &file_ctx, &mut state);
        let result = cmd.finalize(state);

        assert!(
            result.speakers.is_empty(),
            "Should have no speakers with enough tokens"
        );
        assert_eq!(result.warnings.len(), 1);
        assert_eq!(result.warnings[0].speaker, "CHI");
        assert_eq!(result.warnings[0].token_count, 3);
    }

    /// Adequate token counts should produce full trial output and positive D.
    #[test]
    fn vocd_with_enough_tokens() {
        use talkbank_model::Span;
        use talkbank_model::{MainTier, Terminator, Utterance, UtteranceContent, Word};

        let cmd = VocdCommand::new(VocdConfig {
            sample_from: 5,
            sample_to: 10,
            num_samples: 20,
        });
        let mut state = VocdState::default();

        // Build a token sequence with 60 tokens (mix of 20 types, some repeated)
        // This ensures enough tokens for sampling at N=5..10
        let word_pool = [
            "the", "dog", "cat", "ran", "big", "small", "house", "tree", "bird", "fish", "walk",
            "jump", "red", "blue", "green", "fast", "slow", "nice", "good", "bad",
        ];

        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let leaked: &'static talkbank_model::ChatFile = Box::leak(Box::new(chat_file));
        let file_ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: leaked,
            filename: "test",
            line_map: None,
        };

        // Create 3 copies of the word pool to get 60 tokens
        for _ in 0..3 {
            for chunk in word_pool.chunks(5) {
                let content: Vec<UtteranceContent> = chunk
                    .iter()
                    .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
                    .collect();
                let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
                let utt = Utterance::new(main);
                cmd.process_utterance(&utt, &file_ctx, &mut state);
            }
        }

        let result = cmd.finalize(state);

        assert_eq!(result.speakers.len(), 1);
        assert_eq!(result.warnings.len(), 0);

        let speaker = &result.speakers[0];
        assert_eq!(speaker.speaker, "CHI");
        assert_eq!(speaker.tokens, 60);
        assert_eq!(speaker.trials.len(), NUM_TRIALS);
        assert!(
            speaker.d_optimum_average > 0.0,
            "D should be positive, got {}",
            speaker.d_optimum_average
        );
    }

    /// Text rendering should include both per-trial tables and summary block.
    #[test]
    fn vocd_render_text_format() {
        let result = VocdResult {
            speakers: vec![VocdSpeakerResult {
                speaker: "CHI".to_string(),
                types: 100,
                tokens: 500,
                ttr: 0.2,
                trials: vec![VocdTrial {
                    entries: vec![NtEntry {
                        n: 35,
                        samples: 100,
                        mean_ttr: 0.80,
                        std_dev: 0.05,
                        d_value: 50.0,
                    }],
                    d_average: 50.0,
                    d_std_dev: 2.0,
                    d_optimum: 49.5,
                    min_least_sq: 0.001,
                }],
                d_optimum_values: vec![49.5],
                d_optimum_average: 49.5,
            }],
            warnings: vec![],
        };

        let text = result.render_text();
        assert!(text.contains("Speaker: *CHI:"));
        assert!(text.contains("D_optimum"));
        assert!(text.contains("VOCD RESULTS SUMMARY"));
        assert!(text.contains("Types,Tokens,TTR"));
        assert!(text.contains("49.50"));
    }
}
