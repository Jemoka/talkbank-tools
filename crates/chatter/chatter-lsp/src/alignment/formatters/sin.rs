//! `%sin` gesture/sign tier formatting for hover output.
//!
//! Renders `SinItem` tokens and pauses as compact strings. Tokens follow the
//! CHAT `g:lexeme:dpoint` format; the formatter also extracts structured
//! detail fields (type, lexeme, discriminator point) for richer hover metadata.
/// Render one `%sin` item for compact hover display.
pub fn format_sin_item(item: &talkbank_model::model::SinItem) -> String {
    use talkbank_model::model::SinItem;
    match item {
        SinItem::Token(text) => text.as_ref().to_string(),
        SinItem::SinGroup(gestures) => {
            let mut text = String::from("〔");
            for (i, gesture) in gestures.iter().enumerate() {
                if i > 0 {
                    text.push(' ');
                }
                text.push_str(gesture.as_ref());
            }
            text.push('〕');
            text
        }
    }
}

/// Render one `%sin` item with structured detail rows.
///
/// Returns `(display_text, details_list)`.
pub fn format_sin_item_details(
    item: &talkbank_model::model::SinItem,
) -> (String, Vec<(String, String)>) {
    use talkbank_model::model::SinItem;

    match item {
        SinItem::Token(text) => {
            if text.as_ref() == "0" {
                (
                    "No gesture".to_string(),
                    vec![(
                        "Gesture".to_string(),
                        "None (spoken without gesture)".to_string(),
                    )],
                )
            } else {
                // Parse gesture code: g:referent:type
                let parts: Vec<&str> = text.split(':').collect();
                let mut details = vec![("Gesture Code".to_string(), text.as_ref().to_string())];

                if parts.len() >= 3 && parts[0] == "g" {
                    details.push(("Referent".to_string(), parts[1].to_string()));
                    details.push(("Gesture Type".to_string(), parts[2].to_string()));

                    // Add description for common gesture types
                    let description = match parts[2] {
                        "dpoint" => "Deictic pointing",
                        "hold" => "Holding gesture",
                        "give" => "Giving gesture",
                        "show" => "Showing gesture",
                        "point" => "General pointing",
                        "reach" => "Reaching gesture",
                        "take" => "Taking gesture",
                        "touch" => "Touching gesture",
                        "push" => "Pushing gesture",
                        "pull" => "Pulling gesture",
                        _ => "Custom gesture",
                    };
                    details.push(("Description".to_string(), description.to_string()));
                }

                (text.as_ref().to_string(), details)
            }
        }
        SinItem::SinGroup(gestures) => {
            let joined = gestures
                .iter()
                .map(|gesture| gesture.as_ref())
                .collect::<Vec<_>>()
                .join(" ");
            let display = format!("〔{}〕", joined);
            let mut details = vec![(
                "Multiple Gestures".to_string(),
                format!("{} gestures", gestures.len()),
            )];

            // List each gesture in the group
            for (i, gesture) in gestures.iter().enumerate() {
                details.push((format!("Gesture {}", i + 1), gesture.as_ref().to_string()));
            }

            (display, details)
        }
    }
}
