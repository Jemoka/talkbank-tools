use talkbank_derive::error_code_enum;

#[error_code_enum]
enum NonUnitVariant {
    #[code("E001")]
    SomeError,
    #[code("E002")]
    TupleVariant(String),
    #[code("E999")]
    UnknownError,
}

fn main() {}
