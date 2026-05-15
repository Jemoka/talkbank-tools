//! `%pho` hover formatting — phonological transcription items.
//!
//! Renders individual phonological items (words, pauses, group boundaries)
//! as compact strings for hover display.
/// Render one `%pho` item for hover display.
pub fn format_pho_item(item: &talkbank_model::model::PhoItem) -> String {
    use talkbank_model::model::PhoItem;
    match item {
        PhoItem::Word(word) => word.as_str().to_string(),
        PhoItem::Group(words) => {
            let mut text = String::from("‹");
            for (i, word) in words.iter().enumerate() {
                if i > 0 {
                    text.push(' ');
                }
                text.push_str(word.as_str());
            }
            text.push('›');
            text
        }
    }
}
