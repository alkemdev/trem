//! Piano-roll style TUI for [`trem_rung::RungFile`]: time → horizontal, class → vertical.

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyCode, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use num_rational::Rational64;
use ratatui::layout::{Constraint, Layout};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use std::fs;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use trem_rung::{BeatTime, Clip, ClipNote, RungFile};

use crate::rung_playback::RungPlayback;

#[inline]
fn beat_step() -> Rational64 {
    Rational64::new(1, 16)
}

#[inline]
fn min_dur() -> Rational64 {
    Rational64::new(1, 64)
}

/// Horizontal span visible in the roll, left edge (beats), top row = this class (higher = up).
#[derive(Clone, Copy, Debug)]
struct Viewport {
    origin_beat: Rational64,
    width_beats: Rational64,
    top_class: i32,
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            origin_beat: Rational64::from_integer(0),
            width_beats: Rational64::from_integer(4),
            top_class: 72,
        }
    }
}

impl Viewport {
    fn clamp_width(&mut self) {
        let min_w = Rational64::new(1, 16);
        let max_w = Rational64::from_integer(10_000);
        if self.width_beats < min_w {
            self.width_beats = min_w;
        }
        if self.width_beats > max_w {
            self.width_beats = max_w;
        }
    }

    fn zoom_in(&mut self) {
        self.width_beats = self.width_beats * Rational64::new(2, 3);
        self.clamp_width();
    }

    fn zoom_out(&mut self) {
        self.width_beats = self.width_beats * Rational64::new(3, 2);
        self.clamp_width();
    }

    fn pan_time(&mut self, delta: Rational64) {
        self.origin_beat = self.origin_beat + delta;
    }

    /// Column `col` in `0..cols` → [t0, t1) in beats.
    fn col_range(self, col: u16, cols: u16) -> (Rational64, Rational64) {
        if cols == 0 {
            return (self.origin_beat, self.origin_beat);
        }
        let c = Rational64::from_integer(cols as i64);
        let t0 = self.origin_beat + (self.width_beats * Rational64::from_integer(col as i64)) / c;
        let t1 =
            self.origin_beat + (self.width_beats * Rational64::from_integer((col + 1) as i64)) / c;
        (t0, t1)
    }
}

fn clip_bounds(file: &RungFile) -> Option<(Rational64, Rational64, i32, i32)> {
    if file.clip.notes.is_empty() {
        return None;
    }
    let mut t_min = None::<Rational64>;
    let mut t_max = None::<Rational64>;
    let mut c_min = i32::MAX;
    let mut c_max = i32::MIN;
    for n in &file.clip.notes {
        let a = n.t_on.rational();
        let b = n.t_off.rational();
        t_min = Some(t_min.map_or(a, |m| m.min(a)));
        t_max = Some(t_max.map_or(b, |m| m.max(b)));
        c_min = c_min.min(n.class);
        c_max = c_max.max(n.class);
    }
    Some((t_min.unwrap(), t_max.unwrap(), c_min, c_max))
}

fn fit_all(file: &RungFile, vp: &mut Viewport) {
    if let Some((t_min, t_max, _c_min, c_max)) = clip_bounds(file) {
        let span = (t_max - t_min).max(Rational64::new(1, 4));
        let pad = span / Rational64::from_integer(10);
        vp.origin_beat = t_min - pad;
        vp.width_beats = (t_max - t_min) + pad * Rational64::from_integer(2);
        vp.width_beats = vp.width_beats.max(Rational64::new(1, 4));
        vp.clamp_width();
        let margin = 2i32;
        vp.top_class = c_max + margin;
    } else {
        *vp = Viewport::default();
    }
}

fn center_on_selected(file: &RungFile, selected: usize, grid_rows: u16, vp: &mut Viewport) {
    let Some(n) = file.clip.notes.get(selected) else {
        return;
    };
    let mid = (n.t_on.rational() + n.t_off.rational()) / Rational64::from_integer(2);
    vp.origin_beat = mid - vp.width_beats / Rational64::from_integer(2);
    let half = (grid_rows / 2) as i32;
    vp.top_class = n.class + half.max(1);
}

/// Pick display char and whether this cell is the selected note.
fn cell_state(
    class: i32,
    t0: Rational64,
    t1: Rational64,
    file: &RungFile,
    selected: usize,
) -> (char, bool) {
    let mut best: Option<(usize, bool)> = None;
    for (i, n) in file.clip.notes.iter().enumerate() {
        if n.class != class {
            continue;
        }
        if n.t_off.rational() <= t0 || n.t_on.rational() >= t1 {
            continue;
        }
        let sel = i == selected;
        match best {
            None => best = Some((i, sel)),
            Some((_, was_sel)) => {
                if sel && !was_sel {
                    best = Some((i, sel));
                }
            }
        }
    }
    match best {
        Some((_, true)) => ('█', true),
        Some(_) => ('▓', false),
        None => ('·', false),
    }
}

