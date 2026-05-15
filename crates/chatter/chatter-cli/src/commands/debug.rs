//! Debug subcommands for CHAT file inspection.

use std::io::Write;
use std::path::{Path, PathBuf};

use talkbank_model::WriteChat;
use talkbank_model::alignment::helpers::overlap_groups::analyze_file_overlaps;
use talkbank_model::errors::NullErrorSink;

/// Run the overlap-audit debug command.
pub fn run_overlap_audit(paths: &[PathBuf], database_path: Option<&Path>) {
    let mut db_writer: Option<std::io::BufWriter<std::fs::File>> = database_path.map(|p| {
        let file = std::fs::File::create(p).unwrap_or_else(|e| {
            eprintln!("ERROR: cannot create database file {}: {e}", p.display());
            std::process::exit(1);
        });
        std::io::BufWriter::new(file)
    });

    let mut files_processed = 0;
    let mut total_groups = 0;
    let mut total_bottoms = 0;
    let mut total_orphaned_tops = 0;
    let mut total_orphaned_bottoms = 0;
    let mut total_timed = 0;
    let mut total_consistent = 0;
    let mut files_with_overlaps = 0;

    println!(
        "file\tutterances\tgroups\tbottoms\torphan_tops\torphan_bottoms\ttimed\tconsistent\tquality"
    );

    // The tree-sitter CHAT grammar is compiled into the binary and
    // structurally validated by CI, so construction is infallible
    // in practice (matches the pattern used in
    // `crates/chatter-cli/src/lib.rs` chat_parser()).
    #[allow(clippy::expect_used)]
    let parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");

    for path in collect_cha_files(paths) {
        let source = match std::fs::read_to_string(&path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ERROR: cannot read {}: {e}", path.display());
                continue;
            }
        };

        let chat = parser.parse_chat_file_streaming(&source, &NullErrorSink);
        let analysis = analyze_file_overlaps(&chat.lines);

        let utt_count = chat
            .lines
            .iter()
            .filter(|l| matches!(l, talkbank_model::model::Line::Utterance(_)))
            .count();

        if !analysis.has_overlaps() {
            continue;
        }

        files_with_overlaps += 1;

        // Temporal consistency
        let mut timed = 0;
        let mut consistent = 0;
        for group in &analysis.groups {
            for bottom in &group.bottoms {
                if let (Some(top_b), Some(bot_b)) = (group.top.bullet, bottom.bullet) {
                    timed += 1;
                    let tolerance: u64 = 2000;
                    if bot_b.0 <= top_b.1 + tolerance && bot_b.0 + tolerance >= top_b.0 {
                        consistent += 1;
                    }
                }
            }
        }

        let quality = if analysis.orphaned_tops.is_empty() && analysis.orphaned_bottoms.is_empty() {
            "fully_paired"
        } else {
            let total = analysis.groups.len()
                + analysis.orphaned_tops.len()
                + analysis.orphaned_bottoms.len();
            let orphan_frac = (analysis.orphaned_tops.len() + analysis.orphaned_bottoms.len())
                as f64
                / total as f64;
            if orphan_frac > 0.8 {
                "open_only"
            } else {
                "mixed"
            }
        };

        let filename = path.file_stem().unwrap_or_default().to_string_lossy();
        println!(
            "{filename}\t{utt_count}\t{}\t{}\t{}\t{}\t{timed}\t{consistent}\t{quality}",
            analysis.groups.len(),
            analysis.total_bottoms(),
            analysis.orphaned_tops.len(),
            analysis.orphaned_bottoms.len(),
        );

        // Write JSON lines record if database output requested
        if let Some(ref mut db_file) = db_writer {
            let record = serde_json::json!({
                "file": path.to_string_lossy(),
                "filename": filename,
                "utterances": utt_count,
                "groups": analysis.groups.len(),
                "bottoms": analysis.total_bottoms(),
                "orphaned_tops": analysis.orphaned_tops.len(),
                "orphaned_bottoms": analysis.orphaned_bottoms.len(),
                "timed_pairs": timed,
                "temporally_consistent": consistent,
                "quality": quality,
            });
            writeln!(db_file, "{}", record).unwrap_or_else(|e| {
                eprintln!("ERROR: failed to write database record: {e}");
            });
        }

        files_processed += 1;
        total_groups += analysis.groups.len();
        total_bottoms += analysis.total_bottoms();
        total_orphaned_tops += analysis.orphaned_tops.len();
        total_orphaned_bottoms += analysis.orphaned_bottoms.len();
        total_timed += timed;
        total_consistent += consistent;
    }

    eprintln!();
    eprintln!("=== Summary ===");
    eprintln!("  Files processed:    {files_processed}");
    eprintln!("  Files with overlaps: {files_with_overlaps}");
    eprintln!("  Total groups:       {total_groups}");
    eprintln!("  Total bottoms:      {total_bottoms}");
    eprintln!("  Orphaned tops:      {total_orphaned_tops}");
    eprintln!("  Orphaned bottoms:   {total_orphaned_bottoms}");
    if total_timed > 0 {
        let pct = total_consistent as f64 / total_timed as f64 * 100.0;
        eprintln!("  Timed pairs:        {total_timed}");
        eprintln!("  Temporally ok:      {total_consistent} ({pct:.0}%)");
    }
    if let Some(path) = database_path {
        eprintln!("  Database written:   {}", path.display());
    }
}

