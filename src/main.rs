use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;

use trem::dsp;
use trem::event::NoteEvent;
use trem::graph::{Graph, Processor};
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::Tuning;

use trem::graph::{GroupHint, ParamGroup};

const BS: usize = 512;

/// Wrap a mono source processor with a Gain (level + pan) stage, producing a
/// stereo-output nested Graph. The source's param groups and params are
/// re-exposed with remapped IDs, then a "Channel" group is appended with
/// Level and Pan from the stereo output stage.
fn instrument_channel(
    label: &'static str,
    source: Box<dyn Processor>,
    level: f32,
    pan: f32,
) -> Graph {
    let src_params = source.params();
    let src_groups = source.param_groups();

    let mut ch = Graph::labeled(BS, label);

    let src = ch.add_node(source);
    let out = ch.add_node(Box::new(dsp::Gain::with_pan(level, pan)));
    ch.connect(src, 0, out, 0);
    ch.set_output(out, 2);

    let mut group_map = std::collections::HashMap::new();
    for sg in &src_groups {
        let new_id = ch.add_group(ParamGroup {
            id: 0,
            name: sg.name,
            hint: sg.hint,
        });
        group_map.insert(sg.id, new_id);
    }

    for p in &src_params {
        match p.group.and_then(|gid| group_map.get(&gid)) {
            Some(&new_gid) => ch.expose_param_in_group(src, p.id, p.name, new_gid),
            None => ch.expose_param(src, p.id, p.name),
        };
    }

    let g_ch = ch.add_group(ParamGroup {
        id: 0,
        name: "Channel",
        hint: GroupHint::Level,
    });
    ch.expose_param_in_group(out, 0, "Level", g_ch);
    ch.expose_param_in_group(out, 1, "Pan", g_ch);

    ch
}

