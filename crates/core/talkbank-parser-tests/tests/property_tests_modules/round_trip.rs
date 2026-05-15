//! Round-trip tests: parse → to_chat → should equal input
//!
//! Ensures that parsing and serialization are inverse operations.

#[path = "round_trip/dependent_tiers.rs"]
mod dependent_tiers;
#[path = "round_trip/headers.rs"]
mod headers;
#[path = "round_trip/helpers.rs"]
mod helpers;
#[path = "round_trip/main_tier.rs"]
mod main_tier;
#[path = "round_trip/mor.rs"]
mod mor;
#[path = "round_trip/utterance.rs"]
mod utterance;
#[path = "round_trip/word.rs"]
mod word;
