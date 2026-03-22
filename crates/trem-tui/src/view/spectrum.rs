//! Real-time spectrum analyzer: windowed FFT with **peak decay** smoothing (exponential fall),
//! logarithmic binning, and a smooth frequency→hue mapping.
//!
//! FFT plans are cached in [`SpectrumAnalyzerState`] so the UI thread does not rebuild planners
//! every frame. **Each FFT bin** keeps its own decay state:
//! \(s_i \leftarrow \max\bigl(m_i,\, s_i\,e^{-\Delta t/\tau}\bigr)\) with
//! \(\tau =\) [`SpectrumAnalyzerState::fall_ms`] / 1000 (attack is instantaneous).
//! Columns map logarithmically to bin ranges and take the **max** of those bins (not the mean)
//! so one bin’s fall is not smeared by quieter neighbours.
//!
//! **Level mapping:** bar height uses dB relative to a **slowly decaying global peak** (`norm_ref`),
//! not “max bin = full scale” each frame — so near-silent buffers don’t inflate to full height.

use crate::theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::style::{Color, Style};
use ratatui::widgets::{Block, Borders, Padding, Widget};
use realfft::num_complex::Complex;
use realfft::RealFftPlanner;
use std::f64::consts::PI;
use std::sync::Arc;
use std::time::Instant;

const FFT_SIZE: usize = 512;
/// Minimum linear ref for dB mapping (avoids log blow-ups; keep very small).
const REF_FLOOR: f64 = 1e-18;
const BAR_CHARS: [&str; 8] = [
    "\u{2581}", "\u{2582}", "\u{2583}", "\u{2584}", "\u{2585}", "\u{2586}", "\u{2587}", "\u{2588}",
];

/// Cached FFT + smoothed magnitude bins (one analyzer pane, e.g. IN or OUT).
pub struct SpectrumAnalyzerState {
    /// Time constant for spectral **fall** in milliseconds (\(\tau\) in the decay formula).
    pub fall_ms: f64,
    /// After [`Self::analyze`], linear magnitude reference for display (decaying peak tracker).
    pub norm_ref: f64,
    n: usize,
    mono: Vec<f64>,
    fft_input: Vec<f64>,
    spectrum: Vec<Complex<f64>>,
    smoothed: Vec<f64>,
    last_tick: Option<Instant>,
    fft: Option<Arc<dyn realfft::RealToComplex<f64>>>,
}

impl Default for SpectrumAnalyzerState {
    fn default() -> Self {
        Self::new(18.0)
    }
}

impl SpectrumAnalyzerState {
    /// `fall_ms`: per-bin decay time constant τ in ms (smaller = faster spectral “tail”).
    pub fn new(fall_ms: f64) -> Self {
        Self {
            fall_ms: fall_ms.clamp(4.0, 5000.0),
            norm_ref: 0.0,
            n: 0,
            mono: Vec::new(),
            fft_input: Vec::new(),
            spectrum: Vec::new(),
            smoothed: Vec::new(),
            last_tick: None,
            fft: None,
        }
    }

    /// Windowed mono → FFT magnitudes → peak decay; returns smoothed bins **including** DC at index 0
    /// (renderers usually skip `[1..]`), plus [`Self::norm_ref`] for display scaling (same frame).
    pub fn analyze(&mut self, samples: &[f32], now: Instant) -> (&[f64], f64) {
        let stereo_pairs = samples.len() / 2;
        if stereo_pairs < 4 {
            self.smoothed.clear();
            self.norm_ref = 0.0;
            return (&[], 0.0);
        }

        let n = FFT_SIZE.min(stereo_pairs.next_power_of_two()).max(16);
        if n != self.n || self.fft.is_none() {
            self.n = n;
            self.norm_ref = 0.0;
            let mut planner = RealFftPlanner::<f64>::new();
            self.fft = Some(planner.plan_fft_forward(n));
            self.mono.resize(n, 0.0);
            self.fft_input.resize(n, 0.0);
            self.spectrum = self.fft.as_ref().unwrap().make_output_vec();
            let half = n / 2;
            self.smoothed.resize(half, 0.0);
            self.last_tick = None;
        }

        let fft = self.fft.as_ref().unwrap();

        for i in 0..n {
            let si = i * 2;
            let (l, r) = if si + 1 < samples.len() {
                (samples[si] as f64, samples[si + 1] as f64)
            } else {
                (0.0, 0.0)
            };
            let w = if n <= 1 {
                1.0
            } else {
                0.5 * (1.0 - (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos())
            };
            self.mono[i] = (l + r) * 0.5 * w;
        }

        self.fft_input.copy_from_slice(&self.mono);
        if fft
            .process(&mut self.fft_input, &mut self.spectrum)
            .is_err()
        {
            self.smoothed.fill(0.0);
            let nr = self.norm_ref;
            return (&self.smoothed, nr);
        }

        let dt = self
            .last_tick
            .map(|t| now.duration_since(t).as_secs_f64())
            .unwrap_or(1.0 / 60.0)
            .clamp(1.0 / 480.0, 0.35);
        self.last_tick = Some(now);

        let half = n / 2;
        // Raw peak (skip DC): drives the display normalization envelope.
        let mut peak_raw = 0.0f64;
        for i in 1..half {
            peak_raw = peak_raw.max(self.spectrum[i].norm());
        }
        // Slow decay while playing; faster decay when the block is much quieter than the held ref
        // so silence doesn’t sit at “full scale” forever.
        let tau_slow = 0.48_f64;
        let tau_fast = 0.10_f64;
        let well_below = peak_raw < self.norm_ref * 0.18 && self.norm_ref > 1e-12;
        let tau_nr = if well_below { tau_fast } else { tau_slow };
        let decay_nr = (-dt / tau_nr).exp();
        self.norm_ref = peak_raw.max(self.norm_ref * decay_nr).max(REF_FLOOR);

        let tau = (self.fall_ms / 1000.0).max(1e-5);
        let decay = (-dt / tau).exp();

        for i in 0..half {
            let m = self.spectrum[i].norm();
            self.smoothed[i] = m.max(self.smoothed[i] * decay);
        }

        let nr = self.norm_ref;
        (&self.smoothed, nr)
    }
}

/// Renders one spectrum pane from **pre-smoothed** FFT magnitudes (e.g. from [`SpectrumAnalyzerState::analyze`]).
pub struct SpectrumView<'a> {
    pub magnitudes: &'a [f64],
    /// Linear reference from [`SpectrumAnalyzerState::norm_ref`] (decaying peak); height = dB vs this.
    pub norm_ref: f64,
    pub title: &'a str,
    /// If > 0, appended to the title as decay hint (ms).
    pub decay_ms_label: f64,
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

