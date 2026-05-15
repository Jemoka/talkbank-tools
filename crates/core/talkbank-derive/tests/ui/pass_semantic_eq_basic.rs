use talkbank_derive::SemanticEq;
use talkbank_model::Span;
// The SemanticEq macro expansion references talkbank_model::model internally
use talkbank_model::model;

// Verify SemanticEq can be derived on a struct with skip attribute
#[derive(Debug, Clone, SemanticEq)]
struct Simple {
    value: String,
    #[semantic_eq(skip)]
    span: Span,
}

fn main() {}
