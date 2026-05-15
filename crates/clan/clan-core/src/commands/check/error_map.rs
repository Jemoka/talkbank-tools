//! Mapping between CLAN CHECK error numbers and our error codes.
//!
//! CLAN CHECK uses a flat numbering system (1–161) defined in `check.cpp`.
//! This module provides bidirectional mapping between those numbers and our
//! typed `ErrorCode` variants.
//!
//! # Divergences
//!
//! Not all CHECK errors have direct equivalents — some are obsolete (e.g.,
//! error 96 "Word color is now illegal"), some are subsumed by our parser's
//! structural validation, and some check conditions we handle differently.
//! Unmapped errors return number 0.

use talkbank_model::codes::ErrorCode;

/// Map our `ErrorCode` to the closest CHECK error number (1–161).
///
/// Returns 0 if there is no direct CHECK equivalent.
pub fn check_error_number(code: &ErrorCode) -> u16 {
    match code {
        // -- Structure errors --
        ErrorCode::InvalidLineFormat => 1, // Expected @, %, or *
        ErrorCode::MissingColonAfterSpeaker => 2, // Missing ':' and argument
        ErrorCode::SyntaxError => 8,       // Expected @, %, *, TAB
        ErrorCode::MissingSpeaker => 12,   // Missing speaker name/role
        ErrorCode::MissingTerminator => 21, // Utterance delimiter expected

        // -- Header errors --
        ErrorCode::MissingRequiredHeader => 6, // @Begin missing (approximate)
        ErrorCode::MissingEndHeader => 7,      // @End missing
        ErrorCode::MissingUTF8Header => 69,    // UTF8 header missing
        ErrorCode::DuplicateHeader => 44,      // Content after @End / file must end with @End
        ErrorCode::InvalidIDFormat => 143,     // @ID needs 10 fields
        ErrorCode::EmptyParticipantsHeader => 12, // Missing speaker name
        ErrorCode::InvalidParticipantRole => 15, // Illegal role
        ErrorCode::InvalidDateFormat => 34,    // Illegal date
        ErrorCode::InvalidAgeFormat => 153,    // Age format
        ErrorCode::InvalidLanguageCode => 121, // Language code not found
        ErrorCode::OrphanIDHeader => 60,       // @ID missing
        ErrorCode::UnmatchedBeginGem => 45,    // More @Bg than @Eg
        ErrorCode::UnmatchedEndGem => 46,      // @Eg without @Bg
        ErrorCode::MediaFilenameMismatch => 157, // Media name mismatch
        ErrorCode::UnsupportedSex => 64,       // Wrong gender
        ErrorCode::UnsupportedSesValue => 144, // Illegal SES
        ErrorCode::EmptyOptionsHeader => 103,  // Approximate: options issue
        ErrorCode::UnsupportedMediaType => 113, // Illegal media keyword

        // -- Bracket/delimiter matching --
        ErrorCode::UnclosedBracket => 22,     // Unmatched [
        ErrorCode::UnclosedParenthesis => 28, // Unmatched (
        ErrorCode::UnbalancedOverlap => 24,   // Unmatched <
        ErrorCode::MissingOverlapEnd => 25,   // Unmatched >
        ErrorCode::UnmatchedContentAnnotationEnd => 23,
        ErrorCode::UnmatchedUnderlineBegin => 22,
        ErrorCode::UnmatchedUnderlineEnd => 23,
        ErrorCode::UnmatchedLongFeatureBegin => 22,
        ErrorCode::UnmatchedLongFeatureEnd => 23,
        ErrorCode::UnmatchedNonvocalBegin => 22,
        ErrorCode::UnmatchedNonvocalEnd => 23,

        // -- Word errors --
        ErrorCode::IllegalDigits => 47, // Numbers inside words
        ErrorCode::IllegalCharactersInWord => 48, // Illegal characters
        ErrorCode::InvalidWordFormat => 48, // Illegal word format
        ErrorCode::MalformedWordContent => 48, // Malformed word
        ErrorCode::MissingFormType => 48, // Missing special form marker
        ErrorCode::InvalidFormType => 147, // Undeclared form marker in depfile
        ErrorCode::EmptySpokenContent => 155, // "(word)" → use "0word"
        ErrorCode::IllegalUntranscribed => 135, // Illegal use of xxx/yyy/www

        // -- Tier errors --
        ErrorCode::DuplicateDependentTier => 40, // Duplicate code tiers
        ErrorCode::OrphanedDependentTier => 39,  // Code tier after header
        ErrorCode::UnknownHeader => 17,          // Tier not declared in depfile
        ErrorCode::UnsupportedDependentTier => 17, // Unknown dependent tier
        ErrorCode::UnsupportedOption => 11,      // Symbol not in depfile

        // -- Temporal errors --
        ErrorCode::InvalidMediaBullet => 89, // Wrong chars in bullet
        ErrorCode::InvalidTimestamp => 90,   // Illegal time in bullet
        ErrorCode::TimestampBackwards => 82, // BEG > END
        ErrorCode::TierBeginTimeNotMonotonic => 83, // BEG < prev BEG
        ErrorCode::SpeakerSelfOverlap => 133, // Speaker self-overlap
        ErrorCode::BulletOverlap => 84,      // Cross-speaker bullet overlap
        ErrorCode::SpeakerBulletSelfOverlap => 133, // Same-speaker bullet timing overlap
        ErrorCode::BulletGap => 85,          // Gap between tiers
        ErrorCode::MissingBullet => 110,     // Missing bullet (+c mode)

        // -- Tier alignment --
        ErrorCode::MorCountMismatchTooFew | ErrorCode::MorCountMismatchTooMany => 140,
        ErrorCode::MorTerminatorPresenceMismatch => 94,
        ErrorCode::MorTerminatorValueMismatch => 94,

        // -- Mor/Gra structural --
        ErrorCode::MorParseError => 134, // Illegal — run "mor"
        ErrorCode::InvalidMorphologyFormat => 134,

        // -- Annotation errors --
        ErrorCode::UnknownAnnotation => 48, // Illegal character/unknown annotation
        ErrorCode::SyllablePauseNotBetweenSpokenMaterial => 48, // Syllable pause misplacement
        ErrorCode::TierValidationError => 87, // Malformed structure (generic tier error)
        ErrorCode::GraInvalidHeadIndex => 87, // Malformed gra structure
        ErrorCode::TreeParsingError => 48,  // CST traversal error → illegal content

        // -- Formatting --
        ErrorCode::ConsecutiveCommas => 156, // Replace ,, with special character
        ErrorCode::UnbalancedQuotation => 117, // Character pairs
        ErrorCode::UnbalancedCADelimiter => 117,
        ErrorCode::NestedQuotation => 117, // Quotation pairing issue

        // -- Replacement/annotation errors --
        ErrorCode::ReplacementOnNonword => 141, // [: ...] preceded by only one word
        ErrorCode::ReplacementContainsOmission => 158, // [: ...] has real word, not 0...
        ErrorCode::ReplacementContainsUntranscribed => 158, // [: ...] has real word, not xxx
        ErrorCode::ContentAnnotationParseError => 48, // Scoped annotation parse error
        ErrorCode::EmptyParticipantRole => 12,  // Missing speaker name/role

        // -- Misc --
        ErrorCode::EmptyUtterance => 70,   // Expected text or "0"
        ErrorCode::EmptyWordContent => 70, // Word content is empty (subcase of 70)
        ErrorCode::InvalidControlCharacter => 86, // Re-enter using Unicode
        ErrorCode::StructuralOrderError => 87, // Malformed structure
        ErrorCode::InvalidPostcode => 108, // Postcodes before bullet
        ErrorCode::SpeakerNotDefined => 18, // Speaker not in participants
        ErrorCode::UndeclaredSpeaker => 18,

        // -- No direct mapping --
        _ => 0,
    }
}

