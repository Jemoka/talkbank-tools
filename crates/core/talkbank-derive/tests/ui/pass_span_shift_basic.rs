use talkbank_derive::SpanShift;
use talkbank_model::Span;

// Verify SpanShift can be derived on a struct with Span fields
#[derive(Debug, Clone, SpanShift)]
struct Located {
    value: String,
    span: Span,
}

fn main() {}
