//! Content with inline media bullets for dependent tiers.
//!
//! Some dependent tiers (%act, %cod, %com, %exp, %gpx, %sit, %spa, %add, %int)
//! can contain inline media bullets that mark precise timing positions within
//! the text content. This allows researchers to time-align specific portions of
//! dependent tier content with media recordings.
//!
//! # Bullet Format
//!
//! Media timing bullets use the format: `\u0015START_END\u0015`
//! - START: Start time in milliseconds
//! - END: End time in milliseconds
//! - Delimiter: `\u0015` (Unicode character U+0015, Negative Acknowledgement)
//!
//! Picture references (in %com and @Comment): `\u0015%pic:\"filename\"\u0015`
//!
//! # Tiers That Support Bullets
//!
//! - **%act**: Action descriptions
//! - **%cod**: Coding categories
//! - **%com**: Comments
//! - **%exp**: Explanations
//! - **%gpx**: Gestures with timing
//! - **%sit**: Situational context
//! - **%spa**: Speech acts
//! - **%add**: Addressee
//! - **%int**: Intonation
//!
//! # CHAT Format Examples
//!
//! Coding tier with timing bullets:
//! ```text
//! %cod:\tthis is junk 2051689_2052652 and more 2062689_2063652
//! ```
//!
//! This payload is represented internally as ordered segments:
//! - TextSegment(\"this is junk \")
//! - Bullet(2051689, 2052652)
//! - TextSegment(\" and more \")
//! - Bullet(2062689, 2063652)
//!
//! Comment with picture reference:
//! ```text
//! %com:\tChild points to 2051689_2052652 picture %pic:\"toy.jpg\" on table
//! ```
//!
//! Action tier with multiple bullets:
//! ```text
//! %act:\tpicks up toy 1000_2000 then drops it 3000_4000
//! ```
//!
//! # References
//!
//! - [CHAT Manual: Dependent Tiers](https://talkbank.org/0info/manuals/CHAT.html#Dependent_Tiers)
//! - [CHAT Manual: Media Bullets](https://talkbank.org/0info/manuals/CHAT.html#Bullets)

mod content;
mod segment;
mod write;

#[cfg(test)]
mod tests;

pub use content::BulletContent;
pub use segment::{
    BulletContentBullet, BulletContentPicture, BulletContentSegment, BulletContentText,
};
