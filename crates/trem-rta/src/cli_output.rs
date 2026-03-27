//! Shared **`-o` / `--no-play`** flags ([`StereoOutputCli`]) for small offline binaries.
//!
//! Typical flow: **play** with [`crate::preview::AudioPlayer`]; **write** with
//! [`trem_mio::audio::AudioWriter`] or [`trem_mio::audio::write_planar_to_file`] (crate
//! **`trem-mio`**, feature **`audio`** default on). Enable Cargo feature **`cli`** on **`trem-rta`**
//! for [`StereoOutputCli`].
//!
//! [`trem_mio::audio::write_planar_to_file`]: https://docs.rs/trem-mio/latest/trem_mio/audio/fn.write_planar_to_file.html

use clap::Parser;

/// Shared **`-o` / `--no-play`** flags for offline-render CLIs (use with `#[command(flatten)]`).
#[derive(Parser, Debug, Clone)]
pub struct StereoOutputCli {
    /// `-` = speakers via stdout-style path; otherwise **`.wav`** / **`.flac`** (see `trem_mio::audio`).
    #[arg(short = 'o', long, value_name = "PATH|-")]
    pub output: Option<std::path::PathBuf>,

    /// Skip speaker playback for **`-o -`** and for the default (no `-o`) path.
    #[arg(long)]
    pub no_play: bool,
}