/// Run the linker-audit debug command.
///
/// When `anomalies_path` is provided, emits one JSON-lines record per anomaly
/// with file path, line number, anomaly type, speaker, preceding context, the
/// utterance text, and suggested fix. This JSON is the persistent dataset for
/// all downstream analysis and fix planning.
pub fn run_linker_audit(paths: &[PathBuf], anomalies_path: Option<&Path>) {
    use std::collections::HashMap;

    use talkbank_model::model::{Line, Linker, Terminator};

    let mut anomaly_writer: Option<std::io::BufWriter<std::fs::File>> = anomalies_path.map(|p| {
        let file = std::fs::File::create(p).unwrap_or_else(|e| {
            eprintln!("ERROR: cannot create anomalies file {}: {e}", p.display());
            std::process::exit(1);
        });
        std::io::BufWriter::new(file)
    });

    // The tree-sitter CHAT grammar is compiled into the binary and
    // structurally validated by CI, so construction is infallible
    // in practice (matches the pattern used in
    // `crates/chatter-cli/src/lib.rs` chat_parser()).
    #[allow(clippy::expect_used)]
    let parser = talkbank_parser::TreeSitterParser::new().expect("grammar loads");
    let cha_files = collect_cha_files(paths);
    let total_files = cha_files.len();
    eprintln!("Analyzing {total_files} .cha files...");

    // Corpus-wide accumulators
    let mut files_processed: usize = 0;
    let mut files_with_linkers: usize = 0;
    let mut files_with_anomalies: usize = 0;

    // Linker totals
    let mut linker_totals: HashMap<&'static str, usize> = HashMap::new();
    // Terminator totals
    let mut term_totals: HashMap<&'static str, usize> = HashMap::new();

    // ++ pairing
    let mut pp_correct: usize = 0;
    let mut pp_same_speaker: usize = 0;
    let mut pp_wrong_term: usize = 0;
    let mut pp_first: usize = 0;
    // ++ preceding terminator distribution
    let mut pp_prev_term_dist: HashMap<&'static str, usize> = HashMap::new();

    // +, pairing
    let mut sc_correct: usize = 0;
    let mut sc_wrong_term: usize = 0;
    let mut sc_no_prior: usize = 0;
    // +, preceding terminator distribution
    let mut sc_prev_term_dist: HashMap<&'static str, usize> = HashMap::new();

    // +" pairing
    let mut qf_correct: usize = 0;
    let mut qf_chained: usize = 0;
    let mut qf_wrong_term: usize = 0;
    let mut qf_no_prior: usize = 0;

    // +< blocks
    let mut lo_blocks: usize = 0;
    let mut lo_isolated: usize = 0;
    let mut lo_pairs: usize = 0;
    let mut lo_large: usize = 0;
    let mut lo_same_spk_start: usize = 0;
    let mut lo_combined: usize = 0;

    // +^
    let mut qu_same: usize = 0;
    let mut qu_diff: usize = 0;

    // +≋/+≈
    let mut tcu_tech_same: usize = 0;
    let mut tcu_tech_diff: usize = 0;
    let mut tcu_nb_same: usize = 0;
    let mut tcu_nb_diff: usize = 0;

    // Orphans
    let mut trailing_off_total: usize = 0;
    let mut trailing_off_followed: usize = 0;
    let mut interruption_total: usize = 0;
    let mut interruption_followed: usize = 0;

    fn term_label(t: &Terminator) -> &'static str {
        match t {
            Terminator::Period { .. } => ".",
            Terminator::Question { .. } => "?",
            Terminator::Exclamation { .. } => "!",
            Terminator::TrailingOff { .. } => "+...",
            Terminator::TrailingOffQuestion { .. } => "+..?",
            Terminator::Interruption { .. } => "+/.",
            Terminator::InterruptedQuestion { .. } => "+/?",
            Terminator::SelfInterruption { .. } => "+//.",
            Terminator::SelfInterruptedQuestion { .. } => "+//?",
            Terminator::BrokenQuestion { .. } => "+!?",
            Terminator::QuotedNewLine { .. } => "+\"/.",
            Terminator::QuotedPeriodSimple { .. } => "+\".",
            Terminator::BreakForCoding { .. } => "+.",
            Terminator::CaTechnicalBreak { .. } => "≋",
            Terminator::CaTechnicalBreakLinker { .. } => "+≋",
            Terminator::CaNoBreak { .. } => "≈",
            Terminator::CaNoBreakLinker { .. } => "+≈",
            _ => "(ca-intonation)",
        }
    }

    fn is_trailing_off(t: &Terminator) -> bool {
        matches!(
            t,
            Terminator::TrailingOff { .. } | Terminator::TrailingOffQuestion { .. }
        )
    }

    fn is_interruption(t: &Terminator) -> bool {
        matches!(
            t,
            Terminator::Interruption { .. } | Terminator::InterruptedQuestion { .. }
        )
    }

    /// Helper: get the main tier text for an utterance (e.g., "*CHI: hello .").
    fn utt_text(utt: &talkbank_model::Utterance) -> String {
        utt.main.to_string()
    }

    for (file_idx, path) in cha_files.iter().enumerate() {
        if file_idx % 5000 == 0 && file_idx > 0 {
            eprintln!("  ...{file_idx}/{total_files} files");
        }

        let source = match std::fs::read_to_string(path) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("ERROR: cannot read {}: {e}", path.display());
                continue;
            }
        };

        let chat = parser.parse_chat_file_streaming(&source, &NullErrorSink);
        let utterances: Vec<&talkbank_model::Utterance> = chat
            .lines
            .iter()
            .filter_map(|line| match line {
                Line::Utterance(u) => Some(u.as_ref()),
                _ => None,
            })
            .collect();

        if utterances.is_empty() {
            files_processed += 1;
            continue;
        }

        let file_str = path.to_string_lossy();
        let mut file_has_linker = false;
        let mut file_anomalies: usize = 0;

        // Per-speaker tracking
        let mut last_term_by_speaker: HashMap<&str, &'static str> = HashMap::new();
        let mut last_linker_by_speaker: HashMap<&str, Option<&Linker>> = HashMap::new();

        let mut prev_speaker: Option<&str> = None;
        let mut prev_term: Option<&Terminator> = None;
        let mut prev_term_label: Option<&'static str> = None;

        // +< block tracking
        let mut in_lazy_block = false;
        let mut lazy_block_size: usize = 0;
        let mut lazy_block_speakers: Vec<&str> = Vec::new();

        for (idx, utt) in utterances.iter().enumerate() {
            let speaker = utt.main.speaker.as_str();
            let linkers = utt.main.content.linkers.as_slice();
            let terminator = utt.main.content.terminator.as_ref();

            let mut has_lazy = false;
            let mut has_other_linker = false;

            for linker in linkers {
                file_has_linker = true;
                let label = match linker {
                    Linker::LazyOverlapPrecedes => {
                        has_lazy = true;
                        "+<"
                    }
                    Linker::OtherCompletion => "++",
                    Linker::QuickUptakeOverlap => "+^",
                    Linker::QuotationFollows => "+\"",
                    Linker::SelfCompletion => "+,",
                    Linker::TcuContinuation => "+≋",
                    Linker::NoBreakTcuContinuation => "+≈",
                };
                *linker_totals.entry(label).or_default() += 1;
                if !matches!(linker, Linker::LazyOverlapPrecedes) {
                    has_other_linker = true;
                }
            }

            if has_lazy && has_other_linker {
                lo_combined += 1;
            }

            // Count special terminators
            if let Some(term) = terminator {
                let tl = term_label(term);
                if !matches!(tl, "." | "?" | "!" | "(ca-intonation)") {
                    file_has_linker = true;
                    *term_totals.entry(tl).or_default() += 1;
                }
            }

            // ── ++ pairing ─────────────────────────────────────────
            if linkers.iter().any(|l| matches!(l, Linker::OtherCompletion)) {
                if idx == 0 {
                    pp_first += 1;
                    file_anomalies += 1;
                    if let Some(ref mut w) = anomaly_writer {
                        let _ = writeln!(
                            w,
                            "{}",
                            serde_json::json!({
                                "file": file_str, "utt_index": idx,
                                "type": "pp_first_utterance",
                                "severity": "error",
                                "speaker": speaker,
                                "text": utt_text(utt),
                                "suggested_fix": "Remove ++ or add preceding utterance",
                            })
                        );
                    }
                } else if let Some(ps) = prev_speaker {
                    let ptl = prev_term_label.unwrap_or("(none)");
                    *pp_prev_term_dist.entry(ptl).or_default() += 1;
                    if ps == speaker {
                        pp_same_speaker += 1;
                        file_anomalies += 1;
                        if let Some(ref mut w) = anomaly_writer {
                            let _ = writeln!(
                                w,
                                "{}",
                                serde_json::json!({
                                    "file": file_str, "utt_index": idx,
                                    "type": "pp_same_speaker",
                                    "severity": "auto_fixable",
                                    "speaker": speaker,
                                    "prev_speaker": ps,
                                    "prev_terminator": ptl,
                                    "prev_text": utt_text(utterances[idx - 1]),
                                    "text": utt_text(utt),
                                    "suggested_fix": {"action": "replace_linker", "old": "++", "new": "+,"},
                                })
                            );
                        }
                    } else if prev_term.is_some_and(is_trailing_off) {
                        pp_correct += 1;
                    } else {
                        pp_wrong_term += 1;
                        file_anomalies += 1;
                        if let Some(ref mut w) = anomaly_writer {
                            let _ = writeln!(
                                w,
                                "{}",
                                serde_json::json!({
                                    "file": file_str, "utt_index": idx,
                                    "type": "pp_wrong_terminator",
                                    "severity": "review",
                                    "speaker": speaker,
                                    "prev_speaker": ps,
                                    "prev_terminator": ptl,
                                    "prev_text": utt_text(utterances[idx - 1]),
                                    "text": utt_text(utt),
                                    "suggested_fix": {"action": "change_prev_terminator", "old": ptl, "new": "+..."},
                                })
                            );
                        }
                    }
                }
            }

            // ── +, pairing ─────────────────────────────────────────
            if linkers.iter().any(|l| matches!(l, Linker::SelfCompletion)) {
                match last_term_by_speaker.get(speaker) {
                    None => {
                        sc_no_prior += 1;
                        *sc_prev_term_dist.entry("(no_prior)").or_default() += 1;
                        file_anomalies += 1;
                        if let Some(ref mut w) = anomaly_writer {
                            let _ = writeln!(
                                w,
                                "{}",
                                serde_json::json!({
                                    "file": file_str, "utt_index": idx,
                                    "type": "sc_no_prior",
                                    "severity": "error",
                                    "speaker": speaker,
                                    "text": utt_text(utt),
                                    "suggested_fix": "Remove +, or add preceding same-speaker utterance",
                                })
                            );
                        }
                    }
                    Some(&tl) => {
                        *sc_prev_term_dist.entry(tl).or_default() += 1;
                        if matches!(tl, "+/." | "+/?") {
                            sc_correct += 1;
                        } else {
                            sc_wrong_term += 1;
                            file_anomalies += 1;
                            if let Some(ref mut w) = anomaly_writer {
                                let _ = writeln!(
                                    w,
                                    "{}",
                                    serde_json::json!({
                                        "file": file_str, "utt_index": idx,
                                        "type": "sc_wrong_terminator",
                                        "severity": "review",
                                        "speaker": speaker,
                                        "prev_same_speaker_terminator": tl,
                                        "text": utt_text(utt),
                                        "suggested_fix": {"action": "change_prev_terminator", "old": tl, "new": "+/."},
                                    })
                                );
                            }
                        }
                    }
                }
            }

            // ── +" pairing ─────────────────────────────────────────
            if linkers
                .iter()
                .any(|l| matches!(l, Linker::QuotationFollows))
            {
                match last_term_by_speaker.get(speaker) {
                    None => {
                        qf_no_prior += 1;
                        file_anomalies += 1;
                        if let Some(ref mut w) = anomaly_writer {
                            let _ = writeln!(
                                w,
                                "{}",
                                serde_json::json!({
                                    "file": file_str, "utt_index": idx,
                                    "type": "qf_no_prior",
                                    "severity": "error",
                                    "speaker": speaker,
                                    "text": utt_text(utt),
                                    "suggested_fix": "Remove +\" or add preceding same-speaker utterance with +\"/.  terminator",
                                })
                            );
                        }
                    }
                    Some(&"+\"/.") => qf_correct += 1,
                    Some(&prev_tl) => {
                        // Check if previous same-speaker also had +" (chaining)
                        if last_linker_by_speaker
                            .get(speaker)
                            .and_then(|l| *l)
                            .is_some_and(|l| matches!(l, Linker::QuotationFollows))
                        {
                            qf_chained += 1;
                        } else {
                            qf_wrong_term += 1;
                            file_anomalies += 1;
                            if let Some(ref mut w) = anomaly_writer {
                                let _ = writeln!(
                                    w,
                                    "{}",
                                    serde_json::json!({
                                        "file": file_str, "utt_index": idx,
                                        "type": "qf_wrong_terminator",
                                        "severity": "review",
                                        "speaker": speaker,
                                        "prev_same_speaker_terminator": prev_tl,
                                        "text": utt_text(utt),
                                        "suggested_fix": {"action": "change_prev_terminator", "old": prev_tl, "new": "+\"/."},
                                    })
                                );
                            }
                        }
                    }
                }
            }

            // ── +< blocks ──────────────────────────────────────────
            if has_lazy {
                if in_lazy_block {
                    lazy_block_size += 1;
                    if !lazy_block_speakers.contains(&speaker) {
                        lazy_block_speakers.push(speaker);
                    }
                } else {
                    // Flush previous block
                    if lazy_block_size > 0 {
                        lo_blocks += 1;
                        match lazy_block_size {
                            1 => lo_isolated += 1,
                            2 => lo_pairs += 1,
                            _ => lo_large += 1,
                        }
                    }
                    in_lazy_block = true;
                    lazy_block_size = 1;
                    lazy_block_speakers.clear();
                    lazy_block_speakers.push(speaker);
                    if prev_speaker.is_some_and(|ps| ps == speaker) {
                        lo_same_spk_start += 1;
                    }
                }
            } else if in_lazy_block {
                lo_blocks += 1;
                match lazy_block_size {
                    1 => lo_isolated += 1,
                    2 => lo_pairs += 1,
                    _ => lo_large += 1,
                }
                in_lazy_block = false;
                lazy_block_size = 0;
                lazy_block_speakers.clear();
            }

            // ── +^ ─────────────────────────────────────────────────
            if linkers
                .iter()
                .any(|l| matches!(l, Linker::QuickUptakeOverlap))
            {
                if prev_speaker.is_some_and(|ps| ps == speaker) {
                    qu_same += 1;
                } else {
                    qu_diff += 1;
                }
            }

            // ── +≋/+≈ ─────────────────────────────────────────────
            if linkers.iter().any(|l| matches!(l, Linker::TcuContinuation)) {
                if prev_speaker.is_some_and(|ps| ps == speaker) {
                    tcu_tech_same += 1;
                } else {
                    tcu_tech_diff += 1;
                }
            }
            if linkers
                .iter()
                .any(|l| matches!(l, Linker::NoBreakTcuContinuation))
            {
                if prev_speaker.is_some_and(|ps| ps == speaker) {
                    tcu_nb_same += 1;
                } else {
                    tcu_nb_diff += 1;
                }
            }

            // ── Orphans ────────────────────────────────────────────
            if let Some(term) = terminator {
                if is_trailing_off(term) {
                    trailing_off_total += 1;
                }
                if is_interruption(term) {
                    interruption_total += 1;
                }
            }
            if let Some(pt) = prev_term {
                if is_trailing_off(pt)
                    && linkers
                        .iter()
                        .any(|l| matches!(l, Linker::OtherCompletion | Linker::SelfCompletion))
                {
                    trailing_off_followed += 1;
                }
                if is_interruption(pt)
                    && linkers.iter().any(|l| matches!(l, Linker::SelfCompletion))
                {
                    interruption_followed += 1;
                }
            }

            // Update state
            prev_speaker = Some(speaker);
            prev_term = terminator;
            prev_term_label = terminator.map(term_label);
            if let Some(term) = terminator {
                last_term_by_speaker.insert(speaker, term_label(term));
            }
            let primary_linker = linkers
                .iter()
                .find(|l| !matches!(l, Linker::LazyOverlapPrecedes));
            last_linker_by_speaker.insert(speaker, primary_linker);
        }

        // Flush final +< block
        if in_lazy_block && lazy_block_size > 0 {
            lo_blocks += 1;
            match lazy_block_size {
                1 => lo_isolated += 1,
                2 => lo_pairs += 1,
                _ => lo_large += 1,
            }
        }

        if file_has_linker {
            files_with_linkers += 1;
        }
        if file_anomalies > 0 {
            files_with_anomalies += 1;
        }

        // Anomaly records are written inline above; no per-file summary needed

        files_processed += 1;
    }

    // ── Print summary ──────────────────────────────────────────────
    println!("=== LINKER AUDIT RESULTS ===");
    println!();
    println!("Files analyzed:                   {files_processed}");
    println!("Files with linkers/special terms: {files_with_linkers}");
    println!("Files with anomalies:             {files_with_anomalies}");

    println!();
    println!("--- Linker Frequencies ---");
    let mut sorted_linkers: Vec<_> = linker_totals.iter().collect();
    sorted_linkers.sort_by(|a, b| b.1.cmp(a.1));
    for (label, count) in &sorted_linkers {
        println!("  {label:<30} {count:>8}");
    }

    println!();
    println!("--- Special Terminator Frequencies ---");
    let mut sorted_terms: Vec<_> = term_totals.iter().collect();
    sorted_terms.sort_by(|a, b| b.1.cmp(a.1));
    for (label, count) in &sorted_terms {
        println!("  {label:<30} {count:>8}");
    }

    let pp_total = pp_correct + pp_same_speaker + pp_wrong_term + pp_first;
    println!();
    println!("--- ++ (Other Completion) Pairing ---");
    println!("  Total:                          {pp_total:>8}");
    println!(
        "  Correct (diff spk + +...):      {pp_correct:>8} ({})",
        pct(pp_correct, pp_total)
    );
    println!(
        "  ANOMALY: same speaker:          {pp_same_speaker:>8} ({})",
        pct(pp_same_speaker, pp_total)
    );
    println!(
        "  ANOMALY: wrong terminator:      {pp_wrong_term:>8} ({})",
        pct(pp_wrong_term, pp_total)
    );
    println!("  ANOMALY: first utterance:       {pp_first:>8}");
    println!("  Preceding terminator distribution:");
    let mut sorted_pp: Vec<_> = pp_prev_term_dist.iter().collect();
    sorted_pp.sort_by(|a, b| b.1.cmp(a.1));
    for (label, count) in &sorted_pp {
        println!("    {label:<28} {count:>8}");
    }

    let sc_total = sc_correct + sc_wrong_term + sc_no_prior;
    println!();
    println!("--- +, (Self Completion) Pairing ---");
    println!("  Total:                          {sc_total:>8}");
    println!(
        "  Correct (same spk + +/.):       {sc_correct:>8} ({})",
        pct(sc_correct, sc_total)
    );
    println!(
        "  ANOMALY: wrong terminator:      {sc_wrong_term:>8} ({})",
        pct(sc_wrong_term, sc_total)
    );
    println!("  ANOMALY: no prior same-speaker:  {sc_no_prior:>8}");
    println!("  Preceding same-speaker terminator distribution:");
    let mut sorted_sc: Vec<_> = sc_prev_term_dist.iter().collect();
    sorted_sc.sort_by(|a, b| b.1.cmp(a.1));
    for (label, count) in &sorted_sc {
        println!("    {label:<28} {count:>8}");
    }

    let qf_total = qf_correct + qf_chained + qf_wrong_term + qf_no_prior;
    println!();
    println!("--- +\" (Quotation) Pairing ---");
    println!("  Total:                          {qf_total:>8}");
    println!(
        "  Correct (same spk + +\"/.)       {qf_correct:>8} ({})",
        pct(qf_correct, qf_total)
    );
    println!(
        "  Chained (same spk + +\"):        {qf_chained:>8} ({})",
        pct(qf_chained, qf_total)
    );
    println!(
        "  ANOMALY: wrong terminator:      {qf_wrong_term:>8} ({})",
        pct(qf_wrong_term, qf_total)
    );
    println!("  ANOMALY: no prior same-speaker:  {qf_no_prior:>8}");

    println!();
    println!("--- +< (Lazy Overlap) Blocks ---");
    println!("  Total blocks:                   {lo_blocks:>8}");
    println!("  Isolated (size 1):              {lo_isolated:>8}");
    println!("  Pairs (size 2):                 {lo_pairs:>8}");
    println!("  Large (size 3+):                {lo_large:>8}");
    println!("  Same-speaker block start:       {lo_same_spk_start:>8}");
    println!("  Combined with other linker:     {lo_combined:>8}");

    println!();
    println!("--- +^ (Quick Uptake) ---");
    println!("  Same speaker:                   {qu_same:>8}");
    println!("  Different speaker:              {qu_diff:>8}");

    if tcu_tech_same + tcu_tech_diff + tcu_nb_same + tcu_nb_diff > 0 {
        println!();
        println!("--- CA TCU Linkers ---");
        println!("  +≋ same speaker:                {tcu_tech_same:>8}");
        println!("  +≋ diff speaker:                {tcu_tech_diff:>8}");
        println!("  +≈ same speaker:                {tcu_nb_same:>8}");
        println!("  +≈ diff speaker:                {tcu_nb_diff:>8}");
    }

    println!();
    println!("--- Orphaned Special Terminators ---");
    println!("  +... total:                     {trailing_off_total:>8}");
    println!("  +... followed by ++/+,:         {trailing_off_followed:>8}");
    println!(
        "  +... orphaned:                  {:>8}",
        trailing_off_total - trailing_off_followed
    );
    println!("  +/. total:                      {interruption_total:>8}");
    println!("  +/. followed by +,:             {interruption_followed:>8}");
    println!(
        "  +/. orphaned:                   {:>8}",
        interruption_total - interruption_followed
    );

    if let Some(path) = anomalies_path {
        eprintln!("\nAnomalies written: {}", path.display());
    }
}