/// Return the canonical CHECK error message for a given error number.
///
/// These messages are taken directly from CLAN's `check_mess()` function
/// in `check.cpp`, preserving the original wording.
pub fn check_error_message(num: u16) -> &'static str {
    match num {
        1 => "Expected characters are: @ or % or *.",
        2 => "Missing ':' character and argument.",
        3 => "Missing either TAB or SPACE character.",
        4 => "Found a space character instead of TAB character after Tier name.",
        5 => "Colon (:) character is illegal.",
        6 => "\"@Begin\" is missing at the beginning of the file.",
        7 => "\"@End\" is missing at the end of the file.",
        8 => "Expected characters are: @ % * TAB.",
        9 => "Tier name is longer than allowed.",
        10 => "Tier text is too long.",
        11 => "Symbol is not declared in the depfile.",
        12 => "Missing speaker name and/or role.",
        13 => "Duplicate speaker declaration.",
        14 => "Spaces before tier code.",
        15 => "Illegal role.",
        16 => "Illegal use of extended characters in speaker names.",
        17 => "Tier is not declared in depfile file.",
        18 => "Speaker is not specified in a participants list.",
        19 => "Illegal use of delimiter in a word.",
        20 => "Undeclared suffix in depfile.",
        21 => "Utterance delimiter expected.",
        22 => "Unmatched [ found on the tier.",
        23 => "Unmatched ] found on the tier.",
        24 => "Unmatched < found on the tier.",
        25 => "Unmatched > found on the tier.",
        26 => "Unmatched { found on the tier.",
        27 => "Unmatched } found on the tier.",
        28 => "Unmatched ( found on the tier.",
        29 => "Unmatched ) found on the tier.",
        30 => "Text is illegal.",
        31 => "Missing text after the colon.",
        32 => "Code is not declared in depfile.",
        33 => "Either illegal date or time or symbol is not declared in depfile.",
        34 => "Illegal date representation.",
        35 => "Illegal time representation.",
        36 => "Utterance delimiter must be at the end of the utterance.",
        37 => "Undeclared prefix.",
        38 => "Numbers should be written out in words.",
        39 => "Code tier must NOT follow header tier.",
        40 => "Duplicate code tiers per one main tier are NOT allowed.",
        41 => "Parentheses around words are illegal.",
        42 => "Use either \"&\" or \"()\", but not both.",
        43 => "The file must start with \"@Begin\" tier.",
        44 => "The file must end with \"@End\" tier.",
        45 => "There were more @Bg than @Eg tiers found.",
        46 => "This @Eg does not have matching @Bg.",
        47 => "Numbers are not allowed inside words.",
        48 => "Illegal character(s) found.",
        49 => "Upper case letters are not allowed inside a word.",
        50 => "Redundant utterance delimiter.",
        51 => "Expected [ ]; < > should be followed by [ ].",
        52 => "This item must be preceded by text.",
        53 => "Only one \"@Begin\" can be in a file.",
        54 => "Only one \"@End\" can be in a file.",
        55 => "Unmatched ( found in the word.",
        56 => "Unmatched ) found in the word.",
        57 => "Please add space between word and pause symbol.",
        58 => "Tier name is longer than 8 characters.",
        59 => "Expected second character of pair.",
        60 => "\"@ID:\" tier is missing in the file.",
        61 => "\"@Participants:\" tier is expected here.",
        62 => "Missing language information.",
        63 => "Missing Corpus name.",
        64 => "Wrong gender information (Choose: female or male).",
        65 => "This item can not be followed by the next symbol.",
        66 => "Illegal character in a word.",
        67 => "This item must be followed by text.",
        68 => "PARTICIPANTS TIER IS MISSING \"CHI Target_Child\".",
        69 => "The UTF8 header is missing.",
        70 => "Expected either text or \"0\" on this tier.",
        71 => "This item must be before pause (#).",
        72 => "This item must precede the utterance delimiter or CA delimiter.",
        73 => "This item must be preceded by text or '0'.",
        74 => "Only one tab after ':' is allowed.",
        75 => "This item must follow after utterance delimiter.",
        76 => "Only one letter is allowed with '@l'.",
        77 => "\"@Languages:\" tier is expected here.",
        78 => "This item must be used at the beginning of tier.",
        79 => "Only one occurrence of | symbol per word is allowed.",
        80 => "There must be at least one occurrence of '|'.",
        81 => "Bullet must follow utterance delimiter or be followed by end-of-line.",
        82 => "BEG mark of bullet must be smaller than END mark.",
        83 => "Current BEG time is smaller than previous tier BEG time.",
        84 => "Current BEG time is smaller than previous tier END time.",
        85 => "Gap found between current BEG time and previous tier END time.",
        86 => "Illegal character. Please re-enter it using Unicode standard.",
        87 => "Malformed structure.",
        88 => "Illegal use of compounds and special form markers.",
        89 => "Missing or extra or wrong characters found in bullet.",
        90 => "Illegal time representation inside a bullet.",
        91 => "Blank lines are not allowed.",
        92 => "This item must be followed by space or end-of-line.",
        93 => "This item must be preceded by SPACE.",
        94 => "Mismatch of speaker and %mor: utterance delimiters.",
        95 => "Illegal use of capitalized words in compounds.",
        96 => "Word color is now illegal.",
        97 => "Illegal character inside parentheses.",
        98 => "Space is not allowed in media file name inside bullets.",
        99 => "Extension is not allowed at the end of media file name.",
        100 => "Commas at the end of PARTICIPANTS tier are not allowed.",
        101 => "This item must be followed or preceded by text.",
        102 => "Italic markers are no longer legal in CHAT.",
        103 => "Illegal use of both CA and IPA on \"@Options:\" tier.",
        104 => "Please select \"CAfont\" or \"Ascender Uni Duo\" font for CA file.",
        105 => "Please select \"Charis SIL\" font for IPA file.",
        106 => "The whole code must be on one line.",
        107 => "Only single commas are allowed in tier.",
        108 => "All postcodes must precede final bullet.",
        109 => "Postcodes are not allowed on dependent tiers.",
        110 => "No bullet found on this tier.",
        111 => "Illegal pause format. Pause has to have '.'.",
        112 => "Missing @Media tier with media file name in headers section.",
        113 => "Illegal keyword, use \"audio\", \"video\" or look in depfile.cut.",
        114 => "Add media type after the media file name on @Media tier.",
        115 => "Old bullets format found. Please run \"fixbullets\".",
        116 => "Specifying Font for individual lines is illegal.",
        117 => "This character must be used in pairs.",
        118 => "Utterance delimiter must precede final bullet.",
        119 => "Missing word after code.",
        120 => "Please use three letter language code.",
        121 => "Language code not found in ISO-639 file.",
        122 => "Language on @ID tier is not defined on \"@Languages:\" header tier.",
        123 => "Illegal character found in tier text.",
        124 => "Please remove \"unlinked\" from @Media header.",
        125 => "\"@Options\" header must immediately follow \"@Participants:\" header.",
        126 => "\"@ID\" header must immediately follow \"@Participants:\" or \"@Options\" header.",
        127 => {
            "Header must follow \"@ID:\" or \"@Birth of\" or \"@Birthplace of\" or \"@L1 of\" header."
        }
        128 => "Unmatched \u{2039} found on the tier.",
        129 => "Unmatched \u{203A} found on the tier.",
        130 => "Unmatched \u{3014} found on the tier.",
        131 => "Unmatched \u{3015} found on the tier.",
        132 => "Tabs should only be used to mark the beginning of lines.",
        133 => "BEG time is smaller than same speaker's previous END time.",
        134 => "This item is illegal. Please run \"mor\" command on this data.",
        135 => "This item is illegal.",
        136 => "Unmatched \u{201C} found on the tier.",
        137 => "Unmatched \u{201D} found on the tier.",
        138 => "Special quote U2019 must be replaced by single quote (').",
        139 => "Special quote U2018 must be replaced by single quote (').",
        140 => "Tier \"%MOR:\" does not link in size to its speaker tier.",
        141 => "[: ...] has to be preceded by only one word and nothing else.",
        142 => "Speaker's role on @ID tier does not match role on @Participants: tier.",
        143 => "The @ID line needs 10 fields.",
        144 => "Either illegal SES field value or symbol is not declared in depfile.",
        145 => "This intonational marker should be outside paired markers.",
        146 => "The &= symbol must include some code after '=' character.",
        147 => "Undeclared special form marker in depfile.",
        148 => "Space character is not allowed before comma(,) character on \"@Media:\" header.",
        149 => "Illegal character located between a word and [...] code.",
        150 => "Illegal item located between a word and [...] code.",
        151 => "This word has only repetition segments.",
        152 => "Language is not defined on \"@Languages:\" header tier.",
        153 => "Age's month or day are missing initial zero.",
        154 => "Please add \"unlinked\" to @Media header.",
        155 => "Please use \"0word\" instead of \"(word)\".",
        156 => "Please replace ,, with special character.",
        157 => "Media file name has to match datafile name.",
        158 => "[: ...] has to have real word, not 0... or &... or xxx.",
        159 => "Pause markers should appear after retrace markers.",
        160 => "Space character is not allowed after '<' or before '>' character.",
        161 => "Space character is required before '[' code item.",
        _ => "Unknown error.",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_known_mappings() {
        assert_eq!(check_error_number(&ErrorCode::MissingEndHeader), 7);
        assert_eq!(check_error_number(&ErrorCode::MissingUTF8Header), 69);
        assert_eq!(check_error_number(&ErrorCode::TimestampBackwards), 82);
        assert_eq!(check_error_number(&ErrorCode::MorCountMismatchTooFew), 140);
    }

    #[test]
    fn test_unmapped_returns_zero() {
        assert_eq!(check_error_number(&ErrorCode::TestError), 0);
        assert_eq!(check_error_number(&ErrorCode::InternalError), 0);
    }

    #[test]
    fn test_all_messages_non_empty() {
        for n in 1..=161u16 {
            let msg = check_error_message(n);
            assert!(!msg.is_empty(), "Error {n} has empty message");
            assert_ne!(msg, "Unknown error.", "Error {n} has fallback message");
        }
    }
}
