// Baseline CLAN parity golden tests.

parity_case_tests! {
    golden_freq_mor_gra => ParityCase::command("freq", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "freq_mor_gra@clan", "freq_mor_gra@rust");
    golden_freq_ca => ParityCase::command("freq", "ca/overlaps.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "freq_ca@clan", "freq_ca@rust");
    golden_freq_eng => ParityCase::command("freq", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "freq_eng@clan", "freq_eng@rust");
    golden_mlu_mor_gra => ParityCase::command("mlu", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "mlu_mor_gra@clan", "mlu_mor_gra@rust");
    golden_mlt_mor_gra => ParityCase::command("mlt", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "mlt_mor_gra@clan", "mlt_mor_gra@rust");
    golden_wdlen_mor_gra => ParityCase::command("wdlen", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "wdlen_mor_gra@clan", "wdlen_mor_gra@rust");
    golden_freqpos_mor_gra => ParityCase::command("freqpos", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "freqpos_mor_gra@clan", "freqpos_mor_gra@rust");
    golden_cooccur_mor_gra => ParityCase::command("cooccur", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "cooccur_mor_gra@clan", "cooccur_mor_gra@rust");
    golden_dist_mor_gra => ParityCase::command("dist", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "dist_mor_gra@clan", "dist_mor_gra@rust").with_clan_compat("DIST render_clan() should match legacy CLAN output exactly");
    golden_maxwd_mor_gra => ParityCase::command("maxwd", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "maxwd_mor_gra@clan", "maxwd_mor_gra@rust").with_clan_compat("MAXWD render_clan() should match legacy CLAN output exactly");
    golden_kwal_mor_gra => ParityCase::command("kwal", "tiers/mor-gra.cha", &["+scookie"], &["--keyword", "cookie"], FilterSpec::None, OutputFormat::Text, "kwal_mor_gra@clan", "kwal_mor_gra@rust");
    golden_chip_mor_gra => ParityCase::command("chip", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "chip_mor_gra@clan", "chip_mor_gra@rust");
    golden_gemlist_episodes => ParityCase::command("gemlist", "core/headers-episodes.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "gemlist_episodes@clan", "gemlist_episodes@rust");
    golden_phonfreq_pho => ParityCase::command("phonfreq", "tiers/pho.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "phonfreq_pho@clan", "phonfreq_pho@rust");
    golden_vocd_mor_gra => ParityCase::command("vocd", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "vocd_mor_gra@clan", "vocd_mor_gra@rust");
    golden_modrep_pho => ParityCase::command("modrep", "tiers/pho.cha", &["+b%mod", "+c%pho"], &[], FilterSpec::None, OutputFormat::Text, "modrep_pho@clan", "modrep_pho@rust");
    golden_combo_mor_gra => ParityCase::command("combo", "tiers/mor-gra.cha", &["+swant"], &["--search", "want"], FilterSpec::None, OutputFormat::Text, "combo_mor_gra@clan", "combo_mor_gra@rust");
    golden_codes_coding => ParityCase::command("codes", "tiers/coding.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "codes_coding@clan", "codes_coding@rust");
    golden_chains_coding => ParityCase::command("chains", "tiers/coding.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "chains_coding@clan", "chains_coding@rust");
    golden_sugar_mor_gra => ParityCase::command("sugar", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "sugar_mor_gra@clan", "sugar_mor_gra@rust");
    golden_sugar_eng => ParityCase::command("sugar", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "sugar_eng@clan", "sugar_eng@rust");
    golden_timedur_bullets => ParityCase::command("timedur", "content/media-bullets.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "timedur_bullets@clan", "timedur_bullets@rust");
    golden_trnfix_pho => ParityCase::command("trnfix", "tiers/pho.cha", &["+b%pho", "+c%mod"], &["--tier1", "pho", "--tier2", "mod"], FilterSpec::None, OutputFormat::Text, "trnfix_pho@clan", "trnfix_pho@rust");
    golden_uniq_basic => ParityCase::command("uniq", "core/basic-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "uniq_basic@clan", "uniq_basic@rust");
    golden_dss_eng => ParityCase::command("dss", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "dss_eng@clan", "dss_eng@rust");
    golden_eval_eng => ParityCase::command("eval", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "eval_eng@clan", "eval_eng@rust");
    golden_flucalc_retrace => ParityCase::command("flucalc", "annotation/retrace.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "flucalc_retrace@clan", "flucalc_retrace@rust");
    golden_ipsyn_eng => ParityCase::command("ipsyn", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "ipsyn_eng@clan", "ipsyn_eng@rust");
    golden_kideval_eng => ParityCase::command("kideval", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "kideval_eng@clan", "kideval_eng@rust");
    golden_keymap_coding => ParityCase::command("keymap", "tiers/coding.cha", &["+s$NOM", "+d%cod"], &["--keyword", "$NOM", "--tier", "cod"], FilterSpec::None, OutputFormat::Text, "keymap_coding@clan", "keymap_coding@rust");
}
