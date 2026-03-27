//! Planar **f32** load/save with format **auto-detection** from the file extension (path APIs) or an
//! explicit codec (buffer APIs).
//!
//! Requires Cargo feature **`audio`** on **`trem-mio`** (`hound`, `flacenc`, `claxon`), enabled by default.
//!
//! **Native / WASI:** use [`AudioWriter::open`], [`AudioReader::open`], and [`write_planar_to_file`].
//!
//! **`wasm32` without WASI** (typical browser): there is no real filesystem — use
//! [`AudioWriter::open_memory`] then [`AudioWriter::done_into_vec`], plus [`AudioReader::from_bytes`]
//! or [`AudioReader::from_bytes_with_name_hint`], and [`write_planar_to_vec`].
//!
//! - **Save**: [`AudioWriter`] — **WAV** (IEEE float) is written incrementally to a file, or buffered
//!   for in-memory encoding. **FLAC** (16-bit PCM) buffers all PCM until [`AudioWriter::done`] /
//!   [`AudioWriter::done_into_vec`]. If you drop a **file** **WAV** writer without `done()`, `Drop`
//!   still finalizes (errors ignored). **FLAC** and **memory** **WAV** must finish with `done()` /
//!   `done_into_vec`; dropping without that panics.
//! - **Load**: [`AudioReader`] — pulls **WAV** / **FLAC** through a decode cursor with a small
//!   interleaved scratch buffer; peak memory is not the full decoded PCM.
//!
//! On disk, PCM is interleaved; this module translates to/from planar `&[&[f32]]` / `&mut [Vec<f32>]`.
//!
//! # Examples
//!
//! ```ignore
//! use std::path::Path;
//! use trem_mio::audio::{AudioFormat, AudioReader, AudioWriter};
//! use trem::signal::{ChannelCount, SampleRateHz};
//!
//! let left = vec![0.0f32; 64];
//! let right = vec![1.0f32; 64];
//! let planar = [left.as_slice(), right.as_slice()];
//!
//! let rate = SampleRateHz::new(48_000).unwrap();
//! let ch = ChannelCount::new(2).unwrap();
//! let mut writer = AudioWriter::open(Path::new("t.wav"), AudioFormat::Wav, rate, ch)?;
//! writer.feed(&planar)?;
//! writer.done()?;
//!
//! let mut reader = AudioReader::open(Path::new("t.wav"), AudioFormat::Auto)?;
//! let mut bufs = vec![Vec::new(), Vec::new()];
//! let _ = reader.take(128, &mut bufs)?;
//! ```

use std::fmt;
use std::fs::File;
use std::io::{BufReader, BufWriter, Cursor, Read, Seek};
use std::path::{Path, PathBuf};

use trem::signal::{ChannelCount, SampleRateHz};

/// Output / input container and codec (extension mapping for [`AudioFormat::Auto`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AudioFormat {
    /// Infer from the path extension (`.wav`, `.flac`; case-insensitive).
    Auto,
    /// IEEE 32-bit float WAV.
    Wav,
    /// FLAC with 16-bit PCM (`f32` is quantized at encode time).
    Flac,
}

/// Failure from [`AudioWriter`] or [`AudioReader`].
#[derive(Debug)]
pub enum AudioError {
    /// OS or file error.
    Io(std::io::Error),
    /// WAV read/write error from `hound`.
    Wav(hound::Error),
    /// FLAC encode error (flacenc).
    FlacEncode(String),
    /// FLAC decode error (claxon).
    FlacDecode(claxon::Error),
    /// Unsupported format, layout, or sample type.
    Unsupported(String),
}

impl fmt::Display for AudioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AudioError::Io(e) => write!(f, "{e}"),
            AudioError::Wav(e) => write!(f, "{e}"),
            AudioError::FlacEncode(s) | AudioError::Unsupported(s) => f.write_str(s),
            AudioError::FlacDecode(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for AudioError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            AudioError::Io(e) => Some(e),
            AudioError::Wav(e) => Some(e),
            AudioError::FlacDecode(e) => Some(e),
            AudioError::FlacEncode(_) | AudioError::Unsupported(_) => None,
        }
    }
}

impl From<std::io::Error> for AudioError {
    fn from(e: std::io::Error) -> Self {
        AudioError::Io(e)
    }
}

impl From<hound::Error> for AudioError {
    fn from(e: hound::Error) -> Self {
        AudioError::Wav(e)
    }
}

impl From<claxon::Error> for AudioError {
    fn from(e: claxon::Error) -> Self {
        AudioError::FlacDecode(e)
    }
}

fn resolve_extension_format(path: &Path) -> Result<AudioFormat, AudioError> {
    resolve_extension_string(path.extension().and_then(|s| s.to_str()))
}

fn resolve_extension_string(ext: Option<&str>) -> Result<AudioFormat, AudioError> {
    let Some(ext) = ext else {
        return Err(AudioError::Unsupported(
            "path has no extension; use AudioFormat::Wav or ::Flac, or add .wav / .flac".into(),
        ));
    };
    match ext.to_ascii_lowercase().as_str() {
        "wav" => Ok(AudioFormat::Wav),
        "flac" | "fla" => Ok(AudioFormat::Flac),
        other => Err(AudioError::Unsupported(format!(
            "unknown audio extension .{other} (supported: .wav, .flac)"
        ))),
    }
}

fn effective_save_format(path: &Path, format: AudioFormat) -> Result<AudioFormat, AudioError> {
    match format {
        AudioFormat::Auto => resolve_extension_format(path),
        f => Ok(f),
    }
}

