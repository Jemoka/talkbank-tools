//! String newtype wrappers for CHAT header payload fields.
//!
//! Each wrapper maps one header payload slot to a dedicated model type.
//! Per-type docs include a direct CHAT manual anchor for that header or field.
//!
//! Using distinct newtypes keeps header assembly strongly typed and prevents
//! accidental field swaps (for example, passing a `@PID` value where a
//! `@Situation` description is expected).
//! These wrappers intentionally perform no semantic normalization so parser
//! roundtrips can preserve corpus-authored header text exactly.

use crate::string_newtype;

string_newtype!(
    /// Persistent identifier recorded in `@PID`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#PID_Header>
    pub struct PidValue;
);

string_newtype!(
    /// Description attached to `@Situation`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Situation_Header>
    pub struct SituationDescription;
);

string_newtype!(
    /// Location text recorded in `@Tape Location`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Tape_Location_Header>
    pub struct TapeLocationDescription;
);

string_newtype!(
    /// Location description from `@Location`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Location_Header>
    pub struct LocationDescription;
);

string_newtype!(
    /// Room layout description from `@Room Layout`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Room_Layout_Header>
    pub struct RoomLayoutDescription;
);

string_newtype!(
    /// Label for gem headers (`@Bg`, `@Eg`, `@G`).
    ///
    /// References:
    /// - <https://talkbank.org/0info/manuals/CHAT.html#Bg_Header>
    /// - <https://talkbank.org/0info/manuals/CHAT.html#Eg_Header>
    /// - <https://talkbank.org/0info/manuals/CHAT.html#G_Header>
    pub struct GemLabel;
);

string_newtype!(
    /// Description of the birthplace in `@Birthplace of`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Birthplace_Header>
    pub struct BirthplaceDescription;
);

string_newtype!(
    /// Human-readable language name recorded in `@L1 of`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#L1_Header>
    pub struct LanguageName;
);

string_newtype!(
    /// Transcriber name captured in `@Transcriber`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Transcriber_Header>
    pub struct TranscriberName;
);

string_newtype!(
    /// Warning text from `@Warning`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Warning_Header>
    pub struct WarningText;
);

string_newtype!(
    /// Corpus name stored in `@ID` (field 2).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Corpus_Field>
    pub struct CorpusName;
);

string_newtype!(
    /// Group identifier captured in `@ID` (field 6).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Group_Field>
    pub struct GroupName;
);

// SesDescription was replaced by the typed SesValue enum in ses.rs.

string_newtype!(
    /// Education description from `@ID` (field 9).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Education_Field>
    pub struct EducationDescription;
);

string_newtype!(
    /// Custom field text from `@ID` (field 10).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Custom_Field>
    pub struct CustomIdField;
);

string_newtype!(
    /// Activities list recorded in `@Activities`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Activities_Header>
    pub struct ActivitiesDescription;
);

string_newtype!(
    /// Background context stored in `@Bck`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Bck_Header>
    pub struct BackgroundDescription;
);

string_newtype!(
    /// Page identifier from `@Page`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Page_Header>
    pub struct PageNumber;
);

string_newtype!(
    /// Video references listed in `@Videos`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Videos_Header>
    pub struct VideoSpec;
);

string_newtype!(
    /// Inline thumbnail marker text from `@T`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Thumbnail_Header>
    pub struct TDescription;
);

string_newtype!(
    /// Font specification declared in `@Font`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Font_Header>
    pub struct FontSpec;
);

string_newtype!(
    /// Window geometry captured in `@Window`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Window_Header>
    pub struct WindowGeometry;
);

string_newtype!(
    /// Color word palette listed in `@Color words`.
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#ColorWords_Header>
    pub struct ColorWordList;
);

string_newtype!(
    /// Media filename recorded in `@Media` (without extension).
    ///
    /// Reference: <https://talkbank.org/0info/manuals/CHAT.html#Media_Header>
    pub struct MediaFilename;
);
