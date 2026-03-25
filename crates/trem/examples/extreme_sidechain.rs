//! Sidechain pump: detuned saws + [`trem_dsp::StereoMixer`] → [`trem_dsp::SidechainDucker`] (`duk`),
//! keyed by [`trem_dsp::KickSynth`] on a **looped one-beat** [`TimedEvent`] pattern.
//!
//! ```bash
//! cargo run -p trem --features wav --example extreme_sidechain
//! cargo run -p trem --features wav --example extreme_sidechain -- -o clip.wav
//! cargo run -p trem --features wav --example extreme_sidechain -- -o -
//! ```

use std::ffi::OsStr;

use clap::Parser;
use trem::event::{GraphEvent, TimedEvent};
use trem::graph::{Graph, NodeId};
use trem::render::{loop_timed_events, render_captures};
use trem::wav::write_stereo_wav_f32;
use trem_dsp::{KickSynth, Oscillator, SidechainDucker, StereoMixer, Waveform};
use trem_rta::preview::play_stereo_f32;

const SR: f64 = 48_000.0;
const BPM: f64 = 128.0;
const BLOCK: usize = 512;
const SECS: usize = 2;
const KICK_VOICE: u32 = 0;

#[derive(Parser)]
struct Cli {
    #[arg(short, long, value_name = "PATH|-")]
    output: Option<std::path::PathBuf>,
}

/// Two detuned saws into a stereo bus, kick on voice `KICK_VOICE`, then [`SidechainDucker`].
fn demo_graph() -> (Graph, NodeId, NodeId) {
    let mut g = Graph::new(BLOCK);

    let mut l = Oscillator::new(Waveform::Saw);
    l.frequency = 110.0;
    let mut r = Oscillator::new(Waveform::Saw);
    r.frequency = 110.0;
    r.detune = 0.14;

    let ol = g.add_node(Box::new(l));
    let or = g.add_node(Box::new(r));
    let mix = g.add_node(Box::new(StereoMixer::with_level(2, 0.12)));
    g.connect(ol, 0, mix, 0);
    g.connect(or, 0, mix, 3);

    let kick = g.add_node(Box::new(KickSynth::new(KICK_VOICE)));
    let duck = g.add_node(Box::new(SidechainDucker::new(0.94, 0.25, 95.0)));
    g.connect(mix, 0, duck, 0);
    g.connect(mix, 1, duck, 1);
    g.connect(kick, 0, duck, 2);

    (g, mix, duck)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let samples = (SR as usize) * SECS;
    let beat_samples = (60.0 / BPM * SR).round() as usize;
    let one_hit = [TimedEvent {
        sample_offset: 0,
        event: GraphEvent::NoteOn {
            frequency: 55.0,
            velocity: 1.0,
            voice: KICK_VOICE,
        },
    }];
    let events = loop_timed_events(&one_hit, beat_samples, samples);

    let (mut graph, mix, duck) = demo_graph();
    let taps = render_captures(
        &mut graph,
        &events,
        samples,
        SR,
        BLOCK,
        &[(mix, 0), (mix, 1), (duck, 0), (duck, 1)],
    )?;

    let (pad_l, pad_r, out_l, out_r) = (&taps[0], &taps[1], &taps[2], &taps[3]);
    let (rms_in, rms_out, min_gain) = duck_stats(pad_l, pad_r, out_l, out_r);

    println!("{SECS}s @ {SR} Hz, {BPM} BPM — looped kick pattern + duk");
    println!(
        "RMS in ≈ {rms_in:.4}, out ≈ {rms_out:.4}  ({:.1} dB)",
        20.0 * (rms_out / rms_in.max(1e-12)).log10()
    );
    println!(
        "Min stereo gain vs input magnitude: {:.1}%",
        (min_gain * 100.0) as f64
    );

    if let Some(path) = cli.output.as_ref() {
        if path.as_os_str() == OsStr::new("-") {
            play_stereo_f32(out_l, out_r, SR)?;
            println!("Played on default output.");
        } else {
            write_stereo_wav_f32(path.as_path(), out_l, out_r, SR as u32)?;
            println!("Wrote {}", path.display());
        }
    }

    Ok(())
}

fn duck_stats(pad_l: &[f32], pad_r: &[f32], out_l: &[f32], out_r: &[f32]) -> (f64, f64, f32) {
    let mut sum_in = 0.0f64;
    let mut sum_out = 0.0f64;
    let mut min_g = 1.0f32;
    let n = pad_l
        .len()
        .min(pad_r.len())
        .min(out_l.len())
        .min(out_r.len());
    for i in 0..n {
        let il = pad_l[i] as f64;
        let ir = pad_r[i] as f64;
        let ol = out_l[i] as f64;
        let or = out_r[i] as f64;
        let m_in = (il * il + ir * ir).sqrt();
        let m_out = (ol * ol + or * or).sqrt();
        if m_in > 1e-6 {
            min_g = min_g.min((m_out / m_in) as f32);
        }
        sum_in += m_in * m_in;
        sum_out += m_out * m_out;
    }
    let nn = n.max(1) as f64;
    ((sum_in / nn).sqrt(), (sum_out / nn).sqrt(), min_g)
}