/// Signal chain diagram:
///
/// ```text
/// ┌────────────────────────────────────────────────────────────────┐
/// │                      Top-Level Graph                           │
/// │                                                                │
/// │  Lead > ────────┐                                              │
/// │                  ├── Inst Bus > ──┐                             │
/// │  Bass > ────────┘                 │                             │
/// │                                    ├── Main Bus > ── [output]  │
/// │  Kick > ────┐                     │                             │
/// │  Snare > ───┼── Drum Bus > ──────┘                             │
/// │  Hat > ─────┘                                                  │
/// └────────────────────────────────────────────────────────────────┘
/// ```
///
/// Every node marked `>` is a nested Graph you can Enter to inspect:
///   - **Lead/Bass**: synth -> level/pan
///   - **Kick/Snare/Hat**: drum synth -> level/pan
///   - **Inst Bus**: mixer -> compressor -> vol
///   - **Drum Bus**: mixer -> limiter -> vol
///   - **Main Bus**: mixer -> EQ -> delay -> reverb -> limiter -> vol
fn build_graph() -> (Graph, u32, Vec<(u32, String)>) {
    let mut g = Graph::new(BS);

    // ── Sources ──────────────────────────────────────────────────────────

    let mut lead_synth = dsp::analog_voice(0, BS);
    lead_synth.set_param(0, 0.12);
    lead_synth.set_param(1, 0.4);
    lead_synth.set_param(2, 3200.0);
    lead_synth.set_param(3, 1.8);
    lead_synth.set_param(4, 0.003);
    lead_synth.set_param(5, 0.15);
    lead_synth.set_param(6, 0.2);
    lead_synth.set_param(7, 0.12);
    lead_synth.set_param(8, 0.8);
    let lead = g.add_node(Box::new(instrument_channel(
        "lead",
        Box::new(lead_synth),
        0.70,
        0.15,
    )));

    let mut bass_synth = dsp::analog_voice(1, BS);
    bass_synth.set_param(0, 0.0);
    bass_synth.set_param(1, 0.25);
    bass_synth.set_param(2, 600.0);
    bass_synth.set_param(3, 2.5);
    bass_synth.set_param(4, 0.008);
    bass_synth.set_param(5, 0.25);
    bass_synth.set_param(6, 0.5);
    bass_synth.set_param(7, 0.18);
    bass_synth.set_param(8, 0.5);
    let bass = g.add_node(Box::new(instrument_channel(
        "bass",
        Box::new(bass_synth),
        0.10,
        0.0,
    )));

    let kick = g.add_node(Box::new(instrument_channel(
        "kick",
        Box::new(dsp::KickSynth::new(2)),
        0.60,
        0.0,
    )));
    let snare = g.add_node(Box::new(instrument_channel(
        "snare",
        Box::new(dsp::SnareSynth::new(3)),
        0.28,
        -0.05,
    )));
    let hat = g.add_node(Box::new(instrument_channel(
        "hat",
        Box::new(dsp::HatSynth::new(4)),
        0.15,
        0.25,
    )));

    // ── Drum Bus: mixer -> limiter -> vol ────────────────────────────────

    let drum_bus = {
        let mut bus = Graph::labeled(BS, "drum_bus");
        let input = bus.add_node(Box::new(trem::graph::GraphInput::new(6)));
        bus.set_input(input, 6);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(3)));
        for p in 0..6 {
            bus.connect(input, p, mix, p);
        }

        let lim = bus.add_node(Box::new(dsp::Limiter::new(-1.0, 80.0)));
        bus.connect(mix, 0, lim, 0);
        bus.connect(mix, 1, lim, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(0.9)));
        bus.connect(lim, 0, vol, 0);
        bus.connect(lim, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_lim = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Limiter",
            hint: trem::graph::GroupHint::Level,
        });
        let g_vol = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Output",
            hint: trem::graph::GroupHint::Level,
        });
        bus.expose_param_in_group(lim, 0, "Ceiling", g_lim);
        bus.expose_param_in_group(lim, 1, "Release", g_lim);
        bus.expose_param_in_group(vol, 0, "Level", g_vol);

        bus
    };
    let drum_bus_id = g.add_node(Box::new(drum_bus));
    g.connect(kick, 0, drum_bus_id, 0);
    g.connect(kick, 1, drum_bus_id, 1);
    g.connect(snare, 0, drum_bus_id, 2);
    g.connect(snare, 1, drum_bus_id, 3);
    g.connect(hat, 0, drum_bus_id, 4);
    g.connect(hat, 1, drum_bus_id, 5);

    // ── Inst Bus: mixer -> compressor -> vol ─────────────────────────────

    let inst_bus = {
        let mut bus = Graph::labeled(BS, "inst_bus");
        let input = bus.add_node(Box::new(trem::graph::GraphInput::new(4)));
        bus.set_input(input, 4);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(2)));
        for p in 0..4 {
            bus.connect(input, p, mix, p);
        }

        let comp = bus.add_node(Box::new(dsp::Compressor::new(-18.0, 3.0, 8.0, 120.0)));
        bus.connect(mix, 0, comp, 0);
        bus.connect(mix, 1, comp, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(0.85)));
        bus.connect(comp, 0, vol, 0);
        bus.connect(comp, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_comp = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Compressor",
            hint: trem::graph::GroupHint::Level,
        });
        let g_vol = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Output",
            hint: trem::graph::GroupHint::Level,
        });
        bus.expose_param_in_group(comp, 0, "Threshold", g_comp);
        bus.expose_param_in_group(comp, 1, "Ratio", g_comp);
        bus.expose_param_in_group(comp, 2, "Attack", g_comp);
        bus.expose_param_in_group(comp, 3, "Release", g_comp);
        bus.expose_param_in_group(comp, 4, "Makeup", g_comp);
        bus.expose_param_in_group(vol, 0, "Level", g_vol);

        bus
    };
    let inst_bus_id = g.add_node(Box::new(inst_bus));
    g.connect(lead, 0, inst_bus_id, 0);
    g.connect(lead, 1, inst_bus_id, 1);
    g.connect(bass, 0, inst_bus_id, 2);
    g.connect(bass, 1, inst_bus_id, 3);

    // ── Main Bus: mixer -> EQ -> delay -> reverb -> limiter -> vol ───────

    let main_bus = {
        let mut bus = Graph::labeled(BS, "main_bus");
        let input = bus.add_node(Box::new(trem::graph::GraphInput::new(4)));
        bus.set_input(input, 4);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(2)));
        for p in 0..4 {
            bus.connect(input, p, mix, p);
        }

        let mut eq_proc = dsp::ParametricEq::with_bands(150.0, 3000.0, 8000.0);
        eq_proc.set_param(1, -1.5);
        eq_proc.set_param(4, 3.0);
        eq_proc.set_param(7, 1.5);
        let eq = bus.add_node(Box::new(eq_proc));
        bus.connect(mix, 0, eq, 0);
        bus.connect(mix, 1, eq, 1);

        let dly = bus.add_node(Box::new(dsp::StereoDelay::new(345.0, 0.30, 0.15)));
        bus.connect(eq, 0, dly, 0);
        bus.connect(eq, 1, dly, 1);

        let vrb = bus.add_node(Box::new(dsp::PlateReverb::new(0.35, 0.6, 0.12)));
        bus.connect(dly, 0, vrb, 0);
        bus.connect(dly, 1, vrb, 1);

        let lim = bus.add_node(Box::new(dsp::Limiter::new(-0.3, 100.0)));
        bus.connect(vrb, 0, lim, 0);
        bus.connect(vrb, 1, lim, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(1.0)));
        bus.connect(lim, 0, vol, 0);
        bus.connect(lim, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_eq = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "EQ",
            hint: trem::graph::GroupHint::Filter,
        });
        let g_dly = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Delay",
            hint: trem::graph::GroupHint::TimeBased,
        });
        let g_vrb = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Reverb",
            hint: trem::graph::GroupHint::TimeBased,
        });
        let g_lim = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Limiter",
            hint: trem::graph::GroupHint::Level,
        });
        let g_vol = bus.add_group(trem::graph::ParamGroup {
            id: 0,
            name: "Output",
            hint: trem::graph::GroupHint::Level,
        });

        bus.expose_param_in_group(eq, 1, "EQ Lo", g_eq);
        bus.expose_param_in_group(eq, 4, "EQ Mid", g_eq);
        bus.expose_param_in_group(eq, 7, "EQ Hi", g_eq);
        bus.expose_param_in_group(dly, 0, "Delay Time", g_dly);
        bus.expose_param_in_group(dly, 1, "Feedback", g_dly);
        bus.expose_param_in_group(dly, 2, "Delay Mix", g_dly);
        bus.expose_param_in_group(vrb, 0, "Room Size", g_vrb);
        bus.expose_param_in_group(vrb, 1, "Damping", g_vrb);
        bus.expose_param_in_group(vrb, 2, "Reverb Mix", g_vrb);
        bus.expose_param_in_group(lim, 0, "Ceiling", g_lim);
        bus.expose_param_in_group(lim, 1, "Lim Release", g_lim);
        bus.expose_param_in_group(vol, 0, "Level", g_vol);

        bus
    };
    let main_bus_id = g.add_node(Box::new(main_bus));
    g.connect(inst_bus_id, 0, main_bus_id, 0);
    g.connect(inst_bus_id, 1, main_bus_id, 1);
    g.connect(drum_bus_id, 0, main_bus_id, 2);
    g.connect(drum_bus_id, 1, main_bus_id, 3);

    // ── Output (the main_bus output node) ────────────────────────────────

    let nodes = vec![
        (lead, "Lead".into()),
        (bass, "Bass".into()),
        (kick, "Kick".into()),
        (snare, "Snare".into()),
        (hat, "Hat".into()),
        (inst_bus_id, "Inst Bus".into()),
        (drum_bus_id, "Drum Bus".into()),
        (main_bus_id, "Main Bus".into()),
    ];

    (g, main_bus_id, nodes)
}

