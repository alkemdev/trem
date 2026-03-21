//! TUI color palette and small [`ratatui::style::Style`] builders shared by widgets.
//!
//! **Palette:** `BG`/`FG` base text; `SURFACE` panels; `ACCENT`/`TITLE` emphasis; `DIM`/`MUTED` de-emphasis;
//! `HIGHLIGHT` cursor; `ACTIVE_ROW` playhead; `NOTE_COLOR` filled cells; `GREEN`/`YELLOW` status accents.

use ratatui::style::{Color, Modifier, Style};

pub const BG: Color = Color::Rgb(18, 18, 24);
pub const FG: Color = Color::Rgb(204, 204, 220);
pub const ACCENT: Color = Color::Rgb(255, 90, 120);
pub const DIM: Color = Color::Rgb(88, 88, 108);
pub const HIGHLIGHT: Color = Color::Rgb(60, 60, 90);
pub const ACTIVE_ROW: Color = Color::Rgb(35, 55, 35);
pub const NOTE_COLOR: Color = Color::Rgb(120, 215, 255);
pub const SURFACE: Color = Color::Rgb(28, 28, 38);
pub const MUTED: Color = Color::Rgb(55, 55, 70);
pub const GREEN: Color = Color::Rgb(80, 200, 120);
pub const YELLOW: Color = Color::Rgb(230, 200, 80);

/// Top bar / section titles.
pub fn header() -> Style {
    Style::new().fg(ACCENT).bg(BG).add_modifier(Modifier::BOLD)
}

/// Default empty grid cell.
pub fn cell_empty() -> Style {
    Style::new().fg(DIM).bg(BG)
}

/// Cell that contains a note (before velocity tinting).
pub fn cell_note() -> Style {
    Style::new().fg(NOTE_COLOR).bg(BG)
}

/// Highlight for the editor cursor cell.
pub fn cell_cursor() -> Style {
    Style::new()
        .fg(FG)
        .bg(HIGHLIGHT)
        .add_modifier(Modifier::BOLD)
}

/// Transport strip background/text.
pub fn transport() -> Style {
    Style::new().fg(FG).bg(SURFACE)
}

/// Widget borders and dividers.
pub fn border() -> Style {
    Style::new().fg(MUTED).bg(BG)
}

/// Secondary labels (dim on canvas).
pub fn label() -> Style {
    Style::new().fg(DIM).bg(BG)
}

/// Primary values next to labels.
pub fn value() -> Style {
    Style::new().fg(FG).bg(BG)
}

/// Panel titles (accent, bold).
pub fn title() -> Style {
    Style::new().fg(ACCENT).bg(BG).add_modifier(Modifier::BOLD)
}

/// Map note velocity (0.0–1.0) to a color gradient.
/// Ghost notes are dim, loud notes are vivid.
pub fn note_velocity_color(vel: f64) -> Color {
    let t = vel.clamp(0.0, 1.0) as f32;
    if t < 0.5 {
        let s = t * 2.0;
        Color::Rgb(
            (40.0 + s * 50.0) as u8,
            (55.0 + s * 120.0) as u8,
            (100.0 + s * 130.0) as u8,
        )
    } else {
        let s = (t - 0.5) * 2.0;
        Color::Rgb(
            (90.0 + s * 165.0) as u8,
            (175.0 + s * 65.0) as u8,
            (230.0 + s * 25.0) as u8,
        )
    }
}
