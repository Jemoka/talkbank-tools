use talkbank_derive::error_code_enum;

#[error_code_enum]
enum MissingCodeAttr {
    #[code("E001")]
    SomeError,
    MissingAttr,
    #[code("E999")]
    UnknownError,
}

fn main() {}
