use std::cmp::Ordering;
use std::fmt;
use std::ops::{Add, Div, Mul, Neg, Sub};

fn gcd(mut a: u64, mut b: u64) -> u64 {
    while b != 0 {
        let t = b;
        b = a % b;
        a = t;
    }
    a
}

pub fn lcm(a: u64, b: u64) -> u64 {
    if a == 0 || b == 0 {
        0
    } else {
        a / gcd(a, b) * b
    }
}

/// Exact rational number p/q, always in lowest terms with q > 0.
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Rational {
    pub num: i64,
    pub den: u64,
}

impl Rational {
    pub fn new(num: i64, den: u64) -> Self {
        assert!(den != 0, "Rational denominator must be nonzero");
        if num == 0 {
            return Self { num: 0, den: 1 };
        }
        let g = gcd(num.unsigned_abs(), den);
        Self {
            num: num / g as i64,
            den: den / g,
        }
    }

    pub fn integer(n: i64) -> Self {
        Self { num: n, den: 1 }
    }

    pub fn zero() -> Self {
        Self { num: 0, den: 1 }
    }

    pub fn one() -> Self {
        Self { num: 1, den: 1 }
    }

    pub fn is_zero(self) -> bool {
        self.num == 0
    }

    pub fn is_positive(self) -> bool {
        self.num > 0
    }

    pub fn is_negative(self) -> bool {
        self.num < 0
    }

    pub fn abs(self) -> Self {
        Self {
            num: self.num.abs(),
            den: self.den,
        }
    }

    pub fn recip(self) -> Self {
        assert!(self.num != 0, "Cannot take reciprocal of zero");
        if self.num > 0 {
            Self::new(self.den as i64, self.num as u64)
        } else {
            Self::new(-(self.den as i64), (-self.num) as u64)
        }
    }

    pub fn floor(self) -> i64 {
        if self.den == 1 {
            return self.num;
        }
        let d = self.den as i64;
        if self.num >= 0 {
            self.num / d
        } else {
            (self.num - d + 1) / d
        }
    }

    pub fn ceil(self) -> i64 {
        if self.den == 1 {
            return self.num;
        }
        let d = self.den as i64;
        if self.num >= 0 {
            (self.num + d - 1) / d
        } else {
            self.num / d
        }
    }

    pub fn to_f64(self) -> f64 {
        self.num as f64 / self.den as f64
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
        // a/b vs c/d  →  a*d vs c*b (careful with sign; den is always positive)
        let lhs = self.num as i128 * other.den as i128;
        let rhs = other.num as i128 * self.den as i128;
        lhs.cmp(&rhs)
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
        Self {
            num: -self.num,
            den: self.den,
        }
    }
}

impl Add for Rational {
    type Output = Self;
    fn add(self, rhs: Self) -> Self {
        let den = lcm(self.den, rhs.den);
        let num = self.num * (den / self.den) as i64 + rhs.num * (den / rhs.den) as i64;
        Self::new(num, den)
    }
}

impl Sub for Rational {
    type Output = Self;
    fn sub(self, rhs: Self) -> Self {
        self + (-rhs)
    }
}

impl Mul for Rational {
    type Output = Self;
    fn mul(self, rhs: Self) -> Self {
        // Cross-reduce before multiplying to avoid overflow
        let g1 = gcd(self.num.unsigned_abs(), rhs.den);
        let g2 = gcd(rhs.num.unsigned_abs(), self.den);
        let num = (self.num / g1 as i64) * (rhs.num / g2 as i64);
        let den = (self.den / g2) * (rhs.den / g1);
        Self::new(num, den)
    }
}

impl Div for Rational {
    type Output = Self;
    fn div(self, rhs: Self) -> Self {
        self * rhs.recip()
    }
}

impl fmt::Display for Rational {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.den == 1 {
            write!(f, "{}", self.num)
        } else {
            write!(f, "{}/{}", self.num, self.den)
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
        assert_eq!(r.num, 3);
        assert_eq!(r.den, 2);
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
