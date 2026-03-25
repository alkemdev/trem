//! Offline-render a demo pattern to **WAV** (32-bit float) or **FLAC** (16-bit).
//!
//! ```bash
//! cargo run -p trem-dsp --example render_to_file --features export -- -o clip.wav
//! cargo run -p trem-dsp --example render_to_file --features export -- -o clip.flac --bpm 140 --sample-rate 48000
//! ```

use std::path::PathBuf;

use clap::Parser;
use trem::event::NoteEvent;
use trem::graph::Graph;
use trem::math::Rational;
use trem::pitch::Tuning;
use trem::tree::Tree;
use trem_dsp::export::write_audio_file;
use trem_dsp::{Adsr, Gain, Oscillator, Waveform};

#[derive(Parser)]
#[command(name = "render_to_file")]
#[command(about = "Render a short demo phrase to a WAV or FLAC file")]
struct Cli {
    /// Output path; extension must be `.wav` or `.flac` (case-insensitive).
    #[arg(short = 'o', long, value_name = "PATH")]
    output: PathBuf,

    /// Sample rate in Hz.
    #[arg(long, default_value_t = 44_100)]
    sample_rate: u32,

    /// Tempo in beats per minute.
    #[arg(long, default_value_t = 120.0)]
    bpm: f64,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli = Cli::parse();

    let mut graph = Graph::new(512);
    let osc = graph.add_node(Box::new(Oscillator::new(Waveform::Saw)));
    let env = graph.add_node(Box::new(Adsr::new(0.005, 0.1, 0.4, 0.2)));
    let gain = graph.add_node(Box::new(Gain::new(0.5)));
    graph.connect(osc, 0, env, 0);
    graph.connect(env, 0, gain, 0);

    let scale = Tuning::edo12().to_scale();
    let tree = Tree::seq(vec![
        Tree::leaf(NoteEvent::simple(0)),
        Tree::leaf(NoteEvent::simple(4)),
        Tree::leaf(NoteEvent::simple(7)),
        Tree::leaf(NoteEvent::new(0, 1, Rational::new(3, 4))),
    ]);

    let beats = Rational::integer(4);
    let sample_rate = cli.sample_rate as f64;

    let audio = trem::render::render_pattern(
        &tree,
        beats,
        cli.bpm,
        sample_rate,
        &scale,
        440.0,
        &mut graph,
        gain,
    )?;

    write_audio_file(&cli.output, &audio, cli.sample_rate)?;

    let secs = audio[0].len() as f64 / sample_rate;
    println!(
        "Wrote {} ({:.2}s, {} ch, {} Hz)",
        cli.output.display(),
        secs,
        audio.len(),
        cli.sample_rate
    );
    Ok(())
}
