// Option-variant CLAN parity golden tests, part 2.

parity_case_tests! {
    golden_kideval_speaker_spe => ParityCase::command("kideval", "languages/eng-conversation.cha", &["+t*SPE"], &[], FilterSpec::speakers(&["SPE"]), OutputFormat::Text, "kideval_speaker_spe@clan", "kideval_speaker_spe@rust");
    golden_kideval_mor_gra => ParityCase::command("kideval", "tiers/mor-gra.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "kideval_mor_gra@clan", "kideval_mor_gra@rust");
    golden_vocd_eng => ParityCase::command("vocd", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "vocd_eng@clan", "vocd_eng@rust");
    golden_vocd_speaker_chi => ParityCase::command("vocd", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "vocd_speaker_chi@clan", "vocd_speaker_chi@rust");
    golden_mlt_eng => ParityCase::command("mlt", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "mlt_eng@clan", "mlt_eng@rust");
    golden_mlt_speaker_chi => ParityCase::command("mlt", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "mlt_speaker_chi@clan", "mlt_speaker_chi@rust");
    golden_flucalc_speaker_chi => ParityCase::command("flucalc", "annotation/retrace.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "flucalc_speaker_chi@clan", "flucalc_speaker_chi@rust");
    golden_wdlen_eng => ParityCase::command("wdlen", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "wdlen_eng@clan", "wdlen_eng@rust");
    golden_wdlen_speaker_chi => ParityCase::command("wdlen", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "wdlen_speaker_chi@clan", "wdlen_speaker_chi@rust");
    golden_dist_eng => ParityCase::command("dist", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "dist_eng@clan", "dist_eng@rust");
    golden_timedur_speaker_chi => ParityCase::command("timedur", "core/basic-conversation.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "timedur_speaker_chi@clan", "timedur_speaker_chi@rust");
    golden_chains_eng => ParityCase::command("chains", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "chains_eng@clan", "chains_eng@rust");
    golden_cooccur_eng => ParityCase::command("cooccur", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "cooccur_eng@clan", "cooccur_eng@rust");
    golden_sugar_speaker_chi => ParityCase::command("sugar", "tiers/mor-gra.cha", &["+t*CHI"], &[], FilterSpec::speakers(&["CHI"]), OutputFormat::Text, "sugar_speaker_chi@clan", "sugar_speaker_chi@rust");
    golden_mlu_eng => ParityCase::command("mlu", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "mlu_eng@clan", "mlu_eng@rust");
    golden_freqpos_eng => ParityCase::command("freqpos", "languages/eng-conversation.cha", &[], &[], FilterSpec::None, OutputFormat::Text, "freqpos_eng@clan", "freqpos_eng@rust");
}