fn pct(n: usize, total: usize) -> String {
    if total == 0 {
        "0%".to_owned()
    } else {
        format!("{:.1}%", n as f64 / total as f64 * 100.0)
    }
}

/// Recursively collect .cha files from paths.
fn collect_cha_files(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for p in paths {
        if p.is_dir() {
            collect_recursive(p, &mut files);
        } else if p.extension().is_some_and(|ext| ext == "cha") {
            files.push(p.clone());
        }
    }
    files.sort();
    files
}

/// Rewrite whole-utterance `@s` runs into utterance precodes in place.
///
/// Implements `chatter debug fix-s`. Qualifying utterances are rewritten as
/// `[- LANG] ...`, matching per-word language markers are removed, and missing
/// explicit `@s:LANG` codes are appended to `@Languages`. Files with no
/// qualifying rewrites or language-header repairs are left untouched.
pub fn run_fix_s(paths: &[PathBuf]) {
    let files = collect_cha_files(paths);
    if files.is_empty() {
        die("no .cha files found in the provided paths");
    }

    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));
    let mut rewritten_files = 0usize;
    let mut rewritten_utterances = 0usize;
    let mut appended_language_codes = 0usize;

    for path in files {
        let source = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", path.display())));
        let mut parsed = parser
            .parse_chat_file(&source)
            .unwrap_or_else(|e| die(&format!("parse failed for {}: {e:?}", path.display())));
        let stats =
            talkbank_transform::fix_s::rewrite_whole_utterance_language_switches(&mut parsed);
        if stats.is_empty() {
            continue;
        }

        let rewritten = parsed.to_chat_string();
        if rewritten == source {
            continue;
        }

        std::fs::write(&path, &rewritten)
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display())));
        rewritten_files += 1;
        rewritten_utterances += stats.rewritten_utterances;
        appended_language_codes += stats.appended_language_codes;
    }

    if rewritten_files == 0 {
        println!("No fix-s rewrites or @Languages repairs needed.");
    } else {
        println!(
            "Rewrote {rewritten_files} file(s); updated {rewritten_utterances} utterance(s) and appended {appended_language_codes} @Languages code(s)."
        );
    }
}

