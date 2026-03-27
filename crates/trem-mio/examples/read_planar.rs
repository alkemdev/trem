//! Reads a WAV or FLAC with [`AudioReader`]: prints sample rate, channel count, and frame count.
//!
//! ```sh
//! cargo run -p trem-mio --example read_planar
//! cargo run -p trem-mio --example read_planar -- path/to/file.wav
//! ```
//!
//! With no path argument, writes a short stereo demo WAV to `target/trem_read_planar_demo.wav`,
//! then reads it back.

use std::env;
use std::fs;
use std::path::PathBuf;

use trem::signal::SampleRateHz;
use trem_mio::audio::{write_planar_to_file, AudioFormat, AudioReader};

fn main() {
    let path = resolve_path();
    let reader = AudioReader::open(&path, AudioFormat::Auto).expect("open");
    println!("file: {}", path.display());
    println!("sample rate: {} Hz", reader.sample_rate());
    println!("channels: {}", reader.channel_count().as_usize());
    let planar = reader.into_planar_f32().expect("decode");
    let frames = planar.first().map(|c| c.len()).unwrap_or(0);
    println!("frames: {frames}");
}

fn resolve_path() -> PathBuf {
    if let Some(p) = env::args().nth(1) {
        return PathBuf::from(p);
    }
    let path = PathBuf::from("target/trem_read_planar_demo.wav");
    if let Some(parent) = path.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let rate = SampleRateHz::new(48_000).expect("sample rate");
    let n = 256usize;
    let left: Vec<f32> = (0..n).map(|i| (i as f32 * 0.02).sin() * 0.1).collect();
    let right: Vec<f32> = left.iter().map(|s| -s).collect();
    let channels = vec![left, right];
    write_planar_to_file(&path, AudioFormat::Auto, rate, &channels).expect("write demo wav");
    println!("(wrote demo WAV; pass a path to read an existing file)\n");
    path
}