fn effective_load_format(path: &Path, format: AudioFormat) -> Result<AudioFormat, AudioError> {
    match format {
        AudioFormat::Auto => resolve_extension_format(path),
        f => Ok(f),
    }
}

fn validate_planar_lengths(channels: &[&[f32]], n_ch: usize) -> Result<usize, AudioError> {
    if channels.len() != n_ch {
        return Err(AudioError::Unsupported(format!(
            "expected {n_ch} channel buffer(s), got {}",
            channels.len()
        )));
    }
    let n = channels.first().map(|c| c.len()).unwrap_or(0);
    if channels.iter().any(|c| c.len() != n) {
        return Err(AudioError::Unsupported(
            "all planar channel slices must have the same length".into(),
        ));
    }
    Ok(n)
}

/// Incremental writer: planar **f32** in, interleaved on disk after [`Self::done`] (or on `Drop` for WAV).
#[must_use = "call done() to finish the file (required for FLAC); WAV is finalized on drop if needed"]
pub struct AudioWriter {
    sample_rate: SampleRateHz,
    channels: ChannelCount,
    backend: Option<SaveBackend>,
}

enum SaveBackend {
    Wav(hound::WavWriter<BufWriter<File>>),
    /// In-memory WAV: planar samples buffered until [`AudioWriter::done_into_vec`].
    WavBuffered {
        bufs: Vec<Vec<f32>>,
    },
    FlacBuf {
        bufs: Vec<Vec<f32>>,
        path: PathBuf,
    },
    /// In-memory FLAC: planar samples buffered until [`AudioWriter::done_into_vec`].
    FlacBufMem {
        bufs: Vec<Vec<f32>>,
    },
}

impl AudioWriter {
    /// Opens `path` for writing.
    pub fn open(
        path: &Path,
        format: AudioFormat,
        sample_rate: SampleRateHz,
        channels: ChannelCount,
    ) -> Result<Self, AudioError> {
        let sr_u32 = sample_rate.get();
        let ch_u16 = channels.get();
        let fmt = effective_save_format(path, format)?;
        let backend = match fmt {
            AudioFormat::Auto => unreachable!("effective_save_format resolves Auto"),
            AudioFormat::Wav => {
                let spec = hound::WavSpec {
                    channels: ch_u16,
                    sample_rate: sr_u32,
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };
                let w = hound::WavWriter::create(path, spec)?;
                SaveBackend::Wav(w)
            }
            AudioFormat::Flac => {
                if ch_u16 > ChannelCount::MAX_FLAC {
                    return Err(AudioError::FlacEncode(format!(
                        "FLAC supports at most {} channels, got {ch_u16}",
                        ChannelCount::MAX_FLAC
                    )));
                }
                SaveBackend::FlacBuf {
                    bufs: (0..ch_u16).map(|_| Vec::new()).collect(),
                    path: path.to_path_buf(),
                }
            }
        };
        Ok(Self {
            sample_rate,
            channels,
            backend: Some(backend),
        })
    }

    /// In-memory writer for **WAV** or **FLAC** (no `std::fs`; suitable for `wasm32` in the browser).
    ///
    /// `format` must be [`AudioFormat::Wav`] or [`AudioFormat::Flac`], not [`AudioFormat::Auto`].
    /// Finish with [`Self::done_into_vec`].
    pub fn open_memory(
        format: AudioFormat,
        sample_rate: SampleRateHz,
        channels: ChannelCount,
    ) -> Result<Self, AudioError> {
        if matches!(format, AudioFormat::Auto) {
            return Err(AudioError::Unsupported(
                "open_memory requires AudioFormat::Wav or ::Flac".into(),
            ));
        }
        let ch_u16 = channels.get();
        let backend = match format {
            AudioFormat::Auto => unreachable!(),
            AudioFormat::Wav => SaveBackend::WavBuffered {
                bufs: (0..ch_u16).map(|_| Vec::new()).collect(),
            },
            AudioFormat::Flac => {
                if ch_u16 > ChannelCount::MAX_FLAC {
                    return Err(AudioError::FlacEncode(format!(
                        "FLAC supports at most {} channels, got {ch_u16}",
                        ChannelCount::MAX_FLAC
                    )));
                }
                SaveBackend::FlacBufMem {
                    bufs: (0..ch_u16).map(|_| Vec::new()).collect(),
                }
            }
        };
        Ok(Self {
            sample_rate,
            channels,
            backend: Some(backend),
        })
    }

    /// Sample rate passed at [`Self::open`].
    pub fn sample_rate_hz(&self) -> SampleRateHz {
        self.sample_rate
    }

