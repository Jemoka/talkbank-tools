//! COMPLEXITY — Syntactic complexity ratio from `%gra` dependency tier.
//!
//! Computes syntactic complexity by counting subordinating dependency
//! relations in the `%gra` tier and computing their ratio to total tokens.
//!
//! Complexity-contributing relations are clause-embedding dependencies
//! that indicate syntactic subordination. Two sets of relations are
//! supported:
//!
//! - **UD (Universal Dependencies)**: CSUBJ, CCOMP, XCOMP, ACL, ADVCL, APPOS, EXPL
//! - **Legacy CLAN**: CSUBJ, COMP, CPRED, CPOBJ, COBJ, CJCT, XJCT, NJCT, CMOD, XMOD
//!
//! The command auto-detects which set to use based on the relations found.
//!
//! Output per speaker: counts of each relation type, complexity tokens
//! (sum of all matched relations), total tokens (all non-PUNCT entries),
//! and the complexity ratio (complexity_tokens / total_tokens).
//!
//! # CLAN Manual
//!
//! See the [CLAN manual](https://talkbank.org/0info/manuals/CLAN.html) for the
//! original COMPLEXITY command specification.
//!
//! # Differences from CLAN
//!
//! - Uses typed AST `GraTier` with `GrammaticalRelation` entries rather than
//!   raw string scanning of `%gra` tier text.
//! - Auto-detects UD vs legacy relation names (CLAN requires compile-time config).
//! - Supports JSON and CSV output in addition to text/XLS.
//! - Relation matching includes sub-relations (e.g., `CSUBJ:pass` matches CSUBJ).

use std::collections::BTreeMap;
use std::fmt::Write;

use serde::Serialize;
use talkbank_model::{DependentTier, Utterance};

use crate::framework::{
    AnalysisCommand, AnalysisResult, CommandOutput, FileContext, OutputFormat, Section, TableRow,
};

/// Configuration for the COMPLEXITY command.
#[derive(Debug, Clone, Default)]
pub struct ComplexityConfig {}

/// Per-speaker complexity metrics.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SpeakerComplexity {
    /// Speaker identifier.
    pub speaker: String,
    /// CSUBJ (clausal subject) count.
    pub csubj: u64,
    /// CCOMP (clausal complement) count.
    pub ccomp: u64,
    /// XCOMP (open clausal complement) count.
    pub xcomp: u64,
    /// ACL (adnominal clause) count.
    pub acl: u64,
    /// ADVCL (adverbial clause modifier) count.
    pub advcl: u64,
    /// APPOS (appositional modifier) count — UD only.
    pub appos: u64,
    /// EXPL (expletive) count — UD only.
    pub expl: u64,
    /// COMP (complement) count — legacy only.
    pub comp: u64,
    /// CPRED (clausal predicate) count — legacy only.
    pub cpred: u64,
    /// CPOBJ (clausal object of preposition) count — legacy only.
    pub cpobj: u64,
    /// COBJ (clausal object) count — legacy only.
    pub cobj: u64,
    /// CJCT (clausal adjunct) count — legacy only.
    pub cjct: u64,
    /// XJCT (non-finite clausal adjunct) count — legacy only.
    pub xjct: u64,
    /// NJCT (nominal adjunct) count — legacy only.
    pub njct: u64,
    /// CMOD (clausal modifier) count — legacy only.
    pub cmod: u64,
    /// XMOD (non-finite clausal modifier) count — legacy only.
    pub xmod: u64,
    /// Total complexity tokens (sum of matched relations).
    pub tokens: u64,
    /// Total tokens (all non-PUNCT entries).
    pub total_tokens: u64,
}

impl SpeakerComplexity {
    /// Complexity ratio: complexity tokens / total tokens.
    fn ratio(&self) -> f64 {
        if self.total_tokens == 0 {
            0.0
        } else {
            self.tokens as f64 / self.total_tokens as f64
        }
    }
}

/// Whether the corpus uses UD or legacy dependency relations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub enum RelationStyle {
    /// Universal Dependencies (CSUBJ, CCOMP, XCOMP, ACL, ADVCL, APPOS, EXPL).
    Ud,
    /// Legacy CLAN (CSUBJ, COMP, CPRED, CPOBJ, COBJ, CJCT, XJCT, NJCT, CMOD, XMOD).
    Legacy,
}

/// Result of the COMPLEXITY command.
#[derive(Debug, Clone, Serialize)]
pub struct ComplexityResult {
    /// Per-speaker complexity metrics.
    pub speakers: Vec<SpeakerComplexity>,
    /// Detected relation style.
    pub style: RelationStyle,
}

