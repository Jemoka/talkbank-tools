use std::path::{Path, PathBuf};

use crate::common::prepare_audio_fixtures;
use batchalign::options::{
    AlignOptions, CommandOptions, CommonOptions, FaEngineName, UtrEngine, WorTierPolicy,
};

pub(super) struct AlignFixtureJob {
    pub(super) source_path: String,
    pub(super) output_path: String,
    pub(super) before_path: String,
}

pub(super) struct AlignMediaDirJob {
    pub(super) source_path: String,
    pub(super) output_path: String,
    pub(super) media_dir: String,
}

pub(super) fn align_options(engine: FaEngineName, wor: WorTierPolicy) -> CommandOptions {
    align_options_with_media_dir_and_utr(engine, wor, None, None)
}

pub(super) fn align_options_with_media_dir(
    engine: FaEngineName,
    wor: WorTierPolicy,
    media_dir: Option<String>,
) -> CommandOptions {
    align_options_with_media_dir_and_utr(engine, wor, media_dir, None)
}

pub(super) fn align_options_with_utr(
    engine: FaEngineName,
    wor: WorTierPolicy,
    utr_engine: UtrEngine,
) -> CommandOptions {
    align_options_with_media_dir_and_utr(engine, wor, None, Some(utr_engine))
}

pub(super) fn align_options_with_media_dir_and_utr(
    engine: FaEngineName,
    wor: WorTierPolicy,
    media_dir: Option<String>,
    utr_engine: Option<UtrEngine>,
) -> CommandOptions {
    let common = CommonOptions {
        override_media_cache: true,
        ..CommonOptions::default()
    };

    CommandOptions::Align(AlignOptions {
        common,
        fa_engine: engine,
        utr_engine,
        wor,
        media_dir,
        ..AlignOptions::default()
    })
}

pub(super) fn prepare_align_fixture_job(state_dir: &Path, label: &str) -> Option<AlignFixtureJob> {
    let fixtures = prepare_audio_fixtures(state_dir)?;
    Some(AlignFixtureJob {
        source_path: fixtures.stripped_chat.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "test.cha")
            .to_string_lossy()
            .into_owned(),
        before_path: fixtures.chat.to_string_lossy().into_owned(),
    })
}

pub(super) fn prepare_align_media_dir_job(
    state_dir: &Path,
    label: &str,
) -> Option<AlignMediaDirJob> {
    let fixtures = prepare_audio_fixtures(state_dir)?;
    let source_path = fixtures.stripped_chat.clone();
    let source_dir = source_path
        .parent()
        .expect("align stripped fixture should have a parent dir");
    let media_basename = source_path
        .file_stem()
        .expect("align stripped fixture should have a stem")
        .to_string_lossy()
        .to_string();
    let adjacent_media = source_dir.join(format!("{media_basename}.mp3"));

    let media_dir = state_dir.join(format!("media_{label}"));
    std::fs::create_dir_all(&media_dir).expect("mkdir media dir");
    let relocated_media = media_dir.join(format!("{media_basename}.mp3"));
    std::fs::copy(&adjacent_media, &relocated_media).expect("copy relocated media");
    std::fs::remove_file(&adjacent_media).expect("remove adjacent media");

    Some(AlignMediaDirJob {
        source_path: source_path.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "test.cha")
            .to_string_lossy()
            .into_owned(),
        media_dir: media_dir.to_string_lossy().into_owned(),
    })
}

fn standard_output_path(state_dir: &Path, label: &str, filename: &str) -> PathBuf {
    let out_dir = state_dir.join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    out_dir.join(filename)
}
