//! Small typed dimensions for PCM and file I/O.
//!
//! These types are shared with [`trem_mio::audio`](https://docs.rs/trem-mio/latest/trem_mio/audio/index.html)
//! and can be adopted gradually elsewhere in the engine.

use std::fmt;
use std::num::{NonZeroU16, NonZeroU32};

/// Invalid construction of a [`SampleRateHz`] or [`ChannelCount`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignalError {
    /// Sample rate was zero or rounded to zero.
    ZeroSampleRate,
    /// Channel count was zero.
    ZeroChannels,
}

impl fmt::Display for SignalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            SignalError::ZeroSampleRate => write!(f, "sample rate must be positive"),
            SignalError::ZeroChannels => write!(f, "channel count must be at least 1"),
        }
    }
}

impl std::error::Error for SignalError {}

/// Audio sample rate in whole Hz (e.g. 44_100, 48_000).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SampleRateHz(NonZeroU32);

impl SampleRateHz {
    /// Constructs from a positive integer Hz value.
    pub fn new(hz: u32) -> Option<Self> {
        NonZeroU32::new(hz).map(Self)
    }

    /// Rounds `hz` to the nearest integer and builds a rate, or errors if the result is zero.
    pub fn from_hz_rounded(hz: f64) -> Result<Self, SignalError> {
        let r = hz.round() as u32;
        Self::new(r).ok_or(SignalError::ZeroSampleRate)
    }

    /// Raw Hz value.
    pub fn get(self) -> u32 {
        self.0.get()
    }
}

/// Number of PCM channels (1 = mono, 2 = stereo, …).
///
/// FLAC encoding in **`trem_mio::audio`** supports at most [`ChannelCount::MAX_FLAC`] channels.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ChannelCount(NonZeroU16);

impl ChannelCount {
    /// Maximum channel count supported by the FLAC encoder path in **`trem_mio::audio`**.
    pub const MAX_FLAC: u16 = 256;

    /// Constructs from a positive channel count.
    pub fn new(count: u16) -> Option<Self> {
        NonZeroU16::new(count).map(Self)
    }

    /// Raw channel count.
    pub fn get(self) -> u16 {
        self.0.get()
    }

    /// Channel count as `usize`.
    pub fn as_usize(self) -> usize {
        self.get() as usize
    }
}

/// A non-negative number of PCM frames (one sample per channel per frame).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct FrameCount(pub usize);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_rate_roundtrip() {
        let r = SampleRateHz::new(48_000).unwrap();
        assert_eq!(r.get(), 48_000);
        assert_eq!(
            SampleRateHz::from_hz_rounded(48_000.4).unwrap().get(),
            48_000
        );
    }

    #[test]
    fn sample_rate_rejects_zero() {
        assert!(SampleRateHz::new(0).is_none());
        assert!(SampleRateHz::from_hz_rounded(0.4).is_err());
    }

    #[test]
    fn channel_count() {
        let c = ChannelCount::new(2).unwrap();
        assert_eq!(c.get(), 2);
        assert_eq!(c.as_usize(), 2);
        assert!(ChannelCount::new(0).is_none());
    }
}
