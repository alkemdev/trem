use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;

use trem::dsp::{
    analog_voice, Gain, HatSynth, KickSynth, ParametricEq, PlateReverb, SnareSynth, StereoDelay,
    StereoMixer,
};
use trem::event::NoteEvent;
use trem::graph::{Graph, Processor};
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::Tuning;

fn main() -> Result<()> {
    let scale = Tuning::edo12().to_scale();

    // 12-EDO rooted on A440. Degrees = semitones from A.
    // A minor pentatonic: A(0) C(3) D(5) E(7) G(10)

    // --- Build audio graph ---
    let mut graph = Graph::new(512);

    // Lead synth: bright dual-osc, plucky envelope, slight pan right
    let mut lead_synth = analog_voice(0, 512);
    lead_synth.set_param(0, 0.12); // Detune: +0.12 st (chorus)
    lead_synth.set_param(1, 0.4); // Osc Mix: favor saw
    lead_synth.set_param(2, 3200.0); // Cutoff: bright
    lead_synth.set_param(3, 1.8); // Resonance: slight bite
    lead_synth.set_param(4, 0.003); // Attack: plucky
    lead_synth.set_param(5, 0.15); // Decay: short
    lead_synth.set_param(6, 0.2); // Sustain: low
    lead_synth.set_param(7, 0.12); // Release: tight
    lead_synth.set_param(8, 0.8); // Level
    let lead = graph.add_node(Box::new(lead_synth));
    let gain1 = graph.add_node(Box::new(Gain::with_pan(0.70, 0.15)));
    graph.connect(lead, 0, gain1, 0);

    // Bass synth: dark filtered sub, no detune, longer release
    let mut bass_synth = analog_voice(1, 512);
    bass_synth.set_param(0, 0.0); // Detune: none (tight)
    bass_synth.set_param(1, 0.25); // Osc Mix: mostly saw
    bass_synth.set_param(2, 600.0); // Cutoff: dark
    bass_synth.set_param(3, 2.5); // Resonance: round
    bass_synth.set_param(4, 0.008); // Attack: soft onset
    bass_synth.set_param(5, 0.25); // Decay
    bass_synth.set_param(6, 0.5); // Sustain: warm hold
    bass_synth.set_param(7, 0.18); // Release
    bass_synth.set_param(8, 0.5); // Level
    let bass = graph.add_node(Box::new(bass_synth));
    let gain2 = graph.add_node(Box::new(Gain::new(0.10)));
    graph.connect(bass, 0, gain2, 0);

    // Kick — punchy, center
    let kick = graph.add_node(Box::new(KickSynth::new(2)));
    let kick_gain = graph.add_node(Box::new(Gain::new(0.60)));
    graph.connect(kick, 0, kick_gain, 0);

    // Snare
    let snare = graph.add_node(Box::new(SnareSynth::new(3)));
    let snare_gain = graph.add_node(Box::new(Gain::with_pan(0.28, -0.05)));
    graph.connect(snare, 0, snare_gain, 0);

    // Hat — panned right for width
    let hat = graph.add_node(Box::new(HatSynth::new(4)));
    let hat_gain = graph.add_node(Box::new(Gain::with_pan(0.15, 0.25)));
    graph.connect(hat, 0, hat_gain, 0);

    // Instrument bus
    let inst_mix = graph.add_node(Box::new(StereoMixer::new(2)));
    graph.connect(gain1, 0, inst_mix, 0);
    graph.connect(gain1, 1, inst_mix, 1);
    graph.connect(gain2, 0, inst_mix, 2);
    graph.connect(gain2, 1, inst_mix, 3);

    // Drum bus
    let drum_mix = graph.add_node(Box::new(StereoMixer::new(3)));
    graph.connect(kick_gain, 0, drum_mix, 0);
    graph.connect(kick_gain, 1, drum_mix, 1);
    graph.connect(snare_gain, 0, drum_mix, 2);
    graph.connect(snare_gain, 1, drum_mix, 3);
    graph.connect(hat_gain, 0, drum_mix, 4);
    graph.connect(hat_gain, 1, drum_mix, 5);

    // Submix
    let submix = graph.add_node(Box::new(StereoMixer::new(2)));
    graph.connect(inst_mix, 0, submix, 0);
    graph.connect(inst_mix, 1, submix, 1);
    graph.connect(drum_mix, 0, submix, 2);
    graph.connect(drum_mix, 1, submix, 3);

    // FX: EQ — presence boost, low-end cleanup, air
    let mut eq_proc = ParametricEq::with_bands(150.0, 3000.0, 8000.0);
    eq_proc.set_param(1, -1.5); // Lo: -1.5 dB (clean up mud)
    eq_proc.set_param(4, 3.0); // Mid: +3 dB (presence)
    eq_proc.set_param(7, 1.5); // Hi: +1.5 dB (air)
    let eq = graph.add_node(Box::new(eq_proc));
    graph.connect(submix, 0, eq, 0);
    graph.connect(submix, 1, eq, 1);

    // FX: Delay — dotted 8th at 130 BPM (~345ms), subtle
    let delay = graph.add_node(Box::new(StereoDelay::new(345.0, 0.30, 0.15)));
    graph.connect(eq, 0, delay, 0);
    graph.connect(eq, 1, delay, 1);

    // FX: Reverb — tight room, well damped
    let reverb = graph.add_node(Box::new(PlateReverb::new(0.35, 0.6, 0.12)));
    graph.connect(delay, 0, reverb, 0);
    graph.connect(delay, 1, reverb, 1);

    // Master output
    let master = graph.add_node(Box::new(StereoMixer::new(1)));
    graph.connect(reverb, 0, master, 0);
    graph.connect(reverb, 1, master, 1);

    // --- Graph topology snapshot ---
    let graph_nodes: Vec<(u32, String)> = vec![
        (lead, "Lead Synth".into()),
        (gain1, "Lead Gain".into()),
        (bass, "Bass Synth".into()),
        (gain2, "Bass Gain".into()),
        (kick, "Kick".into()),
        (kick_gain, "Kick Gain".into()),
        (snare, "Snare".into()),
        (snare_gain, "Snare Gain".into()),
        (hat, "Hat".into()),
        (hat_gain, "Hat Gain".into()),
        (inst_mix, "Inst Bus".into()),
        (drum_mix, "Drum Bus".into()),
        (submix, "Submix".into()),
        (eq, "EQ".into()),
        (delay, "Delay".into()),
        (reverb, "Reverb".into()),
        (master, "Master".into()),
    ];

    let graph_edges: Vec<(u32, u16, u32, u16)> = graph
        .topology()
        .1
        .iter()
        .map(|e| (e.src_node, e.src_port, e.dst_node, e.dst_port))
        .collect();

    let node_ids: Vec<u32> = graph_nodes.iter().map(|(id, _)| *id).collect();
    let param_snapshot = graph.snapshot_all_params(&node_ids);

    // --- Pattern: 16-step loop, A minor, 130 BPM ---
    let instrument_names = vec![
        "Lead".into(),
        "Bass".into(),
        "Kick".into(),
        "Snare".into(),
        "Hat".into(),
    ];
    let voice_ids = vec![0, 1, 2, 3, 4];

    let mut grid = Grid::new(16, 5);

    let n = |deg: i32, oct: i32, vel_n: i64, vel_d: u64| {
        NoteEvent::new(deg, oct, Rational::new(vel_n, vel_d))
    };

    // Voice 0 — Lead: pentatonic arp in A minor (oct -1 = A3 region)
    //   A(0) C(3) D(5) E(7) G(10)
    grid.set(0, 0, Some(n(7, -1, 3, 4))); // E  — downbeat hook
    grid.set(2, 0, Some(n(10, -1, 5, 8))); // G  — quick answer
    grid.set(4, 0, Some(n(7, -1, 3, 4))); // E  — beat 2, rhythmic anchor
    grid.set(5, 0, Some(n(10, -1, 1, 2))); // G  — syncopated push
    grid.set(7, 0, Some(n(5, -1, 5, 8))); // D  — step down, tension
    grid.set(8, 0, Some(n(3, -1, 3, 4))); // C  — phrase 2, drop lower
    grid.set(10, 0, Some(n(5, -1, 1, 2))); // D  — rising
    grid.set(11, 0, Some(n(7, -1, 7, 8))); // E  — accent, peak
    grid.set(13, 0, Some(n(5, -1, 1, 2))); // D  — descending
    grid.set(14, 0, Some(n(3, -1, 5, 8))); // C  — resolve, breathe

    // Voice 1 — Bass: saw, driving root movement (oct -3 = A1 region)
    grid.set(0, 1, Some(n(0, -3, 3, 4))); // A  — root, locks with kick
    grid.set(3, 1, Some(n(0, -3, 5, 8))); // A  — syncopated push
    grid.set(6, 1, Some(n(3, -3, 3, 4))); // C  — minor third, movement
    grid.set(7, 1, Some(n(3, -3, 1, 2))); // C  — double tap
    grid.set(8, 1, Some(n(5, -3, 3, 4))); // D  — fourth, climbs
    grid.set(11, 1, Some(n(5, -3, 5, 8))); // D  — push
    grid.set(12, 1, Some(n(7, -3, 3, 4))); // E  — fifth, peak tension
    grid.set(15, 1, Some(n(10, -3, 7, 8))); // G  — seventh, drives back to root

    // Voice 2 — Kick: broken pattern with syncopation
    for step in [0, 3, 4, 8, 10, 12, 15] {
        let vel = match step {
            0 | 4 | 8 | 12 => Rational::new(7, 8), // strong on beats
            _ => Rational::new(5, 8),              // ghosts
        };
        grid.set(step, 2, Some(NoteEvent::new(0, 0, vel)));
    }

    // Voice 3 — Snare: backbeat + ghost notes for groove
    grid.set(2, 3, Some(n(0, 0, 1, 3))); // ghost
    grid.set(4, 3, Some(n(0, 0, 3, 4))); // CRACK — beat 2
    grid.set(6, 3, Some(n(0, 0, 1, 4))); // ghost
    grid.set(10, 3, Some(n(0, 0, 1, 3))); // ghost
    grid.set(12, 3, Some(n(0, 0, 3, 4))); // CRACK — beat 4
    grid.set(14, 3, Some(n(0, 0, 1, 4))); // ghost

    // Voice 4 — Hats: 16ths with velocity dynamics (strong/soft/med/soft)
    for step in 0..16 {
        let vel = match step % 4 {
            0 => Rational::new(7, 8), // downbeat — strong
            2 => Rational::new(1, 2), // upbeat — medium
            _ => Rational::new(1, 4), // ghost 16ths
        };
        grid.set(step, 4, Some(NoteEvent::new(0, 0, vel)));
    }

    // --- Audio engine ---
    let (bridge, audio_bridge) = trem_cpal::create_bridge(1024);
    let _engine = trem_cpal::AudioEngine::new(audio_bridge, graph, master, 44100.0)?;

    // --- TUI ---
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let mut app = trem_tui::App::new(
        grid,
        scale,
        "12-EDO".to_string(),
        bridge,
        instrument_names,
        voice_ids,
    )
    .with_graph_info(graph_nodes, graph_edges, param_snapshot);

    app.bpm = 130.0;
    app.bridge.send(trem_cpal::Command::SetBpm(130.0));

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
