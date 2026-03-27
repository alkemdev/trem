//! Writes a short stereo IEEE-float WAV using [`trem_mio::audio::write_planar_to_file`].
//!
//! ```sh
//! cargo run -p trem-mio --example save_planar_wav
//! ```
//!
//! Optional path argument (default: `target/trem_save_planar_stereo.wav`).

use std::env;
use std::fs;
use std::path::PathBuf;

use trem::signal::SampleRateHz;
use trem_mio::audio::{write_planar_to_file, AudioFormat};

fn main() {
    let out = env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("target/trem_save_planar_stereo.wav"));
    if let Some(parent) = out.parent() {
        let _ = fs::create_dir_all(parent);
    }

    let rate = SampleRateHz::new(48_000).expect("sample rate");
    let frames = 512usize;
    let left: Vec<f32> = (0..frames).map(|i| (i as f32 * 0.02).sin() * 0.1).collect();
    let right: Vec<f32> = left.iter().map(|s| -s).collect();
    let channels = vec![left, right];

    write_planar_to_file(&out, AudioFormat::Auto, rate, &channels).expect("write wav");
    println!("wrote {}", out.display());
}
