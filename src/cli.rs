//! CLI: default is the synth TUI; **`trem rung …`** for clip tools.

use anyhow::{Context, Result};
use clap::{Parser, Subcommand};
use std::fs;
use std::path::{Path, PathBuf};

use trem_rung::midi::{import_midi_file, MidiImportOptions};

#[derive(Parser)]
#[command(
    name = "trem",
    about = "Mathematical music engine — terminal UI and clip utilities",
    subcommand_required = false
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Commands>,
}

#[derive(Subcommand)]
pub enum Commands {
    /// Pattern + graph terminal UI (synth demo; same as bare `trem`)
    Tui,
    /// Rung clip interchange (.rung.json) — import, edit
    Rung {
        #[command(subcommand)]
        sub: RungCommands,
    },
}

#[derive(Subcommand)]
pub enum RungCommands {
    /// Convert Standard MIDI File (.mid) to Rung JSON
    Import {
        /// Input .mid path
        input: PathBuf,
        /// Output path (default: <input stem>.rung.json)
        #[arg(short, long)]
        output: Option<PathBuf>,
        /// Added to every MIDI note number → `ClipNote::class`
        #[arg(long, default_value_t = 0)]
        class_offset: i32,
    },
    /// Piano-roll editor for a .rung.json file (interactive TTY)
    Edit {
        /// Path to .rung.json
        path: PathBuf,
    },
}

pub fn run_rung_import(input: PathBuf, output: Option<PathBuf>, class_offset: i32) -> Result<()> {
    let bytes = fs::read(&input).with_context(|| format!("read {}", input.display()))?;
    let file = import_midi_file(
        &bytes,
        MidiImportOptions {
            class_offset,
            ..MidiImportOptions::default()
        },
    )
    .map_err(|e| anyhow::anyhow!("{e}"))?;
    let json = file
        .to_json_pretty()
        .map_err(|e| anyhow::anyhow!("serialize: {e}"))?;

    let out_path = output.unwrap_or_else(|| default_rung_path(&input));
    if let Some(parent) = out_path.parent() {
        if !parent.as_os_str().is_empty() {
            fs::create_dir_all(parent).with_context(|| format!("mkdir {}", parent.display()))?;
        }
    }
    fs::write(&out_path, json).with_context(|| format!("write {}", out_path.display()))?;
    eprintln!(
        "wrote {} ({} notes)",
        out_path.display(),
        file.clip.notes.len()
    );
    Ok(())
}

fn default_rung_path(input: &Path) -> PathBuf {
    let stem = input.file_stem().and_then(|s| s.to_str()).unwrap_or("out");
    input
        .parent()
        .unwrap_or(Path::new("."))
        .join(format!("{stem}.rung.json"))
}
