//! In-memory WAV: [`write_planar_to_vec`] → [`AudioReader::from_bytes`] → [`AudioReader::into_planar_f32`].
//!
//! ```sh
//! cargo run -p trem-mio --example roundtrip_memory
//! ```

use trem::signal::SampleRateHz;
use trem_mio::audio::{write_planar_to_vec, AudioFormat, AudioReader};

fn main() {
    let rate = SampleRateHz::new(44_100).expect("sample rate");
    let frames = 128usize;
    let mono: Vec<f32> = (0..frames)
        .map(|i| (i as f32 * 0.01).sin() * 0.05)
        .collect();
    let channels = vec![mono];

    let bytes = write_planar_to_vec(AudioFormat::Wav, rate, &channels).expect("encode");
    let reader = AudioReader::from_bytes(bytes, AudioFormat::Wav).expect("decode open");
    assert_eq!(reader.sample_rate(), rate.get());
    assert_eq!(reader.channel_count().as_usize(), 1);

    let out = reader.into_planar_f32().expect("decode");
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].len(), frames);
    let eps = 1e-5f32;
    for (a, b) in channels[0].iter().zip(&out[0]) {
        assert!((a - b).abs() < eps, "sample mismatch: {a} vs {b}");
    }
    println!("roundtrip OK: {frames} frames mono @ {} Hz", rate.get());
}
