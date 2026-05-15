//! Test module for debug roundtrip in `talkbank-chat`.
//!
//! These tests document expected behavior and regressions.

use talkbank_model::WriteChat;
use talkbank_parser::TreeSitterParser;

/// Tests pho groupings roundtrip.
#[test]
fn test_pho_groupings_roundtrip() {
    let path = match std::env::var("DEBUG_ROUNDTRIP_FILE") {
        Ok(p) => p,
        Err(_) => {
            eprintln!("DEBUG_ROUNDTRIP_FILE not set, skipping debug roundtrip test");
            return;
        }
    };
    let input = match std::fs::read_to_string(&path) {
        Ok(input) => input,
        Err(err) => {
            panic!("Failed to read test file {path}: {err}");
        }
    };

    let parser = TreeSitterParser::new().expect("grammar loads");
    match parser.parse_chat_file(&input) {
        Ok(file) => {
            println!("\n=== Parsed successfully! {} lines ===", file.lines.len());

            // Find the CHI utterance
            for line in &file.lines {
                if let talkbank_model::Line::Utterance(utt) = line
                    && utt.main.speaker.as_str() == "CHI"
                {
                    println!("\n=== CHI Main Tier ===");
                    println!("Content count: {}", utt.main.content.content.len());
                    for (i, content) in utt.main.content.content.iter().enumerate() {
                        println!("  [{}] {:?}", i, content);
                    }

                    // Serialize
                    let mut output = String::new();
                    let _ = utt.main.write_chat(&mut output);
                    println!("\n=== Serialized ===\n{}", output);
                }
            }

            // Full roundtrip
            let mut full_output = String::new();
            let _ = file.write_chat(&mut full_output);

            let input_normalized = input.replace("\r\n", "\n").replace("\r", "\n");
            let output_normalized = full_output.replace("\r\n", "\n").replace("\r", "\n");

            if input_normalized != output_normalized {
                println!("\n=== MISMATCH ===");
                let input_lines: Vec<&str> = input_normalized.lines().collect();
                let output_lines: Vec<&str> = output_normalized.lines().collect();

                for (i, (inp, out)) in input_lines.iter().zip(output_lines.iter()).enumerate() {
                    if inp != out {
                        println!("Line {}: ", i + 1);
                        println!("  IN:  {:?}", inp);
                        println!("  OUT: {:?}", out);
                    }
                }
            }
        }
        Err(errors) => {
            println!("\n=== PARSE ERRORS: {} ===", errors.errors.len());
            for err in &errors.errors {
                println!("[{}] {}", err.severity, err.message);
            }
            panic!("Parse failed");
        }
    }
}
