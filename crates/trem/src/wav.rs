//! IEEE **32-bit float** WAV export (interleaved PCM).
//!
//! Enable Cargo feature **`wav`** (`hound`). For WAV + FLAC and heavier export, see the
//! **`export`** feature on **`trem-dsp`** ([`trem_dsp::export`]).
//!
//! [`trem_dsp::export`]: https://docs.rs/trem-dsp/latest/trem_dsp/export/index.html

use std::fmt;
use std::path::Path;

/// Failure writing [`write_wav_f32`] or [`write_stereo_wav_f32`].
#[derive(Debug)]
pub enum WavError {
    Io(std::io::Error),
    Encode(hound::Error),
    /// e.g. mismatched channel lengths.
    Invalid(String),
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WavError::Io(e) => write!(f, "{e}"),
            WavError::Encode(e) => write!(f, "{e}"),
            WavError::Invalid(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for WavError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WavError::Io(e) => Some(e),
            WavError::Encode(e) => Some(e),
            WavError::Invalid(_) => None,
        }
    }
}

impl From<std::io::Error> for WavError {
    fn from(e: std::io::Error) -> Self {
        WavError::Io(e)
    }
}

impl From<hound::Error> for WavError {
    fn from(e: hound::Error) -> Self {
        WavError::Encode(e)
    }
}

fn ensure_same_len(channels: &[Vec<f32>]) -> Result<usize, WavError> {
    let n = channels.first().map(|c| c.len()).unwrap_or(0);
    if channels.iter().any(|c| c.len() != n) {
        return Err(WavError::Invalid(
            "all channels must have the same length".into(),
        ));
    }
    Ok(n)
}

/// Writes `channels` as one interleaved float WAV (`ch0[0], ch1[0], …` per frame).
pub fn write_wav_f32(path: &Path, channels: &[Vec<f32>], sample_rate: u32) -> Result<(), WavError> {
    let n = ensure_same_len(channels)?;
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(WavError::Invalid(format!(
            "unsupported channel count {n_ch}"
        )));
    }
    let spec = hound::WavSpec {
        channels: n_ch as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for i in 0..n {
        for ch in channels {
            w.write_sample(ch[i])?;
        }
    }
    w.finalize()?;
    Ok(())
}

/// Stereo shortcut: interleaved float WAV without extra channel buffers.
pub fn write_stereo_wav_f32(
    path: &Path,
    left: &[f32],
    right: &[f32],
    sample_rate: u32,
) -> Result<(), WavError> {
    if left.len() != right.len() {
        return Err(WavError::Invalid("left/right length mismatch".into()));
    }
    let spec = hound::WavSpec {
        channels: 2,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };
    let mut w = hound::WavWriter::create(path, spec)?;
    for i in 0..left.len() {
        w.write_sample(left[i])?;
        w.write_sample(right[i])?;
    }
    w.finalize()?;
    Ok(())
}