    /// Raw sample rate in Hz (convenience).
    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.get()
    }

    /// Channel count fixed at [`Self::open`].
    pub fn channel_count(&self) -> ChannelCount {
        self.channels
    }

    /// Appends one block of **planar** frames.
    pub fn feed(&mut self, channels: &[&[f32]]) -> Result<(), AudioError> {
        let frames = validate_planar_lengths(channels, self.channels.as_usize())?;
        let backend = self
            .backend
            .as_mut()
            .expect("AudioWriter: backend must be present until done() or drop");
        match backend {
            SaveBackend::Wav(w) => {
                for i in 0..frames {
                    for ch in channels {
                        w.write_sample(ch[i])?;
                    }
                }
                Ok(())
            }
            SaveBackend::WavBuffered { bufs }
            | SaveBackend::FlacBuf { bufs, .. }
            | SaveBackend::FlacBufMem { bufs } => {
                for i in 0..frames {
                    for (buf, slice) in bufs.iter_mut().zip(channels.iter()) {
                        buf.push(slice[i]);
                    }
                }
                Ok(())
            }
        }
    }

    /// Finalizes a **file** writer opened with [`Self::open`]. Consumes `self`.
    ///
    /// For [`Self::open_memory`], use [`Self::done_into_vec`] instead.
    pub fn done(mut self) -> Result<(), AudioError> {
        let backend = self
            .backend
            .take()
            .ok_or_else(|| AudioError::Unsupported("AudioWriter::done called twice".into()))?;
        match backend {
            SaveBackend::Wav(w) => {
                w.finalize()?;
                Ok(())
            }
            SaveBackend::FlacBuf { bufs, path } => {
                write_flac_from_planar_bufs(&path, &bufs, self.sample_rate.get())
            }
            SaveBackend::WavBuffered { .. } | SaveBackend::FlacBufMem { .. } => Err(
                AudioError::Unsupported("memory writer: use done_into_vec()".into()),
            ),
        }
    }

    /// Finalizes an in-memory writer from [`Self::open_memory`] and returns encoded bytes.
    pub fn done_into_vec(mut self) -> Result<Vec<u8>, AudioError> {
        let backend = self.backend.take().ok_or_else(|| {
            AudioError::Unsupported("AudioWriter::done_into_vec called twice".into())
        })?;
        let sr = self.sample_rate.get();
        match backend {
            SaveBackend::WavBuffered { bufs } => write_wav_ieee_f32_planar_to_vec(&bufs, sr),
            SaveBackend::FlacBufMem { bufs } => flac_encode_planar_to_vec(&bufs, sr),
            SaveBackend::Wav(_) | SaveBackend::FlacBuf { .. } => Err(AudioError::Unsupported(
                "done_into_vec is only for open_memory writers".into(),
            )),
        }
    }
}

impl Drop for AudioWriter {
    fn drop(&mut self) {
        let Some(backend) = self.backend.take() else {
            return;
        };
        if std::thread::panicking() {
            return;
        }
        match backend {
            SaveBackend::Wav(w) => {
                let _ = w.finalize();
            }
            SaveBackend::FlacBuf { .. } | SaveBackend::FlacBufMem { .. } => {
                panic!(
                    "AudioWriter: FLAC output requires done() or done_into_vec() before drop; \
                     buffered PCM was not written"
                );
            }
            SaveBackend::WavBuffered { .. } => {
                panic!(
                    "AudioWriter: in-memory WAV requires done_into_vec() before drop; \
                     buffered PCM was not written"
                );
            }
        }
    }
}

/// Writes planar `channels` in one shot (`open` → [`AudioWriter::feed`] → [`AudioWriter::done`]).
pub fn write_planar_to_file(
    path: &Path,
    format: AudioFormat,
    sample_rate: SampleRateHz,
    channels: &[Vec<f32>],
) -> Result<(), AudioError> {
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(AudioError::Unsupported(format!(
            "unsupported channel count: {n_ch}"
        )));
    }
    let n0 = channels.first().map(|c| c.len()).unwrap_or(0);
    if channels.iter().any(|c| c.len() != n0) {
        return Err(AudioError::Unsupported(
            "all planar channels must have the same length".into(),
        ));
    }
    let ch = ChannelCount::new(n_ch as u16)
        .ok_or_else(|| AudioError::Unsupported("invalid channel count".into()))?;
    let slices: Vec<&[f32]> = channels.iter().map(|v| v.as_slice()).collect();
    let mut io = AudioWriter::open(path, format, sample_rate, ch)?;
    io.feed(&slices)?;
    io.done()
}

/// One-shot encode of planar **f32** to a **WAV** or **FLAC** byte vector (no `std::fs`).
///
/// `format` must be [`AudioFormat::Wav`] or [`AudioFormat::Flac`], not [`AudioFormat::Auto`].
pub fn write_planar_to_vec(
    format: AudioFormat,
    sample_rate: SampleRateHz,
    channels: &[Vec<f32>],
) -> Result<Vec<u8>, AudioError> {
    if matches!(format, AudioFormat::Auto) {
        return Err(AudioError::Unsupported(
            "write_planar_to_vec requires AudioFormat::Wav or ::Flac".into(),
        ));
    }
    let n_ch = channels.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(AudioError::Unsupported(format!(
            "unsupported channel count: {n_ch}"
        )));
    }
    let n0 = channels.first().map(|c| c.len()).unwrap_or(0);
    if channels.iter().any(|c| c.len() != n0) {
        return Err(AudioError::Unsupported(
            "all planar channels must have the same length".into(),
        ));
    }
    let ch = ChannelCount::new(n_ch as u16)
        .ok_or_else(|| AudioError::Unsupported("invalid channel count".into()))?;
    let slices: Vec<&[f32]> = channels.iter().map(|v| v.as_slice()).collect();
    let mut io = AudioWriter::open_memory(format, sample_rate, ch)?;
    io.feed(&slices)?;
    io.done_into_vec()
}

fn f32_to_i16_i32(s: f32) -> i32 {
    let x = (s.clamp(-1.0, 1.0) * 32767.0).round() as i32;
    x.clamp(-32_768, 32_767)
}

