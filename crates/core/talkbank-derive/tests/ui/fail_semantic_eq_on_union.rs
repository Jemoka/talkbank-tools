use talkbank_derive::SemanticEq;

#[derive(SemanticEq)]
union BadUnion {
    a: i32,
    b: f32,
}

fn main() {}
