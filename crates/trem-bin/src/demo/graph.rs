//! Nested-bus demo graph: lead (w/ flutter), bass, drums → inst/drum buses → main FX → out.
//!
//! ```text
//! ┌────────────────────────────────────────────────────────────────┐
//! │                      Top-Level Graph                           │
//! │  Lead > (delay) ┐                                              │
//! │                  ├── Inst Bus > ──┐                             │
//! │  Bass > ────────┘                 │                             │
//! │                                    ├── Main Bus > ── [output]  │
//! │  Kick > ────┐                     │                             │
//! │  Snare >(dst)──┼── Drum Bus > ──────┘                          │
//! │  Hat > ─────┘                                                  │
//! └────────────────────────────────────────────────────────────────┘
//! ```

use std::collections::HashMap;

use trem::graph::{Graph, GraphInput, GroupHint, Node, ParamDescriptor, ParamGroup};
use trem_dsp::standard as dsp;

use super::levels::{channel, drum_bus, inst_bus, lead_delay, main_bus, BLOCK_SIZE};

/// Copy source node param groups + params into a nested channel graph with remapped group IDs.
fn expose_source_params(
    ch: &mut Graph,
    src: trem::graph::NodeId,
    params: &[ParamDescriptor],
    groups: &[ParamGroup],
) {
    let mut group_map = HashMap::new();
    for sg in groups {
        let new_id = ch.add_group(ParamGroup {
            id: 0,
            name: sg.name,
            hint: sg.hint,
        });
        group_map.insert(sg.id, new_id);
    }
    for p in params {
        match p.group.and_then(|gid| group_map.get(&gid)) {
            Some(&new_gid) => {
                ch.expose_param_in_group(src, p.id, p.name, new_gid);
            }
            None => {
                ch.expose_param(src, p.id, p.name);
            }
        }
    }
}

fn channel_gain_group(ch: &mut Graph, gain: trem::graph::NodeId) {
    let g_ch = ch.add_group(ParamGroup {
        id: 0,
        name: "Channel",
        hint: GroupHint::Level,
    });
    ch.expose_param_in_group(gain, 0, "Level", g_ch);
    ch.expose_param_in_group(gain, 1, "Pan", g_ch);
}

/// Mono source → gain/pan → stereo out.
pub fn instrument_channel(
    label: &'static str,
    source: Box<dyn Node>,
    level: f32,
    pan: f32,
) -> Graph {
    let src_params = source.params();
    let src_groups = source.param_groups();

    let mut ch = Graph::labeled(BLOCK_SIZE, label);
    let src = ch.add_node(source);
    let gain = ch.add_node(Box::new(dsp::Gain::with_pan(level, pan)));
    ch.connect(src, 0, gain, 0);
    ch.set_output(gain, 2);

    expose_source_params(&mut ch, src, &src_params, &src_groups);
    channel_gain_group(&mut ch, gain);
    ch
}

/// Mono source → gain/pan → short stereo delay → out (+ “Lead flutter” group).
pub fn instrument_channel_with_delay(
    label: &'static str,
    source: Box<dyn Node>,
    synth_level: f32,
    pan: f32,
    delay_ms: f64,
    delay_feedback: f64,
    delay_mix: f64,
) -> Graph {
    let src_params = source.params();
    let src_groups = source.param_groups();

    let mut ch = Graph::labeled(BLOCK_SIZE, label);
    let src = ch.add_node(source);
    let gain = ch.add_node(Box::new(dsp::Gain::with_pan(synth_level, pan)));
    ch.connect(src, 0, gain, 0);

    let delay_id = ch.add_node(Box::new(dsp::StereoDelay::new(
        delay_ms,
        delay_feedback,
        delay_mix,
    )));
    ch.connect(gain, 0, delay_id, 0);
    ch.connect(gain, 1, delay_id, 1);
    ch.set_output(delay_id, 2);

    expose_source_params(&mut ch, src, &src_params, &src_groups);

    let g_echo = ch.add_group(ParamGroup {
        id: 0,
        name: "Lead flutter",
        hint: GroupHint::TimeBased,
    });
    ch.expose_param_in_group(delay_id, 0, "Echo ms", g_echo);
    ch.expose_param_in_group(delay_id, 1, "Echo FB", g_echo);
    ch.expose_param_in_group(delay_id, 2, "Echo mix", g_echo);

    channel_gain_group(&mut ch, gain);
    ch
}