fn beat_tick_col(vp: Viewport, beat: Rational64, cols: u16) -> Option<u16> {
    if cols == 0 {
        return None;
    }
    if vp.width_beats <= Rational64::from_integer(0) {
        return None;
    }
    if beat < vp.origin_beat || beat >= vp.origin_beat + vp.width_beats {
        return None;
    }
    let rel = beat - vp.origin_beat;
    let x = (rel * Rational64::from_integer(cols as i64)) / vp.width_beats;
    let num = *x.numer();
    let den = *x.denom();
    if den == 0 {
        return None;
    }
    let col = num
        .div_euclid(den)
        .clamp(0, i64::from(cols.saturating_sub(1)));
    Some(col as u16)
}

fn ruler_line(vp: Viewport, cols: u16) -> Line<'static> {
    if cols == 0 {
        return Line::from("");
    }
    let mut buf = vec![b' '; cols as usize];
    let start = floor_beat(vp.origin_beat);
    let end = ceil_beat(vp.origin_beat + vp.width_beats) + 1;
    for b in start..=end {
        let bt = Rational64::from_integer(b);
        if let Some(col) = beat_tick_col(vp, bt, cols) {
            let i = col as usize;
            if i < buf.len() {
                buf[i] = if b.rem_euclid(4) == 0 { b':' } else { b'|' };
            }
        }
    }
    Line::from(String::from_utf8_lossy(&buf).into_owned())
}

fn floor_beat(r: Rational64) -> i64 {
    let n = *r.numer();
    let d = *r.denom();
    if d == 0 {
        return 0;
    }
    n.div_euclid(d)
}

fn ceil_beat(r: Rational64) -> i64 {
    let n = *r.numer();
    let d = *r.denom();
    if d == 0 {
        return 0;
    }
    (n + d - 1).div_euclid(d)
}

fn render_roll_row(
    class: i32,
    vp: Viewport,
    cols: u16,
    file: &RungFile,
    selected: usize,
) -> Line<'static> {
    let mut spans = Vec::new();
    for col in 0..cols {
        let (t0, t1) = vp.col_range(col, cols);
        let (ch, sel) = cell_state(class, t0, t1, file, selected);
        let st = if sel {
            Style::default().fg(Color::Yellow).bg(Color::DarkGray)
        } else if ch == '▓' {
            Style::default().fg(Color::LightBlue)
        } else {
            Style::default().fg(Color::DarkGray)
        };
        spans.push(Span::styled(ch.to_string(), st));
    }
    Line::from(spans)
}

fn reload_playback_pattern(pb: &mut Option<RungPlayback>, clip: &Clip) {
    if let Some(p) = pb.as_mut() {
        p.reload_clip(clip);
    }
}

fn rational_to_f64(r: Rational64) -> f64 {
    let n = *r.numer() as f64;
    let d = *r.denom() as f64;
    if d == 0.0 {
        0.0
    } else {
        n / d
    }
}

