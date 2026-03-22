//! **Perf / host / meters** drawing for the bottom of [`super::info::InfoView`].

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};

fn meter_bar(peak: f32, width: usize) -> (String, Style) {
    let filled = ((peak.clamp(0.0, 1.0) * width as f32).round() as usize).min(width);
    let bar: String = "\u{2588}".repeat(filled) + &"\u{2591}".repeat(width - filled);
    let color = if peak > 0.9 {
        theme::ACCENT
    } else if peak > 0.6 {
        theme::YELLOW
    } else {
        theme::GREEN
    };
    (bar, Style::new().fg(color).bg(theme::BG))
}

/// **This `trem` process only** (same PID as the TUI), refreshed periodically by [`crate::App`].
#[derive(Clone, Debug, Default)]
pub struct HostStatsSnapshot {
    /// CPU % for this process only ([`sysinfo::Process::cpu_usage`]): **per full core**, so values
    /// **can exceed 100%** if the process uses more than one core at once.
    pub process_cpu_pct: f32,
    /// Resident set size for this process (MiB).
    pub process_rss_mb: u64,
}

/// Draw **PERF** / **HOST** / **OUT** below keys, aligned with the info column (`x`, `w`).
pub(crate) fn draw_perf_sections(
    buf: &mut Buffer,
    x: u16,
    w: u16,
    y: &mut u16,
    y_max: u16,
    host_stats: &HostStatsSnapshot,
    peak_l: f32,
    peak_r: f32,
    playing: bool,
    bpm: f64,
) {
    if *y >= y_max || w < 8 {
        return;
    }

    let dim = Style::new().fg(theme::DIM).bg(theme::BG);
    let val = theme::value();
    let section = Style::new()
        .fg(theme::ACCENT)
        .bg(theme::BG)
        .add_modifier(Modifier::BOLD);

    let draw_section = |buf: &mut Buffer, y: &mut u16, label: &str| -> bool {
        if *y >= y_max {
            return false;
        }
        let pad = w.saturating_sub(label.len() as u16 + 1) as usize;
        let line = Line::from(vec![
            Span::styled(format!(" {} ", label), section),
            Span::styled("\u{2500}".repeat(pad), dim),
        ]);
        buf.set_line(x, *y, &line, w);
        *y += 1;
        true
    };

    let draw_kv = |buf: &mut Buffer, y: &mut u16, key: &str, value_spans: Vec<Span>| -> bool {
        if *y >= y_max {
            return false;
        }
        let mut spans = vec![Span::styled(format!(" {:<7}", key), dim)];
        spans.extend(value_spans);
        buf.set_line(x, *y, &Line::from(spans), w);
        *y += 1;
        true
    };

    *y += 1;
    if *y >= y_max {
        return;
    }

    if !draw_section(buf, y, "PERF") {
        return;
    }

    let play_st = if playing {
        Style::new().fg(theme::GREEN).bg(theme::BG)
    } else {
        dim
    };
    draw_kv(
        buf,
        y,
        "Play",
        vec![
            Span::styled(if playing { "on" } else { "off" }, play_st),
            Span::styled(" · ", dim),
            Span::styled(format!("{:.0} BPM", bpm), val),
        ],
    );

    *y += 1;
    if *y >= y_max {
        return;
    }

    if !draw_section(buf, y, "PROC") {
        return;
    }
    draw_kv(
        buf,
        y,
        "trem",
        vec![Span::styled(
            format!("{:.0}% CPU", host_stats.process_cpu_pct),
            val,
        )],
    );
    draw_kv(
        buf,
        y,
        "RSS",
        vec![Span::styled(
            format!("{} MiB", host_stats.process_rss_mb),
            Style::new().fg(theme::YELLOW).bg(theme::BG),
        )],
    );

    *y += 1;
    if *y >= y_max {
        return;
    }

    if !draw_section(buf, y, "OUT") {
        return;
    }

    let meter_w = (w as usize).saturating_sub(9).min(12);
    if meter_w > 0 {
        let (bar_l, style_l) = meter_bar(peak_l, meter_w);
        draw_kv(buf, y, "L", vec![Span::styled(bar_l, style_l)]);
        let (bar_r, style_r) = meter_bar(peak_r, meter_w);
        draw_kv(buf, y, "R", vec![Span::styled(bar_r, style_r)]);
    }
}