/// Mono source → distortion → gain/pan → stereo out (+ “Drive” group).
pub fn instrument_channel_with_distortion(
    label: &'static str,
    source: Box<dyn Node>,
    distortion: dsp::Distortion,
    level: f32,
    pan: f32,
) -> Graph {
    let src_params = source.params();
    let src_groups = source.param_groups();

    let mut ch = Graph::labeled(BLOCK_SIZE, label);
    let src = ch.add_node(source);
    let dst = ch.add_node(Box::new(distortion));
    ch.connect(src, 0, dst, 0);

    let gain = ch.add_node(Box::new(dsp::Gain::with_pan(level, pan)));
    ch.connect(dst, 0, gain, 0);
    ch.set_output(gain, 2);

    expose_source_params(&mut ch, src, &src_params, &src_groups);

    let g_dst = ch.add_group(ParamGroup {
        id: 0,
        name: "Drive",
        hint: GroupHint::Generic,
    });
    ch.expose_param_in_group(dst, 0, "Shape", g_dst);
    ch.expose_param_in_group(dst, 1, "Drive", g_dst);
    ch.expose_param_in_group(dst, 2, "Mix", g_dst);
    ch.expose_param_in_group(dst, 3, "Out", g_dst);

    channel_gain_group(&mut ch, gain);
    ch
}

