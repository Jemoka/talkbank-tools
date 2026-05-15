//! Integration tests for byte-offset and LSP position conversion helpers.

// Integration test targets compile as separate crates; the
// `cfg_attr(test, ...)` allow at lib.rs's crate root does not apply
// here. Test code uses `unwrap`/`expect` by convention.
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::panic,
    clippy::unreachable
)]

mod position_conversion;

mod ascii {
    include!("position_conversion/ascii.rs");
}

mod unicode {
    include!("position_conversion/unicode.rs");
}

mod bounds {
    include!("position_conversion/bounds.rs");
}

mod roundtrip {
    include!("position_conversion/roundtrip.rs");
}

mod chat {
    include!("position_conversion/chat.rs");
}
