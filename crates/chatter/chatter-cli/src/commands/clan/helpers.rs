//! Shared CLI helper functions for CLAN command wrappers.
//!
//! This module owns the outer-wrapper responsibilities that should stay in the
//! CLI layer: reading and writing files, building CLI-owned filter selections,
//! and adapting typed `AnalysisCommandName` plus `AnalysisOptions` values into
//! calls on the library-owned [`clan_core::service::AnalysisRequestBuilder`]
//! and [`clan_core::service::AnalysisService`]. The actual CLAN analysis
//! execution boundary lives in `clan-core`.

use std::path::{Path, PathBuf};

use crate::cli::{ClanOutputFormat, CommonAnalysisArgs};
use clan_core::framework::{
    DiscoveredChatFiles, FilterConfig, GemFilter, GemLabel, OutputFormat, SpeakerFilter,
    TransformCommand, WordFilter, WordPattern, run_transform,
};
use clan_core::service::AnalysisService;
use clan_core::service_types::{
    AnalysisCommandName, AnalysisOptions, AnalysisPlan, AnalysisRequest, AnalysisRequestBuilder,
};
use talkbank_model::SpeakerCode;

pub(super) fn run_normalize_alias(path: &Path, output: Option<&Path>) {
    let content = read_file_or_exit(path);
    let options = talkbank_model::ParseValidateOptions::default();
    match talkbank_transform::normalize_chat(&content, options) {
        Ok(normalized) => write_output_or_exit(&normalized, output),
        Err(e) => exit_with_error(format!("Error: {e}")),
    }
}

pub(super) fn read_file_or_exit(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|e| {
        exit_with_error(format!("Error reading {}: {e}", path.display()));
    })
}

pub(super) fn parse_chat_or_exit(path: &Path) -> talkbank_model::ChatFile {
    let content = read_file_or_exit(path);
    talkbank_transform::parse_and_validate(
        &content,
        talkbank_model::ParseValidateOptions::default(),
    )
    .unwrap_or_else(|e| exit_with_error(format!("Error parsing {}: {e}", path.display())))
}

pub(super) fn write_output_or_exit(content: &str, output: Option<&Path>) {
    if let Some(path) = output {
        if let Err(e) = std::fs::write(path, content) {
            exit_with_error(format!("Error writing {}: {e}", path.display()));
        }
    } else {
        print!("{content}");
    }
}

pub(super) fn run_converter(
    result: Result<talkbank_model::ChatFile, clan_core::framework::TransformError>,
    output: Option<&Path>,
) {
    match result {
        Ok(chat) => write_output_or_exit(&chat.to_string(), output),
        Err(e) => exit_with_error(format!("Error: {e}")),
    }
}

pub(super) fn run_analysis_and_print(
    command_name: AnalysisCommandName,
    options: AnalysisOptions,
    paths: &[PathBuf],
    common: &CommonAnalysisArgs,
) {
    let plan = build_analysis_plan_or_exit(command_name, options);
    let AnalysisPlan::Service(request) = plan else {
        exit_with_error(format!(
            "Error: {command_name} requires paired-file execution"
        ));
    };

    run_request_and_print(request, paths, common);
}

pub(super) fn run_paired_analysis_and_print(
    command_name: AnalysisCommandName,
    options: AnalysisOptions,
    primary_file: &Path,
    format: ClanOutputFormat,
) {
    let plan = build_analysis_plan_or_exit(command_name, options);
    let AnalysisPlan::Rely(request) = plan else {
        exit_with_error(format!(
            "Error: {command_name} does not support paired-file execution"
        ));
    };

    let service = AnalysisService::new();
    match service.execute_rely_rendered(request, primary_file, convert_format(format)) {
        Ok(result) => print!("{result}"),
        Err(error) => exit_with_error(format!("Error: {error}")),
    }
}

fn build_analysis_plan_or_exit(
    command_name: AnalysisCommandName,
    options: AnalysisOptions,
) -> AnalysisPlan {
    AnalysisRequestBuilder::new(command_name, options)
        .build()
        .unwrap_or_else(|error| exit_with_error(format!("Error: {error}")))
}

fn run_request_and_print(request: AnalysisRequest, paths: &[PathBuf], common: &CommonAnalysisArgs) {
    let discovered_files = DiscoveredChatFiles::from_paths(paths);
    for skipped_path in discovered_files.skipped_paths() {
        eprintln!(
            "Warning: {:?} is not a file or directory, skipping",
            skipped_path
        );
    }

    let files = discovered_files.into_files();
    if files.is_empty() {
        exit_with_error("Error: no .cha files found".to_owned());
    }

    let filter = build_filter(common);
    let service = AnalysisService::with_filter(filter);
    let format = convert_format(common.format);

    if common.per_file {
        match service.execute_rendered_per_file(request, &files, format) {
            Ok(results) => {
                for (path, result) in results {
                    println!("From file: {}", path.display());
                    print!("{result}");
                    println!();
                }
            }
            Err(e) => exit_with_error(format!("Error: {e}")),
        }
    } else {
        match service.execute_rendered(request, &files, format) {
            Ok(result) => {
                print!("{result}");
            }
            Err(e) => exit_with_error(format!("Error: {e}")),
        }
    }
}

pub(super) fn run_transform_or_exit<T: TransformCommand>(
    cmd: &T,
    path: &Path,
    output: Option<&Path>,
) {
    if let Err(e) = run_transform(cmd, path, output) {
        exit_with_error(format!("Error: {e}"));
    }
}

pub(super) fn build_filter(common: &CommonAnalysisArgs) -> FilterConfig {
    let speaker_filter = SpeakerFilter {
        include: common.speaker.iter().map(SpeakerCode::new).collect(),
        exclude: common
            .exclude_speaker
            .iter()
            .map(SpeakerCode::new)
            .collect(),
    };

    let gem_filter = GemFilter {
        include: common
            .gem
            .iter()
            .map(|s| GemLabel::from(s.as_str()))
            .collect(),
        exclude: common
            .exclude_gem
            .iter()
            .map(|s| GemLabel::from(s.as_str()))
            .collect(),
    };

    let word_filter = WordFilter {
        include: common
            .include_word
            .iter()
            .map(|s| WordPattern::from(s.as_str()))
            .collect(),
        exclude: common
            .exclude_word
            .iter()
            .map(|s| WordPattern::from(s.as_str()))
            .collect(),
    };

    FilterConfig {
        speakers: speaker_filter,
        gems: gem_filter,
        words: word_filter,
        utterance_range: common.range,
        id_filter: common.id_filter.clone(),
        ..FilterConfig::default()
    }
}

pub(super) fn convert_format(format: ClanOutputFormat) -> OutputFormat {
    match format {
        ClanOutputFormat::Text => OutputFormat::Text,
        ClanOutputFormat::Json => OutputFormat::Json,
        ClanOutputFormat::Csv => OutputFormat::Csv,
        ClanOutputFormat::Clan => OutputFormat::Clan,
    }
}

pub(super) fn exit_with_error(message: String) -> ! {
    eprintln!("{message}");
    std::process::exit(1);
}
