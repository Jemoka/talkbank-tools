//! WriteChat implementation for Header.
//!
//! Serializes Header enum variants to CHAT format strings.
//!
//! References:
//! - <https://talkbank.org/0info/manuals/CHAT.html#File_Headers>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Languages_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#Participants_Header>
//! - <https://talkbank.org/0info/manuals/CHAT.html#ID_Header>

use super::{
    WriteChat,
    header_enum::{ChatOptionFlag, Header},
};

impl WriteChat for Header {
    /// Serializes each header variant to its canonical `@Header:\t...` CHAT line.
    fn write_chat<W: std::fmt::Write>(&self, w: &mut W) -> std::fmt::Result {
        match self {
            // Required structure headers
            Header::Utf8 => write!(w, "@UTF8"),
            Header::Begin => write!(w, "@Begin"),
            Header::End => write!(w, "@End"),

            // Participant headers
            Header::Languages { codes } => {
                let joined = codes
                    .iter()
                    .map(|code| code.as_str())
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(w, "@Languages:\t{}", joined)
            }

            Header::Participants { entries } => {
                write!(w, "@Participants:\t")?;
                for (i, entry) in entries.iter().enumerate() {
                    if i > 0 {
                        write!(w, ", ")?;
                    }
                    write!(w, "{}", entry.speaker_code)?;
                    if let Some(name) = &entry.name {
                        write!(w, " {}", name)?;
                    }
                    write!(w, " {}", entry.role)?;
                }
                Ok(())
            }

            Header::ID(id_header) => id_header.write_chat(w),

            // Metadata headers
            Header::Date { date } => write!(w, "@Date:\t{}", date),

            Header::Comment { content } => {
                w.write_str("@Comment:\t")?;
                content.write_chat(w)
            }

            Header::Pid { pid } => write!(w, "@PID:\t{}", pid),

            Header::Media(media_header) => media_header.write_chat(w),

            Header::Situation { text } => write!(w, "@Situation:\t{}", text),

            Header::Types(types_header) => types_header.write_chat(w),

            // Gem headers
            Header::BeginGem { label } => {
                write!(w, "@Bg")?;
                if let Some(lbl) = label {
                    write!(w, ":\t{}", lbl)?;
                }
                Ok(())
            }

            Header::EndGem { label } => {
                write!(w, "@Eg")?;
                if let Some(lbl) = label {
                    write!(w, ":\t{}", lbl)?;
                }
                Ok(())
            }

            Header::LazyGem { label } => {
                write!(w, "@G")?;
                if let Some(lbl) = label {
                    write!(w, ":\t{}", lbl)?;
                }
                Ok(())
            }

            // CLAN display headers
            Header::Font { font } => write!(w, "@Font:\t{}", font),
            Header::Window { geometry } => write!(w, "@Window:\t{}", geometry),
            Header::ColorWords { colors } => write!(w, "@Color words:\t{}", colors),

            // Recording/session headers
            Header::Number { number } => {
                write!(w, "@Number:\t{}", number.as_str())
            }

            Header::RecordingQuality { quality } => {
                write!(w, "@Recording Quality:\t{}", quality.as_str())
            }

            Header::Transcription { transcription } => {
                write!(w, "@Transcription:\t{}", transcription.as_str())
            }

            Header::NewEpisode => write!(w, "@New Episode"),
            Header::TapeLocation { location } => write!(w, "@Tape Location:\t{}", location),
            Header::TimeDuration { duration } => write!(w, "@Time Duration:\t{}", duration),
            Header::TimeStart { start } => write!(w, "@Time Start:\t{}", start),
            Header::Location { location } => write!(w, "@Location:\t{}", location),
            Header::RoomLayout { layout } => write!(w, "@Room Layout:\t{}", layout),

            // Participant-specific headers
            Header::Birth { participant, date } => {
                write!(w, "@Birth of {}:\t{}", participant, date)
            }
            Header::Birthplace { participant, place } => {
                write!(w, "@Birthplace of {}:\t{}", participant, place)
            }
            Header::L1Of {
                participant,
                language,
            } => {
                write!(w, "@L1 of {}:\t{}", participant, language)
            }

            // Other headers
            Header::Blank => write!(w, "@Blank"),
            Header::Transcriber { transcriber } => write!(w, "@Transcriber:\t{}", transcriber),
            Header::Warning { text } => write!(w, "@Warning:\t{}", text),
            Header::Unknown { text, .. } => {
                // Unknown headers store the raw text as-is (including @ prefix)
                write!(w, "{}", text)
            }
            Header::Activities { activities } => write!(w, "@Activities:\t{}", activities),
            Header::Bck { bck } => write!(w, "@Bck:\t{}", bck),
            Header::Options { options } => {
                let joined = options
                    .iter()
                    .map(ChatOptionFlag::as_str)
                    .collect::<Vec<_>>()
                    .join(", ");
                write!(w, "@Options:\t{}", joined)
            }
            Header::Page { page } => write!(w, "@Page:\t{}", page),
            Header::Videos { videos } => write!(w, "@Videos:\t{}", videos),
            Header::T { text } => write!(w, "@T:\t{}", text),
        }
    }
}