/// Sanitize a single CHAT file and write the result to `output_path` (or
/// stdout when `None`).
///
/// Implements `chatter debug sanitize`. Runs the strict sanitization
/// policy from `talkbank-transform::redact`. See
/// `talkbank/docs/protected-corpus-debugging-workflow.md` for context.
pub fn run_sanitize(input: &Path, output_path: Option<&Path>) {
    let source = std::fs::read_to_string(input)
        .unwrap_or_else(|e| die(&format!("cannot read {}: {e}", input.display())));
    let parser = talkbank_parser::TreeSitterParser::new()
        .unwrap_or_else(|e| die(&format!("parser initialization failed: {e:?}")));
    let parsed = parser
        .parse_chat_file(&source)
        .unwrap_or_else(|e| die(&format!("parse failed for {}: {e:?}", input.display())));

    let policy = talkbank_transform::redact::SanitizationPolicy::strict();
    let sanitized = talkbank_transform::redact::sanitize(parsed, &policy)
        .unwrap_or_else(|e| die(&format!("sanitize failed for {}: {e}", input.display())));
    let chat_text = sanitized.to_chat_string();

    match output_path {
        Some(path) => std::fs::write(path, &chat_text)
            .unwrap_or_else(|e| die(&format!("cannot write {}: {e}", path.display()))),
        None => std::io::stdout()
            .lock()
            .write_all(chat_text.as_bytes())
            .unwrap_or_else(|e| die(&format!("cannot write to stdout: {e}"))),
    }
}

/// Print a user-facing error and exit non-zero.
fn die(msg: &str) -> ! {
    eprintln!("ERROR: {msg}");
    std::process::exit(1);
}

fn collect_recursive(dir: &PathBuf, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                collect_recursive(&path, files);
            } else if path.extension().is_some_and(|ext| ext == "cha") {
                files.push(path);
            }
        }
    }
}
