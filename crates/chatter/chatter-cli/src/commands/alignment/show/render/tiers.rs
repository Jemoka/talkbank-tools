//! Per-tier alignment rendering for `%mor`, `%gra`, `%pho`, and `%sin`.
//!
//! Each `render_*` function prints the aligned pairs for one tier on one
//! utterance, using the extraction helpers from [`crate::commands::alignment::helpers`]
//! to convert model indices to display text. Unresolved indices show `???`.
//! All renderers return `(shown, errors)` counts for the orchestrator's totals.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

use talkbank_model::ParseError;
use talkbank_model::alignment::IndexPair;
use talkbank_model::{AlignmentSet, Utterance};

use crate::commands::alignment::helpers::{
    get_gra_relation_text, get_main_content_text, get_mor_item_text, get_pho_form_text,
    get_sin_item_text,
};

/// Render main-to-%mor alignments for one utterance.
pub(super) fn render_main_to_mor(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    compact: bool,
) -> (usize, usize) {
    let main_to_mor = match &alignments.mor {
        Some(alignment) => alignment,
        None => return (0, 0),
    };

    if compact {
        print!("Utt#{} Main→Mor: ", utterance_index + 1);
        for (i, pair) in main_to_mor.pairs.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            let main_text = main_text(utterance, pair.source());
            let mor_text = mor_text(utterance, pair.target());
            print!("\"{}\" → \"{}\"", main_text, mor_text);
        }
        println!();
    } else {
        println!("Main → %mor alignment:");
        for pair in &main_to_mor.pairs {
            let main_text = main_text(utterance, pair.source());
            let mor_text = mor_text(utterance, pair.target());
            println!("  \"{}\" → \"{}\"", main_text, mor_text);
        }
    }

    let errors = render_errors(&main_to_mor.errors, compact);
    (1, errors)
}

/// Render %mor-to-%gra alignments for one utterance.
pub(super) fn render_mor_to_gra(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    compact: bool,
) -> (usize, usize) {
    let mor_to_gra = match &alignments.gra {
        Some(alignment) => alignment,
        None => return (0, 0),
    };

    if compact {
        print!("Utt#{} Mor→Gra: ", utterance_index + 1);
        for (i, pair) in mor_to_gra.pairs.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            let mor_text = mor_text(utterance, pair.source());
            let gra_text = gra_text(utterance, pair.target());
            print!("\"{}\" → \"{}\"", mor_text, gra_text);
        }
        println!();
    } else {
        println!("%mor → %gra alignment:");
        for pair in &mor_to_gra.pairs {
            let mor_text = mor_text(utterance, pair.source());
            let gra_text = gra_text(utterance, pair.target());
            println!("  \"{}\" → \"{}\"", mor_text, gra_text);
        }
    }

    let errors = render_errors(&mor_to_gra.errors, compact);
    (1, errors)
}

/// Render main-to-%pho alignments for one utterance.
pub(super) fn render_main_to_pho(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    compact: bool,
) -> (usize, usize) {
    let main_to_pho = match &alignments.pho {
        Some(alignment) => alignment,
        None => return (0, 0),
    };

    if compact {
        print!("Utt#{} Main→Pho: ", utterance_index + 1);
        for (i, pair) in main_to_pho.pairs.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            let main_text = main_text(utterance, pair.source());
            let pho_text = pho_text(utterance, pair.target());
            print!("\"{}\" → \"{}\"", main_text, pho_text);
        }
        println!();
    } else {
        println!("Main → %pho alignment:");
        for pair in &main_to_pho.pairs {
            let main_text = main_text(utterance, pair.source());
            let pho_text = pho_text(utterance, pair.target());
            println!("  \"{}\" → \"{}\"", main_text, pho_text);
        }
    }

    let errors = render_errors(&main_to_pho.errors, compact);
    (1, errors)
}

/// Render main-to-%sin alignments for one utterance.
pub(super) fn render_main_to_sin(
    utterance: &Utterance,
    utterance_index: usize,
    alignments: &AlignmentSet,
    compact: bool,
) -> (usize, usize) {
    let main_to_sin = match &alignments.sin {
        Some(alignment) => alignment,
        None => return (0, 0),
    };

    if compact {
        print!("Utt#{} Main→Sin: ", utterance_index + 1);
        for (i, pair) in main_to_sin.pairs.iter().enumerate() {
            if i > 0 {
                print!(", ");
            }
            let main_text = main_text(utterance, pair.source());
            let sin_text = sin_text(utterance, pair.target());
            print!("\"{}\" → \"{}\"", main_text, sin_text);
        }
        println!();
    } else {
        println!("Main → %sin alignment:");
        for pair in &main_to_sin.pairs {
            let main_text = main_text(utterance, pair.source());
            let sin_text = sin_text(utterance, pair.target());
            println!("  \"{}\" → \"{}\"", main_text, sin_text);
        }
    }

    let errors = render_errors(&main_to_sin.errors, compact);
    (1, errors)
}

/// Render a main-tier token for an alignment index (or `???` when missing).
fn main_text(utterance: &Utterance, index: Option<usize>) -> String {
    index
        .and_then(|idx| get_main_content_text(&utterance.main, idx))
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %mor item for an alignment index (or `???` when missing).
fn mor_text(utterance: &Utterance, index: Option<usize>) -> String {
    index
        .and_then(|idx| utterance.mor_tier().and_then(|m| get_mor_item_text(m, idx)))
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %gra relation for an alignment index (or `???` when missing).
fn gra_text(utterance: &Utterance, index: Option<usize>) -> String {
    index
        .and_then(|idx| {
            utterance
                .gra_tier()
                .and_then(|g| get_gra_relation_text(g, idx))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %pho item for an alignment index (or `???` when missing).
fn pho_text(utterance: &Utterance, index: Option<usize>) -> String {
    index
        .and_then(|idx| {
            utterance
                .pho()
                .as_ref()
                .and_then(|p| get_pho_form_text(p, idx))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Render a %sin item for an alignment index (or `???` when missing).
fn sin_text(utterance: &Utterance, index: Option<usize>) -> String {
    index
        .and_then(|idx| {
            utterance
                .sin()
                .as_ref()
                .and_then(|s| get_sin_item_text(s, idx))
        })
        .unwrap_or_else(|| "???".to_string())
}

/// Print alignment errors for a tier pair and return the count.
fn render_errors(errors: &[ParseError], compact: bool) -> usize {
    if errors.is_empty() {
        return 0;
    }

    if compact {
        println!("  ⚠ {} error(s)", errors.len());
    } else {
        println!("  Errors:");
        for err in errors {
            println!("    • {}", err.message);
        }
    }

    errors.len()
}
