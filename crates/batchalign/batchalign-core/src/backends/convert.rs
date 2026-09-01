//! Native Rust media conversion backend.

use crate::backends::{Backend, BatchPolicy};
use crate::base::{Task, TaskInput, TaskOutput};
use crate::proto::convert::{MediaFormat, MediaOutput};
use crate::utils::{BAError, BAResult, PreparedAudio};
use oxideav_mp3::{ChannelMode, Mp3Encoder, QualityPreset};

const MP3_SAMPLE_RATES: [u32; 9] = [
    8_000, 11_025, 12_000, 16_000, 22_050, 24_000, 32_000, 44_100, 48_000,
];

#[derive(Clone, Debug)]
pub struct ConvertBackend {
    name: String,
    tasks: Vec<Task>,
    format: MediaFormat,
    mp3_bitrate_kbps: Option<u32>,
    mp3_sample_rate_hz: Option<u32>,
}

impl ConvertBackend {
    pub fn new(format: MediaFormat) -> Self {
        let encoder = match format {
            MediaFormat::Mp3 => "oxideav-mp3-0.1.3",
            MediaFormat::Wav => "pcm16",
        };
        Self {
            name: format!("convert:rust:{encoder}:{}:v1", format.extension()),
            tasks: vec![Task::Convert],
            format,
            mp3_bitrate_kbps: None,
            mp3_sample_rate_hz: None,
        }
    }

    /// Construct an MP3 encoder with an explicit constant bitrate.
    ///
    /// The ordinary Convert task deliberately keeps its established
    /// 128/192 kbps defaults. Cloud backends can select a speech-appropriate
    /// bitrate when transfer size matters more than music fidelity.
    pub fn mp3(bitrate_kbps: u32, sample_rate_hz: u32) -> Self {
        Self {
            name: format!(
                "convert:rust:oxideav-mp3-0.1.3:mp3:{bitrate_kbps}kbps:{sample_rate_hz}hz:v1"
            ),
            tasks: vec![Task::Convert],
            format: MediaFormat::Mp3,
            mp3_bitrate_kbps: Some(bitrate_kbps),
            mp3_sample_rate_hz: Some(sample_rate_hz),
        }
    }

    /// Encode already-prepared PCM through the same implementation used by
    /// the Convert task runner.
    pub fn encode(&self, audio: &PreparedAudio) -> BAResult<Vec<u8>> {
        match self.format {
            MediaFormat::Wav => encode_wav(audio),
            MediaFormat::Mp3 => encode_mp3(audio, self.mp3_bitrate_kbps, self.mp3_sample_rate_hz),
        }
    }
}

impl Backend for ConvertBackend {
    fn name(&self) -> &str {
        &self.name
    }

    fn tasks(&self) -> &[Task] {
        &self.tasks
    }

    fn batch_policy(&self) -> BatchPolicy {
        // Each input can hold an entire decoded media file. Keep dispatch
        // atomic so a backend batch cannot multiply that memory footprint.
        BatchPolicy::one()
    }

    fn call(&self, batch: Vec<TaskInput>) -> BAResult<Vec<TaskOutput>> {
        batch
            .into_iter()
            .map(|input| {
                let TaskInput::Convert(input) = input else {
                    return Err(BAError::Internal(format!(
                        "ConvertBackend received non-Convert input: {:?}",
                        input.task()
                    )));
                };
                let encoded_bytes = self.encode(&input.audio)?;
                Ok(TaskOutput::Convert(MediaOutput {
                    source_id: input.source_id,
                    format: self.format,
                    encoded_bytes,
                }))
            })
            .collect()
    }
}

