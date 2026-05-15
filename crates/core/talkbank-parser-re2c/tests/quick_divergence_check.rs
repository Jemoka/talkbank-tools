//! Quick divergence checker: test a few known-divergent files and report
//! exactly what differs between TreeSitter and Re2c.
//!
//! Run: `cargo test -p talkbank-parser-re2c --test quick_divergence_check -- --nocapture`

mod fixture_utils;

use talkbank_model::{ErrorCollector, SemanticEq};
use talkbank_parser::TreeSitterParser;

#[test]
fn check_sample_divergences() {
    let base = format!("{}/data", fixture_utils::meta_repo_root().display());

    let files = [
        "aphasia-data/English/NonProtocol/Goodwin/muffin.cha",
        "aphasia-data/English/Protocol/MSU/PWA/MSU03b.cha",
        "biling-data/Bangor/Siarad/davies11.cha",
        "asd-data/English/NYU-Emerson/2028.cha",
        "aphasia-data/English/NonProtocol/Goodwin/seven.cha",
        // Additional samples from different corpus areas
        "childes-eng-na-data/Eng-NA/MacWhinney/010411a.cha",
        "dementia-data/English/Pitt/Control/027-0.cha",
        "ca-data/CallHome/eng/4074.cha",
        "ca-data/SBCSAE/SBC001.cha",
        "fluency-data/English/UCLASS-RAP/H1a.cha",
    ];

    let ts = TreeSitterParser::new().expect("grammar");
    let mut divergent_count = 0;

    for file_path in &files {
        let full_path = format!("{base}/{file_path}");
        let Ok(content) = std::fs::read_to_string(&full_path) else {
            eprintln!("{file_path}: FILE NOT FOUND");
            continue;
        };

        let errors = ErrorCollector::new();
        let ts_file = ts.parse_chat_file_streaming(&content, &errors);

        let re2c_errors = ErrorCollector::new();
        let re2c_parsed =
            talkbank_parser_re2c::parser::parse_chat_file_streaming(&content, &re2c_errors);
        let re2c_file = talkbank_model::model::ChatFile::from(&re2c_parsed);

        if ts_file.semantic_eq(&re2c_file) {
            eprintln!("{file_path}: MATCH");
        } else {
            eprintln!("{file_path}: DIVERGENT");
            let min_lines = ts_file.lines.len().min(re2c_file.lines.len());
            if ts_file.lines.len() != re2c_file.lines.len() {
                eprintln!(
                    "  line count: ts={} re2c={}",
                    ts_file.lines.len(),
                    re2c_file.lines.len()
                );
            }
            // Dump first divergent file's JSON for detailed analysis
            if divergent_count == 0 {
                let _ = std::fs::write(
                    "/tmp/ts_output.json",
                    serde_json::to_string_pretty(&ts_file).unwrap(),
                );
                let _ = std::fs::write(
                    "/tmp/re2c_output.json",
                    serde_json::to_string_pretty(&re2c_file).unwrap(),
                );
            }
            divergent_count += 1;
            for i in 0..min_lines {
                if !ts_file.lines[i].semantic_eq(&re2c_file.lines[i]) {
                    eprintln!("  first diff at line {i}");
                    break;
                }
            }
        }
    }
}