fn build_pattern() -> Grid {
    let mut grid = Grid::new(16, 5);

    let n = |deg: i32, oct: i32, vel_n: i64, vel_d: u64| {
        NoteEvent::new(deg, oct, Rational::new(vel_n, vel_d))
    };

    // Voice 0 — Lead: A minor pentatonic arp
    grid.set(0, 0, Some(n(7, -1, 3, 4)));
    grid.set(2, 0, Some(n(10, -1, 5, 8)));
    grid.set(4, 0, Some(n(7, -1, 3, 4)));
    grid.set(5, 0, Some(n(10, -1, 1, 2)));
    grid.set(7, 0, Some(n(5, -1, 5, 8)));
    grid.set(8, 0, Some(n(3, -1, 3, 4)));
    grid.set(10, 0, Some(n(5, -1, 1, 2)));
    grid.set(11, 0, Some(n(7, -1, 7, 8)));
    grid.set(13, 0, Some(n(5, -1, 1, 2)));
    grid.set(14, 0, Some(n(3, -1, 5, 8)));

    // Voice 1 — Bass: driving root movement
    grid.set(0, 1, Some(n(0, -3, 3, 4)));
    grid.set(3, 1, Some(n(0, -3, 5, 8)));
    grid.set(6, 1, Some(n(3, -3, 3, 4)));
    grid.set(7, 1, Some(n(3, -3, 1, 2)));
    grid.set(8, 1, Some(n(5, -3, 3, 4)));
    grid.set(11, 1, Some(n(5, -3, 5, 8)));
    grid.set(12, 1, Some(n(7, -3, 3, 4)));
    grid.set(15, 1, Some(n(10, -3, 7, 8)));

    // Voice 2 — Kick: broken pattern with syncopation
    for step in [0, 3, 4, 8, 10, 12, 15] {
        let vel = match step {
            0 | 4 | 8 | 12 => Rational::new(7, 8),
            _ => Rational::new(5, 8),
        };
        grid.set(step, 2, Some(NoteEvent::new(0, 0, vel)));
    }

    // Voice 3 — Snare: backbeat + ghosts
    grid.set(2, 3, Some(n(0, 0, 1, 3)));
    grid.set(4, 3, Some(n(0, 0, 3, 4)));
    grid.set(6, 3, Some(n(0, 0, 1, 4)));
    grid.set(10, 3, Some(n(0, 0, 1, 3)));
    grid.set(12, 3, Some(n(0, 0, 3, 4)));
    grid.set(14, 3, Some(n(0, 0, 1, 4)));

    // Voice 4 — Hats: 16ths with velocity dynamics
    for step in 0..16 {
        let vel = match step % 4 {
            0 => Rational::new(7, 8),
            2 => Rational::new(1, 2),
            _ => Rational::new(1, 4),
        };
        grid.set(step, 4, Some(NoteEvent::new(0, 0, vel)));
    }

    grid
}

