//! CLAN command implementations.
//!
//! Most modules implement analysis commands using the
//! [`AnalysisCommand`](crate::framework::AnalysisCommand) trait.
//! The [`check`] module implements CLAN's CHECK validation command.

pub mod chains;
pub mod check;
pub mod chip;
pub mod codes;
pub mod combo;
pub mod complexity;
pub mod cooccur;
pub mod corelex;
pub mod dist;
pub mod dss;
pub mod eval;
pub mod eval_columns;
pub mod flucalc;
pub mod freq;
pub mod freqpos;
pub mod gemlist;
pub mod ipsyn;
pub mod keymap;
pub mod kideval;
pub mod kideval_columns;
pub mod kwal;
pub mod linker_audit;
pub mod maxwd;
pub mod megrasp;
pub mod mlt;
pub mod mlu;
pub mod modrep;
pub mod mor;
pub mod mortable;
pub mod overlap_audit;
pub mod phonfreq;
pub mod post;
pub mod postlist;
pub mod postmodrules;
pub mod posttrain;
pub mod rely;
pub mod script;
pub mod sugar;
pub mod timedur;
pub mod trnfix;
pub mod uniq;
pub mod vocd;
pub mod wdlen;
pub mod wdsize;
