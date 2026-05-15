use talkbank_derive::SpanShift;

#[derive(SpanShift)]
union BadUnion {
    a: i32,
    b: f32,
}

fn main() {}
