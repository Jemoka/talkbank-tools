#![allow(unused_variables, unused_imports)]
//! Test module for parser suite in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.
//!
//! # Module layout
//!
//! | Module | Purpose |
//! |--------|---------|
//! | [`parser_impl`] | `ParserImpl` type alias (TreeSitterParser) + `parser_suite()` constructor |
//! | [`header_roundtrip`] | Reference-corpus whole-file vs `parse_header()` parity |
//! | [`dependent_tier_roundtrip`] | Reference-corpus whole-file vs dependent-tier fragment parity |
//! | [`word_tests`] | Golden word roundtrip tests |
//! | [`snapshot_tests`] | Insta snapshot tests for words, %mor, and utterances |
//! | [`tier_roundtrip`] | Golden tier roundtrip: main, %mor, %gra |
//! | [`tier_roundtrip_extra`] | Golden tier roundtrip: %pho, %sin |
//! | [`tier_roundtrip_text`] | Golden tier roundtrip: %wor, %com |

mod dependent_tier_roundtrip;
mod header_roundtrip;
mod legacy_mor;
mod main_and_utterance_roundtrip;
mod parser_impl;
mod snapshot_tests;
mod tier_roundtrip;
mod tier_roundtrip_extra;
mod tier_roundtrip_text;
mod word_tests;
