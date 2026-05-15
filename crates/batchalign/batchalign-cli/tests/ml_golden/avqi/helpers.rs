use std::path::{Path, PathBuf};

use crate::common::prepare_audio_fixtures;
use batchalign::options::{AvqiOptions, CommandOptions, CommonOptions};

pub(super) struct AvqiFixtureJob {
    pub(super) cs_source_path: String,
    pub(super) output_path: String,
}

pub(super) fn avqi_options() -> CommandOptions {
    CommandOptions::Avqi(AvqiOptions {
        common: CommonOptions {
            override_media_cache: true,
            ..CommonOptions::default()
        },
    })
}

pub(super) fn prepare_avqi_fixture_job(state_dir: &Path, label: &str) -> Option<AvqiFixtureJob> {
    let fixtures = prepare_audio_fixtures(state_dir)?;
    let pair_dir = state_dir.join(format!("avqi_{label}"));
    std::fs::create_dir_all(&pair_dir).expect("mkdir avqi pair dir");

    let cs_path = pair_dir.join("test.cs.mp3");
    let sv_path = pair_dir.join("test.sv.mp3");
    std::fs::copy(&fixtures.audio, &cs_path).expect("copy avqi cs audio");
    std::fs::copy(&fixtures.audio, &sv_path).expect("copy avqi sv audio");

    Some(AvqiFixtureJob {
        cs_source_path: cs_path.to_string_lossy().into_owned(),
        output_path: standard_output_path(state_dir, label, "test.txt")
            .to_string_lossy()
            .into_owned(),
    })
}

fn standard_output_path(state_dir: &Path, label: &str, filename: &str) -> PathBuf {
    let out_dir = state_dir.join(format!("out_{label}"));
    std::fs::create_dir_all(&out_dir).expect("mkdir output dir");
    out_dir.join(filename)
}
