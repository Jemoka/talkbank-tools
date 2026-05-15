//! Supertype matchers for header and pre-begin-header node kinds.
//!
//! # Related CHAT Manual Sections
//!
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#UTF8_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Begin_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#End_Header>

/// Check if a node kind is a `header` subtype
///
/// **Subtypes:** activities_header, bck_header, bg_header, birth_of_header, etc.
pub fn is_header(kind: &str) -> bool {
    use crate::node_types::{
        ACTIVITIES_HEADER, BCK_HEADER, BEGIN_HEADER, BG_HEADER, BIRTH_OF_HEADER,
        BIRTHPLACE_OF_HEADER, BLANK_HEADER, COMMENT_HEADER, DATE_HEADER, EG_HEADER, END_HEADER,
        G_HEADER, HEADER, ID_HEADER, L1_OF_HEADER, LANGUAGES_HEADER, LOCATION_HEADER, MEDIA_HEADER,
        NEW_EPISODE_HEADER, NUMBER_HEADER, OPTIONS_HEADER, PAGE_HEADER, PARTICIPANTS_HEADER,
        RECORDING_QUALITY_HEADER, ROOM_LAYOUT_HEADER, SITUATION_HEADER, T_HEADER,
        TAPE_LOCATION_HEADER, TIME_DURATION_HEADER, TIME_START_HEADER, TRANSCRIBER_HEADER,
        TRANSCRIPTION_HEADER, TYPES_HEADER, UNSUPPORTED_HEADER, UTF8_HEADER, VIDEOS_HEADER,
        WARNING_HEADER,
    };

    matches!(
        kind,
        HEADER
            | ACTIVITIES_HEADER
            | BCK_HEADER
            | BEGIN_HEADER
            | BG_HEADER
            | BIRTH_OF_HEADER
            | BIRTHPLACE_OF_HEADER
            | BLANK_HEADER
            | COMMENT_HEADER
            | DATE_HEADER
            | EG_HEADER
            | END_HEADER
            | G_HEADER
            | ID_HEADER
            | L1_OF_HEADER
            | LANGUAGES_HEADER
            | LOCATION_HEADER
            | MEDIA_HEADER
            | NEW_EPISODE_HEADER
            | NUMBER_HEADER
            | OPTIONS_HEADER
            | PAGE_HEADER
            | PARTICIPANTS_HEADER
            | RECORDING_QUALITY_HEADER
            | ROOM_LAYOUT_HEADER
            | SITUATION_HEADER
            | T_HEADER
            | TAPE_LOCATION_HEADER
            | TIME_DURATION_HEADER
            | TIME_START_HEADER
            | TRANSCRIBER_HEADER
            | TRANSCRIPTION_HEADER
            | TYPES_HEADER
            | UNSUPPORTED_HEADER
            | UTF8_HEADER
            | VIDEOS_HEADER
            | WARNING_HEADER
    )
}

/// Check if a node kind is a `pre_begin_header` subtype
///
/// **Subtypes:** pid_header, color_words_header, window_header, font_header
pub fn is_pre_begin_header(kind: &str) -> bool {
    use crate::node_types::{
        COLOR_WORDS_HEADER, FONT_HEADER, PID_HEADER, PRE_BEGIN_HEADER, WINDOW_HEADER,
    };

    matches!(
        kind,
        PRE_BEGIN_HEADER | COLOR_WORDS_HEADER | FONT_HEADER | PID_HEADER | WINDOW_HEADER
    )
}
