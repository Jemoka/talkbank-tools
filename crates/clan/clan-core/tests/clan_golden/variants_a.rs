// Option-variant CLAN parity golden tests, part 1.

parity_case_tests! {
    golden_freq_mor_tier => ParityCase::command("freq", "tiers/mor-gra.cha", &["+t%mor"], &["--mor"], FilterSpec::None, OutputFormat::Text, "freq_mor_tier@clan", "freq_mor_tier@rust");
    golden_freq_speaker_chi => ParityCase::command("freq", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "freq_speaker_chi@clan", "freq_speaker_chi@rust");
    golden_freq_word_include => ParityCase::command("freq", "tiers/mor-gra.cha", &["+scookie"], &[], FilterSpec::words(&["cookie"]), OutputFormat::Text, "freq_word_include@clan", "freq_word_include@rust");
    golden_freq_eng_mor_tier => ParityCase::command("freq", "languages/eng-conversation.cha", &["+t%mor"], &["--mor"], FilterSpec::None, OutputFormat::Text, "freq_eng_mor_tier@clan", "freq_eng_mor_tier@rust");
    golden_mlu_speaker_chi => ParityCase::command("mlu", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "mlu_speaker_chi@clan", "mlu_speaker_chi@rust");
    golden_mlu_words => ParityCase::command("mlu", "tiers/mor-gra.cha", &["-t%mor"], &["--words"], FilterSpec::None, OutputFormat::Text, "mlu_words@clan", "mlu_words@rust");
    golden_kwal_multiple_keywords => ParityCase::command("kwal", "tiers/mor-gra.cha", &["+swant", "+scookie"], &["--keyword", "want", "--keyword", "cookie"], FilterSpec::None, OutputFormat::Text, "kwal_multiple@clan", "kwal_multiple@rust");
    golden_kwal_eng => ParityCase::command("kwal", "languages/eng-conversation.cha", &["+sgoing"], &["--keyword", "going"], FilterSpec::None, OutputFormat::Text, "kwal_eng@clan", "kwal_eng@rust");
    golden_combo_and_search => ParityCase::command("combo", "tiers/mor-gra.cha", &["+swant+cookies"], &["--search", "want+cookies"], FilterSpec::None, OutputFormat::Text, "combo_and@clan", "combo_and@rust");
    golden_combo_eng => ParityCase::command("combo", "languages/eng-conversation.cha", &["+skept+going"], &["--search", "kept+going"], FilterSpec::None, OutputFormat::Text, "combo_eng@clan", "combo_eng@rust");
    golden_freq_range => ParityCase::command("freq", "core/basic-conversation.cha", &["+z1-1"], &[], FilterSpec::range(1, 1), OutputFormat::Text, "freq_range@clan", "freq_range@rust");
    golden_dss_speaker_spe => ParityCase::command("dss", "languages/eng-conversation.cha", &["+t*SPE"], &[], FilterSpec::speakers(&["SPE"]), OutputFormat::Text, "dss_speaker_spe@clan", "dss_speaker_spe@rust");
    golden_dss_mor_gra => ParityCase::command("dss", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "dss_mor_gra@clan", "dss_mor_gra@rust");
    golden_ipsyn_speaker_spe => ParityCase::command("ipsyn", "languages/eng-conversation.cha", &["+t*SPE"], &[], FilterSpec::speakers(&["SPE"]), OutputFormat::Text, "ipsyn_speaker_spe@clan", "ipsyn_speaker_spe@rust");
    golden_ipsyn_mor_gra => ParityCase::command("ipsyn", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "ipsyn_mor_gra@clan", "ipsyn_mor_gra@rust");
    golden_eval_speaker_spe => ParityCase::command("eval", "languages/eng-conversation.cha", &["+t*SPE"], &[], FilterSpec::speakers(&["SPE"]), OutputFormat::Text, "eval_speaker_spe@clan", "eval_speaker_spe@rust");
    golden_eval_mor_gra => ParityCase::command("eval", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "eval_mor_gra@clan", "eval_mor_gra@rust");
}
