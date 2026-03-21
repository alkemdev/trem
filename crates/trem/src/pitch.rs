use crate::math::Rational;
use std::f64::consts::LN_2;

/// Pitch as log2(freq / reference_freq).
///
/// 0.0 = reference frequency, 1.0 = one octave up.
/// 12-EDO semitone = 1/12, just fifth = log2(3/2) ≈ 0.58496.
/// This representation makes octave transposition addition and
/// equal temperaments uniform grids.
#[derive(Copy, Clone, Debug, PartialEq, PartialOrd)]
pub struct Pitch(pub f64);

impl Pitch {
    pub const UNISON: Pitch = Pitch(0.0);
    pub const OCTAVE: Pitch = Pitch(1.0);

    /// Pitch from a frequency ratio (e.g. 3/2 for a just fifth).
    pub fn from_ratio(num: f64, den: f64) -> Self {
        Self((num / den).ln() / LN_2)
    }

    /// Pitch from a rational frequency ratio.
    pub fn from_rational(r: Rational) -> Self {
        Self::from_ratio(r.numer() as f64, r.denom() as f64)
    }

    /// Pitch from cents (1200 cents = 1 octave).
    pub fn from_cents(cents: f64) -> Self {
        Self(cents / 1200.0)
    }

    /// Convert to frequency in Hz given a reference frequency.
    pub fn to_hz(self, reference_hz: f64) -> f64 {
        reference_hz * (self.0 * LN_2).exp()
    }

    /// Convert to cents (1200 per octave).
    pub fn to_cents(self) -> f64 {
        self.0 * 1200.0
    }

    /// Transpose by another pitch (addition in log space).
    pub fn transpose(self, interval: Pitch) -> Self {
        Self(self.0 + interval.0)
    }

    /// Invert the interval.
    pub fn invert(self) -> Self {
        Self(-self.0)
    }
}

/// A scale: an ordered set of pitch classes within one period.
///
/// The period is typically one octave (Pitch(1.0)) but can be anything —
/// Bohlen-Pierce uses a tritave (Pitch(log2(3))).
#[derive(Clone, Debug)]
pub struct Scale {
    pub period: Pitch,
    pub classes: Vec<Pitch>,
}

impl Scale {
    /// Resolve an integer degree to a Pitch.
    ///
    /// Degree 0 maps to the first pitch class.
    /// Degrees wrap at the period boundary: degree N in a scale of size S
    /// maps to `classes[N % S] + period * (N / S)`.
    pub fn resolve(&self, degree: i32) -> Pitch {
        let n = self.classes.len() as i32;
        // Euclidean division so negative degrees wrap correctly
        let idx = degree.rem_euclid(n) as usize;
        let octave = degree.div_euclid(n);
        Pitch(self.classes[idx].0 + self.period.0 * octave as f64)
    }

    pub fn len(&self) -> usize {
        self.classes.len()
    }

    pub fn is_empty(&self) -> bool {
        self.classes.is_empty()
    }
}

/// A tuning system that generates scales.
#[derive(Clone, Debug)]
pub enum Tuning {
    /// N equal divisions of an interval.
    /// 12-EDO: `Equal { divisions: 12, interval: Pitch::OCTAVE }`
    /// 19-EDO: `Equal { divisions: 19, interval: Pitch::OCTAVE }`
    /// Bohlen-Pierce: `Equal { divisions: 13, interval: Pitch(log2(3)) }`
    Equal { divisions: u32, interval: Pitch },

    /// Just intonation from frequency ratios.
    /// Each ratio is relative to the reference pitch.
    Just { ratios: Vec<Rational> },

    /// Arbitrary list of pitch classes.
    Free { pitches: Vec<Pitch> },
}

impl Tuning {
    /// Build the Scale this tuning defines.
    pub fn to_scale(&self) -> Scale {
        match self {
            Tuning::Equal {
                divisions,
                interval,
            } => {
                let classes = (0..*divisions)
                    .map(|i| Pitch(interval.0 * i as f64 / *divisions as f64))
                    .collect();
                Scale {
                    period: *interval,
                    classes,
                }
            }
            Tuning::Just { ratios } => {
                let mut classes: Vec<Pitch> =
                    ratios.iter().map(|r| Pitch::from_rational(*r)).collect();
                classes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let period = if classes.is_empty() {
                    Pitch::OCTAVE
                } else {
                    Pitch::OCTAVE
                };
                Scale { period, classes }
            }
            Tuning::Free { pitches } => {
                let mut classes = pitches.clone();
                classes.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                let period = Pitch::OCTAVE;
                Scale { period, classes }
            }
        }
    }

    /// Standard 12-tone equal temperament.
    pub fn edo12() -> Self {
        Tuning::Equal {
            divisions: 12,
            interval: Pitch::OCTAVE,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edo12_scale() {
        let scale = Tuning::edo12().to_scale();
        assert_eq!(scale.len(), 12);
        let a4 = 440.0;
        // Degree 0 = reference (A4), degree 12 = A5
        let a5 = scale.resolve(12).to_hz(a4);
        assert!((a5 - 880.0).abs() < 0.01);
    }

    #[test]
    fn negative_degrees() {
        let scale = Tuning::edo12().to_scale();
        // Degree -12 should be one octave down
        let p = scale.resolve(-12);
        assert!((p.0 - (-1.0)).abs() < 1e-10);
    }

    #[test]
    fn just_intonation() {
        let tuning = Tuning::Just {
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
        let scale = tuning.to_scale();
        assert_eq!(scale.len(), 7);
        let fifth = scale.resolve(4);
        let expected = Pitch::from_ratio(3.0, 2.0);
        assert!((fifth.0 - expected.0).abs() < 1e-10);
    }

    #[test]
    fn pitch_cents_roundtrip() {
        let p = Pitch::from_cents(700.0);
        assert!((p.to_cents() - 700.0).abs() < 1e-10);
    }
}