/// IEEE float **WAV** (format tag 3), interleaved from planar `bufs`.
fn write_wav_ieee_f32_planar_to_vec(
    bufs: &[Vec<f32>],
    sample_rate: u32,
) -> Result<Vec<u8>, AudioError> {
    let n_ch = bufs.len();
    if n_ch == 0 || n_ch > u16::MAX as usize {
        return Err(AudioError::Unsupported(format!(
            "unsupported channel count: {n_ch}"
        )));
    }
    let n = bufs.first().map(|c| c.len()).unwrap_or(0);
    if bufs.iter().any(|c| c.len() != n) {
        return Err(AudioError::Unsupported(
            "all planar channels must have the same length".into(),
        ));
    }
    let n_ch_u16 = n_ch as u16;
    let block_align = n_ch_u16.saturating_mul(4);
    let byte_rate = sample_rate
        .checked_mul(block_align as u32)
        .ok_or_else(|| AudioError::Unsupported("byte_rate overflow".into()))?;
    let data_bytes = n
        .checked_mul(n_ch)
        .and_then(|x| x.checked_mul(4))
        .ok_or_else(|| AudioError::Unsupported("data size overflow".into()))?;
    let mut out = Vec::with_capacity(12 + 8 + 16 + 8 + data_bytes);
    out.extend_from_slice(b"RIFF");
    let riff_len_pos = out.len();
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(b"WAVE");
    out.extend_from_slice(b"fmt ");
    out.extend_from_slice(&16u32.to_le_bytes());
    out.extend_from_slice(&3u16.to_le_bytes());
    out.extend_from_slice(&n_ch_u16.to_le_bytes());
    out.extend_from_slice(&sample_rate.to_le_bytes());
    out.extend_from_slice(&byte_rate.to_le_bytes());
    out.extend_from_slice(&block_align.to_le_bytes());
    out.extend_from_slice(&32u16.to_le_bytes());
    out.extend_from_slice(b"data");
    out.extend_from_slice(&(data_bytes as u32).to_le_bytes());
    for i in 0..n {
        for ch in bufs {
            out.extend_from_slice(&ch[i].to_bits().to_le_bytes());
        }
    }
    let file_len: u32 = out
        .len()
        .try_into()
        .map_err(|_| AudioError::Unsupported("WAV file size overflow".into()))?;
    let riff_chunk = file_len
        .checked_sub(8)
        .ok_or_else(|| AudioError::Unsupported("invalid RIFF size".into()))?;
    out[riff_len_pos..riff_len_pos + 4].copy_from_slice(&riff_chunk.to_le_bytes());
    Ok(out)
}

fn flac_encode_planar_to_vec(bufs: &[Vec<f32>], sample_rate: u32) -> Result<Vec<u8>, AudioError> {
    use flacenc::bitsink::ByteSink;
    use flacenc::component::BitRepr;
    use flacenc::error::Verify;

    let n_ch = bufs.len();
    if n_ch == 0 || n_ch > 256 {
        return Err(AudioError::FlacEncode(format!(
            "unsupported channel count: {n_ch}"
        )));
    }
    let n = bufs.first().map(|c| c.len()).unwrap_or(0);
    if bufs.iter().any(|c| c.len() != n) {
        return Err(AudioError::Unsupported(
            "all planar channels must have the same length for FLAC".into(),
        ));
    }

    let mut interleaved: Vec<i32> = Vec::with_capacity(n * n_ch);
    for i in 0..n {
        for ch in bufs {
            interleaved.push(f32_to_i16_i32(ch[i]));
        }
    }

    let config = flacenc::config::Encoder::default()
        .into_verified()
        .map_err(|(_, e)| AudioError::FlacEncode(format!("{e:?}")))?;

    let source =
        flacenc::source::MemSource::from_samples(&interleaved, n_ch, 16, sample_rate as usize);

    let block_size = config.block_size;
    let flac_stream = flacenc::encode_with_fixed_block_size(&config, source, block_size)
        .map_err(|e| AudioError::FlacEncode(format!("{e:?}")))?;

    let mut sink = ByteSink::new();
    flac_stream
        .write(&mut sink)
        .map_err(|e| AudioError::FlacEncode(format!("{e:?}")))?;

    Ok(sink.as_slice().to_vec())
}

fn write_flac_from_planar_bufs(
    path: &Path,
    bufs: &[Vec<f32>],
    sample_rate: u32,
) -> Result<(), AudioError> {
    let bytes = flac_encode_planar_to_vec(bufs, sample_rate)?;
    std::fs::write(path, bytes)?;
    Ok(())
}

// --- Streaming load -----------------------------------------------------------------------------

fn wav_refill_float<R: Read + Seek>(
    reader: &mut hound::WavReader<R>,
    spec: &hound::WavSpec,
    pending: &mut Vec<f32>,
    eof: &mut bool,
    min_interleaved: usize,
) -> Result<(), AudioError> {
    if *eof || pending.len() >= min_interleaved {
        return Ok(());
    }
    let mut samples = reader.samples::<f32>();
    while pending.len() < min_interleaved {
        match samples.next() {
            Some(Ok(s)) => pending.push(s),
            Some(Err(e)) => return Err(e.into()),
            None => {
                *eof = true;
                break;
            }
        }
    }
    let n_ch = spec.channels as usize;
    if !pending.is_empty() && pending.len() % n_ch != 0 {
        return Err(AudioError::Unsupported(
            "WAV sample count is not a multiple of channel count".into(),
        ));
    }
    Ok(())
}

