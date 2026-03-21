//! Save/load project state to JSON files.
//!
//! Serializes only the editable musical state: grid cells, BPM, swing, octave,
//! and graph parameter overrides. The audio graph topology and scale are
//! reconstructed from the binary on load.

use serde::{Deserialize, Serialize};
use std::path::Path;
use trem::event::NoteEvent;
use trem::math::Rational;

#[derive(Serialize, Deserialize)]
struct CellData {
    deg: i32,
    oct: i32,
    vel_n: i64,
    vel_d: u64,
    gate_n: i64,
    gate_d: u64,
}

#[derive(Serialize, Deserialize)]
struct ParamOverride {
    node_idx: usize,
    param_id: u32,
    value: f64,
}

#[derive(Serialize, Deserialize)]
pub struct ProjectData {
    pub bpm: f64,
    pub swing: f64,
    pub octave: i32,
    pub rows: u32,
    pub columns: u32,
    cells: Vec<Option<CellData>>,
    param_overrides: Vec<ParamOverride>,
}

impl ProjectData {
    pub fn from_app(app: &crate::App) -> Self {
        let cells: Vec<Option<CellData>> = app
            .grid
            .cells
            .iter()
            .map(|cell| {
                cell.as_ref().map(|n| CellData {
                    deg: n.degree,
                    oct: n.octave,
                    vel_n: n.velocity.num,
                    vel_d: n.velocity.den,
                    gate_n: n.gate.num,
                    gate_d: n.gate.den,
                })
            })
            .collect();

        let mut param_overrides = Vec::new();
        for (node_idx, values) in app.graph_param_values.iter().enumerate() {
            let descs = match app.graph_params.get(node_idx) {
                Some(d) => d,
                None => continue,
            };
            for (i, &val) in values.iter().enumerate() {
                if let Some(desc) = descs.get(i) {
                    if (val - desc.default).abs() > 1e-9 {
                        param_overrides.push(ParamOverride {
                            node_idx,
                            param_id: desc.id,
                            value: val,
                        });
                    }
                }
            }
        }

        Self {
            bpm: app.bpm,
            swing: app.swing,
            octave: app.octave,
            rows: app.grid.rows,
            columns: app.grid.columns,
            cells,
            param_overrides,
        }
    }

    pub fn apply_to_app(&self, app: &mut crate::App) {
        app.bpm = self.bpm;
        app.swing = self.swing;
        app.octave = self.octave;

        if app.grid.rows == self.rows && app.grid.columns == self.columns {
            for (i, cell) in self.cells.iter().enumerate() {
                if i < app.grid.cells.len() {
                    app.grid.cells[i] = cell.as_ref().map(|c| {
                        NoteEvent::new(c.deg, c.oct, Rational::new(c.vel_n, c.vel_d))
                            .with_gate(Rational::new(c.gate_n, c.gate_d))
                    });
                }
            }
        }

        for ov in &self.param_overrides {
            if let Some(values) = app.graph_param_values.get_mut(ov.node_idx) {
                if let Some(descs) = app.graph_params.get(ov.node_idx) {
                    if let Some(pos) = descs.iter().position(|d| d.id == ov.param_id) {
                        values[pos] = ov.value;
                        let node_id = app.graph_nodes[ov.node_idx].0;
                        app.bridge.send(trem_cpal::Command::SetParam {
                            node: node_id,
                            param_id: ov.param_id,
                            value: ov.value,
                        });
                    }
                }
            }
        }

        app.bridge.send(trem_cpal::Command::SetBpm(self.bpm));
    }
}

pub fn save(path: &Path, data: &ProjectData) -> anyhow::Result<()> {
    let json = serde_json::to_string_pretty(data)?;
    std::fs::write(path, json)?;
    Ok(())
}

pub fn load(path: &Path) -> anyhow::Result<ProjectData> {
    let json = std::fs::read_to_string(path)?;
    let data: ProjectData = serde_json::from_str(&json)?;
    Ok(data)
}
