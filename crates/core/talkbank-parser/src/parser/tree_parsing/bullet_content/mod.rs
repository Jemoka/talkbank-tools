//! Bullet content parsing - handles inline bullets and picture references in tier content
//!
//! This module parses tree-sitter nodes of type `text_with_bullets` or `text_with_bullets_and_pics`
//! into structured `BulletContent` with segments for text, inline bullets, and picture references.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#Working_with_Media>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers>

mod inline_bullet;
mod inline_pic;
mod parse;
#[cfg(test)]
mod tests;

pub use parse::parse_bullet_content;