fn wav_refill_int<R: Read + Seek>(
    reader: &mut hound::WavReader<R>,
    spec: &hound::WavSpec,
    scale: f32,
    pending: &mut Vec<f32>,
    eof: &mut bool,
    min_interleaved: usize,
) -> Result<(), AudioError> {
    if *eof || pending.len() >= min_interleaved {
        return Ok(());
    }
    let mut samples = reader.samples::<i32>();
    while pending.len() < min_interleaved {
        match samples.next() {
            Some(Ok(v)) => pending.push(v as f32 / scale),
            Some(Err(e)) => return Err(e.into()),
            None => {
                *eof = true;
                break;
            }
        }
    }
    let n_ch = spec.channels as usize;
    if !pending.is_empty() && pending.len() % n_ch != 0 {
        return Err(AudioError::Unsupported(
            "WAV sample count is not a multiple of channel count".into(),
        ));
    }
    Ok(())
}

fn flac_refill<R: Read>(
    reader: &mut claxon::FlacReader<R>,
    channels: u16,
    scale: f32,
    pending: &mut Vec<f32>,
    eof: &mut bool,
    min_interleaved: usize,
) -> Result<(), AudioError> {
    if *eof || pending.len() >= min_interleaved {
        return Ok(());
    }
    let n_ch = channels as usize;
    let mut samples = reader.samples();
    while pending.len() < min_interleaved {
        match samples.next() {
            Some(Ok(v)) => pending.push(v as f32 / scale),
            Some(Err(e)) => return Err(e.into()),
            None => {
                *eof = true;
                break;
            }
        }
    }
    if !pending.is_empty() && pending.len() % n_ch != 0 {
        return Err(AudioError::Unsupported(
            "FLAC sample count is not a multiple of channel count".into(),
        ));
    }
    Ok(())
}

enum LoadInner {
    WavFloat {
        reader: hound::WavReader<BufReader<File>>,
        spec: hound::WavSpec,
        pending: Vec<f32>,
        eof: bool,
    },
    WavFloatMem {
        reader: hound::WavReader<BufReader<Cursor<Vec<u8>>>>,
        spec: hound::WavSpec,
        pending: Vec<f32>,
        eof: bool,
    },
    WavInt {
        reader: hound::WavReader<BufReader<File>>,
        spec: hound::WavSpec,
        scale: f32,
        pending: Vec<f32>,
        eof: bool,
    },
    WavIntMem {
        reader: hound::WavReader<BufReader<Cursor<Vec<u8>>>>,
        spec: hound::WavSpec,
        scale: f32,
        pending: Vec<f32>,
        eof: bool,
    },
    Flac {
        reader: claxon::FlacReader<File>,
        channels: u16,
        scale: f32,
        pending: Vec<f32>,
        eof: bool,
    },
    FlacMem {
        reader: claxon::FlacReader<Cursor<Vec<u8>>>,
        channels: u16,
        scale: f32,
        pending: Vec<f32>,
        eof: bool,
    },
}

impl LoadInner {
    fn refill(&mut self, min_interleaved: usize) -> Result<(), AudioError> {
        match self {
            LoadInner::WavFloat {
                reader,
                spec,
                pending,
                eof,
            } => wav_refill_float(reader, spec, pending, eof, min_interleaved),
            LoadInner::WavFloatMem {
                reader,
                spec,
                pending,
                eof,
            } => wav_refill_float(reader, spec, pending, eof, min_interleaved),
            LoadInner::WavInt {
                reader,
                spec,
                scale,
                pending,
                eof,
            } => wav_refill_int(reader, spec, *scale, pending, eof, min_interleaved),
            LoadInner::WavIntMem {
                reader,
                spec,
                scale,
                pending,
                eof,
            } => wav_refill_int(reader, spec, *scale, pending, eof, min_interleaved),
            LoadInner::Flac {
                reader,
                channels,
                scale,
                pending,
                eof,
            } => flac_refill(reader, *channels, *scale, pending, eof, min_interleaved),
            LoadInner::FlacMem {
                reader,
                channels,
                scale,
                pending,
                eof,
            } => flac_refill(reader, *channels, *scale, pending, eof, min_interleaved),
        }
    }

    fn pending_frames(&self, ch: usize) -> usize {
        let pend = match self {
            LoadInner::WavFloat { pending, .. }
            | LoadInner::WavFloatMem { pending, .. }
            | LoadInner::WavInt { pending, .. }
            | LoadInner::WavIntMem { pending, .. }
            | LoadInner::Flac { pending, .. }
            | LoadInner::FlacMem { pending, .. } => pending.len(),
        };
        pend / ch
    }

    fn drain_frames(&mut self, n: usize, ch: usize, out: &mut [Vec<f32>]) {
        let take_i = n * ch;
        let slice = &self.pending_mut()[..take_i];
        for f in 0..n {
            for c in 0..ch {
                out[c].push(slice[f * ch + c]);
            }
        }
        self.pending_mut().drain(0..take_i);
    }

    fn pending_mut(&mut self) -> &mut Vec<f32> {
        match self {
            LoadInner::WavFloat { pending, .. }
            | LoadInner::WavFloatMem { pending, .. }
            | LoadInner::WavInt { pending, .. }
            | LoadInner::WavIntMem { pending, .. }
            | LoadInner::Flac { pending, .. }
            | LoadInner::FlacMem { pending, .. } => pending,
        }
    }

