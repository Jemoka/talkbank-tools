use std::path::{Path, PathBuf};

use crate::common::prepare_audio_fixtures;
use batchalign::options::{CommandOptions, CommonOptions, OpensmileOptions};

pub(super) struct OpensmileFixtureJob {
    pub(super) source_path: String,
    pub(super) output_path: String,
}

pub(super) fn opensmile_options(feature_set: &str) -> CommandOptions {
    CommandOptions::Opensmile(OpensmileOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
        feature_set: feature_set.into(),
    })
}

pub(super) fn prepare_opensmile_fixture_job(
    state_dir: &Path,
    label: &str,
) -> Option<OpensmileFixtureJob> {
    let fixtures = prepare_audio_fixtures(state_dir)?;
    Some(OpensmileFixtureJob {
        source_path: fixtures.audio.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "test.csv")
            .to_string_lossy()
            .into_owned(),
    })
}

fn standard_output_path(state_dir: &Path, label: &str, filename: &str) -> PathBuf {
    let out_dir = state_dir.join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    out_dir.join(filename)
}
