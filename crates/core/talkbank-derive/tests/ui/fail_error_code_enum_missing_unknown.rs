use talkbank_derive::error_code_enum;

#[error_code_enum]
enum MissingUnknown {
    #[code("E001")]
    SomeError,
    #[code("E002")]
    AnotherError,
}

fn main() {}