    fn eof(&self) -> bool {
        match self {
            LoadInner::WavFloat { eof, .. }
            | LoadInner::WavFloatMem { eof, .. }
            | LoadInner::WavInt { eof, .. }
            | LoadInner::WavIntMem { eof, .. }
            | LoadInner::Flac { eof, .. }
            | LoadInner::FlacMem { eof, .. } => *eof,
        }
    }
}

/// Streaming decoder: planar output via [`Self::take`].
pub struct AudioReader {
    sample_rate: SampleRateHz,
    channels: ChannelCount,
    inner: LoadInner,
    frames_out: u64,
    frames_total: Option<u64>,
}

impl AudioReader {
    /// Opens `path` for reading (headers + decode cursor only; PCM is pulled on [`Self::take`]).
    pub fn open(path: &Path, format: AudioFormat) -> Result<Self, AudioError> {
        let fmt = effective_load_format(path, format)?;
        match fmt {
            AudioFormat::Auto => unreachable!(),
            AudioFormat::Wav => open_wav_streaming(path),
            AudioFormat::Flac => open_flac_streaming(path),
        }
    }

    /// Decode from an in-memory buffer (e.g. `wasm32` in the browser). Use [`AudioFormat::Wav`]
    /// or [`AudioFormat::Flac`]; for [`AudioFormat::Auto`], use [`Self::from_bytes_with_name_hint`].
    pub fn from_bytes(data: Vec<u8>, format: AudioFormat) -> Result<Self, AudioError> {
        match format {
            AudioFormat::Auto => Err(AudioError::Unsupported(
                "from_bytes requires ::Wav or ::Flac (or use from_bytes_with_name_hint)".into(),
            )),
            AudioFormat::Wav => open_wav_streaming_cursor(data),
            AudioFormat::Flac => open_flac_streaming_cursor(data),
        }
    }

    /// Like [`Self::from_bytes`], but infers WAV vs FLAC from `name`'s extension (e.g. `clip.wav`).
    pub fn from_bytes_with_name_hint(data: Vec<u8>, name: &str) -> Result<Self, AudioError> {
        let fmt = resolve_extension_format(Path::new(name))?;
        Self::from_bytes(data, fmt)
    }

    pub fn sample_rate_hz(&self) -> SampleRateHz {
        self.sample_rate
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate.get()
    }

    pub fn channel_count(&self) -> ChannelCount {
        self.channels
    }

    /// Full frames not yet consumed by [`Self::take`] (including decoded samples still in the scratch buffer).
    ///
    /// [`None`] if the file did not report a total frame count and the stream has not ended yet.
    pub fn frames_remaining(&self) -> Option<u64> {
        let ch = self.channels.as_usize();
        let pending_f = self.inner.pending_frames(ch) as u64;
        let decoded = self.frames_out.saturating_add(pending_f);
        match self.frames_total {
            Some(t) => Some(t.saturating_sub(decoded)),
            None => {
                if self.inner.eof() && pending_f == 0 {
                    Some(0)
                } else {
                    None
                }
            }
        }
    }

    /// Pulls up to `max_frames` frames into planar `out` (appends to each channel vec).
    pub fn take(&mut self, max_frames: usize, out: &mut [Vec<f32>]) -> Result<usize, AudioError> {
        let ch = self.channels.as_usize();
        if out.len() != ch {
            return Err(AudioError::Unsupported(format!(
                "expected {} channel output vectors, got {}",
                ch,
                out.len()
            )));
        }
        if max_frames == 0 {
            return Ok(0);
        }
        let need = max_frames * ch;
        self.inner.refill(need)?;
        let avail = self.inner.pending_frames(ch).min(max_frames);
        if avail == 0 {
            return Ok(0);
        }
        self.inner.drain_frames(avail, ch, out);
        self.frames_out += avail as u64;
        Ok(avail)
    }

    /// Drains the rest of the stream into planar vectors.
    pub fn into_planar_f32(mut self) -> Result<Vec<Vec<f32>>, AudioError> {
        let ch = self.channels.as_usize();
        let mut out: Vec<Vec<f32>> = (0..ch).map(|_| Vec::new()).collect();
        loop {
            let n = self.take(4096, out.as_mut_slice())?;
            if n == 0 {
                break;
            }
        }
        Ok(out)
    }
}

fn open_wav_streaming(path: &Path) -> Result<AudioReader, AudioError> {
    let reader = hound::WavReader::open(path)?;
    let frames_total = Some(u64::from(reader.duration()));
    let spec = reader.spec();
    let channels_u = spec.channels;
    let channels = u16::try_from(channels_u).map_err(|_| {
        AudioError::Unsupported(format!(
            "WAV channel count {channels_u} does not fit in u16"
        ))
    })?;
    let ch = ChannelCount::new(channels)
        .ok_or_else(|| AudioError::Unsupported("WAV reported zero channels".into()))?;
    let sample_rate = SampleRateHz::new(spec.sample_rate)
        .ok_or_else(|| AudioError::Unsupported("WAV reported zero sample rate".into()))?;

    let inner = match spec.sample_format {
        hound::SampleFormat::Float => LoadInner::WavFloat {
            reader,
            spec,
            pending: Vec::new(),
            eof: false,
        },
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            if bits > 32 {
                return Err(AudioError::Unsupported(format!(
                    "unsupported WAV integer bit depth {bits}"
                )));
            }
            let scale = ((1i64 << (bits.saturating_sub(1))) as f32).max(1.0);
            LoadInner::WavInt {
                reader,
                spec,
                scale,
                pending: Vec::new(),
                eof: false,
            }
        }
    };

    Ok(AudioReader {
        sample_rate,
        channels: ch,
        inner,
        frames_out: 0,
        frames_total,
    })
}