/// Builds the demo routing graph. Returns `(graph, main_out_node, inst_bus_node, editor_node_list)`.
pub fn build_graph() -> (Graph, u32, u32, Vec<(u32, String)>) {
    let mut g = Graph::new(BLOCK_SIZE);

    let mut lead_synth = dsp::lead_voice(0, BLOCK_SIZE);
    lead_synth.set_param(0, 0.05);
    lead_synth.set_param(1, 0.78);
    lead_synth.set_param(2, 0.32);
    lead_synth.set_param(3, 0.85);
    lead_synth.set_param(4, 1900.0);
    lead_synth.set_param(5, 0.82);
    lead_synth.set_param(6, 0.11);
    lead_synth.set_param(7, 260.0);
    lead_synth.set_param(8, 0.028);
    lead_synth.set_param(9, 0.22);
    lead_synth.set_param(10, 0.35);
    lead_synth.set_param(11, 0.42);
    lead_synth.set_param(12, 0.68);

    let lead = g.add_node(Box::new(instrument_channel_with_delay(
        "lead",
        Box::new(lead_synth),
        channel::LEAD_LEVEL,
        channel::LEAD_PAN,
        lead_delay::MS,
        lead_delay::FEEDBACK,
        lead_delay::MIX,
    )));

    let mut bass_synth = dsp::analog_voice(1, BLOCK_SIZE);
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
        channel::BASS_LEVEL,
        0.0,
    )));

    let kick = g.add_node(Box::new(instrument_channel(
        "kick",
        Box::new(dsp::KickSynth::new(2)),
        channel::KICK_LEVEL,
        0.0,
    )));
    let snare = g.add_node(Box::new(instrument_channel_with_distortion(
        "snare",
        Box::new(dsp::SnareSynth::new(3)),
        dsp::Distortion::snare_default(),
        channel::SNARE_LEVEL,
        channel::SNARE_PAN,
    )));
    let hat = g.add_node(Box::new(instrument_channel(
        "hat",
        Box::new(dsp::HatSynth::new(4)),
        channel::HAT_LEVEL,
        channel::HAT_PAN,
    )));

    let drum_bus = {
        let mut bus = Graph::labeled(BLOCK_SIZE, "drum_bus");
        let input = bus.add_node(Box::new(GraphInput::new(6)));
        bus.set_input(input, 6);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(3)));
        for p in 0..6 {
            bus.connect(input, p, mix, p);
        }

        let lim = bus.add_node(Box::new(dsp::Limiter::new(
            drum_bus::LIMITER_CEILING_DB,
            drum_bus::LIMITER_RELEASE_MS,
        )));
        bus.connect(mix, 0, lim, 0);
        bus.connect(mix, 1, lim, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(drum_bus::OUTPUT_GAIN)));
        bus.connect(lim, 0, vol, 0);
        bus.connect(lim, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_lim = bus.add_group(ParamGroup {
            id: 0,
            name: "Limiter",
            hint: GroupHint::Level,
        });
        let g_vol = bus.add_group(ParamGroup {
            id: 0,
            name: "Output",
            hint: GroupHint::Level,
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

    let inst_bus = {
        let mut bus = Graph::labeled(BLOCK_SIZE, "inst_bus");
        let input = bus.add_node(Box::new(GraphInput::new(4)));
        bus.set_input(input, 4);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(2)));
        for p in 0..4 {
            bus.connect(input, p, mix, p);
        }

        let comp = bus.add_node(Box::new(dsp::Compressor::new(
            inst_bus::COMP_THRESHOLD_DB,
            inst_bus::COMP_RATIO,
            inst_bus::COMP_ATTACK_MS,
            inst_bus::COMP_RELEASE_MS,
        )));
        bus.connect(mix, 0, comp, 0);
        bus.connect(mix, 1, comp, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(inst_bus::OUTPUT_GAIN)));
        bus.connect(comp, 0, vol, 0);
        bus.connect(comp, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_comp = bus.add_group(ParamGroup {
            id: 0,
            name: "Compressor",
            hint: GroupHint::Level,
        });
        let g_vol = bus.add_group(ParamGroup {
            id: 0,
            name: "Output",
            hint: GroupHint::Level,
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

    let main_bus = {
        let mut bus = Graph::labeled(BLOCK_SIZE, "main_bus");
        let input = bus.add_node(Box::new(GraphInput::new(4)));
        bus.set_input(input, 4);

        let mix = bus.add_node(Box::new(dsp::StereoMixer::new(2)));
        for p in 0..4 {
            bus.connect(input, p, mix, p);
        }

        let mut eq_proc = dsp::ParametricEq::with_bands(150.0, 3000.0, 8000.0);
        eq_proc.set_param(1, main_bus::EQ_LOW_DB);
        eq_proc.set_param(4, main_bus::EQ_MID_DB);
        eq_proc.set_param(7, main_bus::EQ_HI_DB);
        let eq = bus.add_node(Box::new(eq_proc));
        bus.connect(mix, 0, eq, 0);
        bus.connect(mix, 1, eq, 1);

        let dly = bus.add_node(Box::new(dsp::StereoDelay::new(
            main_bus::DELAY_MS,
            main_bus::DELAY_FB,
            main_bus::DELAY_MIX,
        )));
        bus.connect(eq, 0, dly, 0);
        bus.connect(eq, 1, dly, 1);

        let vrb = bus.add_node(Box::new(dsp::PlateReverb::new(
            main_bus::REVERB_SIZE,
            main_bus::REVERB_DAMP,
            main_bus::REVERB_MIX,
        )));
        bus.connect(dly, 0, vrb, 0);
        bus.connect(dly, 1, vrb, 1);

        let lim = bus.add_node(Box::new(dsp::Limiter::new(
            main_bus::LIMITER_CEILING_DB,
            main_bus::LIMITER_RELEASE_MS,
        )));
        bus.connect(vrb, 0, lim, 0);
        bus.connect(vrb, 1, lim, 1);

        let vol = bus.add_node(Box::new(dsp::StereoGain::new(main_bus::OUTPUT_GAIN)));
        bus.connect(lim, 0, vol, 0);
        bus.connect(lim, 1, vol, 1);
        bus.set_output(vol, 2);

        let g_eq = bus.add_group(ParamGroup {
            id: 0,
            name: "EQ",
            hint: GroupHint::Filter,
        });
        let g_dly = bus.add_group(ParamGroup {
            id: 0,
            name: "Delay",
            hint: GroupHint::TimeBased,
        });
        let g_vrb = bus.add_group(ParamGroup {
            id: 0,
            name: "Reverb",
            hint: GroupHint::TimeBased,
        });
        let g_lim = bus.add_group(ParamGroup {
            id: 0,
            name: "Limiter",
            hint: GroupHint::Level,
        });
        let g_vol = bus.add_group(ParamGroup {
            id: 0,
            name: "Output",
            hint: GroupHint::Level,
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

    (g, main_bus_id, inst_bus_id, nodes)
}
