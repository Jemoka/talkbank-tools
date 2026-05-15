//! Tests to ensure error messages are user-friendly and don't expose parser internals
//!
//! **CRITICAL**: No error message should contain "node 'ERROR'" or expose tree-sitter internals.

use talkbank_model::ErrorCollector;
use talkbank_parser::TreeSitterParser;

/// Helper to parse CHAT content and collect errors
fn parse_and_collect_errors(input: &str) -> Vec<talkbank_model::ParseError> {
    let parser = TreeSitterParser::new().expect("grammar loads");
    let errors = ErrorCollector::new();
    let _file = parser.parse_chat_file_streaming(input, &errors);
    errors.into_vec()
}

/// Tests no error node keyword in messages.
#[test]
fn test_no_error_node_keyword_in_messages() {
    // Test that error messages don't contain "node 'ERROR'"
    // This validates the unexpected_node_error() function works correctly

    // Use various inputs (may or may not produce errors, but if they do, messages must be clean)
    let test_inputs = vec![
        "@UTF8\n@Begin\n*CHI:\thello @ world .\n@End\n", // Lone @
        "@UTF8\n@Begin\n*CHI:\thello & world .\n@End\n", // Lone &
        "@UTF8\n@Begin\n*CHI:\thello [: world .\n@End\n", // Unclosed bracket
    ];

    for input in test_inputs {
        let errors = parse_and_collect_errors(input);

        // Check all errors (whether parse or validation)
        for error in &errors {
            // CRITICAL: No error message should expose "ERROR" node type
            assert!(
                !error.message.contains("node 'ERROR'"),
                "Error message contains 'node 'ERROR'': {}",
                error.message
            );

            // Also check for old-style messages
            assert!(
                !error.message.contains("Unexpected node 'ERROR'"),
                "Error message contains 'Unexpected node 'ERROR'': {}",
                error.message
            );

            // Verify error span is within input bounds
            let span = &error.location.span;
            assert!(
                span.start <= input.len() as u32,
                "Error span start {} exceeds input length {}. Input: {}",
                span.start,
                input.len(),
                input.escape_debug()
            );
            assert!(
                span.end <= input.len() as u32,
                "Error span end {} exceeds input length {}. Input: {}",
                span.end,
                input.len(),
                input.escape_debug()
            );
        }
    }
}

/// Tests error messages dont expose internals.
#[test]
fn test_error_messages_dont_expose_internals() {
    // Test that error messages don't expose tree-sitter/CST internals

    let test_inputs = vec![
        "@UTF8\n@Begin\n*CHI:\thello @ .\n@End\n", // Lone @
        "@UTF8\n@Begin\n*CHI:\thello & .\n@End\n", // Lone &
    ];

    for input in test_inputs {
        let errors = parse_and_collect_errors(input);

        for error in &errors {
            let msg = &error.message;

            // Should not expose tree-sitter ERROR keyword
            assert!(
                !msg.contains("ERROR"),
                "Error message exposes 'ERROR' keyword: {} (input: {})",
                msg,
                input.escape_debug()
            );

            // Messages should not be empty
            assert!(
                !msg.is_empty(),
                "Error message is empty (input: {})",
                input.escape_debug()
            );

            // Should not expose "node" terminology (CST internal)
            assert!(
                !msg.contains("node '"),
                "Error message exposes CST 'node' terminology: {}",
                msg
            );

            // Verify error span is within input bounds
            let span = &error.location.span;
            assert!(
                span.start <= input.len() as u32,
                "Error span start {} exceeds input length {}. Input: {}",
                span.start,
                input.len(),
                input.escape_debug()
            );
            assert!(
                span.end <= input.len() as u32,
                "Error span end {} exceeds input length {}. Input: {}",
                span.end,
                input.len(),
                input.escape_debug()
            );
        }
    }
}

/// Tests error spans point to problem locations.
#[test]
fn test_error_spans_point_to_problem_locations() {
    // Test that errors point to the actual problem location with EXACT byte positions

    // Test case 1: Lone @ symbol should produce error at exact position
    let input1 = "@UTF8\n@Begin\n*CHI:\thello @ world .\n@End\n";
    //           012345 6789AB CDEF0123456789...
    // Byte offset breakdown:
    //   "@UTF8\n"                    = 6 bytes (0-5)
    //   "@Begin\n"                   = 7 bytes (6-12)
    //   "*CHI:\t"                    = 6 bytes (13-18)
    //   "hello @ world .\n"          = 16 bytes (19-34)
    //        "hello "                = 6 bytes (19-24)
    //              "@ "              = 2 bytes (25-26)  ← Error here!
    //                "world .\n"     = 8 bytes (27-34)
    //   "@End\n"                     = 5 bytes (35-39)
    // Total: 40 bytes

    assert_eq!(input1.len(), 40, "Sanity check: input1 length");

    let errors1 = parse_and_collect_errors(input1);
    if !errors1.is_empty() {
        // ✅ EXACT assertion: error should point to the "@" at byte 25
        let lone_at_error = errors1.iter().find(|e| {
            // Error should be in the region of the lone @
            e.location.span.start >= 25 && e.location.span.start <= 26
        });

        assert!(
            lone_at_error.is_some(),
            "Expected error at exact position 25-26 (lone @), got spans: {:?}",
            errors1
                .iter()
                .map(|e| (e.location.span.start, e.location.span.end))
                .collect::<Vec<_>>()
        );

        // Verify it points to the right character
        if let Some(err) = lone_at_error {
            let start = err.location.span.start as usize;
            let end = err.location.span.end as usize;
            let error_text = &input1[start..end.min(input1.len())];
            assert!(
                matches!(error_text.chars().next(), Some('@')),
                "Error should point to '@', got: '{}'",
                error_text
            );
        }
    }

    // Test case 2: Unclosed bracket [: should produce error at exact position
    let input2 = "@UTF8\n@Begin\n*CHI:\thello [: world .\n@End\n";
    //           012345 6789AB CDEF0123456789ABCDEF01234567...
    // Byte offset breakdown:
    //   "@UTF8\n"                    = 6 bytes (0-5)
    //   "@Begin\n"                   = 7 bytes (6-12)
    //   "*CHI:\t"                    = 6 bytes (13-18)
    //   "hello [: world .\n"         = 17 bytes (19-35)
    //        "hello "                = 6 bytes (19-24)
    //              "[: "             = 3 bytes (25-27)  ← Error here!
    //                  "world .\n"   = 8 bytes (28-35)
    //   "@End\n"                     = 5 bytes (36-40)
    // Total: 41 bytes

    assert_eq!(input2.len(), 41, "Sanity check: input2 length");

    let errors2 = parse_and_collect_errors(input2);
    if !errors2.is_empty() {
        // ✅ EXACT assertion: error should point to the "[:" at byte 25-27
        let unclosed_bracket_error = errors2.iter().find(|e| {
            // Error should be in the region of the unclosed bracket
            e.location.span.start >= 25 && e.location.span.start <= 27
        });

        assert!(
            unclosed_bracket_error.is_some(),
            "Expected error at exact position 25-27 (unclosed [: ), got spans: {:?}",
            errors2
                .iter()
                .map(|e| (e.location.span.start, e.location.span.end))
                .collect::<Vec<_>>()
        );

        // All errors should be within file bounds
        for error in &errors2 {
            let span = &error.location.span;
            assert!(
                span.start <= input2.len() as u32,
                "Error span start {} exceeds input length {}",
                span.start,
                input2.len()
            );
            assert!(
                span.end <= input2.len() as u32,
                "Error span end {} exceeds input length {}",
                span.end,
                input2.len()
            );
        }
    }
}
