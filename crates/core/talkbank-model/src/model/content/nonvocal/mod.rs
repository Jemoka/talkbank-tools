//! Nonvocal-event markers (`&{n=LABEL`, `&}n=LABEL`, `&{n=LABEL}`).
//!
//! The module separates scoped begin/end boundaries from point-event markers
//! so validation can enforce pairing and label consistency where needed.
//! Scoped markers are validated across utterance boundaries, while simple
//! markers model stand-alone events that do not require balancing.
//!
//! # CHAT Format References
//!
//! - [Long Nonverbal Event](https://talkbank.org/0info/manuals/CHAT.html#LongNonverbalEvent)

mod begin;
mod end;
mod label;
mod simple;

pub use begin::NonvocalBegin;
pub use end::NonvocalEnd;
pub use label::NonvocalLabel;
pub use simple::NonvocalSimple;