fn main() -> Result<()> {
    let scale = Tuning::edo12().to_scale();
    let (graph, output_node, graph_nodes) = build_graph();

    let graph_edges: Vec<(u32, u16, u32, u16)> = graph
        .topology()
        .1
        .iter()
        .map(|e| (e.src_node, e.src_port, e.dst_node, e.dst_port))
        .collect();

    let node_ids: Vec<u32> = graph_nodes.iter().map(|(id, _)| *id).collect();
    let param_snapshot = graph.snapshot_all_params(&node_ids);
    let descriptions: Vec<String> = node_ids
        .iter()
        .map(|&id| graph.node_description(id).to_string())
        .collect();
    let has_children: Vec<bool> = node_ids
        .iter()
        .map(|&id| graph.node_has_children(id))
        .collect();

    let grid = build_pattern();

    let instrument_names = vec![
        "Lead".into(),
        "Bass".into(),
        "Kick".into(),
        "Snare".into(),
        "Hat".into(),
    ];
    let voice_ids = vec![0, 1, 2, 3, 4];

    let (bridge, audio_bridge) = trem_cpal::create_bridge(1024);
    let _engine = trem_cpal::AudioEngine::new(audio_bridge, graph, output_node, 44100.0)?;

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

    app.set_node_descriptions(descriptions);
    app.set_node_children(has_children);
    app.bpm = 130.0;
    app.bridge.send(trem_cpal::Command::SetBpm(130.0));

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
