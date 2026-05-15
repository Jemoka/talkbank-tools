/// Rust-only golden tests for commands without CLAN parity coverage.

#[test]
fn golden_mortable_mor_gra() {
    use clan_core::commands::mortable::{MortableCommand, MortableConfig};
    use clan_core::framework::{AnalysisRunner, CommandOutput, FilterConfig};

    let file = corpus_file("tiers/mor-gra.cha");

    // Create a minimal mortable script that categorizes common POS tags
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let script_path = temp_dir.path().join("eng.cut");
    std::fs::write(
        &script_path,
        r#"OR
"Nouns" +noun +propn
"Verbs" +verb +aux
"Pronouns" +pron
"Other" -noun -propn -verb -aux -pron
"#,
    )
    .expect("Failed to write script file");

    let config = MortableConfig {
        script_path: script_path.clone(),
    };
    let cmd = MortableCommand::new(config).expect("MORTABLE init failed");

    let files = vec![file];
    let runner = AnalysisRunner::with_filter(FilterConfig::default());
    let rust_output = match runner.run(&cmd, &files) {
        Ok(r) => r.render(OutputFormat::Text),
        Err(e) => format!("Error: {e}"),
    };

    insta::assert_snapshot!("mortable_mor_gra@rust", rust_output);
}

#[test]
fn golden_rely_coding() {
    use clan_core::commands::rely::{RelyConfig, run_rely};
    use clan_core::framework::CommandOutput;

    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");

    // File 1: coder A
    let file1_path = temp_dir.path().join("coder_a.cha");
    std::fs::write(
        &file1_path,
        "@UTF8
@Begin
@Languages:	eng
\
@Participants:	CHI Child, MOT Mother
\
@ID:	eng|corpus|CHI|3;00.||||Child|||
\
@ID:	eng|corpus|MOT|||||Mother|||
\
*CHI:	hello there .
\
%cod:	$NOM $ADJ
\
*MOT:	goodbye now .
\
%cod:	$VRB $ADV
\
@End
",
    )
    .expect("Failed to write file 1");

    // File 2: coder B (partial agreement with coder A)
    let file2_path = temp_dir.path().join("coder_b.cha");
    std::fs::write(
        &file2_path,
        "@UTF8
@Begin
@Languages:	eng
\
@Participants:	CHI Child, MOT Mother
\
@ID:	eng|corpus|CHI|3;00.||||Child|||
\
@ID:	eng|corpus|MOT|||||Mother|||
\
*CHI:	hello there .
\
%cod:	$NOM $VRB
\
*MOT:	goodbye now .
\
%cod:	$VRB $ADV
\
@End
",
    )
    .expect("Failed to write file 2");

    let config = RelyConfig {
        tier: clan_core::framework::TierKind::Cod,
    };
    let result = run_rely(&config, &file1_path, &file2_path).expect("RELY failed");
    let rust_output = result.render_text();

    insta::assert_snapshot!("rely_coding@rust", rust_output);
}

#[test]
fn golden_script_basic() {
    use clan_core::commands::script::{ScriptCommand, ScriptConfig};
    use clan_core::framework::{AnalysisRunner, CommandOutput, FilterConfig};

    let file = corpus_file("core/basic-conversation.cha");

    // Create a template with a subset of the words in the subject file
    let temp_dir = tempfile::tempdir().expect("Failed to create temp dir");
    let template_path = temp_dir.path().join("template.cha");
    std::fs::write(
        &template_path,
        "@UTF8
@Begin
@Languages:	eng
\
@Participants:	CHI Child, MOT Mother
\
@ID:	eng|corpus|CHI|3;00.||||Child|||
\
@ID:	eng|corpus|MOT|||||Mother|||
\
*CHI:	I want some cookies .
\
*MOT:	what kind of cookies ?
\
@End
",
    )
    .expect("Failed to write template file");

    let config = ScriptConfig {
        template_path: template_path.clone(),
    };
    let cmd = ScriptCommand::new(config).expect("SCRIPT init failed");

    let files = vec![file];
    let runner = AnalysisRunner::with_filter(FilterConfig::default());
    let rust_output = match runner.run(&cmd, &files) {
        Ok(r) => r.render(OutputFormat::Text),
        Err(e) => format!("Error: {e}"),
    };

    insta::assert_snapshot!("script_basic@rust", rust_output);
}

rust_snapshot_tests! {
    golden_complexity_mor_gra => RustSnapshotCase::new("complexity", "tiers/mor-gra.cha", &[], OutputFormat::Text, "complexity_mor_gra@rust");
    golden_complexity_mor_gra_json => RustSnapshotCase::new("complexity", "tiers/mor-gra.cha", &[], OutputFormat::Json, "complexity_mor_gra_json@rust");
    golden_corelex_mor_gra => RustSnapshotCase::new("corelex", "tiers/mor-gra.cha", &[], OutputFormat::Text, "corelex_mor_gra@rust");
    golden_corelex_threshold_1 => RustSnapshotCase::new("corelex", "tiers/mor-gra.cha", &["--threshold", "1"], OutputFormat::Text, "corelex_threshold_1@rust");
    golden_wdsize_mor_gra => RustSnapshotCase::new("wdsize", "tiers/mor-gra.cha", &[], OutputFormat::Text, "wdsize_mor_gra@rust");
    golden_wdsize_main_tier => RustSnapshotCase::new("wdsize", "tiers/mor-gra.cha", &["--main-tier"], OutputFormat::Text, "wdsize_main_tier@rust");
}