impl CommandOutput for ComplexityResult {
    fn render_text(&self) -> String {
        self.to_analysis_result().render(OutputFormat::Text)
    }

    fn render_clan(&self) -> String {
        let mut out = String::new();
        let _ = write!(
            out,
            "File,Language,Corpus,Code,Age,Sex,Group,Race,SES,Role,Education,Custom_field"
        );
        let _ = write!(out, ",CSUBJ");
        if self.style == RelationStyle::Ud {
            let _ = write!(out, ",CCOMP,XCOMP,ACL,ADVCL,APPOS,EXPL");
        } else {
            let _ = write!(out, ",COMP,CPRED,CPOBJ,COBJ,CJCT,XJCT,NJCT,CMOD,XMOD");
        }
        let _ = writeln!(out, ",Tokens,TotalTokens,Ratio");

        for sp in &self.speakers {
            let _ = write!(out, ".,.,.,{},.,.,.,.,.,.,.,.", sp.speaker);
            let _ = write!(out, ",{}", sp.csubj);
            if self.style == RelationStyle::Ud {
                let _ = write!(
                    out,
                    ",{},{},{},{},{},{}",
                    sp.ccomp, sp.xcomp, sp.acl, sp.advcl, sp.appos, sp.expl
                );
            } else {
                let _ = write!(
                    out,
                    ",{},{},{},{},{},{},{},{},{}",
                    sp.comp,
                    sp.cpred,
                    sp.cpobj,
                    sp.cobj,
                    sp.cjct,
                    sp.xjct,
                    sp.njct,
                    sp.cmod,
                    sp.xmod
                );
            }
            let _ = writeln!(out, ",{},{},{:.6}", sp.tokens, sp.total_tokens, sp.ratio());
        }
        out
    }
}

impl ComplexityResult {
    fn to_analysis_result(&self) -> AnalysisResult {
        let mut result = AnalysisResult::new("complexity");
        for sp in &self.speakers {
            let mut rows = vec![TableRow {
                values: vec!["CSUBJ".to_owned(), sp.csubj.to_string()],
            }];
            if self.style == RelationStyle::Ud {
                rows.extend([
                    TableRow {
                        values: vec!["CCOMP".to_owned(), sp.ccomp.to_string()],
                    },
                    TableRow {
                        values: vec!["XCOMP".to_owned(), sp.xcomp.to_string()],
                    },
                    TableRow {
                        values: vec!["ACL".to_owned(), sp.acl.to_string()],
                    },
                    TableRow {
                        values: vec!["ADVCL".to_owned(), sp.advcl.to_string()],
                    },
                    TableRow {
                        values: vec!["APPOS".to_owned(), sp.appos.to_string()],
                    },
                    TableRow {
                        values: vec!["EXPL".to_owned(), sp.expl.to_string()],
                    },
                ]);
            } else {
                rows.extend([
                    TableRow {
                        values: vec!["COMP".to_owned(), sp.comp.to_string()],
                    },
                    TableRow {
                        values: vec!["CPRED".to_owned(), sp.cpred.to_string()],
                    },
                    TableRow {
                        values: vec!["CPOBJ".to_owned(), sp.cpobj.to_string()],
                    },
                    TableRow {
                        values: vec!["COBJ".to_owned(), sp.cobj.to_string()],
                    },
                    TableRow {
                        values: vec!["CJCT".to_owned(), sp.cjct.to_string()],
                    },
                    TableRow {
                        values: vec!["XJCT".to_owned(), sp.xjct.to_string()],
                    },
                    TableRow {
                        values: vec!["NJCT".to_owned(), sp.njct.to_string()],
                    },
                    TableRow {
                        values: vec!["CMOD".to_owned(), sp.cmod.to_string()],
                    },
                    TableRow {
                        values: vec!["XMOD".to_owned(), sp.xmod.to_string()],
                    },
                ]);
            }
            rows.extend([
                TableRow {
                    values: vec!["Tokens".to_owned(), sp.tokens.to_string()],
                },
                TableRow {
                    values: vec!["TotalTokens".to_owned(), sp.total_tokens.to_string()],
                },
                TableRow {
                    values: vec!["Ratio".to_owned(), format!("{:.6}", sp.ratio())],
                },
            ]);
            result.add_section(Section::with_table(
                format!("Speaker: {}", sp.speaker),
                vec!["Relation".to_owned(), "Count".to_owned()],
                rows,
            ));
        }
        result
    }
}

