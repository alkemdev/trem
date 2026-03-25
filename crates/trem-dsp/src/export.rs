//! Write interleaved multi-channel `f32` PCM to **WAV** (IEEE float) or **FLAC** (16-bit).
//!
//! Requires crate feature **`export`** (`hound` + `flacenc`).
//!
//! # Examples
//!
//! ```ignore
//! use std::path::Path;
//! use trem_dsp::export::write_audio_file;
//!
//! let channels = vec![vec![0.0f32; 1024]; 2];
//! write_audio_file(Path::new("out.wav"), &channels, 48_000)?;
//! write_audio_file(Path::new("out.flac"), &channels, 48_000)?;
//! ```

use std::fmt;
use std::path::Path;

/// Failure while writing [`write_audio_file`].
#[derive(Debug)]
pub enum ExportError {
    /// OS or file error.
    Io(std::io::Error),
    /// WAV writer error.
    Wav(hound::Error),
    /// FLAC encoder or configuration error.
    Flac(String),
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ExportError::Io(e) => write!(f, "{e}"),
            ExportError::Wav(e) => write!(f, "{e}"),
            ExportError::Flac(s) => f.write_str(s),
        }
    }
}

impl std::error::Error for ExportError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ExportError::Io(e) => Some(e),
            ExportError::Wav(e) => Some(e),
            ExportError::Flac(_) => None,
        }
    }
}

impl From<std::io::Error> for ExportError {
    fn from(e: std::io::Error) -> Self {
        ExportError::Io(e)
    }
}

impl From<hound::Error> for ExportError {
    fn from(e: hound::Error) -> Self {
        ExportError::Wav(e)
    }
}

fn ensure_uniform_length(channels: &[Vec<f32>]) -> Result<usize, ExportError> {
    let len = channels.first().map(|c| c.len()).unwrap_or(0);
    if channels.iter().any(|c| c.len() != len) {
        return Err(ExportError::Flac(
            "all channels must have the same length".into(),
        ));
    }
    Ok(len)
}

/// Writes `channels` as **32-bit float WAV** (interleaved).
pub fn write_wav_f32(
    path: &Path,
    channels: &[Vec<f32>],
    sample_rate: u32,
) -> Result<(), ExportError> {
    let n = ensure_uniform_length(channels)?;
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(ExportError::Flac(format!(
            "unsupported channel count: {n_ch}"
        )));
    }

    let spec = hound::WavSpec {
        channels: n_ch as u16,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec)?;
    for i in 0..n {
        for ch in channels {
            writer.write_sample(ch[i])?;
        }
    }
    writer.finalize()?;
    Ok(())
}

fn f32_to_i16_i32(s: f32) -> i32 {
    let x = (s.clamp(-1.0, 1.0) * 32767.0).round() as i32;
    x.clamp(-32_768, 32_767)
}

/// Encodes `channels` as **16-bit FLAC** (interleaved).
pub fn write_flac_f16(
    path: &Path,
    channels: &[Vec<f32>],
    sample_rate: u32,
) -> Result<(), ExportError> {
    use flacenc::bitsink::ByteSink;
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let n = ensure_uniform_length(channels)?;
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > 256 {
        return Err(ExportError::Flac(format!(
            "unsupported channel count: {n_ch}"
        )));
    }

    let mut interleaved: Vec<i32> = Vec::with_capacity(n * n_ch);
    for i in 0..n {
        for ch in channels {
            interleaved.push(f32_to_i16_i32(ch[i]));
        }
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| ExportError::Flac(format!("{e:?}")))?;

    let source =
        flacenc::source::MemSource::from_samples(&interleaved, n_ch, 16, sample_rate as usize);

    let block_size = config.block_size;
    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, block_size)
        .map_err(|e| ExportError::Flac(format!("{e:?}")))?;

    let mut sink = ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| ExportError::Flac(format!("{e:?}")))?;

    std::fs::write(path, sink.as_slice())?;
    Ok(())
}

/// Writes `channels` to `path`, choosing the format from the extension (`.wav` or `.flac`, case-insensitive).
pub fn write_audio_file(
    path: &Path,
    channels: &[Vec<f32>],
    sample_rate: u32,
) -> Result<(), ExportError> {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .map(|s| s.to_ascii_lowercase());

    match ext.as_deref() {
        Some("wav") => write_wav_f32(path, channels, sample_rate),
        Some("flac") => write_flac_f16(path, channels, sample_rate),
        _ => Err(ExportError::Flac(
            "output extension must be .wav or .flac".into(),
        )),
    }
}