fn pcm_f32(audio: &PreparedAudio) -> BAResult<Vec<f32>> {
    if audio.channels == 0 {
        return Err(BAError::Worker(
            "convert: decoded audio has zero channels".into(),
        ));
    }
    if audio.pcm_f32le.len() % 4 != 0 {
        return Err(BAError::Worker(
            "convert: decoded PCM byte length is not divisible by four".into(),
        ));
    }
    let samples: Vec<f32> = audio
        .pcm_f32le
        .chunks_exact(4)
        .map(|bytes| f32::from_le_bytes(bytes.try_into().expect("four-byte chunk")))
        .collect();
    if samples.len() % usize::from(audio.channels) != 0 {
        return Err(BAError::Worker(
            "convert: decoded PCM sample count is not channel-aligned".into(),
        ));
    }
    Ok(samples)
}

fn sample_to_i16(sample: f32) -> i16 {
    let value = sample.clamp(-1.0, 1.0);
    if value < 0.0 {
        (value * 32_768.0).round() as i16
    } else {
        (value * 32_767.0).round() as i16
    }
}

fn encode_wav(audio: &PreparedAudio) -> BAResult<Vec<u8>> {
    let samples = pcm_f32(audio)?;
    let data_len = samples
        .len()
        .checked_mul(2)
        .and_then(|n| u32::try_from(n).ok())
        .ok_or_else(|| BAError::Worker("convert: WAV output exceeds RIFF's 4 GiB limit".into()))?;
    let channels = audio.channels;
    let block_align = channels
        .checked_mul(2)
        .ok_or_else(|| BAError::Worker("convert: WAV channel layout is too large".into()))?;
    let byte_rate = audio
        .sample_rate
        .checked_mul(u32::from(block_align))
        .ok_or_else(|| BAError::Worker("convert: WAV byte rate overflow".into()))?;
    let riff_len = 36u32
        .checked_add(data_len)
        .ok_or_else(|| BAError::Worker("convert: WAV RIFF length overflow".into()))?;

    let mut out = Vec::with_capacity(data_len as usize + 44);
    out.extend_from_slice(b"RIFF");
    out.extend_from_slice(&riff_len.to_le_bytes());
    out.extend_from_slice(b"WAVEfmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // PCM
    out.extend_from_slice(&channels.to_le_bytes());
    out.extend_from_slice(&audio.sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&16u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&data_len.to_le_bytes());
    for sample in samples {
        out.extend_from_slice(&sample_to_i16(sample).to_le_bytes());
    }
    Ok(out)
}

fn encode_mp3(
    audio: &PreparedAudio,
    bitrate_kbps: Option<u32>,
    sample_rate_hz: Option<u32>,
) -> BAResult<Vec<u8>> {
    let mut samples = pcm_f32(audio)?;
    let mut channels = usize::from(audio.channels);
    // Layer III carries at most two channels. Preserve ordinary mono/stereo
    // sources; for wider layouts, average all channels to a safe mono mix.
    if channels > 2 {
        samples = samples
            .chunks_exact(channels)
            .map(|frame| frame.iter().copied().sum::<f32>() / channels as f32)
            .collect();
        channels = 1;
    }

    let target_rate = sample_rate_hz.unwrap_or_else(|| nearest_mp3_rate(audio.sample_rate));
    if target_rate != audio.sample_rate {
        samples = resample_linear(&samples, channels, audio.sample_rate, target_rate);
    }
    let pcm_i16: Vec<i16> = samples.into_iter().map(sample_to_i16).collect();

    let mode = if channels == 1 {
        ChannelMode::SingleChannel
    } else {
        ChannelMode::Stereo
    };
    let bitrate_kbps = bitrate_kbps.unwrap_or_else(|| {
        if target_rate >= 32_000 && channels == 2 {
            192
        } else {
            128
        }
    });
    let mut encoder =
        Mp3Encoder::new_with_quality_preset(bitrate_kbps, target_rate, mode, QualityPreset::High)
            .map_err(|err| BAError::Worker(format!("convert: initialize MP3 encoder: {err:?}")))?;
    encoder
        .push_samples(&pcm_i16)
        .map_err(|err| BAError::Worker(format!("convert: encode MP3 samples: {err:?}")))?;
    let mut out = Vec::new();
    encoder
        .finish(&mut out)
        .map_err(|err| BAError::Worker(format!("convert: finish MP3 stream: {err:?}")))?;
    Ok(out)
}

fn nearest_mp3_rate(source: u32) -> u32 {
    MP3_SAMPLE_RATES
        .iter()
        .copied()
        .min_by_key(|rate| rate.abs_diff(source))
        .expect("MP3 sample-rate table is non-empty")
}

/// Channel-interleaved linear resampler used only for off-ladder rates.
fn resample_linear(
    samples: &[f32],
    channels: usize,
    source_rate: u32,
    target_rate: u32,
) -> Vec<f32> {
    if samples.is_empty() || source_rate == target_rate {
        return samples.to_vec();
    }
    let input_frames = samples.len() / channels;
    let output_frames = ((input_frames as u128 * u128::from(target_rate)
        + u128::from(source_rate) / 2)
        / u128::from(source_rate)) as usize;
    let mut out = Vec::with_capacity(output_frames * channels);
    for output_frame in 0..output_frames {
        let pos = output_frame as f64 * f64::from(source_rate) / f64::from(target_rate);
        let left = (pos.floor() as usize).min(input_frames - 1);
        let right = (left + 1).min(input_frames - 1);
        let frac = (pos - left as f64) as f32;
        for channel in 0..channels {
            let a = samples[left * channels + channel];
            let b = samples[right * channels + channel];
            out.push(a + (b - a) * frac);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::utils::SourceId;

    fn sine_audio(rate: u32, channels: u16) -> PreparedAudio {
        let frames = rate / 20;
        let mut pcm_f32le = Vec::new();
        for i in 0..frames {
            let sample = ((i as f32 * 440.0 * std::f32::consts::TAU) / rate as f32).sin() * 0.2;
            for _ in 0..channels {
                pcm_f32le.extend_from_slice(&sample.to_le_bytes());
            }
        }
        PreparedAudio {
            pcm_f32le,
            sample_rate: rate,
            channels,
            frame_count: u64::from(frames),
        }
    }

    #[test]
    fn wav_has_pcm_header_and_payload() {
        let wav = encode_wav(&sine_audio(16_000, 2)).unwrap();
        assert_eq!(&wav[..4], b"RIFF");
        assert_eq!(&wav[8..12], b"WAVE");
        assert_eq!(u16::from_le_bytes([wav[22], wav[23]]), 2);
        assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 16_000);
        assert!(wav.len() > 44);
    }

    #[test]
    fn mp3_encoder_emits_layer_three_frames() {
        let backend = ConvertBackend::new(MediaFormat::Mp3);
        let input = crate::proto::convert::ConvertInput {
            source_id: SourceId::try_new("tone.wav").unwrap(),
            audio: sine_audio(44_100, 1),
        };
        let output = backend.call(vec![TaskInput::Convert(input)]).unwrap();
        let TaskOutput::Convert(output) = &output[0] else {
            panic!("wrong output")
        };
        assert!(output.encoded_bytes.len() > 100);
        assert_eq!(output.encoded_bytes[0], 0xff);
        assert_eq!(output.encoded_bytes[1] & 0xe0, 0xe0);
    }

    #[test]
    fn explicit_mp3_bitrate_reduces_cloud_payload_size() {
        let audio = sine_audio(16_000, 1);
        let ordinary = ConvertBackend::new(MediaFormat::Mp3)
            .encode(&audio)
            .unwrap();
        let speech = ConvertBackend::mp3(16, 16_000).encode(&audio).unwrap();

        assert_eq!(speech[0], 0xff);
        assert_eq!(speech[1] & 0xe0, 0xe0);
        assert!(speech.len() < ordinary.len());
    }

    #[test]
    fn off_ladder_rate_preserves_approximate_duration() {
        let input = vec![0.0f32; 96_000];
        let output = resample_linear(&input, 1, 96_000, 48_000);
        assert_eq!(output.len(), 48_000);
    }
}
