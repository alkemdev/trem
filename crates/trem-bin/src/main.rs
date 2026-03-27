//! `trem` — default: synth TUI. **`trem rung`** / **`trem clip`** — import MIDI, edit Rung JSON.

mod cli;
mod demo;
mod rung_editor;
mod rung_playback;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use std::io;

fn main() -> Result<()> {
    let cli = cli::Cli::parse();
    match cli.command {
        None | Some(cli::Commands::Tui) => run_tui(),
        Some(cli::Commands::Rung { sub }) => match sub {
            cli::RungCommands::Import {
                input,
                output,
                class_offset,
            } => cli::run_rung_import(input, output, class_offset),
            cli::RungCommands::Edit { path } => rung_editor::run(path),
        },
    }
}

fn run_tui() -> Result<()> {
    let workspace =
        trem_tui::project::ProjectWorkspace::load(&trem_tui::project::default_project_path())?;
    let (graph, output_node, inst_bus_id, _graph_nodes) = demo::graph::build_graph();
    let (bridge, audio_bridge) = trem_rta::create_bridge(4096);
    let _engine =
        trem_rta::AudioEngine::new(audio_bridge, graph, output_node, Some(inst_bus_id), 44100.0)?;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;

    let backend = ratatui::backend::CrosstermBackend::new(stdout);
    let mut terminal = ratatui::Terminal::new(backend)?;

    let app = trem_tui::App::from_workspace(workspace, bridge);

    let result = app.run(&mut terminal);

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    result
}
