//! Overlap comma functionality for this subsystem.
//!

use talkbank_parser::TreeSitterParser;

/// Entry point for this binary target.
fn main() {
    let input =
        "@UTF8\n@Begin\n*CHI:\t<a ⌈ top begin overlap , top end overlap ⌉ here> [= foo] .\n@End";

    let parser = TreeSitterParser::new().expect("grammar loads");
    let chat_file = match parser.parse_chat_file(input) {
        Ok(file) => file,
        Err(errors) => {
            println!("Parse errors: {}", errors.errors.len());
            for error in &errors.errors {
                println!("  - {}", error);
            }
            return;
        }
    };

    if let Some(utt) = chat_file.utterances().next() {
        println!(
            "\nMain tier content ({} items):",
            utt.main.content.content.len()
        );
        for (i, item) in utt.main.content.content.iter().enumerate() {
            println!("  [{}]: {:?}", i, item);
        }

        let serialized = utt.main.to_string();
        let expected_main_tier =
            "*CHI:\t<a ⌈ top begin overlap , top end overlap ⌉ here> [= foo] .";

        println!(
            "
Serialized: {}",
            serialized
        );
        println!("Expected:   {}", expected_main_tier);

        if serialized == expected_main_tier {
            println!("✓ Match!");
        } else {
            println!("✗ Mismatch!");
            println!(
                "
Differences:"
            );
            println!("  Expected: '{}'", expected_main_tier);
            println!("  Got:      '{}'", serialized);
        }
    }
}