/// Per-speaker accumulator.
#[derive(Debug, Default)]
struct SpeakerAccum {
    csubj: u64,
    ccomp: u64,
    xcomp: u64,
    acl: u64,
    advcl: u64,
    appos: u64,
    expl: u64,
    comp: u64,
    cpred: u64,
    cpobj: u64,
    cobj: u64,
    cjct: u64,
    xjct: u64,
    njct: u64,
    cmod: u64,
    xmod: u64,
    tokens: u64,
    total_tokens: u64,
    has_ud: bool,
    has_legacy: bool,
}

impl SpeakerAccum {
    /// Process a single dependency relation label.
    fn count_relation(&mut self, label: &str) {
        // Strip sub-type suffixes (e.g., "CSUBJ:pass" → "CSUBJ", "ACL-relcl" → "ACL")
        let base = label
            .split(['-', ':'])
            .next()
            .unwrap_or(label)
            .to_uppercase();

        if base == "PUNCT" {
            return;
        }
        self.total_tokens += 1;

        match base.as_str() {
            "CSUBJ" => {
                self.csubj += 1;
                self.tokens += 1;
            }
            "CCOMP" => {
                self.ccomp += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "XCOMP" => {
                self.xcomp += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "ACL" => {
                self.acl += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "ADVCL" => {
                self.advcl += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "APPOS" => {
                self.appos += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "EXPL" => {
                self.expl += 1;
                self.tokens += 1;
                self.has_ud = true;
            }
            "COMP" => {
                self.comp += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "CPRED" => {
                self.cpred += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "CPOBJ" => {
                self.cpobj += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "COBJ" => {
                self.cobj += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "CJCT" => {
                self.cjct += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "XJCT" => {
                self.xjct += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "NJCT" => {
                self.njct += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "CMOD" => {
                self.cmod += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            "XMOD" => {
                self.xmod += 1;
                self.tokens += 1;
                self.has_legacy = true;
            }
            _ => {}
        }
    }

    fn into_result(self, speaker: &str) -> SpeakerComplexity {
        SpeakerComplexity {
            speaker: speaker.to_owned(),
            csubj: self.csubj,
            ccomp: self.ccomp,
            xcomp: self.xcomp,
            acl: self.acl,
            advcl: self.advcl,
            appos: self.appos,
            expl: self.expl,
            comp: self.comp,
            cpred: self.cpred,
            cpobj: self.cpobj,
            cobj: self.cobj,
            cjct: self.cjct,
            xjct: self.xjct,
            njct: self.njct,
            cmod: self.cmod,
            xmod: self.xmod,
            tokens: self.tokens,
            total_tokens: self.total_tokens,
        }
    }
}

/// Accumulated state for COMPLEXITY across all files.
#[derive(Debug, Default)]
pub struct ComplexityState {
    by_speaker: BTreeMap<String, SpeakerAccum>,
}

/// COMPLEXITY command implementation.
#[derive(Debug, Clone, Default)]
pub struct ComplexityCommand;

impl AnalysisCommand for ComplexityCommand {
    type Config = ComplexityConfig;
    type State = ComplexityState;
    type Output = ComplexityResult;

    fn process_utterance(
        &self,
        utterance: &Utterance,
        _file_context: &FileContext<'_>,
        state: &mut Self::State,
    ) {
        let speaker = utterance.main.speaker.to_string();

        // Find %gra tier and use typed relations
        let gra_tier = utterance.dependent_tiers.iter().find_map(|dep| {
            if let DependentTier::Gra(gra) = dep {
                Some(gra)
            } else {
                None
            }
        });

        let Some(gra_tier) = gra_tier else { return };

        let accum = state.by_speaker.entry(speaker).or_default();

        for relation in gra_tier.relations().iter() {
            accum.count_relation(relation.relation.as_str());
        }
    }

    fn finalize(&self, state: Self::State) -> ComplexityResult {
        let mut has_ud = false;
        let mut has_legacy = false;
        let mut speakers = Vec::new();

        for (speaker, accum) in state.by_speaker {
            has_ud |= accum.has_ud;
            has_legacy |= accum.has_legacy;
            speakers.push(accum.into_result(&speaker));
        }

        let style = if has_legacy && !has_ud {
            RelationStyle::Legacy
        } else {
            RelationStyle::Ud
        };

        ComplexityResult { speakers, style }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use talkbank_model::Span;
    use talkbank_model::{
        GraTier, GrammaticalRelation, MainTier, Terminator, UtteranceContent, Word,
    };

    fn make_utterance_with_gra(
        speaker: &str,
        words: &[&str],
        relations: Vec<GrammaticalRelation>,
    ) -> Utterance {
        let content: Vec<UtteranceContent> = words
            .iter()
            .map(|w| UtteranceContent::Word(Box::new(Word::simple(*w))))
            .collect();
        let main = MainTier::new(speaker, content, Terminator::Period { span: Span::DUMMY });
        let mut utt = Utterance::new(main);
        utt.dependent_tiers
            .push(DependentTier::Gra(GraTier::new_gra(relations)));
        utt
    }

    fn file_ctx() -> (talkbank_model::ChatFile, FileContext<'static>) {
        let chat_file = talkbank_model::ChatFile::new(vec![]);
        let ctx = FileContext {
            path: std::path::Path::new("test.cha"),
            chat_file: unsafe { &*(&chat_file as *const _) },
            filename: "test",
            line_map: None,
        };
        (chat_file, ctx)
    }

    #[test]
    fn counts_ud_relations() {
        let cmd = ComplexityCommand;
        let mut state = ComplexityState::default();
        let (_cf, ctx) = file_ctx();

        let u = make_utterance_with_gra(
            "CHI",
            &["he", "said", "wants", "go"],
            vec![
                GrammaticalRelation::new(1, 2, "APPOS"),
                GrammaticalRelation::new(2, 0, "ROOT"),
                GrammaticalRelation::new(3, 2, "CCOMP"),
                GrammaticalRelation::new(4, 3, "XCOMP"),
            ],
        );
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.style, RelationStyle::Ud);
        assert_eq!(result.speakers.len(), 1);

        let sp = &result.speakers[0];
        assert_eq!(sp.speaker, "CHI");
        assert_eq!(sp.appos, 1);
        assert_eq!(sp.ccomp, 1);
        assert_eq!(sp.xcomp, 1);
        assert_eq!(sp.tokens, 3);
        assert_eq!(sp.total_tokens, 4); // ROOT counted too
    }

    #[test]
    fn counts_legacy_relations() {
        let cmd = ComplexityCommand;
        let mut state = ComplexityState::default();
        let (_cf, ctx) = file_ctx();

        let u = make_utterance_with_gra(
            "PAR",
            &["he", "said", "that"],
            vec![
                GrammaticalRelation::new(1, 2, "SUBJ"),
                GrammaticalRelation::new(2, 0, "ROOT"),
                GrammaticalRelation::new(3, 2, "COMP"),
            ],
        );
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert_eq!(result.style, RelationStyle::Legacy);

        let sp = &result.speakers[0];
        assert_eq!(sp.comp, 1);
        assert_eq!(sp.tokens, 1);
        assert_eq!(sp.total_tokens, 3);
    }

    #[test]
    fn skips_punct() {
        let cmd = ComplexityCommand;
        let mut state = ComplexityState::default();
        let (_cf, ctx) = file_ctx();

        let u = make_utterance_with_gra(
            "CHI",
            &["go"],
            vec![
                GrammaticalRelation::new(1, 0, "ROOT"),
                GrammaticalRelation::new(2, 1, "PUNCT"),
            ],
        );
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        let sp = &result.speakers[0];
        assert_eq!(sp.total_tokens, 1); // PUNCT excluded
    }

    #[test]
    fn no_gra_tier_skips_utterance() {
        let cmd = ComplexityCommand;
        let mut state = ComplexityState::default();
        let (_cf, ctx) = file_ctx();

        let content = vec![UtteranceContent::Word(Box::new(Word::simple("hello")))];
        let main = MainTier::new("CHI", content, Terminator::Period { span: Span::DUMMY });
        let u = Utterance::new(main);
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        assert!(result.speakers.is_empty());
    }

    #[test]
    fn ratio_calculation() {
        let cmd = ComplexityCommand;
        let mut state = ComplexityState::default();
        let (_cf, ctx) = file_ctx();

        let u = make_utterance_with_gra(
            "CHI",
            &["he", "said", "he", "wants", "go"],
            vec![
                GrammaticalRelation::new(1, 2, "APPOS"),
                GrammaticalRelation::new(2, 0, "ROOT"),
                GrammaticalRelation::new(3, 4, "EXPL"),
                GrammaticalRelation::new(4, 2, "CCOMP"),
                GrammaticalRelation::new(5, 4, "XCOMP"),
            ],
        );
        cmd.process_utterance(&u, &ctx, &mut state);

        let result = cmd.finalize(state);
        let sp = &result.speakers[0];
        // APPOS(1) + EXPL(1) + CCOMP(1) + XCOMP(1) = 4 tokens, 5 total
        assert_eq!(sp.tokens, 4);
        assert_eq!(sp.total_tokens, 5);
        assert!((sp.ratio() - 0.8).abs() < 0.001);
    }
}
