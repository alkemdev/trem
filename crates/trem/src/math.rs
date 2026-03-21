//! Exact arithmetic for time and rhythm backed by [`num_rational::Rational64`].
//!
//! [`Rational`] is a newtype wrapper preserving the trem API (unsigned denominator
//! constructor, `to_f64`, integer `floor`/`ceil`) while delegating all math to
//! the battle-tested `num-rational` crate.

use num_traits::ToPrimitive;
use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

/// Smallest positive integer divisible by both `a` and `b`; returns `0` if either input is zero.
pub fn lcm(a: u64, b: u64) -> u64 {
    num_integer::lcm(a, b)
}

/// Exact rational number, always in lowest terms.
///
/// Thin wrapper around [`num_rational::Rational64`] that keeps the original trem
/// constructor signature (`num: i64, den: u64`) and convenience methods.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "serde", serde(transparent))]
pub struct Rational(num_rational::Rational64);

impl Rational {
    pub fn new(num: i64, den: u64) -> Self {
        debug_assert!(den <= i64::MAX as u64, "denominator overflows i64");
        Self(num_rational::Rational64::new(num, den as i64))
    }

    pub fn integer(n: i64) -> Self {
        Self(num_rational::Rational64::from_integer(n))
    }

    pub fn zero() -> Self {
        Self(num_rational::Rational64::from_integer(0))
    }

    pub fn one() -> Self {
        Self(num_rational::Rational64::from_integer(1))
    }

    pub fn is_zero(self) -> bool {
        *self.0.numer() == 0
    }

    pub fn is_positive(self) -> bool {
        *self.0.numer() > 0
    }

    pub fn is_negative(self) -> bool {
        *self.0.numer() < 0
    }

    pub fn abs(self) -> Self {
        if self.is_negative() {
            -self
        } else {
            self
        }
    }

    pub fn recip(self) -> Self {
        Self(self.0.recip())
    }

    pub fn floor(self) -> i64 {
        self.0.floor().to_integer()
    }

    pub fn ceil(self) -> i64 {
        self.0.ceil().to_integer()
    }

    pub fn to_f64(self) -> f64 {
        self.0.to_f64().unwrap_or(0.0)
    }

    pub fn min(self, other: Self) -> Self {
        if self <= other {
            self
        } else {
            other
        }
    }

    pub fn max(self, other: Self) -> Self {
        if self >= other {
            self
        } else {
            other
        }
    }

    /// Numerator (signed).
    pub fn numer(self) -> i64 {
        *self.0.numer()
    }

    /// Denominator (always positive after reduction).
    pub fn denom(self) -> i64 {
        *self.0.denom()
    }
}

impl From<i64> for Rational {
    fn from(n: i64) -> Self {
        Self::integer(n)
    }
}

impl From<(i64, u64)> for Rational {
    fn from((num, den): (i64, u64)) -> Self {
        Self::new(num, den)
    }
}

impl Ord for Rational {
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.cmp(&other.0)
    }
}

impl PartialOrd for Rational {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Neg for Rational {
    type Output = Self;
    fn neg(self) -> Self {
        Self(-self.0)
    }
}

impl Add for Rational {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl Sub for Rational {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        Self(self.0 - rhs.0)
    }
}

impl Mul for Rational {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        Self(self.0 * rhs.0)
    }
}

impl Div for Rational {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        Self(self.0 / rhs.0)
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if *self.0.denom() == 1 {
            write!(f, "{}", self.0.numer())
        } else {
            write!(f, "{}/{}", self.0.numer(), self.0.denom())
        }
    }
}

impl fmt::Debug for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Rational({self})")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reduction() {
        let r = Rational::new(6, 4);
        assert_eq!(r.numer(), 3);
        assert_eq!(r.denom(), 2);
    }

    #[test]
    fn arithmetic() {
        let a = Rational::new(1, 3);
        let b = Rational::new(1, 4);
        assert_eq!(a + b, Rational::new(7, 12));
        assert_eq!(a - b, Rational::new(1, 12));
        assert_eq!(a * b, Rational::new(1, 12));
        assert_eq!(a / b, Rational::new(4, 3));
    }

    #[test]
    fn ordering() {
        assert!(Rational::new(1, 3) < Rational::new(1, 2));
        assert!(Rational::new(-1, 2) < Rational::zero());
    }

    #[test]
    fn floor_ceil() {
        assert_eq!(Rational::new(7, 3).floor(), 2);
        assert_eq!(Rational::new(7, 3).ceil(), 3);
        assert_eq!(Rational::new(-7, 3).floor(), -3);
        assert_eq!(Rational::new(-7, 3).ceil(), -2);
    }

    #[test]
    fn display() {
        assert_eq!(Rational::new(3, 4).to_string(), "3/4");
        assert_eq!(Rational::integer(5).to_string(), "5");
    }
}
