use std::{cell::RefCell, rc::Rc};

use anyhow::Result;
use ratzilla::event::{
    KeyCode as WebKeyCode, KeyEvent as WebKeyEvent, MouseEventKind as WebMouseEventKind,
};
use ratzilla::ratatui::Terminal;
use ratzilla::{DomBackend, WebRenderer};
use trem::dsp::{
    Adsr, Gain, HatSynth, KickSynth, Oscillator, ParametricEq, PlateReverb, SnareSynth,
    StereoDelay, StereoMixer, Waveform,
};
use trem::event::NoteEvent;
use trem::graph::{Graph, Processor};
use trem::grid::Grid;
use trem::math::Rational;
use trem::pitch::Tuning;
use trem_cpal::Command;
use trem_tui::input::{AppKeyCode, AppKeyEvent, InputContext};
use wasm_bindgen::prelude::*;

struct PendingAudio {
    audio_bridge: trem_cpal::AudioBridge,
    graph: Graph,
    output_node: u32,
    scope_input_node: Option<u32>,
    sample_rate: f64,
}

fn warn(message: &str) {
    ratzilla::web_sys::console::warn_1(&JsValue::from_str(message));
}

fn map_web_key(key: WebKeyEvent) -> AppKeyEvent {
    let code = match key.code {
        WebKeyCode::Char(ch) => AppKeyCode::Char(ch),
        WebKeyCode::F(n) => AppKeyCode::F(n),
        WebKeyCode::Backspace => AppKeyCode::Backspace,
        WebKeyCode::Enter => AppKeyCode::Enter,
        WebKeyCode::Left => AppKeyCode::Left,
        WebKeyCode::Right => AppKeyCode::Right,
        WebKeyCode::Up => AppKeyCode::Up,
        WebKeyCode::Down => AppKeyCode::Down,
        WebKeyCode::Tab => AppKeyCode::Tab,
        WebKeyCode::Delete => AppKeyCode::Delete,
        WebKeyCode::Home => AppKeyCode::Home,
        WebKeyCode::End => AppKeyCode::End,
        WebKeyCode::PageUp => AppKeyCode::PageUp,
        WebKeyCode::PageDown => AppKeyCode::PageDown,
        WebKeyCode::Esc => AppKeyCode::Esc,
        WebKeyCode::Unidentified => AppKeyCode::Unknown,
    };

    AppKeyEvent {
        code,
        ctrl: key.ctrl,
        alt: key.alt,
        shift: key.shift,
    }
}

fn start_audio_if_needed(
    pending_audio: &Rc<RefCell<Option<PendingAudio>>>,
    audio_engine: &Rc<RefCell<Option<trem_cpal::AudioEngine>>>,
    app: &Rc<RefCell<trem_tui::App>>,
) {
    if audio_engine.borrow().is_some() {
        return;
    }

    let Some(pending) = pending_audio.borrow_mut().take() else {
        return;
    };

    match trem_cpal::AudioEngine::new(
        pending.audio_bridge,
        pending.graph,
        pending.output_node,
        pending.scope_input_node,
        pending.sample_rate,
    ) {
        Ok(engine) => {
            *audio_engine.borrow_mut() = Some(engine);
            let mut app = app.borrow_mut();
            let bpm = app.bpm;
            app.bridge.send(Command::SetBpm(bpm));
        }
        Err(err) => {
            warn(&format!(
                "trem-web: audio init failed, continuing without audio: {err}"
            ));
        }
    }
}

