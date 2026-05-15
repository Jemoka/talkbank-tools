use std::path::{Path, PathBuf};

use crate::common::{prepare_audio_fixtures, prepare_multi_speaker_audio, prepare_named_audio};
use batchalign::options::{
    AsrEngineName, CommandOptions, CommonOptions, TranscribeOptions, WorTierPolicy,
};

pub(super) struct TranscribeFixtureJob {
    pub(super) source_path: String,
    pub(super) output_path: String,
}

pub(super) fn transcribe_options(
    engine: AsrEngineName,
    diarize: bool,
    wor: WorTierPolicy,
) -> CommandOptions {
    CommandOptions::Transcribe(TranscribeOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        asr_engine: engine,
        diarize,
        wor,
        merge_abbrev: false.into(),
        batch_size: 8,
        utseg_fallback: false.into(),
    })
}

pub(super) fn prepare_transcribe_fixture_job(
    state_dir: &Path,
    label: &str,
) -> Option<TranscribeFixtureJob> {
    let fixtures = prepare_audio_fixtures(state_dir)?;
    Some(TranscribeFixtureJob {
        source_path: fixtures.audio.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "test.cha")
            .to_string_lossy()
            .into_owned(),
    })
}

pub(super) fn prepare_named_transcribe_fixture_job(
    state_dir: &Path,
    label: &str,
    audio_name: &str,
) -> Option<TranscribeFixtureJob> {
    let fixtures = prepare_named_audio(state_dir, audio_name, None)?;
    Some(TranscribeFixtureJob {
        source_path: fixtures.audio.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, &format!("{audio_name}.cha"))
            .to_string_lossy()
            .into_owned(),
    })
}

pub(super) fn prepare_multi_speaker_transcribe_fixture_job(
    state_dir: &Path,
    label: &str,
) -> Option<TranscribeFixtureJob> {
    let fixtures = prepare_multi_speaker_audio(state_dir)?;
    Some(TranscribeFixtureJob {
        source_path: fixtures.audio.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "eng_multi_speaker.cha")
            .to_string_lossy()
            .into_owned(),
    })
}

fn standard_output_path(state_dir: &Path, label: &str, filename: &str) -> PathBuf {
    let out_dir = state_dir.join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    out_dir.join(filename)
}