        let useful = if self.magnitudes.len() > 2 {
            &self.magnitudes[1..]
        } else {
            self.magnitudes
        };
        if useful.is_empty() {
            return;
        }

        let cols = inner.width as usize;
        let h = inner.height as usize;
        let levels = h * 8;

        let bin_count = useful.len();
        // Display dB relative to decaying peak; ~54 dB of range below 0 dB(re:ref).
        const RANGE_DB: f64 = 54.0;
        let instant_peak = useful.iter().copied().fold(0.0f64, f64::max);
        // When the block is essentially silent, nudge ref upward vs the noise floor so bins
        // don’t all cluster at “full scale”.
        const SILENCE_PEAK: f64 = 4e-8;
        let ref_lin = if instant_peak < SILENCE_PEAK {
            let noise_floor = instant_peak.max(REF_FLOOR) * 120.0;
            self.norm_ref.max(noise_floor)
        } else {
            self.norm_ref
        }
        .max(REF_FLOOR);

        let label_style = Style::new().fg(theme::DIM).bg(theme::BG);
        let title = if self.title.is_empty() {
            "SPEC"
        } else {
            self.title
        };
        let tw = inner.width as usize;
        let suffix = if self.decay_ms_label > 0.5 {
            format!(" {}ms", self.decay_ms_label.round())
        } else {
            String::new()
        };
        let head = format!("{title}{suffix}");
        let shown: String = head.chars().take(tw.saturating_sub(1)).collect();
        buf.set_string(inner.x, inner.y, &shown, label_style);

        for col in 0..cols {
            let frac_lo = col as f64 / cols as f64;
            let frac_hi = (col + 1) as f64 / cols as f64;
            let bin_lo = (bin_count as f64).powf(frac_lo) as usize;
            let bin_hi = ((bin_count as f64).powf(frac_hi)).ceil() as usize;
            let bin_lo = bin_lo.min(bin_count);
            let bin_hi = bin_hi.min(bin_count).max(bin_lo + 1);

            // Max preserves per-bin decay: averaging would hide a dropping bin behind neighbours.
            let mag = useful[bin_lo..bin_hi]
                .iter()
                .copied()
                .fold(0.0f64, f64::max);

            let db_rel = 20.0 * (mag / ref_lin).max(1e-15).log10();
            let norm = ((db_rel + RANGE_DB) / RANGE_DB).clamp(0.0, 1.0);

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

/// Returns magnitude of N/2 frequency bins via SIMD-accelerated real FFT (stateless; for tests).
pub fn fft_magnitudes(data: &[f64]) -> Vec<f64> {
    let n = data.len();
    let mut planner = RealFftPlanner::<f64>::new();
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
    use ratatui::buffer::Buffer;
    use ratatui::layout::Rect;

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

    #[test]
    fn spectrum_view_renders_with_title() {
        let mags: Vec<f64> = (0..256).map(|i| (i as f64 * 0.01).sin().abs()).collect();
        let area = Rect::new(0, 0, 40, 8);
        let mut b = Buffer::empty(area);
        let peak = mags.iter().copied().fold(0.0f64, f64::max).max(REF_FLOOR);
        SpectrumView {
            magnitudes: &mags,
            norm_ref: peak,
            title: "TEST",
            decay_ms_label: 0.0,
        }
        .render(area, &mut b);
        assert!(area.width > 0);
    }

    #[test]
    fn analyzer_peak_decay_falls() {
        let mut st = SpectrumAnalyzerState::new(50.0);
        let mut buf = vec![0.0f32; 512 * 2];
        for i in 0..512 {
            buf[i * 2] = 0.5;
            buf[i * 2 + 1] = 0.5;
        }
        let t0 = Instant::now();
        let (s0, _) = st.analyze(&buf, t0);
        let m0 = s0.to_vec();
        let peak0 = m0[1..].iter().copied().fold(0.0f64, f64::max);
        let t1 = t0 + std::time::Duration::from_millis(80);
        let zeros = vec![0.0f32; 512 * 2];
        let (s1, _) = st.analyze(&zeros, t1);
        let peak1 = s1[1..].iter().copied().fold(0.0f64, f64::max);
        assert!(
            peak1 < peak0 && peak1 > 0.0,
            "decay should lower peak but not instantly: {peak0} -> {peak1}"
        );
    }
}
