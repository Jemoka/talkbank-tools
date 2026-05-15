//! CLAN transform command implementations.
//!
//! Transform commands modify CHAT files in place, unlike analysis commands which
//! produce read-only statistics. Each module implements a specific CLAN transform
//! using the [`TransformCommand`](crate::framework::TransformCommand) trait.
//!
//! The general pipeline is: parse → transform → serialize → write. The
//! [`run_transform()`](crate::framework::run_transform) function handles this
//! pipeline for most commands; a few (DATACLEAN, LINES) use custom run
//! functions because they operate on serialized text rather than the AST.
//!
//! # Available transforms
//!
//! | Module | Command | Description |
//! |--------|---------|-------------|
//! | [`chstring`] | CHSTRING | String replacement using a changes file |
//! | [`combtier`] | COMBTIER | Combine multiple dependent tiers of the same type |
//! | [`compound`] | COMPOUND | Normalize compound word formatting (dash to plus) |
//! | [`dataclean`] | DATACLEAN | Fix common CHAT formatting errors (spacing, brackets) |
//! | [`dates`] | DATES | Compute participant ages from `@Birth`/`@Date` headers |
//! | [`delim`] | DELIM | Add missing terminators to main tiers |
//! | [`fixit`] | FIXIT | Normalize CHAT formatting via parse-serialize roundtrip |
//! | [`fixbullets`] | FIXBULLETS | Fix parsed bullet timing order and apply offsets |
//! | [`flo`] | FLO | Generate simplified fluent output on `%flo:` tier |
//! | [`indent`] | INDENT | Align CA overlap markers by column position |
//! | [`lines`] | LINES | Add or remove line numbers on CHAT tiers |
//! | [`longtier`] | LONGTIER | Remove line continuation wrapping |
//! | [`lowcase`] | LOWCASE | Lowercase all words on main tiers |
//! | [`makemod`] | MAKEMOD | Generate `%mod` tier from pronunciation lexicon |
//! | [`ort`] | ORT | Orthographic conversion via dictionary lookup |
//! | [`postmortem`] | POSTMORTEM | Pattern-matching rules for `%mor` post-processing |
//! | [`quotes`] | QUOTES | Extract quoted text to separate utterances |
//! | [`repeat`] | REPEAT | Mark utterances containing revisions with `[+ rep]` |
//! | [`retrace`] | RETRACE | Add `%ret:` tier with verbatim main-tier copy |
//! | [`tierorder`] | TIERORDER | Reorder dependent tiers to canonical order |
//! | [`trim`] | TRIM | Remove selected dependent tiers |

pub mod chstring;
pub mod combtier;
pub mod compound;
pub mod dataclean;
pub mod dates;
pub mod delim;
pub mod fixbullets;
pub mod fixit;
pub mod flo;
pub mod gem;
pub mod indent;
pub mod lines;
pub mod longtier;
pub mod lowcase;
pub mod makemod;
pub mod ort;
pub mod postmortem;
pub mod quotes;
pub mod repeat;
pub mod retrace;
pub mod roles;
pub mod tierorder;
pub mod trim;
