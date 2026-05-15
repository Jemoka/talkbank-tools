//! Compare TreeSitterParser and TreeSitterParser output for a single file

use std::fs;

/// Entry point for this binary target.
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let file = match std::env::args().nth(1) {
        Some(arg) => arg,
        None => {
            eprintln!("Usage: compare_file <path.cha>");
            std::process::exit(2);
        }
    };
    let content = fs::read_to_string(&file)?;

    let ts = talkbank_parser::TreeSitterParser::new()?;
    let direct = talkbank_parser::TreeSitterParser::new()?;

    let ts_result = ts.parse_chat_file(&content);
    let direct_result = direct.parse_chat_file(&content);

    match (&ts_result, &direct_result) {
        (Ok(ts_file), Ok(direct_file)) => {
            let ts_json = serde_json::to_string_pretty(ts_file)?;
            let direct_json = serde_json::to_string_pretty(direct_file)?;

            if ts_json == direct_json {
                println!("✓ Files are semantically equivalent");
                std::process::exit(0);
            } else {
                fs::write("/tmp/ts.json", &ts_json)?;
                fs::write("/tmp/direct.json", &direct_json)?;
                println!("✗ Files differ - wrote /tmp/ts.json and /tmp/direct.json");

                // Show first 15 differences
                let ts_lines: Vec<_> = ts_json.lines().collect();
                let direct_lines: Vec<_> = direct_json.lines().collect();
                let mut diffs = 0;
                for (i, (ts_line, direct_line)) in
                    ts_lines.iter().zip(direct_lines.iter()).enumerate()
                {
                    if ts_line != direct_line && diffs < 15 {
                        println!(
                            "
Line {}:",
                            i + 1
                        );
                        println!("  TS:     {}", ts_line);
                        println!("  Direct: {}", direct_line);
                        diffs += 1;
                    }
                }

                if ts_lines.len() != direct_lines.len() {
                    println!("\nLength difference:");
                    println!("  TS:     {} lines", ts_lines.len());
                    println!("  Direct: {} lines", direct_lines.len());
                }

                std::process::exit(1);
            }
        }
        (Err(e), Ok(_)) => {
            println!("✗ TreeSitter failed: {:?}", e);
            std::process::exit(1);
        }
        (Ok(_), Err(e)) => {
            println!("✗ Direct failed: {:?}", e);
            std::process::exit(1);
        }
        (Err(ts_err), Err(direct_err)) => {
            println!("✗ Both failed:");
            println!("  TS: {:?}", ts_err);
            println!("  Direct: {:?}", direct_err);
            std::process::exit(1);
        }
    }
}
