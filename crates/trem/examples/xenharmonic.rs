//! Xenharmonic tuning systems: EDO, just intonation, and free scales.
//!
//! Run with: `cargo run -p trem --example xenharmonic`

use trem::math::Rational;
use trem::pitch::{Pitch, Tuning};

fn show_scale(name: &str, tuning: &Tuning, reference_hz: f64) {
    let scale = tuning.to_scale();
    println!("{name} ({} notes per period):", scale.len());
    for i in 0..scale.len() as i32 {
        let p = scale.resolve(i);
        let hz = p.to_hz(reference_hz);
        let cents = p.to_cents();
        println!("  degree {i:>2}: {hz:>8.2} Hz  ({cents:>7.1} cents)");
    }
    let octave_hz = scale.resolve(scale.len() as i32).to_hz(reference_hz);
    println!("  -------- period boundary: {octave_hz:.2} Hz\n");
}

fn main() {
    let a4 = 440.0;

    let edo12 = Tuning::edo12();
    show_scale("12-EDO (standard)", &edo12, a4);

    let edo19 = Tuning::Equal {
        divisions: 19,
        interval: Pitch::OCTAVE,
    };
    show_scale("19-EDO", &edo19, a4);

    let ji = Tuning::Just {
        ratios: vec![
            Rational::new(1, 1),
            Rational::new(9, 8),
            Rational::new(5, 4),
            Rational::new(4, 3),
            Rational::new(3, 2),
            Rational::new(5, 3),
            Rational::new(15, 8),
        ],
    };
    show_scale("7-limit just intonation", &ji, a4);

    let bp = Tuning::Equal {
        divisions: 13,
        interval: Pitch::from_ratio(3.0, 1.0),
    };
    show_scale("Bohlen-Pierce (13-ED tritave)", &bp, a4);
}
