//! Real-time spectrum analyzer: windowed FFT of the scope buffer rendered as
//! frequency bars with logarithmic binning and a smooth color gradient.
//!
//! FFT computation is delegated to [`realfft`] (SIMD-accelerated via `rustfft`).

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Padding, Widget};
use std::f64::consts::PI;

const FFT_SIZE: usize = 512;
const BAR_CHARS: [&str; 8] = [
    "\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}", "\u{2585}", "\u{2586}", "\u{2587}", "\u{2588}",
];

/// FFT spectrum analyzer widget. Windowed FFT with logarithmic frequency
/// binning, rendered as coloured vertical bars.
pub struct SpectrumView<'a> {
    pub samples: &'a [f32],
}

impl<'a> Widget for SpectrumView<'a> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let block = Block::new()
            .borders(Borders::TOP)
            .border_style(theme::border())
            .padding(Padding::ZERO)
            .style(Style::new().bg(theme::BG));

        let inner = block.inner(area);
        block.render(area, buf);

        if inner.width < 4 || inner.height == 0 {
            return;
        }

        let stereo_pairs = self.samples.len() / 2;
        if stereo_pairs < 4 {
            return;
        }

        let n = FFT_SIZE.min(stereo_pairs.next_power_of_two()).max(16);
        let mut mono = vec![0.0f64; n];
        for i in 0..n.min(stereo_pairs) {
            let si = i * 2;
            if si + 1 < self.samples.len() {
                let l = self.samples[si] as f64;
                let r = self.samples[si + 1] as f64;
                let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos());
                mono[i] = (l + r) * 0.5 * w;
            }
        }

        let mags = fft_magnitudes(&mono);
        let useful = &mags[1..]; // skip DC
        if useful.is_empty() {
            return;
        }

        let cols = inner.width as usize;
        let h = inner.height as usize;
        let levels = h * 8;

        let max_mag = useful.iter().copied().fold(0.0f64, f64::max).max(1e-12);
        let bin_count = useful.len();

        let label_style = Style::new().fg(theme::DIM).bg(theme::BG);
        buf.set_string(inner.x, inner.y, "SPECTRUM", label_style);

        for col in 0..cols {
            let frac_lo = col as f64 / cols as f64;
            let frac_hi = (col + 1) as f64 / cols as f64;
            let bin_lo = (bin_count as f64).powf(frac_lo) as usize;
            let bin_hi = ((bin_count as f64).powf(frac_hi)).ceil() as usize;
            let bin_lo = bin_lo.min(bin_count);
            let bin_hi = bin_hi.min(bin_count).max(bin_lo + 1);

            let mut sum = 0.0;
            let mut count = 0;
            for &m in &useful[bin_lo..bin_hi] {
                sum += m;
                count += 1;
            }
            let mag = if count > 0 { sum / count as f64 } else { 0.0 };

            let db = 20.0 * (mag / max_mag).max(1e-10).log10();
            let norm = ((db + 60.0) / 60.0).clamp(0.0, 1.0);

            let level = (norm * levels as f64) as usize;
            let color = freq_color(col as f64 / cols as f64);
            let style = Style::new().fg(color).bg(theme::BG);

            for row in 0..h {
                let row_base = (h - 1 - row) * 8;
                let x = inner.x + col as u16;
                let y = inner.y + row as u16;
                if level > row_base + 8 {
                    buf.set_string(x, y, "\u{2588}", style);
                } else if level > row_base {
                    let sub = (level - row_base).min(7);
                    buf.set_string(x, y, BAR_CHARS[sub], style);
                }
            }
        }
    }
}

fn freq_color(t: f64) -> Color {
    let (r, g, b) = if t < 0.3 {
        let s = t / 0.3;
        (100.0 + s * (-60.0), 60.0 + s * 140.0, 220.0)
    } else if t < 0.6 {
        let s = (t - 0.3) / 0.3;
        (40.0 + s * 30.0, 200.0 - s * 10.0, 220.0 - s * 180.0)
    } else {
        let s = (t - 0.6) / 0.4;
        (70.0 + s * 185.0, 190.0 + s * 10.0, 40.0 - s * 40.0)
    };
    Color::Rgb(
        r.clamp(0.0, 255.0) as u8,
        g.clamp(0.0, 255.0) as u8,
        b.clamp(0.0, 255.0) as u8,
    )
}

/// Returns magnitude of N/2 frequency bins via SIMD-accelerated real FFT.
fn fft_magnitudes(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut planner = realfft::RealFftPlanner::<f64>::new();
    let r2c = planner.plan_fft_forward(n);

    let mut input = data.to_vec();
    let mut spectrum = r2c.make_output_vec();

    if r2c.process(&mut input, &mut spectrum).is_err() {
        return vec![0.0; n / 2];
    }

    spectrum.iter().map(|c| c.norm()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fft_single_tone() {
        let n = 256;
        let mut signal = vec![0.0f64; n];
        let bin = 10;
        for i in 0..n {
            signal[i] = (2.0 * PI * bin as f64 * i as f64 / n as f64).sin();
        }
        let mags = fft_magnitudes(&signal);
        let peak_bin = mags
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.partial_cmp(b.1).unwrap())
            .unwrap()
            .0;
        assert_eq!(
            peak_bin, bin,
            "FFT peak should be at the input frequency bin"
        );
    }
}