fn open_flac_streaming(path: &Path) -> Result<AudioReader, AudioError> {
    let reader = claxon::FlacReader::open(path)?;
    let info = reader.streaminfo();
    let sample_rate = SampleRateHz::new(info.sample_rate)
        .ok_or_else(|| AudioError::Unsupported("FLAC reported zero sample rate".into()))?;
    let channels_u32 = info.channels;
    let channels = u16::try_from(channels_u32).map_err(|_| {
        AudioError::Unsupported(format!(
            "FLAC channel count {channels_u32} does not fit in u16"
        ))
    })?;
    let ch = ChannelCount::new(channels)
        .ok_or_else(|| AudioError::Unsupported("FLAC reported zero channels".into()))?;
    let bits = info.bits_per_sample;
    if bits == 0 || bits > 32 {
        return Err(AudioError::Unsupported(format!(
            "unsupported FLAC bit depth {bits}"
        )));
    }
    let scale = ((1u64 << (bits - 1)) as f32).max(1.0);
    let frames_total = info.samples.map(|s| {
        let n_ch = channels as u64;
        if n_ch == 0 {
            0
        } else {
            s / n_ch
        }
    });

    let inner = LoadInner::Flac {
        reader,
        channels,
        scale,
        pending: Vec::new(),
        eof: false,
    };

    Ok(AudioReader {
        sample_rate,
        channels: ch,
        inner,
        frames_out: 0,
        frames_total,
    })
}

fn open_wav_streaming_cursor(data: Vec<u8>) -> Result<AudioReader, AudioError> {
    let reader = hound::WavReader::new(BufReader::new(Cursor::new(data)))?;
    let frames_total = Some(u64::from(reader.duration()));
    let spec = reader.spec();
    let channels_u = spec.channels;
    let channels = u16::try_from(channels_u).map_err(|_| {
        AudioError::Unsupported(format!(
            "WAV channel count {channels_u} does not fit in u16"
        ))
    })?;
    let ch = ChannelCount::new(channels)
        .ok_or_else(|| AudioError::Unsupported("WAV reported zero channels".into()))?;
    let sample_rate = SampleRateHz::new(spec.sample_rate)
        .ok_or_else(|| AudioError::Unsupported("WAV reported zero sample rate".into()))?;

    let inner = match spec.sample_format {
        hound::SampleFormat::Float => LoadInner::WavFloatMem {
            reader,
            spec,
            pending: Vec::new(),
            eof: false,
        },
        hound::SampleFormat::Int => {
            let bits = spec.bits_per_sample;
            if bits > 32 {
                return Err(AudioError::Unsupported(format!(
                    "unsupported WAV integer bit depth {bits}"
                )));
            }
            let scale = ((1i64 << (bits.saturating_sub(1))) as f32).max(1.0);
            LoadInner::WavIntMem {
                reader,
                spec,
                scale,
                pending: Vec::new(),
                eof: false,
            }
        }
    };

    Ok(AudioReader {
        sample_rate,
        channels: ch,
        inner,
        frames_out: 0,
        frames_total,
    })
}

