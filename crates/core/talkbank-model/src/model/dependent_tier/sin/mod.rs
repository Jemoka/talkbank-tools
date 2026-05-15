//! Gesture and sign annotation tier (%sin) for CHAT transcripts.
//!
//! The %sin tier captures non-verbal communication including gestures, pointing,
//! and sign language. It's essential for multimodal communication research,
//! particularly in child language development and sign language studies.
//!
//! # Format
//!
//! Gesture codes use colon-separated components:
//! ```text
//! g:referent:gesture_type
//! ```
//!
//! Where:
//! - **g**: Gesture marker
//! - **referent**: What the gesture refers to (object, location, person)
//! - **gesture_type**: Type of gesture (dpoint, hold, give, etc.)
//!
//! # Token Types
//!
//! - **`0`**: No gesture (word spoken without accompanying gesture)
//! - **Simple gesture**: `g:ball:dpoint` (deictic point to ball)
//! - **Multiple gestures**: `〔g:toy:hold g:toy:shake〕` (multiple gestures for one word)
//!
//! # Common Gesture Types
//!
//! - **dpoint**: Deictic pointing
//! - **hold**: Holding gesture
//! - **give**: Giving gesture
//! - **show**: Showing gesture
//! - **point**: General pointing
//! - **reach**: Reaching gesture
//!
//! # CHAT Manual Reference
//!
//! - [Gestures](https://talkbank.org/0info/manuals/CHAT.html#Gestures)
//!
//! # Examples
//!
//! ```text
//! *CHI: I want ball .
//! %sin: 0 0 g:ball:dpoint .
//!
//! *CHI: give me cookie .
//! %sin: g:mom:reach 0 g:cookie:point .
//! ```

mod item;
#[cfg(test)]
mod tests;
mod tier;

pub use item::{SinGroupGestures, SinItem, SinToken};
pub use tier::SinTier;
