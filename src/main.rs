//! `trem` — terminal UI entrypoint. Patch + pattern live in [`demo`].

mod demo;

use anyhow::Result;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;

use trem::pitch::Tuning;

const DEMO_BPM: f64 = 146.0;

fn main() -> Result<()> {
    let scale = Tuning::edo12().to_scale();
    let (graph, output_node, inst_bus_id, graph_nodes) = demo::build_graph();

    let graph_edges = graph.topology().1;

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
    let grid = demo::build_pattern();

    let instrument_names = vec![
        "Lead".into(),
        "Bass".into(),
        "Kick".into(),
        "Snare".into(),
        "Hat".into(),
    ];
    let voice_ids = vec![0, 1, 2, 3, 4];

    let (bridge, audio_bridge) = trem_cpal::create_bridge(1024);
    let _engine =
        trem_cpal::AudioEngine::new(audio_bridge, graph, output_node, Some(inst_bus_id), 44100.0)?;

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
    .with_graph_info(graph_nodes, graph_edges, param_snapshot)
    .with_nested_graph_snapshots(nested_snapshots);

    app.set_node_descriptions(descriptions);
    app.set_node_children(has_children);
    app.bpm = DEMO_BPM;
    app.bridge.send(trem_cpal::Command::SetBpm(DEMO_BPM));

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
