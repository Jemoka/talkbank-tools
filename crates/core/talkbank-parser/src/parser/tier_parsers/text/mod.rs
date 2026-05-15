//! Parsers for utterance-level text-based dependent tiers
//!
//! These parsers handle text-based tiers that can contain inline bullets and picture references:
//! - %com (comments) - supports bullets and pictures
//! - %exp (explanation/expansion) - supports bullets
//! - %add (addressee) - supports bullets
//! - %spa (speech act) - supports bullets
//! - %sit (situation) - supports bullets
//! - %gpx (gem/pause extension) - supports bullets
//! - %int (intonation) - supports bullets
//!
//! All these tiers use structured content with text_with_bullets or text_with_bullets_and_pics nodes.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Comment_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Explanation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Addressee_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Situation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Intonation_Tier>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Speech_Act>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Gems>

mod add;
mod com;
mod exp;
mod gpx;
mod helpers;
mod int;
mod sit;
mod spa;

#[cfg(test)]
mod tests;

pub use add::parse_add_tier;
pub use com::parse_com_tier;
pub use exp::parse_exp_tier;
pub use gpx::parse_gpx_tier;
pub use int::parse_int_tier;
pub use sit::parse_sit_tier;
pub use spa::parse_spa_tier;
