use talkbank_derive::ValidationTagged;
use talkbank_model::model;

#[derive(ValidationTagged)]
struct NotAnEnum {
    value: String,
}

fn main() {}
