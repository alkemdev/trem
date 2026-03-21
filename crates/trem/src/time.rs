//! Beat-based durations and half-open time spans using exact [`Rational`] math.
//!
//! Conversions to samples or seconds take BPM and sample rate; they are approximate at the
//! `f64` boundary but beat lengths stay exact until then.

use crate::math::Rational;

/// A duration measured in beats as an exact rational number.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Duration(pub Rational);

impl Duration {
    /// Whole-number duration of `n` beats (`n/1`).
    pub fn beats(n: i64) -> Self {
        Self(Rational::integer(n))
    }

    /// Fractional beat length `num/den`, reduced like [`Rational::new`].
    pub fn new(num: i64, den: u64) -> Self {
        Self(Rational::new(num, den))
    }

    /// Zero beats.
    pub fn zero() -> Self {
        Self(Rational::zero())
    }

    /// Length in samples at `bpm` and `sample_rate` (floating-point; not sample-quantized).
    pub fn to_samples(self, bpm: f64, sample_rate: f64) -> f64 {
        let seconds = self.0.to_f64() * 60.0 / bpm;
        seconds * sample_rate
    }

    /// Length in seconds at `bpm`.
    pub fn to_seconds(self, bpm: f64) -> f64 {
        self.0.to_f64() * 60.0 / bpm
    }
}

/// A half-open time span [start, end) measured in beats.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash)]
pub struct Span {
    pub start: Rational,
    pub end: Rational,
}

impl Span {
    /// Half-open interval `[start, end)` in beats; callers should keep `start <= end` for sensible geometry.
    pub fn new(start: Rational, end: Rational) -> Self {
        Self { start, end }
    }

    /// `end - start`; may be negative if `end < start`.
    pub fn duration(&self) -> Rational {
        self.end - self.start
    }

    /// `true` if `t` is on or after `start` and strictly before `end`.
    pub fn contains(&self, t: Rational) -> bool {
        t >= self.start && t < self.end
    }

    /// `true` if the two half-open intervals intersect with positive measure (touching at an endpoint does not count).
    pub fn overlaps(&self, other: &Span) -> bool {
        self.start < other.end && other.start < self.end
    }

    /// Subdivide this span into `n` equal parts.
    pub fn subdivide(&self, n: u32) -> Vec<Span> {
        let step = self.duration() * Rational::new(1, n as u64);
        (0..n)
            .map(|i| {
                let s = self.start + step * Rational::integer(i as i64);
                Span::new(s, s + step)
            })
            .collect()
    }
}

/// Convert a rational beat position to a sample offset.
pub fn beat_to_sample(beat: Rational, bpm: f64, sample_rate: f64) -> f64 {
    beat.to_f64() * 60.0 / bpm * sample_rate
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duration_to_samples() {
        let d = Duration::beats(1);
        // At 120 BPM, 1 beat = 0.5 seconds = 22050 samples at 44100 Hz
        let s = d.to_samples(120.0, 44100.0);
        assert!((s - 22050.0).abs() < 1.0);
    }

    #[test]
    fn span_subdivide() {
        let span = Span::new(Rational::zero(), Rational::one());
        let parts = span.subdivide(4);
        assert_eq!(parts.len(), 4);
        assert_eq!(parts[0].start, Rational::zero());
        assert_eq!(parts[0].end, Rational::new(1, 4));
        assert_eq!(parts[3].end, Rational::one());
    }

    #[test]
    fn span_overlap() {
        let a = Span::new(Rational::zero(), Rational::new(1, 2));
        let b = Span::new(Rational::new(1, 4), Rational::new(3, 4));
        assert!(a.overlaps(&b));
        let c = Span::new(Rational::new(1, 2), Rational::one());
        assert!(!a.overlaps(&c));
    }
}
