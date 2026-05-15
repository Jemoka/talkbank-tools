//! Cross-utterance validation test suite wiring.
//!
//! Submodules are grouped by linker/terminator pattern so rule re-enablement
//! work can be tracked with focused regression fixtures.

#[path = "tests/edge_cases.rs"]
mod edge_cases;
#[path = "tests/helpers.rs"]
mod helpers;
#[path = "tests/other_completion.rs"]
mod other_completion;
#[path = "tests/quotation_follows.rs"]
mod quotation_follows;
#[path = "tests/quotation_precedes.rs"]
mod quotation_precedes;
#[path = "tests/self_completion.rs"]
mod self_completion;
#[path = "tests/terminator_linker_pairing.rs"]
mod terminator_linker_pairing;