pub fn run(path: PathBuf) -> Result<()> {
    if !io::stdout().is_terminal() || !io::stdin().is_terminal() {
        anyhow::bail!(
            "rung edit needs an interactive terminal (stdout and stdin must be TTYs). \
             Run from a terminal, not a pipe or IDE task without a PTY."
        );
    }

    let text = fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
    let mut file = RungFile::from_json(&text).map_err(|e| anyhow::anyhow!("{e}"))?;

    file.clip.notes.sort_by(|a, b| {
        a.t_on
            .rational()
            .cmp(&b.t_on.rational())
            .then_with(|| a.voice.cmp(&b.voice))
            .then_with(|| a.class.cmp(&b.class))
    });

    let mut playback = RungPlayback::try_new();
    match &mut playback {
        Some(p) => {
            p.reload_clip(&file.clip);
        }
        None => {
            eprintln!(
                "trem: rung edit — no audio output (missing device or non-f32 format); editing only."
            );
        }
    }

    let mut selected: usize = 0;
    let mut dirty = false;
    let mut quit_unsaved = false;
    let mut vp = Viewport::default();
    let mut did_autofit = false;

    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let path_display = path.display().to_string();

    let res = loop {
        if !did_autofit && !file.clip.notes.is_empty() {
            if let Ok(sz) = terminal.size() {
                let _ = sz;
                fit_all(&file, &mut vp);
                did_autofit = true;
            }
        }

        if !file.clip.notes.is_empty() {
            selected = selected.min(file.clip.notes.len() - 1);
        } else {
            selected = 0;
        }

        let term_h = terminal.size().map(|s| s.height).unwrap_or(24);
        let grid_rows = term_h.saturating_sub(9).max(4);

        let transport = match &playback {
            Some(p) => format!(
                "  {}  {:.0} BPM  ~{:.2} beat",
                if p.playing { "PLAY" } else { "stop" },
                p.bpm,
                p.last_beat
            ),
            None => "  (no audio)".to_string(),
        };

        terminal.draw(|f| {
            let area = f.area();
            let chunks = Layout::default()
                .direction(Direction::Vertical)
                .constraints([
                    Constraint::Length(1),
                    Constraint::Min(8),
                    Constraint::Length(2),
                    Constraint::Length(2),
                ])
                .split(area);

            let title = if dirty { "* " } else { "" };
            let header = Paragraph::new(Line::from(vec![
                title.into(),
                "rung roll ".into(),
                path_display.clone().cyan(),
                "  ".into(),
                format!("{} notes", file.clip.notes.len()).dim(),
                "  ".into(),
                format!(
                    "span {:.2} beats  [{:.3} … {:.3})",
                    rational_to_f64(vp.width_beats),
                    rational_to_f64(vp.origin_beat),
                    rational_to_f64(vp.origin_beat + vp.width_beats)
                )
                .dim(),
                transport.clone().magenta(),
            ]))
            .block(Block::default().borders(Borders::BOTTOM));
            f.render_widget(header, chunks[0]);

            let body = chunks[1];
            let block = Block::default()
                .borders(Borders::ALL)
                .title(" piano roll — class ↑  time → ");
            let inner = block.inner(body);
            f.render_widget(block, body);

            let cols_lr = Layout::default()
                .direction(Direction::Horizontal)
                .constraints([Constraint::Length(6), Constraint::Min(8)])
                .split(inner);

            let left_col = cols_lr[0];
            let right_col = cols_lr[1];

            let rows_rl = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(4)])
                .split(right_col);

            let ruler_area = rows_rl[0];
            let grid_area = rows_rl[1];
            let roll_cols = grid_area.width.max(1);
            let grid_h = grid_area.height.max(1);

            let ruler = Paragraph::new(ruler_line(vp, ruler_area.width.max(1)));
            f.render_widget(ruler, ruler_area);

            let lines: Vec<Line> = (0..grid_h)
                .map(|row| {
                    let class = vp.top_class - row as i32;
                    render_roll_row(class, vp, roll_cols, &file, selected)
                })
                .collect();
            f.render_widget(Paragraph::new(Text::from(lines)), grid_area);

            let rows_left = Layout::default()
                .direction(Direction::Vertical)
                .constraints([Constraint::Length(1), Constraint::Min(4)])
                .split(left_col);

            let label_head = Paragraph::new(Span::styled("class", Style::default().fg(Color::DarkGray)))
                .alignment(Alignment::Right);
            f.render_widget(label_head, rows_left[0]);

            let label_lines: Vec<Line> = (0..grid_h)
                .map(|row| {
                    let class = vp.top_class - row as i32;
                    Line::from(Span::styled(
                        format!("{:>5}", class),
                        Style::default().fg(Color::Green),
                    ))
                })
                .collect();
            let labels = Paragraph::new(Text::from(label_lines))
                .alignment(Alignment::Right)
                .block(Block::default().borders(Borders::RIGHT));
            f.render_widget(labels, rows_left[1]);

            let detail = if let Some(n) = file.clip.notes.get(selected) {
                let dur = n.t_off.rational() - n.t_on.rational();
                format!(
                    "sel #{selected}  class {:>4}  t_on {}  t_off {}  Δ {}/{}  voice {}  vel {:.2}",
                    n.class,
                    n.t_on,
                    n.t_off,
                    dur.numer(),
                    dur.denom(),
                    n.voice,
                    n.velocity
                )
            } else {
                "(no notes)".to_string()
            };
            let sel_line = Paragraph::new(Line::from(vec![detail.white()]))
                .block(Block::default().borders(Borders::ALL).title(" selection "));
            f.render_widget(sel_line, chunks[2]);

            let help = Paragraph::new(Line::from(vec![
                "hl time  kj rows  zx zoom  bf note  g center  a fit  +/- class  [] t_off  ,. t_on  12 vel  er voice  space play/pause  9/0 BPM  s save  q quit"
                    .yellow(),
            ]));
            f.render_widget(help, chunks[3]);
        })?;

        if let Some(p) = playback.as_mut() {
            p.drain_ui();
        }

        let evt = event::read()?;
        let Event::Key(key) = evt else { continue };
        if key.kind != KeyEventKind::Press {
            continue;
        }

        match key.code {
            KeyCode::Char('q') | KeyCode::Char('Q') => {
                quit_unsaved = dirty;
                break Ok(());
            }
            KeyCode::Char('s') | KeyCode::Char('S') => {
                sync_length(&mut file);
                if let Err(e) = save(&path, &file) {
                    break Err(anyhow::anyhow!("save failed: {e}"));
                }
                dirty = false;
                reload_playback_pattern(&mut playback, &file.clip);
            }
            KeyCode::Char(' ') => {
                if let Some(p) = playback.as_mut() {
                    p.toggle_playback();
                }
            }
            KeyCode::Char('9') => {
                if let Some(p) = playback.as_mut() {
                    p.nudge_bpm(-5.0, &file.clip);
                }
            }
            KeyCode::Char('0') => {
                if let Some(p) = playback.as_mut() {
                    p.nudge_bpm(5.0, &file.clip);
                }
            }
            KeyCode::Char('f') => {
                if !file.clip.notes.is_empty() {
                    selected = (selected + 1).min(file.clip.notes.len() - 1);
                }
            }
            KeyCode::Char('b') => {
                selected = selected.saturating_sub(1);
            }
            KeyCode::Char('h') | KeyCode::Left => {
                vp.pan_time(-vp.width_beats / Rational64::from_integer(8));
            }
            KeyCode::Char('l') | KeyCode::Right => {
                vp.pan_time(vp.width_beats / Rational64::from_integer(8));
            }
            KeyCode::Char('k') | KeyCode::Up => {
                vp.top_class += 1;
            }
            KeyCode::Char('j') | KeyCode::Down => {
                vp.top_class -= 1;
            }
            KeyCode::Char('z') => {
                vp.zoom_in();
            }
            KeyCode::Char('x') => {
                vp.zoom_out();
            }
            KeyCode::Char('g') => {
                center_on_selected(&file, selected, grid_rows, &mut vp);
            }
            KeyCode::Char('a') => {
                fit_all(&file, &mut vp);
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.class = n.class.wrapping_add(1);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.class = n.class.wrapping_sub(1);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char(']') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.t_off = BeatTime(n.t_off.rational() + beat_step());
                    clamp_duration(n);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('[') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.t_off = BeatTime(n.t_off.rational() - beat_step());
                    clamp_duration(n);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('.') | KeyCode::Char('>') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.t_on = BeatTime(n.t_on.rational() + beat_step());
                    clamp_duration(n);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char(',') | KeyCode::Char('<') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.t_on = BeatTime(n.t_on.rational() - beat_step());
                    clamp_duration(n);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('1') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.velocity = (n.velocity - 0.05).clamp(0.0, 1.0);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('2') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.velocity = (n.velocity + 0.05).clamp(0.0, 1.0);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('e') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.voice = n.voice.saturating_sub(1);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            KeyCode::Char('r') => {
                if let Some(n) = file.clip.notes.get_mut(selected) {
                    n.voice = (n.voice + 1).min(15);
                    dirty = true;
                    reload_playback_pattern(&mut playback, &file.clip);
                }
            }
            _ => {}
        }
    };

    if let Some(ref mut p) = playback {
        p.stop();
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res?;
    if quit_unsaved {
        eprintln!("trem: exited rung edit with unsaved changes (use s to write file)");
    }
    Ok(())
}

fn clamp_duration(n: &mut ClipNote) {
    if n.t_off.rational() <= n.t_on.rational() {
        n.t_off = BeatTime(n.t_on.rational() + min_dur());
    }
}

fn sync_length(file: &mut RungFile) {
    file.clip.length_beats = file.clip.notes.iter().map(|n| n.t_off).max();
}

fn save(path: &PathBuf, file: &RungFile) -> Result<()> {
    file.validate().map_err(|e| anyhow::anyhow!("{e}"))?;
    let json = file
        .to_json_pretty()
        .map_err(|e| anyhow::anyhow!("serialize: {e}"))?;
    fs::write(path, json).with_context(|| format!("write {}", path.display()))?;
    eprintln!("saved {}", path.display());
    Ok(())
}
