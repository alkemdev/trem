use divan::Bencher;
use std::f64::consts::PI;

fn main() {
    divan::main();
}

fn make_test_signal(n: usize) -> Vec<f64> {
    let mut signal = vec![0.0f64; n];
    for i in 0..n {
        let t = i as f64 / n as f64;
        signal[i] = (2.0 * PI * 10.0 * t).sin()
            + 0.5 * (2.0 * PI * 50.0 * t).sin()
            + 0.3 * (2.0 * PI * 200.0 * t).sin();
    }
    signal
}

fn make_stereo_signal(pairs: usize) -> Vec<f32> {
    let mut buf = vec![0.0f32; pairs * 2];
    for i in 0..pairs {
        let t = i as f64 / pairs as f64;
        let sample = (2.0 * PI * 440.0 * t).sin() as f32;
        buf[i * 2] = sample;
        buf[i * 2 + 1] = sample;
    }
    buf
}

// ---------------------------------------------------------------------------
// Raw FFT performance (realfft)
// ---------------------------------------------------------------------------

mod fft {
    use super::*;

    #[divan::bench(args = [128, 256, 512, 1024, 2048, 4096])]
    fn realfft_forward(bencher: Bencher, n: usize) {
        let signal = make_test_signal(n);
        let mut planner = realfft::RealFftPlanner::<f64>::new();
        let r2c = planner.plan_fft_forward(n);
        let mut input = signal.clone();
        let mut spectrum = r2c.make_output_vec();

        bencher.bench_local(|| {
            input.copy_from_slice(&signal);
            r2c.process(&mut input, &mut spectrum).unwrap();
        });
    }

    #[divan::bench(args = [128, 256, 512, 1024, 2048, 4096])]
    fn fft_magnitudes(bencher: Bencher, n: usize) {
        let signal = make_test_signal(n);
        let mut planner = realfft::RealFftPlanner::<f64>::new();
        let r2c = planner.plan_fft_forward(n);
        let mut input = signal.clone();
        let mut spectrum = r2c.make_output_vec();

        bencher.bench_local(|| {
            input.copy_from_slice(&signal);
            r2c.process(&mut input, &mut spectrum).unwrap();
            let _mags: Vec<f64> = spectrum.iter().map(|c| c.norm()).collect();
        });
    }
}

// ---------------------------------------------------------------------------
// Full spectrum pipeline (window + FFT + binning)
// ---------------------------------------------------------------------------

mod pipeline {
    use super::*;

    #[divan::bench(args = [256, 512, 1024])]
    fn window_and_fft(bencher: Bencher, n: usize) {
        let stereo = make_stereo_signal(n);

        bencher.bench(|| {
            let mut mono = vec![0.0f64; n];
            for i in 0..n {
                let l = stereo[i * 2] as f64;
                let r = stereo[i * 2 + 1] as f64;
                let w = 0.5 * (1.0 - (2.0 * PI * i as f64 / (n as f64 - 1.0)).cos());
                mono[i] = (l + r) * 0.5 * w;
            }

            let mut planner = realfft::RealFftPlanner::<f64>::new();
            let r2c = planner.plan_fft_forward(n);
            let mut spectrum = r2c.make_output_vec();
            r2c.process(&mut mono, &mut spectrum).unwrap();

            let mags: Vec<f64> = spectrum.iter().map(|c| c.norm()).collect();
            divan::black_box(mags);
        });
    }

    #[divan::bench]
    fn log_binning_80cols(bencher: Bencher) {
        let n = 512;
        let signal = make_test_signal(n);
        let mut planner = realfft::RealFftPlanner::<f64>::new();
        let r2c = planner.plan_fft_forward(n);
        let mut input = signal.clone();
        let mut spectrum = r2c.make_output_vec();
        r2c.process(&mut input, &mut spectrum).unwrap();
        let mags: Vec<f64> = spectrum.iter().map(|c| c.norm()).collect();
        let useful = &mags[1..];
        let cols = 80usize;

        bencher.bench(|| {
            let max_mag = useful.iter().copied().fold(0.0f64, f64::max).max(1e-12);
            let bin_count = useful.len();
            let mut bars = vec![0.0f64; cols];
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
                bars[col] = ((db + 60.0) / 60.0).clamp(0.0, 1.0);
            }
            divan::black_box(bars);
        });
    }
}