fn build_app() -> Result<(trem_tui::App, PendingAudio)> {
    let scale = Tuning::edo12().to_scale();

    // --- Build audio graph ---
    let mut graph = Graph::new(512);

    let osc1 = graph.add_node(Box::new(Oscillator::new(Waveform::Triangle).with_voice(0)));
    let adsr1 = graph.add_node(Box::new(Adsr::new(0.003, 0.12, 0.15, 0.08).with_voice(0)));
    let gain1 = graph.add_node(Box::new(Gain::with_pan(0.70, 0.15)));
    graph.connect(osc1, 0, adsr1, 0);
    graph.connect(adsr1, 0, gain1, 0);

    let osc2 = graph.add_node(Box::new(Oscillator::new(Waveform::Saw).with_voice(1)));
    let adsr2 = graph.add_node(Box::new(Adsr::new(0.008, 0.2, 0.55, 0.12).with_voice(1)));
    let gain2 = graph.add_node(Box::new(Gain::new(0.10)));
    graph.connect(osc2, 0, adsr2, 0);
    graph.connect(adsr2, 0, gain2, 0);

    let kick = graph.add_node(Box::new(KickSynth::new(2)));
    let kick_gain = graph.add_node(Box::new(Gain::new(0.60)));
    graph.connect(kick, 0, kick_gain, 0);

    let snare = graph.add_node(Box::new(SnareSynth::new(3)));
    let snare_gain = graph.add_node(Box::new(Gain::with_pan(0.28, -0.05)));
    graph.connect(snare, 0, snare_gain, 0);

    let hat = graph.add_node(Box::new(HatSynth::new(4)));
    let hat_gain = graph.add_node(Box::new(Gain::with_pan(0.15, 0.25)));
    graph.connect(hat, 0, hat_gain, 0);

    let inst_mix = graph.add_node(Box::new(StereoMixer::new(2)));
    graph.connect(gain1, 0, inst_mix, 0);
    graph.connect(gain1, 1, inst_mix, 1);
    graph.connect(gain2, 0, inst_mix, 2);
    graph.connect(gain2, 1, inst_mix, 3);

    let drum_mix = graph.add_node(Box::new(StereoMixer::new(3)));
    graph.connect(kick_gain, 0, drum_mix, 0);
    graph.connect(kick_gain, 1, drum_mix, 1);
    graph.connect(snare_gain, 0, drum_mix, 2);
    graph.connect(snare_gain, 1, drum_mix, 3);
    graph.connect(hat_gain, 0, drum_mix, 4);
    graph.connect(hat_gain, 1, drum_mix, 5);

    let submix = graph.add_node(Box::new(StereoMixer::new(2)));
    graph.connect(inst_mix, 0, submix, 0);
    graph.connect(inst_mix, 1, submix, 1);
    graph.connect(drum_mix, 0, submix, 2);
    graph.connect(drum_mix, 1, submix, 3);

    let mut eq_proc = ParametricEq::with_bands(150.0, 3000.0, 8000.0);
    eq_proc.set_param(1, -1.5);
    eq_proc.set_param(4, 3.0);
    eq_proc.set_param(7, 1.5);
    let eq = graph.add_node(Box::new(eq_proc));
    graph.connect(submix, 0, eq, 0);
    graph.connect(submix, 1, eq, 1);

    let delay = graph.add_node(Box::new(StereoDelay::new(345.0, 0.30, 0.15)));
    graph.connect(eq, 0, delay, 0);
    graph.connect(eq, 1, delay, 1);

    let reverb = graph.add_node(Box::new(PlateReverb::new(0.35, 0.6, 0.12)));
    graph.connect(delay, 0, reverb, 0);
    graph.connect(delay, 1, reverb, 1);

    let master = graph.add_node(Box::new(StereoMixer::new(1)));
    graph.connect(reverb, 0, master, 0);
    graph.connect(reverb, 1, master, 1);

    let graph_nodes: Vec<(u32, String)> = vec![
        (osc1, "Lead Osc".into()),
        (adsr1, "Lead Env".into()),
        (gain1, "Lead Gain".into()),
        (osc2, "Bass Osc".into()),
        (adsr2, "Bass Env".into()),
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

    let graph_edges = graph.topology().1.clone();

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
    let nested_snapshots = graph.nested_ui_snapshots();

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

    grid.set(0, 1, Some(n(0, -3, 3, 4)));
    grid.set(3, 1, Some(n(0, -3, 5, 8)));
    grid.set(6, 1, Some(n(3, -3, 3, 4)));
    grid.set(7, 1, Some(n(3, -3, 1, 2)));
    grid.set(8, 1, Some(n(5, -3, 3, 4)));
    grid.set(11, 1, Some(n(5, -3, 5, 8)));
    grid.set(12, 1, Some(n(7, -3, 3, 4)));
    grid.set(15, 1, Some(n(10, -3, 7, 8)));

    for step in [0, 3, 4, 8, 10, 12, 15] {
        let vel = match step {
            0 | 4 | 8 | 12 => Rational::new(7, 8),
            _ => Rational::new(5, 8),
        };
        grid.set(step, 2, Some(NoteEvent::new(0, 0, vel)));
    }

    grid.set(2, 3, Some(n(0, 0, 1, 3)));
    grid.set(4, 3, Some(n(0, 0, 3, 4)));
    grid.set(6, 3, Some(n(0, 0, 1, 4)));
    grid.set(10, 3, Some(n(0, 0, 1, 3)));
    grid.set(12, 3, Some(n(0, 0, 3, 4)));
    grid.set(14, 3, Some(n(0, 0, 1, 4)));

    for step in 0..16 {
        let vel = match step % 4 {
            0 => Rational::new(7, 8),
            2 => Rational::new(1, 2),
            _ => Rational::new(1, 4),
        };
        grid.set(step, 4, Some(NoteEvent::new(0, 0, vel)));
    }

    let (bridge, audio_bridge) = trem_cpal::create_bridge(1024);

    let mut app = trem_tui::App::new(
        grid,
        scale,
        "12-EDO".to_string(),
        bridge,
        instrument_names,
        voice_ids,
    )
    .with_graph_info(graph_nodes, graph_edges, param_snapshot)
    .with_nested_graph_snapshots(nested_snapshots);

    app.set_node_descriptions(descriptions);
    app.set_node_children(has_children);
    app.bpm = 130.0;
    Ok((
        app,
        PendingAudio {
            audio_bridge,
            graph,
            output_node: master,
            scope_input_node: Some(inst_mix),
            sample_rate: 44100.0,
        },
    ))
}

#[wasm_bindgen]
pub fn start_trem_web(container_id: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let (app, pending_audio) = build_app().map_err(|e| JsValue::from_str(&e.to_string()))?;

    let backend = DomBackend::new_by_id(container_id)
        .map_err(|e| JsValue::from_str(&format!("failed to create dom backend: {e}")))?;
    let terminal = Terminal::new(backend)
        .map_err(|e| JsValue::from_str(&format!("failed to create terminal: {e}")))?;

    let app = Rc::new(RefCell::new(app));
    let pending_audio = Rc::new(RefCell::new(Some(pending_audio)));
    let audio_engine = Rc::new(RefCell::new(None::<trem_cpal::AudioEngine>));

    terminal.on_key_event({
        let app = app.clone();
        let pending_audio = pending_audio.clone();
        let audio_engine = audio_engine.clone();
        move |key| {
            start_audio_if_needed(&pending_audio, &audio_engine, &app);
            let key = map_web_key(key);
            let (editor, mode, graph_is_nested, help_open) = {
                let app = app.borrow();
                (
                    app.editor,
                    app.mode,
                    !app.graph_path.is_empty(),
                    app.help_open,
                )
            };
            let ctx = InputContext {
                editor,
                mode: &mode,
                graph_is_nested,
                help_open,
            };
            if let Some(action) = trem_tui::input::handle_key_event(key, &ctx) {
                app.borrow_mut().handle_action(action);
            }
        }
    });

    terminal.on_mouse_event({
        let app = app.clone();
        let pending_audio = pending_audio.clone();
        let audio_engine = audio_engine.clone();
        move |mouse| {
            if mouse.event == WebMouseEventKind::Pressed {
                start_audio_if_needed(&pending_audio, &audio_engine, &app);
            }
        }
    });

    terminal.draw_web(move |frame| {
        let _keep_engine_alive = &audio_engine;
        let mut app = app.borrow_mut();
        app.poll_audio();
        app.draw(frame);
    });

    Ok(())
}
