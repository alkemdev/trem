//! IEEE **32-bit float** WAV helpers.
//!
//! Requires feature **`audio`** on **`trem-mio`** (default). Prefer [`crate::audio`] for planar I/O, FLAC, and streaming load.
//!
//! These functions delegate to [`crate::audio`] to avoid duplicate encode paths.

use std::fmt;
use std::path::Path;

use crate::audio::{write_planar_to_file, AudioError, AudioFormat};
use trem::signal::SampleRateHz;

/// Failure writing [`write_wav_f32`] or [`write_stereo_wav_f32`].
#[derive(Debug)]
pub enum WavError {
    Io(std::io::Error),
    Encode(String),
    Invalid(String),
}

impl fmt::Display for WavError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            WavError::Io(e) => write!(f, "{e}"),
            WavError::Encode(s) | WavError::Invalid(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for WavError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            WavError::Io(e) => Some(e),
            WavError::Encode(_) | WavError::Invalid(_) => None,
        }
    }
}

impl From<std::io::Error> for WavError {
    fn from(e: std::io::Error) -> Self {
        WavError::Io(e)
    }
}

impl From<AudioError> for WavError {
    fn from(e: AudioError) -> Self {
        match e {
            AudioError::Io(io) => WavError::Io(io),
            other => WavError::Encode(other.to_string()),
        }
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
    let _n = ensure_same_len(channels)?;
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(WavError::Invalid(format!(
            "unsupported channel count {n_ch}"
        )));
    }
    let rate = SampleRateHz::new(sample_rate)
        .ok_or_else(|| WavError::Invalid("sample rate must be positive".into()))?;
    write_planar_to_file(path, AudioFormat::Wav, rate, channels)?;
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
    let channels = vec![left.to_vec(), right.to_vec()];
    write_wav_f32(path, &channels, sample_rate)
}
