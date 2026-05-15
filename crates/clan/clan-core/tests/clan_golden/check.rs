// CHECK command golden tests.

parity_case_tests! {
    golden_check_valid => ParityCase::check("core/basic-conversation.cha", "check_valid@clan", "check_valid@rust");
    golden_check_mor_gra => ParityCase::check("tiers/mor-gra.cha", "check_mor_gra@clan", "check_mor_gra@rust");
}

#[test]
fn golden_check_target_child() {
    // Test +g2 (check that CHI has Target_Child role)
    use clan_core::commands::check::{CheckConfig, run_check};
    use clan_core::framework::CommandOutput;

    let file = corpus_file("core/basic-conversation.cha");
    let content = std::fs::read_to_string(&file).expect("Failed to read file");

    // basic-conversation.cha has CHI as "Child" role, not "Target_Child"
    let config = CheckConfig {
        check_target_child: true,
        ..CheckConfig::default()
    };
    // Use relative path to avoid machine-specific absolute paths in snapshots
    let corpus = corpus_dir();
    let base = corpus.parent().unwrap_or(&corpus);
    let relative = file.strip_prefix(base).unwrap_or(&file);
    let result = run_check(relative, &content, &config);
    let rust_output = if result.errors.is_empty() && !result.has_errors {
        "ALL FILES CHECKED OUT OK!".to_string()
    } else {
        result.render_text()
    };

    insta::assert_snapshot!("check_target_child@rust", rust_output);
}
