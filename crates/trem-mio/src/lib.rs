//! **Media I/O** for [**trem**](https://docs.rs/trem): import as **`trem_mio`** (e.g. [`audio`]).
//!
//! **Audio** codecs (IEEE float WAV, FLAC) live here today; **images** and other containers can land
//! here without bloating core **`trem`**.
//!
//! # Name
//!
//! Crate **`trem-mio`** / **`trem_mio`** is **not** [**`mio`**](https://docs.rs/mio) (Tokio’s readiness I/O library).
//!
//! # Types
//!
//! PCM dimensions ([`trem::signal::SampleRateHz`], [`trem::signal::ChannelCount`]) stay in **`trem`**.

#[cfg(feature = "audio")]
pub mod audio;
#[cfg(feature = "audio")]
pub mod wav;