fn open_flac_streaming_cursor(data: Vec<u8>) -> Result<AudioReader, AudioError> {
    let reader = claxon::FlacReader::new(Cursor::new(data))?;
    let info = reader.streaminfo();
    let sample_rate = SampleRateHz::new(info.sample_rate)
        .ok_or_else(|| AudioError::Unsupported("FLAC reported zero sample rate".into()))?;
    let channels_u32 = info.channels;
    let channels = u16::try_from(channels_u32).map_err(|_| {
        AudioError::Unsupported(format!(
            "FLAC channel count {channels_u32} does not fit in u16"
        ))
    })?;
    let ch = ChannelCount::new(channels)
        .ok_or_else(|| AudioError::Unsupported("FLAC reported zero channels".into()))?;
    let bits = info.bits_per_sample;
    if bits == 0 || bits > 32 {
        return Err(AudioError::Unsupported(format!(
            "unsupported FLAC bit depth {bits}"
        )));
    }
    let scale = ((1u64 << (bits - 1)) as f32).max(1.0);
    let frames_total = info.samples.map(|s| {
        let n_ch = channels as u64;
        if n_ch == 0 {
            0
        } else {
            s / n_ch
        }
    });

    let inner = LoadInner::FlacMem {
        reader,
        channels,
        scale,
        pending: Vec::new(),
        eof: false,
    };

    Ok(AudioReader {
        sample_rate,
        channels: ch,
        inner,
        frames_out: 0,
        frames_total,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wav_roundtrip_planar() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_audio_wav_test.wav");
        let _ = std::fs::remove_file(&path);

        let l = vec![0.25f32, -0.25, 0.5];
        let r = vec![-0.5f32, 0.125, 0.0];
        let planar = [l.as_slice(), r.as_slice()];

        let rate = SampleRateHz::new(48_000).unwrap();
        let ch = ChannelCount::new(2).unwrap();
        let mut save = AudioWriter::open(&path, AudioFormat::Wav, rate, ch).expect("save open");
        save.feed(&planar).expect("feed");
        save.done().expect("done");

        let mut load = AudioReader::open(&path, AudioFormat::Auto).expect("load open");
        assert_eq!(load.sample_rate(), 48_000);
        assert_eq!(load.channel_count().get(), 2);
        let mut bufs = vec![Vec::new(), Vec::new()];
        let n = load.take(10, &mut bufs).expect("take");
        assert_eq!(n, 3);
        assert_eq!(bufs[0], l);
        assert_eq!(bufs[1], r);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn wav_finalizes_on_drop_without_done() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_audio_wav_drop_test.wav");
        let _ = std::fs::remove_file(&path);

        let l = vec![0.1f32, 0.2];
        let r = vec![0.3f32, 0.4];
        let planar = [l.as_slice(), r.as_slice()];

        {
            let rate = SampleRateHz::new(48_000).unwrap();
            let ch = ChannelCount::new(2).unwrap();
            let mut save = AudioWriter::open(&path, AudioFormat::Wav, rate, ch).expect("open");
            save.feed(&planar).expect("feed");
        }

        let mut load = AudioReader::open(&path, AudioFormat::Auto).expect("load");
        let mut bufs = vec![Vec::new(), Vec::new()];
        assert_eq!(load.take(8, &mut bufs).expect("take"), 2);
        assert_eq!(bufs[0], l);
        assert_eq!(bufs[1], r);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    #[should_panic(expected = "done() or done_into_vec() before drop")]
    fn flac_drop_without_done_panics() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_audio_flac_drop_test.flac");
        let _ = std::fs::remove_file(&path);

        let l = vec![0.0f32; 64];
        let r = vec![0.0f32; 64];
        let planar = [l.as_slice(), r.as_slice()];

        {
            let rate = SampleRateHz::new(48_000).unwrap();
            let ch = ChannelCount::new(2).unwrap();
            let mut save = AudioWriter::open(&path, AudioFormat::Flac, rate, ch).expect("open");
            save.feed(&planar).expect("feed");
        }
    }

    #[test]
    fn flac_roundtrip_planar() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_audio_flac_test.flac");
        let _ = std::fs::remove_file(&path);

        let frames = 512;
        let l: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.001).sin()).collect();
        let r: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.002).cos()).collect();
        let planar = [l.as_slice(), r.as_slice()];

        let rate = SampleRateHz::new(48_000).unwrap();
        let ch = ChannelCount::new(2).unwrap();
        let mut save = AudioWriter::open(&path, AudioFormat::Flac, rate, ch).expect("save open");
        save.feed(&planar).expect("feed");
        save.done().expect("done");

        let load = AudioReader::open(&path, AudioFormat::Auto).expect("load open");
        assert_eq!(load.sample_rate(), 48_000);
        assert_eq!(load.channel_count().get(), 2);
        let got = load.into_planar_f32().expect("all");
        assert_eq!(got.len(), 2);
        assert_eq!(got[0].len(), frames);
        for i in 0..frames {
            assert!(
                (got[0][i] - l[i]).abs() < 2e-3,
                "l[{i}] {} vs {}",
                got[0][i],
                l[i]
            );
            assert!(
                (got[1][i] - r[i]).abs() < 2e-3,
                "r[{i}] {} vs {}",
                got[1][i],
                r[i]
            );
        }

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn auto_detects_extension() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_audio_auto.wav");
        let _ = std::fs::remove_file(&path);

        let mono_buf = vec![0.1f32, -0.1];
        let mono = [mono_buf.as_slice()];
        let rate = SampleRateHz::new(44_100).unwrap();
        let ch = ChannelCount::new(1).unwrap();
        let mut save = AudioWriter::open(&path, AudioFormat::Auto, rate, ch).expect("open");
        save.feed(&mono).expect("feed");
        save.done().expect("done");

        let load = AudioReader::open(&path, AudioFormat::Auto).expect("load");
        let got = load.into_planar_f32().expect("all");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].len(), 2);
        assert!((got[0][0] - 0.1).abs() < 1e-6);

        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn memory_wav_roundtrip_bytes_api() {
        let l = vec![0.25f32, -0.25, 0.5];
        let r = vec![-0.5f32, 0.125, 0.0];
        let channels = vec![l.clone(), r.clone()];
        let rate = SampleRateHz::new(48_000).unwrap();
        let bytes = write_planar_to_vec(AudioFormat::Wav, rate, &channels).expect("vec");

        let mut load = AudioReader::from_bytes_with_name_hint(bytes, "x.wav").expect("load");
        let mut bufs = vec![Vec::new(), Vec::new()];
        assert_eq!(load.take(10, &mut bufs).expect("take"), 3);
        assert_eq!(bufs[0], l);
        assert_eq!(bufs[1], r);
    }

    #[test]
    fn write_planar_to_file_matches_manual_save() {
        let dir = std::env::temp_dir();
        let path = dir.join("trem_write_planar_auto.wav");
        let _ = std::fs::remove_file(&path);

        let channels = vec![vec![0.2f32, -0.2], vec![0.4f32, 0.0]];
        let rate = SampleRateHz::new(44_100).unwrap();
        write_planar_to_file(&path, AudioFormat::Wav, rate, &channels).expect("write_planar");

        let mut load = AudioReader::open(&path, AudioFormat::Auto).expect("load");
        let mut bufs = vec![Vec::new(), Vec::new()];
        assert_eq!(load.take(8, &mut bufs).expect("take"), 2);
        assert_eq!(bufs[0], channels[0]);
        assert_eq!(bufs[1], channels[1]);

        let _ = std::fs::remove_file(&path);
    }
}
