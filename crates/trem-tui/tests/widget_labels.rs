//! Buffer-level checks that important **labels** render (no TTY).
//!
//! Run: `cargo test -p trem-tui --test widget_labels`

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::Widget;
use trem::pitch::Tuning;
use trem_tui::input::{BottomPane, Editor, Mode};
use trem_tui::view::fullscreen::FullscreenHud;
use trem_tui::view::help::HelpOverlay;
use trem_tui::view::info::InfoView;
use trem_tui::view::perf::HostStatsSnapshot;
use trem_tui::view::status::StatusBar;
use trem_tui::view::transport::TransportView;

fn flatten_buffer(area: Rect, buf: &Buffer) -> String {
    let mut s = String::new();
    for y in area.y..area.y + area.height {
        for x in area.x..area.x + area.width {
            if let Some(cell) = buf.cell((x, y)) {
                s.push_str(cell.symbol());
            }
        }
    }
    s
}

#[test]
fn help_overlay_contains_section_titles() {
    let area = Rect::new(0, 0, 92, 46);
    let mut buf = Buffer::empty(area);
    HelpOverlay {
        project_mode: false,
        zone: "GRF",
        mode: "NAV",
        tool: "node-focus",
    }
    .render(area, &mut buf);
    let t = flatten_buffer(area, &buf);
    assert!(t.contains("GLOBAL"), "{}", &t[..t.len().min(200)]);
    assert!(t.contains("SEQ"), "{}", t);
    assert!(t.contains("GRF"), "{}", t);
    assert!(t.contains("zone GRF"), "{}", t);
    assert!(t.contains("help pane") || t.contains("info pane"), "{}", t);
}

#[test]
fn info_view_includes_perf_at_bottom() {
    let area = Rect::new(0, 0, 26, 48);
    let mut buf = Buffer::empty(area);
    let scale = Tuning::edo12().to_scale();
    let mode = Mode::Normal;
    let instruments = vec!["Lead".to_string()];
    let stats = HostStatsSnapshot {
        process_cpu_pct: 12.0,
        process_rss_mb: 99,
    };
    InfoView {
        mode: &mode,
        editor: &Editor::Pattern,
        octave: 0,
        cursor_step: 0,
        cursor_voice: 0,
        grid_steps: 16,
        grid_voices: 4,
        note_at_cursor: None,
        scale: &scale,
        scale_name: "12-EDO",
        instrument_names: &instruments,
        swing: 0.0,
        euclidean_k: 0,
        undo_depth: 0,
        node_description: "",
        param_help: "",
        graph_node_name: None,
        graph_can_enter_nested: false,
        graph_is_nested: false,
        host_stats: &stats,
        peak_l: 0.4,
        peak_r: 0.6,
        playing: true,
        bpm: 146.0,
    }
    .render(area, &mut buf);
    let t = flatten_buffer(area, &buf);
    assert!(t.contains("CURSOR"), "{}", &t[..t.len().min(120)]);
    assert!(t.contains("PROJECT"), "{}", t);
    assert!(t.contains("KEYS"), "{}", t);
    assert!(t.contains("PERF"), "{}", t);
    assert!(t.contains("PROC"), "{}", t);
    assert!(t.contains("OUT"), "{}", t);
    assert!(t.contains("146") || t.contains("BPM"), "{}", t);
}

#[test]
fn transport_still_brackets_active_tab() {
    let area = Rect::new(0, 0, 160, 1);
    let mut buf = Buffer::empty(area);
    let mode = Mode::Normal;
    TransportView {
        bpm: 120.0,
        beat_position: 0.0,
        playing: false,
        mode: &mode,
        editor: &Editor::Graph,
        zone: "GRF",
        mode_label: "NAV",
        tool_label: "node-focus",
        focus_path: "Project > Graph > Lead",
        project_mode: false,
        project_name: None,
        scale_name: "12-EDO",
        octave: 0,
        swing: 0.0,
        bottom_pane: BottomPane::Waveform,
    }
    .render(area, &mut buf);
    let t = flatten_buffer(area, &buf);
    assert!(t.contains("[GRAPH]"), "{}", t);
    assert!(t.contains("SEQ"), "{}", t);
    assert!(t.contains("Project > Graph > Lead"), "{}", t);
}

#[test]
fn status_bar_shows_selection_and_esc_target() {
    let area = Rect::new(0, 0, 160, 1);
    let mut buf = Buffer::empty(area);
    StatusBar {
        selection: "node duck gain -6 dB",
        actions: "e params · Enter focus · ? help",
        esc_hint: Some("Esc back to Graph"),
    }
    .render(area, &mut buf);
    let t = flatten_buffer(area, &buf);
    assert!(t.contains("node duck gain -6 dB"), "{}", t);
    assert!(t.contains("Enter focus"), "{}", t);
    assert!(t.contains("Esc back to Graph"), "{}", t);
}

#[test]
fn fullscreen_hud_shows_exit_chord() {
    let area = Rect::new(0, 0, 120, 1);
    let mut buf = Buffer::empty(area);
    FullscreenHud {
        zone: "ROL",
        mode: "EDIT",
        tool: "move",
        focus_path: "Project > Scene > ROL lead",
        esc_hint: Some("Esc apply + back to Overview"),
    }
    .render(area, &mut buf);
    let t = flatten_buffer(area, &buf);
    assert!(t.contains("FULL"), "{}", t);
    assert!(t.contains("Shift+Enter"), "{}", t);
    assert!(t.contains("ROL"), "{}", t);
}
